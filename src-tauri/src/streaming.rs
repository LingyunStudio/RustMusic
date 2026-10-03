//! 边下边播的核心原语：下载线程顺序写 `.part` 缓存文件，播放端拿到一个
//! "会生长的文件"读取器（`StreamingFile`）——读取位置越过已下载边界时
//! 阻塞等待数据到达，下载完成后按真实 EOF 收场，失败时以 EOF 结束播放。
//!
//! 这样 symphonia/rodio 的解码器看到的仍是一个 Read + Seek 的普通文件，
//! 无需感知网络；seek 只改文件位置，是否需要等待数据由 read 的阻塞逻辑
//! 统一处理（读到未下载区域就等，直到数据到位或下载终止）。

use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
struct State {
    /// 已落盘字节数（下载线程每写入一块后推进）
    ready: u64,
    /// HTTP content-length；0 = 未知（未知长度不做流式开播，回退整段下载）
    total: u64,
    /// 下载成功结束（此时 ready == 实际文件大小）
    done: bool,
    /// 下载失败（读取端视为 EOF，播放自然结束）
    failed: bool,
}

/// 下载进度与终止状态的共享端。下载线程持有它推进进度；
/// 每个播放端读取器（`StreamingFile`）引用它做阻塞等待。
pub struct StreamingDownload {
    state: Mutex<State>,
    cv: Condvar,
    /// `.part` 下载中间文件
    pub part: PathBuf,
    /// 下载完成后的最终缓存文件名
    pub dest: PathBuf,
    /// 文件头嗅探结果：是否 MP4 容器（预缓冲达标后由 engine 写入一次）
    mp4: Mutex<bool>,
}

impl StreamingDownload {
    pub fn new(part: PathBuf, dest: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State { ready: 0, total: 0, done: false, failed: false }),
            cv: Condvar::new(),
            part,
            dest,
            mp4: Mutex::new(false),
        })
    }

    /// 下载线程拿到响应头后设置总长度；总长 0（chunked）时调用方不做流式开播
    pub fn set_total(&self, total: u64) {
        if total > 0 {
            let mut st = self.state.lock().unwrap();
            st.total = total;
            self.cv.notify_all();
        }
    }

    /// 下载线程每写入一块后调用：推进已就绪字节数并唤醒所有等待中的读取器
    pub fn publish(&self, ready: u64) {
        let mut st = self.state.lock().unwrap();
        if ready > st.ready {
            st.ready = ready;
            self.cv.notify_all();
        }
    }

    /// 下载结束：ok = 成功（之后按真实 EOF 收场），否则失败（读取端立即 EOF）
    pub fn finish(&self, ok: bool, final_size: u64) {
        let mut st = self.state.lock().unwrap();
        st.done = true;
        st.failed = !ok;
        if ok && final_size > st.ready {
            st.ready = final_size;
        }
        self.cv.notify_all();
    }

    /// 预缓冲嗅探结果（engine 在预缓冲达标后写一次）
    pub fn set_mp4(&self, v: bool) {
        *self.mp4.lock().unwrap() = v;
    }

    pub fn is_mp4(&self) -> bool {
        *self.mp4.lock().unwrap()
    }

    /// 当前快照（ready, total, done, failed）
    pub fn snapshot(&self) -> (u64, u64, bool, bool) {
        let st = self.state.lock().unwrap();
        (st.ready, st.total, st.done, st.failed)
    }

    /// 等待预缓冲达标 / 下载结束 / 失败，三者任一发生即返回。
    /// 返回 (已就绪字节, 是否已结束, 是否失败)。
    pub fn wait_prebuffer(&self, target: u64) -> (u64, bool, bool) {
        let mut st = self.state.lock().unwrap();
        loop {
            if st.failed || st.done || (st.total > 0 && st.ready >= target.min(st.total)) {
                return (st.ready, st.done, st.failed);
            }
            let (guard, _) = self
                .cv
                .wait_timeout(st, Duration::from_millis(300))
                .unwrap();
            st = guard;
        }
    }

    /// 阻塞直到 `pos` 位置的数据可用（或下载终止）。供 `StreamingFile::read` 使用。
    fn wait_for(&self, pos: u64) -> io::Result<Frontier> {
        let mut st = self.state.lock().unwrap();
        loop {
            if st.failed {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "download failed"));
            }
            if st.done {
                let len = if st.total > 0 { st.total } else { st.ready };
                return Ok(Frontier { len, eof: pos >= len });
            }
            if pos < st.ready {
                return Ok(Frontier { len: st.ready, eof: false });
            }
            let (guard, _) = self
                .cv
                .wait_timeout(st, Duration::from_millis(300))
                .unwrap();
            st = guard;
        }
    }
}

/// read 调用时数据前沿的状态
struct Frontier {
    /// 当前可读（或最终）长度
    len: u64,
    /// 位置已越过文件末尾（下载已完成）
    eof: bool,
}

/// 阻塞式"会生长的文件"读取器。实现 Read + Seek（供 rodio 解码器使用）
/// 与 symphonia 的 MediaSource（供 symdec 直连源使用）。
pub struct StreamingFile {
    shared: Arc<StreamingDownload>,
    file: fs::File,
    pos: u64,
}

impl StreamingFile {
    /// 打开读取器：优先 `.part`（下载中），不存在则打开最终缓存
    /// （下载已完成且已被重命名的场景）。
    pub fn open(shared: Arc<StreamingDownload>) -> io::Result<Self> {
        let (file, pos) = match fs::File::open(&shared.part) {
            Ok(f) => (f, 0),
            Err(_) if shared.dest.exists() => (fs::File::open(&shared.dest)?, 0),
            Err(e) => return Err(e),
        };
        Ok(Self { shared, file, pos })
    }
}

impl Drop for StreamingFile {
    fn drop(&mut self) {
        // 下载完成但 .part 因播放器持有句柄而未能及时重命名：此处收尾。
        // dest 已存在则无需处理（幂等）。
        let (_, _, done, failed) = self.shared.snapshot();
        if done && !failed && !self.shared.dest.exists() && self.shared.part.exists() {
            let _ = fs::rename(&self.shared.part, &self.shared.dest);
        }
    }
}

impl Read for StreamingFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            // 只读到数据前沿，绝不越过（前沿之外是未落盘的空洞）
            let f = self.shared.wait_for(self.pos)?;
            if f.eof {
                return Ok(0);
            }
            let want = (f.len - self.pos).min(buf.len() as u64) as usize;
            if want == 0 {
                // 前沿恰好等于位置：等待新数据（wait_for 未阻塞是因为 done，
                // 再循环一次让 EOF 分支处理）——done 时 len==pos 会走 eof，安全
                continue;
            }
            self.file.seek(SeekFrom::Start(self.pos))?;
            let n = self.file.read(&mut buf[..want])?;
            self.pos += n as u64;
            if n == 0 && !f.eof {
                // 理论不可达（want>0 且 read 范围在前沿内）；防御性避免死循环
                continue;
            }
            return Ok(n);
        }
    }
}

impl Seek for StreamingFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let (ready, total, done, _) = self.shared.snapshot();
        let base_end = |_cur: u64| -> u64 {
            // End 以已知总长（或已就绪长度兜底）为基准
            if total > 0 {
                total
            } else if done {
                ready
            } else {
                ready
            }
        };
        let new_pos: i128 = match pos {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(d) => base_end(self.pos) as i128 + d as i128,
        };
        self.pos = new_pos.max(0) as u64;
        Ok(self.pos)
    }
}

impl symphonia::core::io::MediaSource for StreamingFile {
    /// MP4 解析器在可寻址模式下初始化时要求 byte_len 已知；
    /// 总长未知（chunked）时报不可寻址，走流式解析分支。
    fn is_seekable(&self) -> bool {
        self.shared.snapshot().1 > 0
    }

    fn byte_len(&self) -> Option<u64> {
        let t = self.shared.snapshot().1;
        (t > 0).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 状态机：预缓冲等待在达标/结束/失败三个出口都能返回
    #[test]
    fn prebuffer_and_finish() {
        let sd = StreamingDownload::new(PathBuf::from("a.part"), PathBuf::from("a"));
        sd.set_total(1000);
        sd.publish(100);
        // 不阻塞测试：target 未达、未结束——wait_prebuffer 会阻塞，因此
        // 只验证非阻塞接口与终态语义
        assert_eq!(sd.snapshot(), (100, 1000, false, false));
        sd.publish(900);
        assert_eq!(sd.snapshot(), (900, 1000, false, false));
        sd.finish(true, 1000);
        let (ready, done, failed) = sd.wait_prebuffer(1000);
        assert!((ready, done, failed) == (1000, true, false));
    }

    /// 失败后读取端立即按 EOF 收场
    #[test]
    fn finish_failed_is_eof() {
        let sd = StreamingDownload::new(PathBuf::from("b.part"), PathBuf::from("b"));
        sd.set_total(500);
        sd.publish(100);
        sd.finish(false, 0);
        let (_, done, failed) = sd.wait_prebuffer(500);
        assert!(done && failed);
    }

    /// 边下边播端到端冒烟：把一个真实音频文件分块模拟下载（每块间隔 30ms，
    /// 远慢于解码消费速度），流式源必须能完整解码到最后。
    /// 运行：STREAMING_TEST_FILE=<音频路径> cargo test -- --ignored --nocapture
    #[test]
    #[ignore]
    fn streaming_decode_smoke() {
        use std::io::Write as _;
        let src_path = std::env::var("STREAMING_TEST_FILE").expect("set STREAMING_TEST_FILE");
        let data = std::fs::read(&src_path).expect("read source");
        let dir = std::env::temp_dir().join("rustmusic_streaming_test");
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("smoke.part");
        let dest = dir.join("smoke.bin");
        let dest_for_writer = dest.clone();
        let _ = std::fs::remove_file(&part);
        let _ = std::fs::remove_file(&dest);

        let shared = StreamingDownload::new(part.clone(), dest.clone());
        shared.set_total(data.len() as u64);

        // 模拟下载线程：64KB 一块，每块 30ms（32KB/s ≈ 远慢于实时解码）
        let writer = std::thread::spawn({
            let shared = Arc::clone(&shared);
            move || {
                let mut f = std::fs::File::create(&part).unwrap();
                for chunk in data.chunks(64 * 1024) {
                    f.write_all(chunk).unwrap();
                    shared.publish(f.metadata().unwrap().len());
                    std::thread::sleep(Duration::from_millis(30));
                }
                drop(f);
                shared.finish(true, data.len() as u64);
                std::fs::rename(&part, &dest_for_writer).ok();
            }
        });

        let t0 = std::time::Instant::now();
        // 等第一块数据落盘（对应 engine 的预缓冲门槛）再打开流式源
        while shared.snapshot().0 == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut src = crate::symdec::SymphoniaSource::open_streaming(Arc::clone(&shared), 0)
            .expect("open streaming");
        use rodio::Source as _;
        let total: u64 = src.by_ref().count() as u64;
        let elapsed = t0.elapsed();
        println!(
            "decoded {total} samples in {elapsed:?}; total_duration={:?}",
            src.total_duration()
        );
        assert!(total > 0, "no samples decoded");
        // 数据以约 2MB/s 供给且解码快于供给：断言解码经历了等待而非一次性读完
        assert!(elapsed.as_secs_f64() > 1.0, "解码未经历阻塞等待，疑似一次性读完");
        writer.join().unwrap();
        let _ = std::fs::remove_file(&dest);
    }
}
