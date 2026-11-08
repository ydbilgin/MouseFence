//! Bounded diagnostics. Call only on the tray/UI thread; the hook reports counters to its probe.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_BYTES: usize = 64 * 1024;
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Append one failure, rotating to `error.log.1` before exceeding 64 KiB.
/// An oversized failure keeps its context and newest UTF-8 tail. Both files remain bounded.
pub fn append(path: &Path, context: &str, detail: &str) -> io::Result<()> {
    append_with_fixes(path, context, detail, fence_engine::fixes::Fixes::ALL)
}

pub fn append_with_fixes(
    path: &Path,
    context: &str,
    detail: &str,
    fixes: fence_engine::fixes::Fixes,
) -> io::Result<()> {
    if !fixes.j20_error_log {
        return Ok(());
    }
    let _lock = WRITE_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let header = format!("[{time}] {}\n", utf8_tail(context, 1024));
    let tail = utf8_tail(detail, MAX_BYTES.saturating_sub(header.len() + 5));
    let entry = format!("{header}{tail}\n---\n");
    let current = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if current + entry.len() as u64 > MAX_BYTES as u64 && current > 0 {
        let rotated = path.with_file_name("error.log.1");
        fs::rename(path, rotated)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(entry.as_bytes())
}

fn utf8_tail(text: &str, max: usize) -> &str {
    let mut start = text.len().saturating_sub(max);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

/// Install on the tray thread. Panics elsewhere delegate to the previous hook without touching files.
pub fn install_panic_hook(path: PathBuf) {
    install_panic_hook_with_fixes(path, fence_engine::fixes::Fixes::ALL);
}

pub fn install_panic_hook_with_fixes(path: PathBuf, fixes: fence_engine::fixes::Fixes) {
    if !fixes.j20_error_log {
        return;
    }
    let tray_thread = std::thread::current().id();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log_tray_panic(&path, tray_thread, || info.to_string());
        previous(info);
    }));
}

fn log_tray_panic(path: &Path, tray_thread: std::thread::ThreadId, detail: impl FnOnce() -> String) {
    if std::thread::current().id() == tray_thread {
        let _ = append(path, "FATAL tray panic", &detail());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f08_panic_logging_never_writes_from_a_non_tray_thread() {
        let root = std::env::temp_dir().join(format!(
            "mf-panic-log-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = root.join("error.log");
        let other_path = path.clone();
        let owner = std::thread::current().id();
        std::thread::spawn(move || {
            log_tray_panic(&other_path, owner, || panic!("must not format or write on hook thread"))
        })
        .join()
        .unwrap();
        assert!(!root.exists());
        log_tray_panic(&path, owner, || "tray panic marker".into());
        assert!(fs::read_to_string(&path).unwrap().contains("tray panic marker"));
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn f08_error_log_rotates_bounded_utf8_and_retains_newest_failure() {
        let root = std::env::temp_dir().join(format!(
            "mf-error-log-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = root.join("error.log");
        append(&path, "HANDLED", &"ç".repeat(180_000)).unwrap();
        append(&path, "FATAL", "latest diagnostic marker").unwrap();
        for p in [&path, &root.join("error.log.1")] {
            let bytes = fs::read(p).unwrap();
            assert!(!bytes.is_empty() && bytes.len() <= MAX_BYTES);
            std::str::from_utf8(&bytes).unwrap();
        }
        let latest = fs::read_to_string(&path).unwrap();
        assert!(latest.contains("FATAL") && latest.contains("latest diagnostic marker"));
        // A second rotation must replace our previous archive on Windows as well.
        append(&path, "HANDLED", &"ğ".repeat(180_000)).unwrap();
        assert!(fs::metadata(&path).unwrap().len() <= MAX_BYTES as u64);
        fs::remove_file(path).unwrap();
        fs::remove_file(root.join("error.log.1")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
