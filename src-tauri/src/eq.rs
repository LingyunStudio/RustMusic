use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use rodio::Source;

pub const BAND_FREQS: [f64; 10] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// 均衡器参数（引擎与 UI 共享）
pub struct EqShared {
    pub gains: RwLock<[f32; 10]>,
    pub enabled: AtomicBool,
    pub version: AtomicU64,
}

impl EqShared {
    pub fn new(gains: [f32; 10], enabled: bool) -> Self {
        Self {
            gains: RwLock::new(gains),
            enabled: AtomicBool::new(enabled),
            version: AtomicU64::new(1),
        }
    }

    pub fn set(&self, gains: [f32; 10], enabled: bool) {
        *self.gains.write() = gains;
        self.enabled.store(enabled, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, Default)]
struct Biquad {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl Biquad {
    #[inline]
    fn process(&mut self, c: &[f64; 5], x: f64) -> f64 {
        let y = c[0] * x + c[1] * self.x1 + c[2] * self.x2 - c[3] * self.y1 - c[4] * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn reset(&mut self) {
        *self = Biquad::default();
    }
}

/// RBJ cookbook 双二阶滤波器系数：[b0, b1, b2, a1, a2]（已按 a0 归一化）
fn design_band(i: usize, sr: f64, gain_db: f64) -> [f64; 5] {
    let f0 = BAND_FREQS[i].min(sr * 0.45).max(10.0);
    let w0 = 2.0 * std::f64::consts::PI * f0 / sr;
    let cosw = w0.cos();
    let sinw = w0.sin();
    let a = 10f64.powf(gain_db / 40.0);
    let sq = a.sqrt();

    let (b0, b1, b2, a0, a1, a2);
    if i == 0 {
        // lowshelf，S=1
        let alpha = sinw / 2.0 * std::f64::consts::SQRT_2;
        b0 = a * ((a + 1.0) - (a - 1.0) * cosw + 2.0 * sq * alpha);
        b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cosw);
        b2 = a * ((a + 1.0) - (a - 1.0) * cosw - 2.0 * sq * alpha);
        a0 = (a + 1.0) + (a - 1.0) * cosw + 2.0 * sq * alpha;
        a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cosw);
        a2 = (a + 1.0) + (a - 1.0) * cosw - 2.0 * sq * alpha;
    } else if i == 9 {
        // highshelf，S=1
        let alpha = sinw / 2.0 * std::f64::consts::SQRT_2;
        b0 = a * ((a + 1.0) + (a - 1.0) * cosw + 2.0 * sq * alpha);
        b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cosw);
        b2 = a * ((a + 1.0) + (a - 1.0) * cosw - 2.0 * sq * alpha);
        a0 = (a + 1.0) - (a - 1.0) * cosw + 2.0 * sq * alpha;
        a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cosw);
        a2 = (a + 1.0) - (a - 1.0) * cosw - 2.0 * sq * alpha;
    } else {
        // peaking，约一个倍频程带宽
        let q = 1.414;
        let alpha = sinw / (2.0 * q);
        b0 = 1.0 + alpha * a;
        b1 = -2.0 * cosw;
        b2 = 1.0 - alpha * a;
        a0 = 1.0 + alpha / a;
        a1 = -2.0 * cosw;
        a2 = 1.0 - alpha / a;
    }
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

/// 包裹 rodio 音源的 10 段均衡器，同时负责播放位置统计（供进度条/SMTC 使用）
pub struct EqSource<S> {
    inner: S,
    channels: usize,
    cur: usize,
    sr: f64,
    shared: Arc<EqShared>,
    ver: u64,
    coeffs: Vec<[f64; 5]>,
    states: Vec<Vec<Biquad>>,
    frames: u64,
    base_ms: f64,
    pos_ms: Arc<AtomicU64>,
}

impl<S: Source<Item = f32>> EqSource<S> {
    pub fn new(inner: S, shared: Arc<EqShared>, pos_ms: Arc<AtomicU64>) -> Self {
        Self::with_base(inner, shared, pos_ms, 0.0)
    }

    /// base_ms：流起始的时间偏移（skip_duration 跳过的部分）
    pub fn with_base(
        inner: S,
        shared: Arc<EqShared>,
        pos_ms: Arc<AtomicU64>,
        base_ms: f64,
    ) -> Self {
        let channels = (inner.channels() as usize).max(1);
        let sr = inner.sample_rate() as f64;
        let mut s = Self {
            inner,
            channels,
            cur: 0,
            sr,
            shared,
            ver: u64::MAX,
            coeffs: Vec::new(),
            states: Vec::new(),
            frames: 0,
            base_ms,
            pos_ms,
        };
        s.refresh();
        s
    }

    fn refresh(&mut self) {
        let v = self.shared.version.load(Ordering::Relaxed);
        if v != self.ver {
            let gains = *self.shared.gains.read();
            self.coeffs = (0..10)
                .map(|i| design_band(i, self.sr, gains[i] as f64))
                .collect();
            self.states = vec![vec![Biquad::default(); 10]; self.channels];
            self.ver = v;
        }
    }
}

impl<S: Source<Item = f32>> Iterator for EqSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let x = self.inner.next()?;
        let out = if self.shared.enabled.load(Ordering::Relaxed) {
            self.refresh();
            let mut s = x as f64;
            let ch = self.cur;
            let coeffs = &self.coeffs;
            for (b, c) in self.states[ch].iter_mut().zip(coeffs.iter()) {
                s = b.process(c, s);
            }
            s as f32
        } else {
            x
        };
        self.cur += 1;
        if self.cur >= self.channels {
            self.cur = 0;
            self.frames += 1;
            let pos = self.base_ms + self.frames as f64 * 1000.0 / self.sr;
            self.pos_ms.store(pos.max(0.0) as u64, Ordering::Relaxed);
        }
        Some(out)
    }
}

impl<S: Source<Item = f32>> Source for EqSource<S> {
    fn current_frame_len(&self) -> Option<usize> {
        self.inner.current_frame_len()
    }

    fn channels(&self) -> u16 {
        self.channels as u16
    }

    fn sample_rate(&self) -> u32 {
        self.sr as u32
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.inner.try_seek(pos)?;
        self.base_ms = pos.as_secs_f64() * 1000.0;
        self.frames = 0;
        for ch in &mut self.states {
            for b in ch {
                b.reset();
            }
        }
        self.pos_ms.store(self.base_ms as u64, Ordering::Relaxed);
        Ok(())
    }
}
