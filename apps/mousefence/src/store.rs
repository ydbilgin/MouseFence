//! Settings files and the live-state file (PLAN T15-T16) behind fence-app's `StorePort`.
//!
//! - Settings: fence-config `load` with `%APPDATA%\MouseFence\v2\settings.json` first, the 1.x file
//!   imported read-only, else fresh defaults (Track B owns the rules).
//! - After the settings process exits 0 the tray accepts only a clean v2 load; anything else keeps the live
//!   settings (a failed reload never becomes defaults).
//! - `%LOCALAPPDATA%\MouseFence\v2\state.txt` carries the live switches for the open settings window; it is
//!   written through a temp file and a rename so the reader never sees half a line.

use std::path::{Path, PathBuf};

use fence_app::StorePort;
use fence_config::{LoadSource, Loaded, Paths, Settings};
use fence_types::{RuntimeState, launch};

pub struct FileStore {
    paths: Paths,
    state_file: PathBuf,
}

impl FileStore {
    /// `appdata` = `%APPDATA%`, `local_appdata` = `%LOCALAPPDATA%`.
    pub fn new(appdata: &Path, local_appdata: &Path) -> Self {
        Self {
            paths: Paths::under_appdata(appdata),
            state_file: state_file_under(local_appdata),
        }
    }

    #[cfg(test)]
    pub fn state_file(&self) -> &Path {
        &self.state_file
    }
}

pub fn state_file_under(local_appdata: &Path) -> PathBuf {
    local_appdata
        .join("MouseFence")
        .join("v2")
        .join(launch::STATE_FILE_NAME)
}

/// Write `text` to `path` through `<path>.tmp` and a rename (replaces an existing file).
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

impl StorePort for FileStore {
    fn load(&mut self) -> Loaded {
        fence_config::load_production(
            &self.paths,
            &fence_win::enumerate_monitors(),
            fence_win::has_intel_gpu(),
        )
    }

    fn reload_saved(&mut self) -> Result<Settings, String> {
        let loaded = self.load();
        match loaded.source {
            LoadSource::V2 => Ok(loaded.settings),
            other => Err(format!("not a clean v2 load: {other:?}")),
        }
    }

    fn write_state(&mut self, state: &RuntimeState) {
        if let Err(e) = write_atomic(&self.state_file, &launch::state_to_text(state)) {
            fence_win::debug_log(&format!("state file not written: {e}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh folder under the temp directory, removed by the guard (checked to be ours first).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            let p = std::env::temp_dir().join(format!("MouseFenceTest-{}-{tag}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let tmp = std::env::temp_dir();
            let name = self.0.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if self.0.is_absolute() && self.0.starts_with(&tmp) && name.starts_with("MouseFenceTest-") {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    #[test]
    fn s_app_state_file_round_trips_through_an_atomic_write() {
        let t = TempDir::new("state");
        let mut store = FileStore::new(&t.0.join("Roaming"), &t.0.join("Local"));
        assert!(store.state_file().ends_with("MouseFence/v2/state.txt"));
        let s = RuntimeState {
            gate_open: true,
            side_contain: false,
            confine: true,
            paused: false,
            hook_installed: true,
        };
        store.write_state(&s);
        let text = std::fs::read_to_string(store.state_file()).unwrap();
        assert_eq!(launch::state_from_text(&text), Some(s));
        let s2 = RuntimeState { paused: true, ..s };
        store.write_state(&s2);
        let text = std::fs::read_to_string(store.state_file()).unwrap();
        assert_eq!(launch::state_from_text(&text), Some(s2), "rewritten in place");
        let leftovers: Vec<_> = std::fs::read_dir(store.state_file().parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temp file left behind");
    }

    #[test]
    fn s_app_reload_accepts_only_a_clean_v2_load() {
        // An empty temporary APPDATA has no v2 file: whatever fence-config reports, it is not a clean V2
        // load, so the tray keeps its live settings. Never touches the owner's %APPDATA%.
        let t = TempDir::new("reload");
        let mut store = FileStore::new(&t.0.join("Roaming"), &t.0.join("Local"));
        assert!(store.reload_saved().is_err());
    }
}
