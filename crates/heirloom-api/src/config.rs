//! Settings that belong to this computer, not to an archive: the recent archives and the appearance.
//! Stored in `%APPDATA%\Heirloom\aplikacja.json`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FILE: &str = "aplikacja.json";
const RECENT_LIMIT: usize = 12;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub recent: Vec<RecentArchive>,
    pub appearance: Appearance,
    /// The name last chosen in "Kto edytuje?", offered first next time.
    pub last_editor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentArchive {
    /// The folder, or the `.ged` file when a file was opened directly.
    pub path: String,
    pub name: String,
    /// RFC 3339.
    pub opened_at: String,
    pub people: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Appearance {
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// 100–150 (%).
    pub text_size: u32,
    /// `comfortable` or `compact`.
    pub density: String,
    pub animations: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: "system".into(), text_size: 100, density: "comfortable".into(), animations: true }
    }
}

impl AppConfig {
    /// `%APPDATA%\Heirloom`.
    pub fn default_dir() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("Heirloom"))
    }

    /// A missing or damaged file gives the defaults: these settings are only conveniences.
    pub fn load(dir: &Path) -> AppConfig {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let tmp = dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, json)?;
        std::fs::rename(tmp, dir.join(FILE))
    }

    /// Moves (or adds) an archive to the top of the recent list.
    pub fn remember(&mut self, entry: RecentArchive) {
        self.recent.retain(|r| !same_path(&r.path, &entry.path));
        self.recent.insert(0, entry);
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn forget(&mut self, path: &str) {
        self.recent.retain(|r| !same_path(&r.path, path));
    }
}

/// Windows paths are case-insensitive.
fn same_path(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_list_moves_reopened_archives_to_the_top() {
        let mut config = AppConfig::default();
        let entry = |path: &str| RecentArchive { path: path.into(), name: path.into(), opened_at: String::new(), people: 0 };
        config.remember(entry("D:\\A"));
        config.remember(entry("D:\\B"));
        config.remember(entry("d:\\a"));
        let paths: Vec<_> = config.recent.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["d:\\a", "D:\\B"]);
    }

    #[test]
    fn damaged_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), b"{not json").unwrap();
        assert_eq!(AppConfig::load(dir.path()).appearance, Appearance::default());
    }
}
