//! symphonia 直连的文件解码源：用于 rodio 的 symphonia 包装层无法初始化的
//! fragmented MP4（B 站 DASH m4s 缓存，rodio 0.20.1 初始化时把 SeekError
//! 视为 unreachable 直接 panic）。本模块直接探针+解码，实现 rodio::Source；
//! seek 通过 open_at 跳包实现（demux 级，不解码，瞬时完成）；
//! 缓冲全部持久复用，不在音频回调线程分配内存（避免爆音）。

use std::fs::File;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 连续解码失败上限（超过即判定文件损坏，停止产出样本）
const MAX_DECODE_ERRORS: u32 = 3;
/// refill 攒批目标（帧）：约 340ms@48kHz，摊薄音频线程的解码次数
const REFILL_TARGET_FRAMES: usize = 16_384;

pub struct SymphoniaSource {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    /// 包时间基（seek 跳包用）
    time_base: Option<symphonia::core::units::TimeBase>,
    /// 已解码待取的交错样本（持久缓冲，容量只增不减）
    buf: Vec<f32>,
    idx: usize,
    channels: u16,
    sample_rate: u32,
    total_frames: Option<u64>,
    /// 已产出的帧数（诊断用）
    frames_out: u64,
    /// 文件是否已读到末尾
    eos: bool,
    errors: u32,
}

impl SymphoniaSource {
    /// 打开并探针文件（fragmented MP4 / 常规容器均可）
    pub fn open(path: &str) -> Result<Self, String> {
        Self::open_at(path, 0)
    }

    /// 打开本地文件并跳到指定位置：按包时间戳跳过（不解码），seek 瞬时完成。
    pub fn open_at(path: &str, skip_ms: u64) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("打开音频文件失败: {e}"))?;
        let ext = path.rsplit('.').next().unwrap_or("").to_string();
        Self::open_source(Box::new(file), &ext, skip_ms)
    }

    /// 流式打开（边下边播）：数据源是阻塞式"会生长的文件"读取器，
    /// 读取越过下载前沿时由读取器内部等待，解码逻辑与本地文件完全一致。
    pub fn open_streaming(
        shared: std::sync::Arc<crate::streaming::StreamingDownload>,
        skip_ms: u64,
    ) -> Result<Self, String> {
        let file = crate::streaming::StreamingFile::open(shared)
            .map_err(|e| format!("打开下载缓存失败: {e}"))?;
        Self::open_source(Box::new(file), "", skip_ms)
    }

    fn open_source(src: Box<dyn MediaSource>, ext: &str, skip_ms: u64) -> Result<Self, String> {
        let mss = MediaSourceStream::new(src, Default::default());
        let mut hint = Hint::new();
        // 扩展名提示（m4s 等非标准扩展交给 hint）
        if !ext.is_empty() {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions {
                    enable_gapless: false,
                    ..Default::default()
                },
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("音频容器解析失败: {e}"))?;
        let mut format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("音频文件中没有可解码的轨道")?
            .clone();
        let track_id = track.id;
        let params = &track.codec_params;
        let channels = params.channels.map(|c| c.count() as u16).unwrap_or(2);
        let sample_rate = params.sample_rate.unwrap_or(44_100);
        let time_base = params.time_base;
        let total_frames = params.n_frames.and_then(|n| {
            params
                .time_base
                .map(|tb| (tb.calc_time(n).seconds as f64 * sample_rate as f64) as u64)
        });
        let decoder = symphonia::default::get_codecs()
            .make(params, &DecoderOptions::default())
            .map_err(|e| format!("音频解码器创建失败: {e}"))?;
        let mut src = Self {
            format,
            decoder,
            track_id,
            time_base,
            buf: Vec::new(),
            idx: 0,
            channels,
            sample_rate,
            total_frames,
            frames_out: 0,
            eos: false,
            errors: 0,
        };
        // seek：丢弃时间戳早于目标的包（纯 demux，不解码）
        if skip_ms > 0 {
            let mut landed_ms: Option<u64> = None;
            // 最后一个见到的音频包位置：目标越过曲末时（跳包中途到 EOF），
            // 位置必须落在真实音频内——否则引擎以幻影位置起播、随即 EOS，
            // 被 monitor 误判成"自然播完"而自动切歌（表现为点进度条随机切歌）
            let mut last_ms: Option<u64> = None;
            loop {
                let ts = match src.format.next_packet() {
                    Ok(p) if p.track_id() == src.track_id => p.ts(),
                    Ok(_) => continue, // 非音轨包，跳过
                    Err(_) => break,   // 到末尾
                };
                // 毫秒要含亚秒部分：seconds*1000 会丢掉 <1s 的差异，
                // skip_ms 落在包间隙时误跳到下一包（~1s 误差）。
                // 直接 ts × numer × 1000 / denom，u128 防溢出
                let ms = src
                    .time_base
                    .map(|tb| ((ts as u128 * tb.numer as u128 * 1000) / tb.denom as u128) as u64)
                    .unwrap_or(u64::MAX);
                last_ms = Some(ms);
                if ms >= skip_ms {
                    landed_ms = Some(ms);
                    break;
                }
            }
            // frames_out 按实际落点（首个保留包的时间戳）起算：
            // 引擎报告的播放位置才与真实音频位置对齐（理想 skip_ms 有偏差）
            src.frames_out = match landed_ms.or(last_ms) {
                Some(ms) => ms * src.sample_rate as u64 / 1000,
                None => skip_ms * src.sample_rate as u64 / 1000,
            };
        }
        Ok(src)
    }

    /// 已解码的帧数（诊断用）
    #[allow(dead_code)]
    pub fn frames_decoded(&self) -> u64 {
        self.frames_out
    }

    /// 解码音频包并追加到持久缓冲。返回 false = 到末尾或不可恢复。
    fn decode_one(&mut self) -> bool {
        loop {
            if self.eos {
                return false;
            }
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(SymphoniaError::ResetRequired) | Err(SymphoniaError::IoError(_)) => {
                    // isomp4 到末尾表现为 IoError(UnexpectedEof)
                    self.eos = true;
                    return false;
                }
                Err(e) => {
                    eprintln!("[symdec] 读取音频包失败: {e}");
                    self.eos = true;
                    return false;
                }
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    self.errors = 0;
                    let spec = *decoded.spec();
                    self.channels = spec.channels.count() as u16;
                    self.sample_rate = spec.rate;
                    let frames = decoded.frames() as u64;
                    // SampleBuffer 只在容量不足时重建（避免音频线程分配）
                    let need = decoded.capacity() as u64;
                    let mut sbuf = SampleBuffer::<f32>::new(need, spec);
                    sbuf.copy_interleaved_ref(decoded);
                    self.frames_out += frames;
                    self.buf.extend_from_slice(sbuf.samples());
                    return true;
                }
                Err(SymphoniaError::DecodeError(e)) => {
                    // 单包解码失败可跳过（B 站流末尾偶有坏包），连续失败才放弃
                    self.errors += 1;
                    eprintln!("[symdec] 解码错误（ts={}）: {e}", packet.ts());
                    if self.errors >= MAX_DECODE_ERRORS {
                        self.eos = true;
                        return false;
                    }
                    continue;
                }
                Err(e) => {
                    eprintln!("[symdec] 解码终止: {e}");
                    self.eos = true;
                    return false;
                }
            }
        }
    }

    /// 缓冲不足时连续解码至目标量（摊薄音频线程上的调用次数）
    fn refill(&mut self) -> bool {
        if self.eos {
            return false;
        }
        while self.buf.len() < REFILL_TARGET_FRAMES * self.channels.max(1) as usize {
            if !self.decode_one() {
                break;
            }
        }
        !self.buf.is_empty()
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.idx >= self.buf.len() {
            self.buf.clear();
            self.idx = 0;
            if !self.refill() {
                return None;
            }
        }
        let s = self.buf[self.idx];
        self.idx += 1;
        Some(s)
    }
}

impl rodio::Source for SymphoniaSource {
    fn current_span_len(&self) -> Option<usize> {
        let remaining = (self.buf.len() - self.idx.min(self.buf.len()))
            / self.channels.max(1) as usize;
        if self.eos && remaining == 0 {
            Some(0)
        } else {
            Some(remaining)
        }
    }

    fn channels(&self) -> rodio::ChannelCount {
        rodio::ChannelCount::new(self.channels).unwrap_or(rodio::ChannelCount::new(2).unwrap())
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        rodio::SampleRate::new(self.sample_rate).unwrap_or(rodio::SampleRate::new(48000).unwrap())
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_frames
            .map(|f| Duration::from_secs_f64(f as f64 / self.sample_rate as f64))
    }
}
