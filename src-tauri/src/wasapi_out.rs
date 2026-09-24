//! WASAPI 独占模式输出（可选功能，仅 Windows）
//!
//! 绕过 Windows 系统混音器直接与声卡通信：采样率按源文件直通、不经系统
//! 重采样，独占期间其它应用无法出声。格式协商由本模块完成，任何失败都
//! 返回 Err，由引擎回退到常规共享模式播放，不影响可用性。
//!
//! 采样处理链复用引擎的 EqSource（均衡器 + 播放位置统计），音量按采样
//! 乘法施加（独占模式绕过了系统音量）。采样率与设备不一致时用线性重采样
//! 对齐（与 rodio 共享模式的 Speed 节点同级别质量）。

use rodio::Source;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};

use wasapi::{
    calculate_period_100ns, initialize_mta, BufferFlags, DeviceCollection, Direction, SampleType,
    ShareMode, WaveFormat,
};
use windows51::core::Error as WinError;

/// AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED：周期未按驱动要求对齐，
/// 触发文档规定的重算流程（失败后 GetBufferSize 返回对齐缓冲值）。
const AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED: u32 = 0x8889_0019;

/// 把 wasapi/windows 错误转成安全描述。
/// 切勿对这类错误调用 Display/message()：AUDCLNT 错误码没有系统消息模板，
/// FormatMessageW 返回空指针会触发 UB 检查直接闪退（0x01.8 之前的闪退根因）。
fn wasapi_err(what: &str, e: &(dyn std::error::Error + 'static)) -> String {
    if let Some(we) = e.downcast_ref::<WinError>() {
        format!("{what}（HRESULT 0x{:08X}）", we.code().0 as u32)
    } else {
        what.to_string()
    }
}

/// 会话控制句柄：引擎据此暂停 / 终止独占播放线程
#[derive(Clone)]
pub struct ExclusiveCtl {
    /// 会话存活（线程运行中）
    pub active: Arc<AtomicBool>,
    /// 用户暂停（暂停期间写静音帧，位置统计冻结）
    pub paused: Arc<AtomicBool>,
    /// 请求终止会话
    pub stop: Arc<AtomicBool>,
    /// 线程已退出（音频客户端已释放、设备已交还系统）
    pub exited: Arc<AtomicBool>,
}

/// 等待会话线程退出。必须在重建共享输出流之前调用：
/// 设备被独占客户端占用期间，新的共享流打不开（表现为关独占后无声）。
pub fn wait_session_exit(ctl: &ExclusiveCtl, timeout_ms: u64) -> bool {
    ctl.stop.store(true, Ordering::Relaxed);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    while !ctl.exited.load(Ordering::Relaxed) {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    true
}

/// 会话音频参数（采样源与设备无关的部分由引擎传入）
pub struct ExclusiveParams {
    pub device_pref: Option<String>,
    pub channels: usize,
    pub src_rate: u32,
    pub volume_bits: Arc<AtomicU32>,
    pub speed_bits: Arc<AtomicU32>,
}

/// 启动独占播放会话。返回控制句柄与初始化结果接收端：
/// 会话线程完成格式协商与设备初始化后回传 Ok(()) 或失败原因，
/// 调用方应等待该结果，失败时回退共享模式（采样源已被线程取走，需重新解码）。
pub fn spawn_exclusive_session<S>(
    src: S,
    params: ExclusiveParams,
) -> Result<(ExclusiveCtl, mpsc::Receiver<Result<(), String>>), String>
where
    S: Source<Item = f32> + Send + 'static,
{
    let active = Arc::new(AtomicBool::new(true));
    let paused = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let exited = Arc::new(AtomicBool::new(false));
    let ctl = ExclusiveCtl {
        active: active.clone(),
        paused: paused.clone(),
        stop: stop.clone(),
        exited: exited.clone(),
    };
    let (tx, rx) = mpsc::channel();
    let _handle = std::thread::Builder::new()
        .name("wasapi-exclusive".into())
        .spawn(move || {
            let result = run_session(src, params, active, paused, stop, tx.clone());
            // 线程收尾：先标记退出（设备已随 audio_client 释放），再回传结果
            exited.store(true, Ordering::Relaxed);
            let _ = tx.send(result);
        })
        .map_err(|e| format!("启动独占播放线程失败: {e}"))?;
    Ok((ctl, rx))
}

/// 采样格式（由协商成功的 WaveFormat 解出）
#[derive(Clone, Copy)]
struct FmtSpec {
    kind: SampleKind,
    bytes_per_sample: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum SampleKind {
    I16,
    I24,
    I24In32,
    I32,
    F32,
}

/// 线性重采样器：step = 每个输出帧消耗的输入帧数
/// （= 速率比 × 倍速）。step == 1 时逐帧直通（位一致）。
struct LinearResampler {
    step: f64,
    pos: f64,
    prev: Option<Vec<f32>>,
    cur: Option<Vec<f32>>,
    done: bool,
}

impl LinearResampler {
    /// 从源拉取一帧（ch 个采样）；源耗尽返回 None
    fn pull(src: &mut dyn Iterator<Item = f32>, ch: usize) -> Option<Vec<f32>> {
        let mut f = Vec::with_capacity(ch);
        for _ in 0..ch {
            f.push(src.next()?);
        }
        Some(f)
    }

    fn next_frame(&mut self, src: &mut dyn Iterator<Item = f32>, ch: usize) -> Option<Vec<f32>> {
        if self.done {
            return None;
        }
        if self.cur.is_none() {
            self.cur = Some(Self::pull(src, ch)?);
            // prev 必须有值：否则前几次插值会走“直接输出 cur”分支，
            // 造成重复帧（听感为节奏错乱/倍速）
            if let Some(c) = &self.cur {
                self.prev = Some(c.clone());
            }
        }
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            self.prev = self.cur.take();
            match Self::pull(src, ch) {
                Some(f) => self.cur = Some(f),
                None => {
                    self.done = true;
                    return self.prev.take();
                }
            }
        }
        let out = match (&self.prev, &self.cur) {
            (Some(p), Some(c)) if self.pos > 0.0 => p
                .iter()
                .zip(c.iter())
                .map(|(a, b)| a + (b - a) * self.pos as f32)
                .collect(),
            (_, Some(c)) => c.clone(),
            _ => {
                self.done = true;
                return None;
            }
        };
        self.pos += self.step;
        Some(out)
    }
}

/// 追加一帧采样到字节缓冲（按设备格式转换）
fn append_frame(buf: &mut Vec<u8>, frame: &[f32], fmt: &FmtSpec, volume: f32) {
    for &s in frame {
        let s = if (volume - 1.0).abs() < f32::EPSILON {
            s
        } else {
            (s * volume).clamp(-1.0, 1.0)
        };
        match fmt.kind {
            SampleKind::I16 => {
                let v = (s * 32767.0) as i16;
                buf.extend_from_slice(&v.to_le_bytes());
            }
            SampleKind::I24 => {
                let v = (s * 8388607.0) as i32;
                let b = v.to_le_bytes();
                buf.extend_from_slice(&b[..3]);
            }
            SampleKind::I24In32 => {
                // 24 位有效数据左对齐存入 32 位容器
                let v = ((s * 8388607.0) as i32) << 8;
                buf.extend_from_slice(&v.to_le_bytes());
            }
            SampleKind::I32 => {
                let v = (s * 2147483647.0) as i32;
                buf.extend_from_slice(&v.to_le_bytes());
            }
            SampleKind::F32 => {
                buf.extend_from_slice(&s.to_le_bytes());
            }
        }
    }
}

fn silence_bytes(frames: usize, blockalign: usize) -> Vec<u8> {
    vec![0u8; frames * blockalign]
}

fn silent_flags() -> BufferFlags {
    BufferFlags {
        data_discontinuity: false,
        silent: true,
        timestamp_error: false,
    }
}

/// 选定设备：优先用户偏好（按友好名匹配），否则系统默认
fn pick_device(pref: Option<&str>) -> Result<wasapi::Device, String> {
    if let Some(name) = pref {
        if let Ok(collection) = DeviceCollection::new(&Direction::Render) {
            if let Ok(dev) = collection.get_device_with_name(name) {
                return Ok(dev);
            }
        }
        eprintln!("[wasapi] 未找到输出设备「{name}」，回退系统默认");
    }
    wasapi::get_default_device(&Direction::Render)
        .map_err(|e| wasapi_err("没有可用的音频输出设备", e.as_ref()))
}

/// 关键 HRESULT 的可行动提示
fn exclusive_hint(code: u32) -> Option<&'static str> {
    match code {
        // AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED
        0x8889_000E => Some("系统禁用了独占授权：在声音设置中打开该设备的属性，关掉「音频增强」并允许应用程序独占控制此设备"),
        // AUDCLNT_E_INVALID_STREAM_FLAG：实测部分驱动（蓝牙/网络音箱）对独占一律返回此码
        0x8889_000A => Some("该设备驱动不支持独占模式（常见于蓝牙/网络音箱），请切换到其它输出设备"),
        // AUDCLNT_E_UNSUPPORTED_FORMAT
        0x8889_0008 => Some("设备不接受以上任何候选格式"),
        // AUDCLNT_E_DEVICE_IN_USE
        0x8889_000C => Some("设备正被其它程序以独占方式占用"),
        // AUDCLNT_E_DEVICE_INVALIDATED
        0x8889_0004 => Some("设备已失效（被拔出或已禁用）"),
        _ => None,
    }
}

fn wasapi_err_hint(what: &str, e: &(dyn std::error::Error + 'static)) -> String {
    let code = e
        .downcast_ref::<WinError>()
        .map(|we| we.code().0 as u32);
    let base = wasapi_err(what, e);
    match code.and_then(exclusive_hint) {
        Some(h) => format!("{base}；{h}"),
        None => base,
    }
}

/// 用指定格式与周期初始化独占客户端；周期未对齐（0x88890019）时执行
/// 文档恢复流程：失败态客户端的 GetBufferSize 返回向上对齐的缓冲帧数，
/// 换算成 100ns 周期后先同客户端重试，仍失败则按文档释放旧客户端、
/// 换新客户端重试一次。两种结局都把可继续使用的客户端还给调用方。
fn try_initialize(
    device: &wasapi::Device,
    mut client: wasapi::AudioClient,
    fmt: &WaveFormat,
    period: i64,
    rate: i64,
) -> Result<(WaveFormat, wasapi::AudioClient), (wasapi::AudioClient, String)> {
    let init = |c: &mut wasapi::AudioClient, p: i64| {
        c.initialize_client(fmt, p, &Direction::Render, &ShareMode::Exclusive, false)
    };
    let what = |p: i64| {
        format!(
            "独占初始化失败（{}Hz，周期 {:.2}ms）",
            rate,
            p as f64 / 10_000.0
        )
    };
    match init(&mut client, period) {
        Ok(()) => Ok((fmt.clone(), client)),
        Err(e) => {
            let code = e
                .downcast_ref::<WinError>()
                .map(|we| we.code().0 as u32)
                .unwrap_or(0);
            let first = wasapi_err_hint(&what(period), e.as_ref());
            if code != AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED {
                return Err((client, first));
            }
            let aligned = match client.get_bufferframecount() {
                Ok(frames) => calculate_period_100ns(frames as i64, rate),
                Err(e2) => {
                    let msg = wasapi_err_hint(
                        &format!("{first}；随后获取对齐缓冲也失败"),
                        e2.as_ref(),
                    );
                    return Err((client, msg));
                }
            };
            eprintln!(
                "[wasapi] 周期未对齐（0x88890019），文档恢复：对齐周期 {aligned}（{:.2}ms）",
                aligned as f64 / 10_000.0
            );
            match init(&mut client, aligned) {
                Ok(()) => Ok((fmt.clone(), client)),
                Err(_) => {
                    // 文档步骤：释放旧客户端（drop）、取新客户端、重新 Initialize
                    let mut fresh = match device.get_iaudioclient() {
                        Ok(c) => c,
                        Err(e3) => {
                            let msg =
                                wasapi_err_hint(&format!("{first}；重建客户端也失败"), e3.as_ref());
                            return Err((client, msg));
                        }
                    };
                    drop(client);
                    match init(&mut fresh, aligned) {
                        Ok(()) => Ok((fmt.clone(), fresh)),
                        Err(e4) => {
                            let msg = wasapi_err_hint(
                                &format!("独占初始化失败（{}Hz，对齐周期 {aligned}）", rate),
                                e4.as_ref(),
                            );
                            Err((fresh, msg))
                        }
                    }
                }
            }
        }
    }
}

/// 会话主流程：设备/格式协商 → 回传初始化结果 → 事件驱动喂采样 → 源耗尽退出
fn run_session<S>(
    mut src: S,
    params: ExclusiveParams,
    active: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    tx: mpsc::Sender<Result<(), String>>,
) -> Result<(), String>
where
    S: Source<Item = f32>,
{
    let _ = initialize_mta();
    let init_result = (|| -> Result<(), String> {
        let device = pick_device(params.device_pref.as_deref())?;
        let mut audio_client = device
            .get_iaudioclient()
            .map_err(|e| wasapi_err("获取音频客户端失败", e.as_ref()))?;

        // 格式协商：同一客户端实例上依次尝试「格式 × 周期」。
        // 实测要点（用原生 COM 调用逐设备实测得出）：
        // 1. Initialize 必须使用 quirks 探测返回的格式——驱动接受普通
        //    WAVEFORMATEX 变体时，用原 EXTENSIBLE 结构初始化会被拒；
        // 2. 周期未按驱动要求对齐时返回 0x88890019，按文档流程恢复：
        //    对该客户端调 GetBufferSize 拿「向上对齐」的缓冲帧数，
        //    换算成 100ns 周期后重新 Initialize（必要时换新客户端）；
        // 3. 每轮候选之间检查 stop，避免引擎超时放弃后线程仍继续抢设备。
        let ch = params.channels;
        let candidates: Vec<(usize, usize, SampleType)> = vec![
            (32, 24, SampleType::Int),
            (32, 32, SampleType::Int),
            (24, 24, SampleType::Int),
            (16, 16, SampleType::Int),
            (32, 32, SampleType::Float),
        ];
        let mix_rate = audio_client
            .get_mixformat()
            .map(|f| f.get_samplespersec())
            .unwrap_or(48_000);
        let (def_period, min_period) = audio_client
            .get_periods()
            .map_err(|e| wasapi_err("独占模式获取周期失败", e.as_ref()))?;

        let mut fmt_cands: Vec<(usize, usize, SampleType, u32)> = Vec::new();
        for &(store, valid, kind) in &candidates {
            fmt_cands.push((store, valid, kind, params.src_rate as u32));
        }
        if mix_rate != params.src_rate {
            for &(store, valid, kind) in &candidates {
                fmt_cands.push((store, valid, kind, mix_rate));
            }
        }

        let mut last_err: Option<String> = None;
        let mut probe_rejected = 0usize;
        let mut inited: Option<(WaveFormat, FmtSpec, f64)> = None;

        'formats: for (store, valid, kind, rate) in fmt_cands {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let wf = WaveFormat::new(store, valid, &kind, rate as usize, ch, None);
            let accepted = match audio_client.is_supported_exclusive_with_quirks(&wf) {
                Ok(f) => f,
                Err(_) => {
                    probe_rejected += 1;
                    continue;
                }
            };
            let blockalign = accepted.get_blockalign() as usize;
            let valid_bits = accepted.get_validbitspersample();
            let store_bits = accepted.get_bitspersample();
            let fmt_spec = FmtSpec {
                kind: if accepted.get_subformat().unwrap_or(SampleType::Int)
                    == SampleType::Float
                {
                    SampleKind::F32
                } else if store_bits == 16 {
                    // 普通 WAVEFORMATEX 变体的 wValidBitsPerSample 为 0，
                    // 必须按存储位深判断（按 valid 判断会错落到 I32 造成乱码）
                    SampleKind::I16
                } else if store_bits == 32 && valid_bits == 24 {
                    SampleKind::I24In32
                } else if store_bits == 24 {
                    SampleKind::I24
                } else {
                    SampleKind::I32
                },
                bytes_per_sample: blockalign / ch.max(1),
            };
            let ratio = (params.src_rate as f64) / (rate as f64);

            for &period in &[def_period, min_period] {
                match try_initialize(&device, audio_client, &accepted, period, rate as i64) {
                    Ok((final_fmt, client)) => {
                        audio_client = client;
                        inited = Some((final_fmt, fmt_spec, ratio));
                        eprintln!(
                            "[wasapi] 独占格式就绪：源 {}Hz → 设备 {}Hz × {}ch，{} 字节/帧，有效 {} 位",
                            params.src_rate, rate, ch, blockalign, valid_bits
                        );
                        break 'formats;
                    }
                    Err((client, err)) => {
                        audio_client = client;
                        last_err = Some(err);
                    }
                }
            }
        }

        let (wave_fmt, fmt, rate_ratio) = inited.ok_or_else(|| {
            // 所有候选都在探测阶段被拒：驱动不给独占任何格式，给出可行动提示
            if probe_rejected > 0 && last_err.is_none() {
                return format!(
                    "设备拒绝了独占模式的全部候选格式（{ch} 声道）；该设备驱动不支持独占模式（常见于蓝牙/网络音箱），或系统禁用了独占授权，可切换到其它输出设备再试"
                );
            }
            let detail = last_err.unwrap_or_else(|| "未知".into());
            format!("设备不接受独占模式的任何格式/周期组合（{ch} 声道）。{detail}")
        })?;
        let dev_rate = wave_fmt.get_samplespersec();
        let blockalign = fmt.bytes_per_sample * ch.max(1);

        // 引擎可能在长时间协商期间已放弃（超时/切歌/关开关）：
        // 此时不再占用设备，立即释放并退出
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }

        let h_event = audio_client
            .set_get_eventhandle()
            .map_err(|e| wasapi_err("独占模式创建事件失败", e.as_ref()))?;
        let render = audio_client
            .get_audiorenderclient()
            .map_err(|e| wasapi_err("独占模式获取渲染端失败", e.as_ref()))?;
        audio_client.start_stream().map_err(|e| {
            let e = format!("独占模式启动流失败: {e}");
            let _ = tx.send(Err(e.clone()));
            e
        })?;
        // 初始化完成：立即回传结果（引擎据此决定独占或回退共享），随后进入喂采样循环
        let _ = tx.send(Ok(()));
        eprintln!(
            "[wasapi] 独占播放：{}Hz × {}ch，{} 字节/帧，重采样比 {:.3}",
            dev_rate, ch, blockalign, rate_ratio,
        );

        // step：每个输出帧消耗的输入帧数（速率比 × 倍速）
        let speed = f32::from_bits(params.speed_bits.load(Ordering::Relaxed)).max(0.5);
        let mut resampler = LinearResampler {
            step: rate_ratio * speed as f64,
            pos: 0.0,
            prev: None,
            cur: None,
            done: false,
        };

        // 独占事件驱动模式的标准喂采样循环：每次事件写满整个缓冲区，
        // 不查询 padding（部分驱动的独占 padding 语义不可靠，按空间写会
        // 超量喂入，表现为进度条倍速 + 采样乱码）。
        let buffer_frames = audio_client
            .get_bufferframecount()
            .map_err(|e| wasapi_err("独占模式获取缓冲大小失败", e.as_ref()))?
            as usize;
        eprintln!("[wasapi] 独占缓冲 = {buffer_frames} 帧/周期");
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if h_event.wait_for_event(1000).is_err() {
                break;
            }
            if stop.load(Ordering::Relaxed) {
                break;
            }
            if paused.load(Ordering::Relaxed) {
                // 暂停：写静音帧，不消耗采样源（进度冻结）
                let z = silence_bytes(buffer_frames, blockalign);
                let _ = render.write_to_device(
                    buffer_frames,
                    blockalign,
                    &z,
                    Some(silent_flags()),
                );
                continue;
            }
            let volume = f32::from_bits(params.volume_bits.load(Ordering::Relaxed));
            let mut bytes = Vec::with_capacity(buffer_frames * blockalign);
            let mut written = 0usize;
            let mut ended = false;
            for _ in 0..buffer_frames {
                match resampler.next_frame(&mut src, ch) {
                    Some(frame) => {
                        append_frame(&mut bytes, &frame, &fmt, volume);
                        written += 1;
                    }
                    None => {
                        ended = true;
                        break;
                    }
                }
            }
            // 源耗尽后余下缓冲填静音（保持缓冲完整），随后退出会话
            if written < buffer_frames {
                bytes
                    .extend(silence_bytes(buffer_frames - written, blockalign));
            }
            let _ = render.write_to_device(buffer_frames, blockalign, &bytes, None);
            if ended {
                audio_client.stop_stream().ok();
                active.store(false, Ordering::Relaxed);
                return Ok(());
            }
        }
        audio_client.stop_stream().ok();
        active.store(false, Ordering::Relaxed);
        Ok(())
    })();

    // 无论成败，线程退出即会话结束
    active.store(false, Ordering::Relaxed);
    init_result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 无限静音源（测试手动停止，不依赖自然播完）
    struct Silence {
        rate: u32,
        ch: u16,
    }
    impl Iterator for Silence {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            Some(0.0)
        }
    }
    impl Source for Silence {
        fn sample_rate(&self) -> u32 {
            self.rate
        }
        fn channels(&self) -> u16 {
            self.ch
        }
        fn current_frame_len(&self) -> Option<usize> {
            None
        }
        fn total_duration(&self) -> Option<std::time::Duration> {
            None
        }
    }

    use rodio::cpal::traits::{DeviceTrait, HostTrait};

    fn try_start(device_name: &str, rate: u32) -> Result<ExclusiveCtl, String> {
        let (ctl, rx) = spawn_exclusive_session(
            Silence { rate, ch: 2 },
            ExclusiveParams {
                device_pref: Some(device_name.to_string()),
                channels: 2,
                src_rate: rate,
                volume_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
                speed_bits: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            },
        )?;
        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(ctl),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("初始化超时".into()),
        }
    }

    /// 端到端验证（需要真实音频设备，默认忽略）：
    /// cargo test --bin rustmusic exclusive_lifecycle_and_release -- --ignored --nocapture
    ///
    /// 1. 独占会话能在支持的设备上建立；
    /// 2. 停止 + 等待线程退出后设备立刻交还系统（共享流能重新打开，
    ///    对应"关闭独占后其它软件恢复出声"）；
    /// 3. 释放后立即再开第二个独占会话不撞 DEVICE_IN_USE（切歌/重建场景）。
    #[test]
    #[ignore]
    fn exclusive_lifecycle_and_release() {
        let _ = initialize_mta();
        let collection = DeviceCollection::new(&Direction::Render).unwrap();
        let mut tested = false;
        for dev in collection.into_iter().flatten() {
            let name = dev.get_friendlyname().unwrap_or_default();
            let ctl = match try_start(&name, 48_000) {
                Ok(c) => c,
                Err(e) => {
                    println!("[跳过] {name}：{e}");
                    continue;
                }
            };
            println!("[独占建立] {name}");
            tested = true;

            // 播一小段后停止，等待线程退出并释放设备
            std::thread::sleep(std::time::Duration::from_millis(300));
            assert!(wait_session_exit(&ctl, 2500), "会话线程未在 2.5s 内退出");
            println!("[设备已释放]");

            // 设备交还后共享流必须能打开（其它软件恢复出声的等价条件）
            let host = rodio::cpal::default_host();
            let cpal_dev = host
                .output_devices()
                .unwrap()
                .find(|d| d.name().ok().as_deref() == Some(name.as_str()))
                .expect("cpal 侧找不到同名设备");
            rodio::OutputStream::try_from_device(&cpal_dev)
                .expect("释放独占后共享输出流应能打开");
            println!("[共享流可打开]");

            // 释放后立即重建独占会话（切歌/倍速重建场景，不撞 DEVICE_IN_USE）
            let ctl2 = try_start(&name, 48_000).expect("释放后重建独占会话失败");
            std::thread::sleep(std::time::Duration::from_millis(200));
            assert!(wait_session_exit(&ctl2, 2500));
            println!("[重建独占 OK]");

            // 44100Hz 内容：多数设备默认 48kHz，独占下常触发
            // BUFFER_SIZE_NOT_ALIGNED（0x88890019），验证文档恢复流程可用
            let ctl3 = try_start(&name, 44_100);
            match ctl3 {
                Ok(c) => {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    assert!(wait_session_exit(&c, 2500));
                    println!("[44100Hz 独占 OK]");
                }
                Err(e) => println!("[44100Hz 不支持] {e}"),
            }
            println!();
        }
        assert!(tested, "没有找到支持独占模式的设备（本机可能都不支持）");
    }
}
