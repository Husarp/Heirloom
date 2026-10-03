//! A small file that tells the installer (installer/setup.py) that Heirloom is running and whether closing it now
//! would lose work: `%LOCALAPPDATA%\Heirloom\running\<pid>.json`. Written when the window starts, again whenever
//! that changes (the window reports what its close question would ask about: unsaved changes, an open section's
//! draft), removed when Heirloom exits. One that outlives its process (a crash, Task Manager) is told apart by the
//! installer: no heirloom.exe with that pid is running.

use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the installer is told. `None` folder: nothing is written (the self-test, or Windows gave no folder).
pub struct RunningFile {
    path: Option<PathBuf>,
    /// The last state written: (unsaved changes, an open section's draft, the archive's name).
    state: Mutex<(u32, bool, Option<String>)>,
}

impl RunningFile {
    pub fn default_dir() -> Option<PathBuf> {
        Some(dirs::data_local_dir()?.join("Heirloom").join("running"))
    }

    /// Writes the file at once: Heirloom is running, with nothing unsaved yet.
    pub fn new(dir: Option<PathBuf>) -> RunningFile {
        let file = RunningFile {
            path: dir.map(|d| d.join(format!("{}.json", std::process::id()))),
            state: Mutex::new((0, false, None)),
        };
        file.write(&file.state.lock().unwrap_or_else(|e| e.into_inner()));
        file
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// The window's news: what closing it now would lose.
    pub fn set(&self, unsaved_changes: u32, draft: bool, archive: Option<String>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let new = (unsaved_changes, draft, archive);
        if *state != new {
            *state = new;
            self.write(&state);
        }
    }

    /// Heirloom is exiting.
    pub fn remove(&self) {
        let _state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_file(path.with_extension("json.tmp"));
        }
    }

    /// Whole or not at all: written beside, then renamed over the old one. The installer may be reading it at that
    /// moment (Windows then refuses the rename), so it is tried again a few times.
    fn write(&self, (unsaved, draft, archive): &(u32, bool, Option<String>)) {
        let Some(path) = &self.path else { return };
        let text = json!({
            "pid": std::process::id(),
            "version": VERSION,
            "unsavedChanges": unsaved,
            "draft": draft,
            "archive": archive,
        })
        .to_string();
        let temp = path.with_extension("json.tmp");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&temp, text).is_err() {
            return;
        }
        for attempt in 0..5 {
            if std::fs::rename(&temp, path).is_ok() {
                return;
            }
            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn read(file: &RunningFile) -> Value {
        serde_json::from_str(&std::fs::read_to_string(file.path().unwrap()).unwrap()).unwrap()
    }

    #[test]
    fn written_at_start_updated_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let file = RunningFile::new(Some(dir.path().join("running")));
        let path = file.path().unwrap().clone();
        assert_eq!(path.file_name().unwrap().to_string_lossy(), format!("{}.json", std::process::id()));
        let start = read(&file);
        assert_eq!(start["pid"], std::process::id());
        assert_eq!(start["unsavedChanges"], 0);
        assert_eq!(start["draft"], false);
        assert_eq!(start["archive"], Value::Null);

        file.set(3, true, Some("Kowalscy".into()));
        let now = read(&file);
        assert_eq!((now["unsavedChanges"].as_u64(), now["draft"].as_bool()), (Some(3), Some(true)));
        assert_eq!(now["archive"], "Kowalscy");
        assert!(!path.with_extension("json.tmp").exists());

        file.remove();
        assert!(!path.exists());
    }

    #[test]
    fn nothing_written_without_a_folder() {
        let file = RunningFile::new(None);
        file.set(1, false, None);
        file.remove();
        assert!(file.path().is_none());
    }
}
