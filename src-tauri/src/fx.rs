//! 皮肤音频律动：EqSource 输出分接 → 无锁环形缓冲 → 分析线程
//! （Hann 窗 + 手写 radix-2 FFT，零新依赖）→ 36 个对数频段 + 低频
//! 节拍检测 → `fx://spectrum` 事件（~30Hz）。前端消费方是皮肤律动层
//! components/AudioFxLayer.tsx。
//!
//! 分接点选在 EqSource：共享（rodio sink）与 WASAPI 独占两条播放链
//! 唯一的汇合处，推入的是 EQ 后、按帧声道平均的单声道样本——分析
//! 结果即用户实际听到的声音。写入方运行在音频线程上，push 只有
//! 一次下标计算 + 一次 f32 存储 + 一次原子 store，无锁无分配。
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

/// 频段数（与黑胶皮肤频谱柱一一对应）
pub const BANDS: usize = 36;
/// FFT 窗长（2 的幂）：44.1kHz 下约 46ms，频率分辨率 ~21.5Hz
const WIN: usize = 2048;
/// 环形缓冲容量掩码（容量 32768，约 0.74s @44.1kHz）
const RING_MASK: u64 = (1 << 15) - 1;
/// 分析线程节拍（与前端律动层刷新率匹配）
const TICK_MS: u64 = 33;
/// 频段覆盖范围（对数分布）
const F_LO: f32 = 40.0;
const F_HI: f32 = 15000.0;

/// 引擎与分析线程共享的采样分接器。单生产者（当前播放链的
/// EqSource）单消费者（分析线程）；换曲瞬间新旧链理论上可能
/// 短暂并存，最坏影响是几十毫秒的频谱抖动，无碍。
pub struct FxShared {
    /// f32 位模式存储：音频线程无锁写入（Relaxed store），读取方靠
    /// write 计数器的 Release/Acquire 建立 happens-before
    buf: Vec<AtomicU32>,
    /// 累计写入帧数（单调递增，按模定位）
    write: AtomicU64,
    sr: AtomicU32,
    enabled: AtomicBool,
}

impl FxShared {
    pub fn new() -> Self {
        Self {
            buf: (0..=RING_MASK).map(|_| AtomicU32::new(0)).collect(),
            write: AtomicU64::new(0),
            sr: AtomicU32::new(44100),
            enabled: AtomicBool::new(false),
        }
    }

    /// 音频线程调用：写入一帧单声道样本（无锁、无分配、无系统调用）
    #[inline]
    pub fn push(&self, x: f32) {
        let w = self.write.load(Ordering::Relaxed);
        self.buf[(w & RING_MASK) as usize].store(x.to_bits(), Ordering::Relaxed);
        self.write.store(w + 1, Ordering::Release);
    }

    /// EqSource 构造时上报采样率（频段映射依赖）
    #[inline]
    pub fn set_rate(&self, sr: f64) {
        self.sr.store(sr.max(1.0) as u32, Ordering::Relaxed);
    }

    /// 前端开关：只有皮肤律动层挂载且窗口可见时才打开，避免空发事件
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// 读取最近 out.len() 帧（不足时前面补零），返回实际拷贝数
    fn latest(&self, out: &mut [f32]) -> usize {
        let w = self.write.load(Ordering::Acquire);
        let n = out.len().min(w as usize);
        for i in 0..n {
            let bits =
                self.buf[(w - n as u64 + i as u64 & RING_MASK) as usize].load(Ordering::Relaxed);
            out[i] = f32::from_bits(bits);
        }
        n
    }
}

/// 就地 radix-2 库利-图基 FFT（f32，视觉用途精度足够）。
/// 预计算位反转表与单位根表，run() 内零分配。
struct Fft {
    rev: Vec<u32>,
    /// e^{-2πik/n} 的 cos/sin 表（k < n/2）
    cos_t: Vec<f32>,
    sin_t: Vec<f32>,
}

impl Fft {
    fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        let bits = n.trailing_zeros();
        let rev: Vec<u32> = (0..n as u32)
            .map(|i| i.reverse_bits() >> (32 - bits))
            .collect();
        let half = n / 2;
        let cos_t = (0..half)
            .map(|k| (-2.0 * std::f32::consts::PI * k as f32 / n as f32).cos())
            .collect();
        let sin_t = (0..half)
            .map(|k| (-2.0 * std::f32::consts::PI * k as f32 / n as f32).sin())
            .collect();
        Fft { rev, cos_t, sin_t }
    }

    /// re/im 长度必须等于构造时的 n
    fn run(&self, re: &mut [f32], im: &mut [f32]) {
        let n = re.len();
        for i in 0..n {
            let j = self.rev[i] as usize;
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2usize;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            let mut base = 0;
            while base < n {
                for k in 0..half {
                    let tw = k * step;
                    let (wr, wi) = (self.cos_t[tw], self.sin_t[tw]);
                    let i = base + k;
                    let j = i + half;
                    let (xr, xi) = (re[j], im[j]);
                    let tr = xr * wr - xi * wi;
                    let ti = xr * wi + xi * wr;
                    re[j] = re[i] - tr;
                    im[j] = im[i] - ti;
                    re[i] += tr;
                    im[i] += ti;
                }
                base += len;
            }
            len <<= 1;
        }
    }
}

/// 频段 → FFT bin 区间映射（40Hz..15kHz 对数分布，随采样率重算）
struct BandMap {
    /// 第 i 频段覆盖 bin[start[i]..end[i]]（end 为开区间）
    start: Vec<usize>,
    end: Vec<usize>,
    sr: u32,
}

impl BandMap {
    fn build(sr: u32) -> Self {
        let ratio = (F_HI / F_LO).powf(1.0 / BANDS as f32);
        let bin_of = |hz: f32| (hz * WIN as f32 / sr as f32).round().max(1.0) as usize;
        let mut start = Vec::with_capacity(BANDS);
        let mut end = Vec::with_capacity(BANDS);
        let mut f = F_LO;
        for _ in 0..BANDS {
            let s = bin_of(f);
            let e = (bin_of(f * ratio)).max(s + 1).min(WIN / 2);
            start.push(s);
            end.push(e.max(s + 1));
            f *= ratio;
        }
        BandMap { start, end, sr }
    }
}

/// 启动分析线程（Engine::new 里调用一次，常驻；未启用时每 tick
/// 只做一次原子读，开销可忽略）
pub fn spawn(app: AppHandle, fx: Arc<FxShared>) {
    std::thread::Builder::new()
        .name("fx-spectrum".into())
        .spawn(move || analyze_loop(app, fx))
        .expect("fx-spectrum 线程启动失败");
}

fn analyze_loop(app: AppHandle, fx: Arc<FxShared>) {
    let fft = Fft::new(WIN);
    let hann: Vec<f32> = (0..WIN)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WIN as f32).cos())
        .collect();
    let mut band_map = BandMap::build(44100);
    let mut samples = vec![0f32; WIN];
    let mut re = vec![0f32; WIN];
    let mut im = vec![0f32; WIN];
    // 平滑后的输出频段（攻击快/释放慢）与节拍检测状态
    let mut smooth = [0f32; BANDS];
    let mut level = 0f32;
    let mut bass_hist: VecDeque<f32> = VecDeque::with_capacity(45);
    let mut beat_cooldown = 0u8;
    let mut last_write = u64::MAX;
    let mut idle_ticks = u32::MAX;

    loop {
        std::thread::sleep(Duration::from_millis(TICK_MS));
        if !fx.enabled() {
            // 关闭期间写入量仍在涨：重置基准，重开时按"有新数据"处理
            last_write = u64::MAX;
            idle_ticks = u32::MAX;
            continue;
        }
        let w = fx.write.load(Ordering::Acquire);
        let fresh = w != last_write;
        if !fresh {
            // 暂停/无声：释放衰减继续发一小段时间让柱子落回基线，
            // 全零后静默（约 1.3s）
            if idle_ticks >= 40 {
                continue;
            }
            idle_ticks += 1;
            for b in smooth.iter_mut() {
                *b *= 0.8;
            }
            level *= 0.8;
            emit(&app, &smooth, level, false);
            continue;
        }
        idle_ticks = 0;
        last_write = w;

        let sr = fx.sr.load(Ordering::Relaxed);
        if sr != band_map.sr {
            band_map = BandMap::build(sr);
        }
        let n = fx.latest(&mut samples);
        if n < WIN / 8 {
            continue; // 刚起播数据太少
        }

        let (raw, raw_level) = spectrum_frame(&fft, &hann, &band_map, &samples, &mut re, &mut im);
        for i in 0..BANDS {
            let k = if raw[i] > smooth[i] { 0.55 } else { 0.16 };
            smooth[i] += (raw[i] - smooth[i]) * k;
        }
        level += (raw_level - level) * 0.25;

        // 低频节拍：低频能量 vs 近 1.5s 均值，超阈且过冷却即报拍
        let bass = raw[..6].iter().sum::<f32>() / 6.0;
        bass_hist.push_back(bass);
        if bass_hist.len() > 45 {
            bass_hist.pop_front();
        }
        let avg = bass_hist.iter().sum::<f32>() / bass_hist.len() as f32;
        let beat = beat_cooldown == 0 && bass > avg * 1.35 + 0.012 && bass > 0.08;
        if beat {
            beat_cooldown = 5; // ~165ms
        } else if beat_cooldown > 0 {
            beat_cooldown -= 1;
        }

        emit(&app, &smooth, level, beat);
    }
}

fn emit(app: &AppHandle, bands: &[f32; BANDS], level: f32, beat: bool) {
    let _ = app.emit(
        "fx://spectrum",
        serde_json::json!({
            "bands": bands.as_slice(),
            "level": (level * 1000.0).round() / 1000.0,
            "beat": beat,
        }),
    );
}

/// 单帧频谱：Hann 窗 → FFT → 各频段最大幅度 → dB 归一 0..1。
/// 返回（未平滑频段值, 时域总能量 0..1）。re/im 为调用方提供的
/// 工作缓冲（长度 WIN，避免每帧分配）
fn spectrum_frame(
    fft: &Fft,
    hann: &[f32],
    map: &BandMap,
    samples: &[f32],
    re: &mut [f32],
    im: &mut [f32],
) -> ([f32; BANDS], f32) {
    // 时域 RMS（未加窗）→ 总能量（光晕呼吸用）
    let mut acc = 0f32;
    for &x in samples.iter() {
        acc += x * x;
    }
    let rms = (acc / samples.len() as f32).sqrt();
    let level = (rms * 3.4).clamp(0.0, 1.0);

    for i in 0..samples.len() {
        re[i] = samples[i] * hann[i];
        im[i] = 0.0;
    }
    fft.run(re, im);
    // 幅度归一：/ (N/4) ≈ 满幅正弦 → 1.0（Hann 相干增益 0.5 × 单边 ×2）
    let norm = 4.0 / samples.len() as f32;

    let mut out = [0f32; BANDS];
    for i in 0..BANDS {
        let mut m = 0f32;
        for k in map.start[i]..map.end[i] {
            let mag = (re[k] * re[k] + im[k] * im[k]).sqrt() * norm;
            if mag > m {
                m = mag;
            }
        }
        let db = 20.0 * (m + 1e-9).log10();
        let v = ((db + 62.0) / 58.0).clamp(0.0, 1.0);
        out[i] = v.powf(1.25);
    }
    (out, level)
}

// ---------- 测试 ----------

#[cfg(test)]
mod tests {
    use super::*;

    /// 与教科书 DFT 对拍：随机实信号，频谱幅度误差应远小于视觉阈值
    #[test]
    fn fft_matches_dft() {
        let n = 256;
        let fft = Fft::new(n);
        let sig: Vec<f64> = (0..n)
            .map(|i| {
                (0.3 * (2.0 * std::f64::consts::PI * 5.0 * i as f64 / n as f64)).sin()
                    + (0.7 * (2.0 * std::f64::consts::PI * 37.0 * i as f64 / n as f64)).cos()
            })
            .collect();
        let mut re: Vec<f32> = sig.iter().map(|&x| x as f32).collect();
        let mut im = vec![0f32; n];
        fft.run(&mut re, &mut im);
        for k in 1..n / 2 {
            let got = ((re[k] * re[k] + im[k] * im[k]) as f64).sqrt();
            // 直接按定义算 DFT（e^{-2πikn/N}）
            let (mut dr, mut di) = (0f64, 0f64);
            for (i, &x) in sig.iter().enumerate() {
                let ang = -2.0 * std::f64::consts::PI * k as f64 * i as f64 / n as f64;
                dr += x * ang.cos();
                di += x * ang.sin();
            }
            let want = (dr * dr + di * di).sqrt();
            assert!(
                (got - want).abs() < 0.05 * want.max(1.0),
                "bin {k}: got {got}, want {want}"
            );
        }
    }

    /// 环形缓冲环绕正确性：写入超过容量后读到的应是最近 WIN 帧
    #[test]
    fn ring_wraparound() {
        let fx = FxShared::new();
        let total = (RING_MASK + 1) as usize + 5000;
        for i in 0..total {
            fx.push(i as f32);
        }
        let mut out = vec![0f32; WIN];
        let n = fx.latest(&mut out);
        assert_eq!(n, WIN);
        let first = total - WIN;
        for (i, &v) in out.iter().enumerate() {
            assert_eq!(v as usize, first + i);
        }
    }

    /// 频段映射：区间单调不减且都在有效 bin 范围内
    #[test]
    fn band_map_sane() {
        for sr in [44100u32, 48000] {
            let m = BandMap::build(sr);
            assert_eq!(m.start.len(), BANDS);
            for i in 0..BANDS {
                assert!(m.start[i] >= 1 && m.end[i] <= WIN / 2);
                assert!(m.end[i] > m.start[i], "band {i} empty");
                if i > 0 {
                    assert!(m.start[i] >= m.start[i - 1]);
                }
            }
        }
    }

    /// 端到端频谱数学：合成 440Hz + 8kHz 双音，对应频段应显著起峰、
    /// 其余频段接近零——即视觉柱只会落在正确的位置
    #[test]
    fn sine_tones_hit_expected_bands() {
        let fft = Fft::new(WIN);
        let hann: Vec<f32> = (0..WIN)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WIN as f32).cos())
            .collect();
        let map = BandMap::build(44100);
        let sr = 44100.0f32;
        let samples: Vec<f32> = (0..WIN)
            .map(|i| {
                let t = i as f32 / sr;
                0.5 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()
                    + 0.25 * (2.0 * std::f32::consts::PI * 8000.0 * t).sin()
            })
            .collect();
        let mut re = vec![0f32; WIN];
        let mut im = vec![0f32; WIN];
        let (bands, level) = spectrum_frame(&fft, &hann, &map, &samples, &mut re, &mut im);

        let band_of = |hz: f32| -> usize {
            let mut f = F_LO;
            let ratio = (F_HI / F_LO).powf(1.0 / BANDS as f32);
            for i in 0..BANDS {
                if hz < f * ratio {
                    return i;
                }
                f *= ratio;
            }
            BANDS - 1
        };
        let b440 = band_of(440.0);
        let b8k = band_of(8000.0);
        let bmid = band_of(120.0);
        assert!(bands[b440] > 0.5, "440Hz band {b440} = {}", bands[b440]);
        assert!(bands[b8k] > 0.4, "8kHz band {b8k} = {}", bands[b8k]);
        assert!(bands[bmid] < 0.08, "120Hz band {bmid} = {}", bands[bmid]);
        assert!(level > 0.4 && level <= 1.0, "level {level}");
    }
}
