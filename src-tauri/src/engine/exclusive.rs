//! WASAPI 独占播放会话管理：会话协商/终止、设备切换与倍速下的会话重建、共享流失效标记。
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use rodio::Source;
use tauri::Emitter;

use super::{Engine, PlayState, TrackInfo};
use crate::eq::EqSource;
use crate::wasapi_out;

impl Engine {
    /// 启动 WASAPI 独占播放会话（skip_ms 用于 seek/重建时跳过开头）。
    /// 初始化在会话线程完成，此处阻塞等待协商结果（最多 3s）；
    /// 失败时采样源已被会话线程取走，由调用方重新解码回退共享模式。
    pub(super) fn start_exclusive(&self, info: &TrackInfo, skip_ms: u64) -> Result<(), String> {
        self.rebuilding.store(true, Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            // 终止旧会话并等线程真正退出（设备随音频客户端释放）：
            // 旧会话还占着设备时开新会话，Initialize 会全部撞上 DEVICE_IN_USE
            if let Some(ctl) = self.excl.read().clone() {
                if !wasapi_out::wait_session_exit(&ctl, 2500) {
                    eprintln!("[engine] 旧独占会话未按时退出，继续尝试新会话");
                }
            }
            *self.excl.write() = None;
            // DASH（B 站 fMP4）由 helper 走 symphonia 直连源（rodio 会 panic），
            // 其余格式 rodio 解码 + skip_duration
            let src = self.open_current_source(&info.path, skip_ms)?;
            let wrapped =
                EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), skip_ms as f64);
            let params = wasapi_out::ExclusiveParams {
                device_pref: self.device_pref.read().clone(),
                channels: wrapped.channels().get() as usize,
                src_rate: wrapped.sample_rate().get(),
                volume_bits: self.volume.clone(),
                speed_bits: self.speed.clone(),
            };
            let (ctl, rx) = wasapi_out::spawn_exclusive_session(wrapped, params)?;
            // 会话一创建就注册控制句柄：协商期间超时/切歌/关开关也能终止它，
            // 防止孤儿线程晚一步拿到设备后无人可控（表现为其它软件一直无声）
            *self.excl.write() = Some(ctl.clone());
            match rx.recv_timeout(Duration::from_secs(3)) {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // 会话已退出（结果在退出后才发出），设备已释放
                    *self.excl.write() = None;
                    return Err(e);
                }
                Err(_) => {
                    // 协商超时：要求会话退出并等设备释放
                    let released = wasapi_out::wait_session_exit(&ctl, 2500);
                    if released {
                        *self.excl.write() = None;
                    } else {
                        // 线程卡在驱动调用里：保留句柄以便后续继续尝试终止，
                        // 并标记共享流可能失效（设备或被晚到的会话占用）
                        eprintln!("[engine] 独占会话协商超时且未退出，保留控制句柄");
                        self.shared_broken.store(true, Ordering::Relaxed);
                    }
                    return Err("独占模式初始化超时".into());
                }
            }
            self.shared_broken.store(true, Ordering::Relaxed);
            self.sink.read().clear();
            self.pos_ms.store(skip_ms, Ordering::Relaxed);
            self.dur_ms.store(info.duration_ms, Ordering::Relaxed);
            self.user_paused.store(false, Ordering::Relaxed);
            self.stopped.store(false, Ordering::Relaxed);
            Ok(())
        })();
        self.rebuilding.store(false, Ordering::Relaxed);
        if result.is_ok() {
            let info = info.clone();
            *self.current.write() = Some(info.clone());
            self.notify_smtc();
            let seq = self.play_seq.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = self.app.emit(
                "player://state",
                PlayState {
                    playing: true,
                    info,
                    seq,
                },
            );
        }
        result
    }

    /// 重建独占会话（seek / 倍速 / 设备切换时），保持暂停状态
    pub(super) fn rebuild_exclusive_at(
        &self,
        info: &TrackInfo,
        ms: u64,
        was_paused: bool,
    ) -> Result<(), String> {
        let result = self.start_exclusive(info, ms);
        if result.is_ok() {
            if let Some(ctl) = self.excl.read().as_ref() {
                ctl.paused.store(was_paused, Ordering::Relaxed);
            }
            self.user_paused.store(was_paused, Ordering::Relaxed);
        }
        result
    }

    /// 停止独占会话并立即切回共享模式（从当前进度续播，保持暂停状态）。
    /// 用于关闭独占开关时立刻把设备还给系统混音器，恢复其它应用出声。
    pub fn stop_exclusive_resume_shared(self: &Arc<Self>) -> Result<(), String> {
        // 没有活跃独占会话、共享流也未被破坏时无需动作：
        // 避免反复拨动开关时无故重启共享播放链（当前曲目会跳一下）
        if !self.excl_active() && !self.shared_broken.load(Ordering::Relaxed) {
            return Ok(());
        }
        let info = self.current.read().clone();
        let pos = self.pos_ms.load(Ordering::Relaxed);
        let was_paused = self.user_paused.load(Ordering::Relaxed);
        // 已手动停止：只交还设备、重建共享流，不重新开始播放
        let was_stopped = self.stopped.load(Ordering::Relaxed);
        // 必须先等会话线程退出、设备交还系统混音器，再重建共享输出流：
        // 设备被独占期间新共享流打不开，会落得"关了独占还是无声"
        if let Some(ctl) = self.excl.read().clone() {
            if !wasapi_out::wait_session_exit(&ctl, 2500) {
                eprintln!("[engine] 独占会话未按时退出，设备可能仍被占用");
            }
        }
        *self.excl.write() = None;
        self.shared_broken.store(true, Ordering::Relaxed);
        self.rebuilding.store(true, Ordering::Relaxed);
        let result = (|| -> Result<(), String> {
            self.rebuild_shared_output();
            if let Some(info) = &info {
                if was_stopped {
                    return Ok(());
                }
                let src = self.open_current_source(&info.path, pos)?;
                let wrapped =
                    EqSource::with_base(src, self.eq.clone(), self.pos_ms.clone(), pos as f64);
                {
                    let sink = self.sink.read();
                    sink.clear();
                    sink.append(wrapped);
                    sink.set_volume(f32::from_bits(self.volume.load(Ordering::Relaxed)));
                    sink.set_speed(f32::from_bits(self.speed.load(Ordering::Relaxed)));
                    if was_paused {
                        sink.pause();
                    } else {
                        sink.play();
                    }
                }
                self.stopped.store(false, Ordering::Relaxed);
            }
            Ok(())
        })();
        self.rebuilding.store(false, Ordering::Relaxed);
        self.notify_smtc();
        self.emit_state(!was_paused && info.is_some() && !was_stopped);
        result
    }

    pub(super) fn excl_active(&self) -> bool {
        self.excl
            .read()
            .as_ref()
            .map(|c| c.active.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
    /// WASAPI 独占模式开关（设置项；切换后下一首生效）
    pub fn set_exclusive_enabled(&self, enabled: bool) {
        self.exclusive_enabled.store(enabled, Ordering::Relaxed);
        eprintln!(
            "[engine] WASAPI 独占模式 = {}",
            if enabled { "开" } else { "关" }
        );
    }
}
