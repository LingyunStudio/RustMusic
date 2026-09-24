//! WASAPI 独占模式输出（可选功能，仅 Windows）
//!
//! 绕过 Windows 系统混音器直接与声卡通信：采样率按源文件直通、不经系统
//! 重采样，独占期间其它应用无法出声。格式协商由本模块完成，任何失败都
//! 返回 Err，由引擎回退到常规共享模式播放，不影响可用性。
//!
//! 采样处理链复用引擎的 EqSource（均衡器 + 播放位置统计），音量按采样
//! 乘法施加（独占模式绕过了系统音量）。采样率与设备不一致时用线性重采样
//! 对齐（与 rodio 共享模式的 Speed 节点同级别质量）。

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use rodio::Source;

use wasapi::{
    initialize_mta, BufferFlags, DeviceCollection, Direction, SampleType, ShareMode, WaveFormat,
};
use windows51::core::Error as WinError;

/// AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED
const AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED: i32 = 0x8889_000Au32 as i32;

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
    let ctl = ExclusiveCtl {
        active: active.clone(),
        paused: paused.clone(),
        stop: stop.clone(),
    };
    let (tx, rx) = mpsc::channel();
    let _handle = std::thread::Builder::new()
        .name("wasapi-exclusive".into())
        .spawn(move || {
            let result = run_session(src, params, active, paused, stop, tx.clone());
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
            (Some(p), Some(c)) if self.pos > 0.0 => {
                p.iter()
                    .zip(c.iter())
                    .map(|(a, b)| a + (b - a) * self.pos as f32)
                    .collect()
            }
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

        // 格式协商：优先源文件原生采样率（位深从高到低），全部拒绝则
        // 退到设备混合采样率（此时启用线性重采样）
        let ch = params.channels;
        let candidates: Vec<(usize, usize, SampleType)> = vec![
            (32, 32, SampleType::Int),
            (24, 24, SampleType::Int),
            (16, 16, SampleType::Int),
            (32, 32, SampleType::Float),
        ];
        let mix_rate = audio_client
            .get_mixformat()
            .map(|f| f.get_samplespersec())
            .unwrap_or(48_000);
        let mut fmt: Option<(WaveFormat, FmtSpec, f64)> = None;
        for &(store, valid, kind) in &candidates {
            let wf = WaveFormat::new(store, valid, &kind, params.src_rate as usize, ch, None);
            if let Ok(accepted) = audio_client.is_supported_exclusive_with_quirks(&wf) {
                fmt = Some((
                    accepted,
                    FmtSpec {
                        kind: if kind == SampleType::Float {
                            SampleKind::F32
                        } else if store == 16 {
                            SampleKind::I16
                        } else if store == 24 {
                            SampleKind::I24
                        } else {
                            SampleKind::I32
                        },
                        bytes_per_sample: 0,
                    },
                    1.0,
                ));
                break;
            }
        }
        if fmt.is_none() {
            for &(store, valid, kind) in &candidates {
                let wf = WaveFormat::new(store, valid, &kind, mix_rate as usize, ch, None);
                if let Ok(accepted) = audio_client.is_supported_exclusive_with_quirks(&wf) {
                    let dev_rate = accepted.get_samplespersec();
                    fmt = Some((
                        accepted,
                        FmtSpec {
                            kind: if kind == SampleType::Float {
                                SampleKind::F32
                            } else if store == 16 {
                                SampleKind::I16
                            } else if store == 24 {
                                SampleKind::I24
                            } else {
                                SampleKind::I32
                            },
                            bytes_per_sample: 0,
                        },
                        // 输出帧率 = 设备率；输入消耗 = 源率 × 倍速
                        (params.src_rate as f64) / (dev_rate as f64),
                    ));
                    break;
                }
            }
        }
        let (wave_fmt, mut fmt, rate_ratio) = fmt
            .ok_or_else(|| format!("设备不支持独占模式的任何候选格式（{ch} 声道）"))?;
        let dev_rate = wave_fmt.get_samplespersec();

        // 周期对齐（部分设备如 Intel HDA 要求 128 字节对齐），按官方示例
        // 处理 AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED 的重试
        let (def_period, min_period) = audio_client
            .get_periods()
            .map_err(|e| wasapi_err("独占模式获取周期失败", e.as_ref()))?;
        let mut desired_period = audio_client
            .calculate_aligned_period_near(3 * min_period / 2, Some(128), &wave_fmt)
            .map_err(|e| wasapi_err("独占模式计算周期失败", e.as_ref()))?;
        for attempt in 0..3 {
            match audio_client.initialize_client(
                &wave_fmt,
                desired_period,
                &Direction::Render,
                &ShareMode::Exclusive,
                false,
            ) {
                Ok(()) => break,
                Err(e) => {
                    let code = e
                        .downcast_ref::<WinError>()
                        .map(|we| we.code().0)
                        .unwrap_or(0);
                    let aligned = code == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED;
                    if !aligned || attempt == 2 {
                        return Err(wasapi_err("独占模式初始化失败", e.as_ref()));
                    }
                    // 按文档流程取下一个对齐缓冲大小后重建客户端再试
                    let buffersize = audio_client.get_bufferframecount().unwrap_or(0);
                    desired_period = wasapi::calculate_period_100ns(
                        buffersize as i64,
                        wave_fmt.get_samplespersec() as i64,
                    );
                    audio_client = device
                        .get_iaudioclient()
                        .map_err(|e| wasapi_err("获取音频客户端失败", e.as_ref()))?;
                    audio_client
                        .initialize_client(
                            &wave_fmt,
                            desired_period,
                            &Direction::Render,
                            &ShareMode::Exclusive,
                            false,
                        )
                        .map_err(|e| wasapi_err("独占模式初始化失败", e.as_ref()))?;
                    break;
                }
            }
        }
        let _ = def_period;

        let blockalign = wave_fmt.get_blockalign() as usize;
        fmt.bytes_per_sample = blockalign / ch.max(1);
        // 有效位宽决定采样转换方式（如 24-in-32）
        let valid = wave_fmt.get_validbitspersample();
        let stored = wave_fmt.get_bitspersample();
        fmt.kind = match fmt.kind {
            SampleKind::F32 => SampleKind::F32,
            SampleKind::I16 => SampleKind::I16,
            SampleKind::I24 => SampleKind::I24,
            _ => {
                if stored == 32 && valid == 24 {
                    SampleKind::I24In32
                } else {
                    SampleKind::I32
                }
            }
        };

        let h_event = audio_client
            .set_get_eventhandle()
            .map_err(|e| wasapi_err("独占模式创建事件失败", e.as_ref()))?;
        let render = audio_client
            .get_audiorenderclient()
            .map_err(|e| wasapi_err("独占模式获取渲染端失败", e.as_ref()))?;
        audio_client
            .start_stream()
            .map_err(|e| {
                let e = format!("独占模式启动流失败: {e}");
                let _ = tx.send(Err(e.clone()));
                e
            })?;
        // 初始化完成：立即回传结果（引擎据此决定独占或回退共享），随后进入喂采样循环
        let _ = tx.send(Ok(()));
        eprintln!(
            "[wasapi] 独占播放：{}Hz × {}ch，{:.0} 字节/帧，重采样比 {:.3}",
            dev_rate,
            ch,
            blockalign as f64,
            rate_ratio,
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
            loop {
                let space = match audio_client.get_available_space_in_frames() {
                    Ok(s) => s as usize,
                    Err(_) => break,
                };
                if space == 0 {
                    break;
                }
                let mut bytes = Vec::with_capacity(space * blockalign);
                if paused.load(Ordering::Relaxed) {
                    // 暂停：写静音帧，不消耗采样源（进度冻结）
                    bytes.extend(silence_bytes(space, blockalign));
                    let _ = render.write_to_device(space, blockalign, &bytes, Some(silent_flags()));
                } else {
                    let volume = f32::from_bits(params.volume_bits.load(Ordering::Relaxed));
                    let mut written = 0usize;
                    let mut ended = false;
                    for _ in 0..space {
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
                    if written > 0 {
                        bytes.truncate(written * blockalign);
                        let _ = render.write_to_device(written, blockalign, &bytes, None);
                    }
                    if ended {
                        // 余下缓冲填静音后退出
                        let rest = space - written;
                        if rest > 0 {
                            let z = silence_bytes(rest, blockalign);
                            let _ =
                                render.write_to_device(rest, blockalign, &z, Some(silent_flags()));
                        }
                        audio_client.stop_stream().ok();
                        active.store(false, Ordering::Relaxed);
                        return Ok(());
                    }
                }
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
