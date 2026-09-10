use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::time::Instant;

use crate::error::{Error, Result};
use crate::models::LogLine;
use crate::singbox::job;
use crate::storage;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub const LOG_CAP: usize = 1000;
pub const STARTUP_CONFIRM: Duration = Duration::from_millis(1500);
pub const STARTUP_POLL: Duration = Duration::from_millis(100);
pub const KILL_GRACE: Duration = Duration::from_secs(3);

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Log Ring Buffer
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct LogBuffer {
    inner: Mutex<LogInner>,
}

#[derive(Default)]
struct LogInner {
    ring: VecDeque<LogLine>,
    pending: Vec<LogLine>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, line: LogLine) {
        let mut g = self.inner.lock();
        g.ring.push_back(line.clone());
        while g.ring.len() > LOG_CAP {
            g.ring.pop_front();
        }
        g.pending.push(line);
        let overflow = g.pending.len().saturating_sub(LOG_CAP);
        if overflow > 0 {
            g.pending.drain(..overflow);
        }
    }

    pub fn push_now(&self, level: &str, message: impl Into<String>) {
        self.push(LogLine {
            ts: now_ms(),
            level: level.into(),
            message: message.into(),
        });
    }

    pub fn tail(&self, limit: usize) -> Vec<LogLine> {
        let g = self.inner.lock();
        let skip = g.ring.len().saturating_sub(limit);
        g.ring.iter().skip(skip).cloned().collect()
    }

    pub fn drain_pending(&self) -> Vec<LogLine> {
        std::mem::take(&mut self.inner.lock().pending)
    }

    pub fn clear(&self) {
        let mut g = self.inner.lock();
        g.ring.clear();
        g.pending.clear();
    }
}

pub fn parse_log_line(line: &str) -> LogLine {
    let trimmed = line.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() >= 4 {
        let level = parts.iter().find_map(|p| {
            let lower = p.to_ascii_lowercase();
            match lower.as_str() {
                "info" | "warn" | "warning" | "error" | "fatal" | "debug" | "trace" => Some(lower),
                _ => None,
            }
        });
        if let Some(lvl) = level {
            return LogLine {
                ts: now_ms(),
                level: if lvl == "warning" { "warn".into() } else { lvl },
                message: trimmed.to_string(),
            };
        }
    }

    LogLine {
        ts: now_ms(),
        level: "info".into(),
        message: trimmed.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Binary Discovery & Validation
// ---------------------------------------------------------------------------

pub fn find_singbox_binary(custom_data_dir: Option<&Path>) -> Option<PathBuf> {
    // 1. Check relative to current_exe: <exe_dir>/resources/sing-box.exe and <exe_dir>/sing-box.exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate1 = dir.join("resources").join("sing-box.exe");
            if candidate1.exists() {
                return Some(candidate1);
            }
            let candidate2 = dir.join("sing-box.exe");
            if candidate2.exists() {
                return Some(candidate2);
            }

            // Also check parent directories for development / cargo test
            let mut curr = dir.parent();
            for _ in 0..4 {
                if let Some(p) = curr {
                    let c = p.join("resources").join("sing-box.exe");
                    if c.exists() {
                        return Some(c);
                    }
                    curr = p.parent();
                } else {
                    break;
                }
            }
        }
    }

    // 2. Check %APPDATA%\lightgui\bin\sing-box.exe
    let data_dir = custom_data_dir.map(PathBuf::from).unwrap_or_else(storage::default_data_dir);
    let candidate3 = data_dir.join("bin").join("sing-box.exe");
    if candidate3.exists() {
        return Some(candidate3);
    }

    // 3. Check current directory and parents (for dev/test runners)
    if let Ok(cwd) = std::env::current_dir() {
        let mut curr = Some(cwd.as_path());
        for _ in 0..4 {
            if let Some(p) = curr {
                let c1 = p.join("resources").join("sing-box.exe");
                if c1.exists() {
                    return Some(c1);
                }
                curr = p.parent();
            } else {
                break;
            }
        }
    }

    None
}

pub async fn check_config(exe: &Path, config_path: &Path, work_dir: &Path) -> Result<()> {
    let mut cmd = Command::new(exe);
    cmd.arg("check")
        .arg("-c")
        .arg(config_path)
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .await
        .map_err(|e| Error::CoreStartFailed(format!("failed to execute sing-box check: {e}")))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first_err = stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("unknown validation error");
        Err(Error::CoreStartFailed(format!("config check failed: {first_err}")))
    }
}

// ---------------------------------------------------------------------------
// Process Lifecycle
// ---------------------------------------------------------------------------

pub struct CoreProcess {
    child: Child,
    pub log_buffer: Arc<LogBuffer>,
    stopping: Arc<AtomicBool>,
}

impl CoreProcess {
    pub fn spawn(
        exe: &Path,
        config_path: &Path,
        work_dir: &Path,
        log_buffer: Arc<LogBuffer>,
    ) -> Result<Self> {
        let mut cmd = Command::new(exe);
        cmd.arg("run")
            .arg("-c")
            .arg(config_path)
            .arg("--disable-color")
            .current_dir(work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::CoreStartFailed(format!("could not spawn sing-box: {e}")))?;

        #[cfg(windows)]
        {
            if let Some(raw_handle) = child.raw_handle() {
                let _ = job::assign_current_job(raw_handle);
            }
        }

        let stopping = Arc::new(AtomicBool::new(false));

        if let Some(stdout) = child.stdout.take() {
            let buf = log_buffer.clone();
            tokio::spawn(read_stream(stdout, buf));
        }
        if let Some(stderr) = child.stderr.take() {
            let buf = log_buffer.clone();
            tokio::spawn(read_stream(stderr, buf));
        }

        Ok(Self {
            child,
            log_buffer,
            stopping,
        })
    }

    pub async fn confirm_started(&mut self) -> Result<()> {
        let deadline = Instant::now() + STARTUP_CONFIRM;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    let code = status
                        .code()
                        .map_or_else(|| "unknown".to_string(), |c| c.to_string());
                    return Err(Error::CoreStartFailed(format!(
                        "sing-box exited immediately with code {code}"
                    )));
                }
                Ok(None) => {}
                Err(e) => {
                    return Err(Error::CoreStartFailed(format!(
                        "could not poll child process: {e}"
                    )));
                }
            }
            if Instant::now() >= deadline {
                return Ok(());
            }
            tokio::time::sleep(STARTUP_POLL).await;
        }
    }

    pub async fn stop(&mut self) -> Result<()> {
        self.stopping.store(true, Ordering::SeqCst);
        drop(self.child.stdin.take());

        let _ = self.child.start_kill();
        if tokio::time::timeout(KILL_GRACE, self.child.wait()).await.is_err() {
            let _ = self.child.kill().await;
        }
        Ok(())
    }
}

async fn read_stream<R: AsyncRead + Unpin + Send + 'static>(stream: R, buf: Arc<LogBuffer>) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if !line.trim().is_empty() {
            buf.push(parse_log_line(&line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_push_and_tail() {
        let buf = LogBuffer::new();
        buf.push_now("info", "hello world");
        buf.push_now("error", "something failed");

        let tail = buf.tail(10);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].message, "hello world");
        assert_eq!(tail[1].message, "something failed");
    }

    #[test]
    fn test_find_singbox_binary_finds_resources() {
        let found = find_singbox_binary(None);
        assert!(found.is_some(), "sing-box binary should be found in resources");
        let path = found.unwrap();
        assert!(path.exists(), "found path {path:?} must exist");
    }
}
