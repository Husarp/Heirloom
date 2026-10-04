//! Settings that belong to this computer, not to an archive: the recent archives, the appearance, update checks
//! and the last place in each archive. Stored in `%APPDATA%\Heirloom\aplikacja.json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE: &str = "aplikacja.json";
const RECENT_LIMIT: usize = 12;
/// Places are kept for this many archives (the most recently used ones).
const PLACES_LIMIT: usize = 20;
/// „Ostatnio oglądane” on Start.
const VIEWED_LIMIT: usize = 8;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub recent: Vec<RecentArchive>,
    pub appearance: Appearance,
    /// The name last chosen in "Kto edytuje?", offered first next time.
    pub last_editor: Option<String>,
    /// Where each archive was left, by archive id. Kept on this computer, never in the archive folder: on a shared
    /// server, one person's last screen must not move another's.
    pub places: BTreeMap<String, Place>,
    pub updates: Updates,
}

/// Where an archive was left, for „Po otwarciu archiwum: Ostatnie miejsce”.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Place {
    /// The screen (a route of the UI; not read here).
    pub route: Value,
    /// The tree as last seen: its view, person, zoom and position (also a route of the UI).
    pub tree: Value,
    /// „Ostatnio oglądane”: the profiles viewed, newest first (xrefs).
    pub viewed: Vec<String>,
    /// RFC 3339; the oldest places go first when there are too many.
    pub at: String,
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
    /// `archive`, or `set` for a `.heirloom-zestaw` file („Otwórz razem…”). Older files have no key: archives.
    #[serde(default = "archive_kind")]
    pub kind: String,
    /// How many archives a set joins („zestaw · 2 archiwa”).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archives: Option<usize>,
}

fn archive_kind() -> String {
    "archive".into()
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
    /// What opening an archive shows: `start` (the Start screen) or `last` (the place where it was left).
    pub start_in: String,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance { theme: "system".into(), text_size: 100, density: "comfortable".into(), animations: true, start_in: "start".into() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Updates {
    /// „Sprawdzaj aktualizacje”: ask GitHub for the newest version at start and on coming back to the window. Off
    /// until switched on (the owner's decision): Heirloom stays fully offline unless asked.
    pub check: bool,
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
        // A temporary name of this process's own: two windows saving at once must not write into one file.
        let tmp = dir.join(format!("{FILE}.tmp-{}", std::process::id()));
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, dir.join(FILE)).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
    }

    /// Loads the file as it is now, applies one change and writes it back. Every window (each its own process) changes
    /// only what it means to change, on top of what another window wrote meanwhile, instead of writing back the whole
    /// file it read at start. A failed write still gives the changed settings: they are only conveniences. When the
    /// file can't be read just then (missing, damaged, held by another program), `current` (this window's copy) is
    /// changed instead, so a moment's failure doesn't empty the recent list.
    pub fn update(dir: &Path, current: &AppConfig, change: impl FnOnce(&mut AppConfig)) -> AppConfig {
        let on_disk = std::fs::read(dir.join(FILE)).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let mut config = on_disk.unwrap_or_else(|| current.clone());
        change(&mut config);
        let _ = config.save(dir);
        config
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

    /// Updates the parts of an archive's place that are given.
    pub fn set_place(&mut self, archive_id: &str, route: Option<Value>, tree: Option<Value>, viewed: Option<Vec<String>>, at: String) {
        let place = self.places.entry(archive_id.to_string()).or_default();
        if let Some(route) = route {
            place.route = route;
        }
        if let Some(tree) = tree {
            place.tree = tree;
        }
        if let Some(mut viewed) = viewed {
            viewed.truncate(VIEWED_LIMIT);
            place.viewed = viewed;
        }
        place.at = at;
        while self.places.len() > PLACES_LIMIT {
            let Some(oldest) = self.places.iter().min_by(|a, b| a.1.at.cmp(&b.1.at)).map(|(id, _)| id.clone()) else { break };
            self.places.remove(&oldest);
        }
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
        let entry = |path: &str| RecentArchive { path: path.into(), name: path.into(), opened_at: String::new(), people: 0, kind: archive_kind(), archives: None };
        config.remember(entry("D:\\A"));
        config.remember(entry("D:\\B"));
        config.remember(entry("d:\\a"));
        let paths: Vec<_> = config.recent.iter().map(|r| r.path.as_str()).collect();
        assert_eq!(paths, ["d:\\a", "D:\\B"]);
    }

    #[test]
    fn places_merge_and_keep_the_newest_archives() {
        let mut config = AppConfig::default();
        let route = serde_json::json!({ "name": "people" });
        config.set_place("a", Some(route.clone()), None, Some((0..12).map(|i| format!("@I{i}@")).collect()), "2026-10-01T10:00:00Z".into());
        config.set_place("a", None, Some(serde_json::json!({ "name": "tree" })), None, "2026-10-01T10:00:01Z".into());
        let a = &config.places["a"];
        assert_eq!((&a.route, a.tree["name"].as_str(), a.viewed.len()), (&route, Some("tree"), 8));
        for k in 0..PLACES_LIMIT {
            config.set_place(&format!("b{k}"), Some(route.clone()), None, None, format!("2026-10-02T10:00:{k:02}Z"));
        }
        assert_eq!(config.places.len(), PLACES_LIMIT);
        assert!(!config.places.contains_key("a"), "the oldest place goes");
    }

    #[test]
    fn an_older_file_loads_with_the_new_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), br#"{"appearance": {"theme": "dark"}}"#).unwrap();
        let config = AppConfig::load(dir.path());
        assert_eq!((config.appearance.theme.as_str(), config.appearance.start_in.as_str()), ("dark", "start"));
        assert!(config.places.is_empty());
    }

    #[test]
    fn two_windows_keep_each_others_changes() {
        let dir = tempfile::tempdir().unwrap();
        let entry = |path: &str| RecentArchive { path: path.into(), name: path.into(), opened_at: String::new(), people: 0, kind: archive_kind(), archives: None };
        // Both windows read the file at start; each then changes only its own part.
        let first = AppConfig::update(dir.path(), &AppConfig::default(), |c| c.remember(entry("D:\\A")));
        let second = AppConfig::update(dir.path(), &first, |c| c.remember(entry("D:\\B")));
        assert_eq!(first.recent.len(), 1);
        assert_eq!(second.recent.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["D:\\B", "D:\\A"]);
        let after = AppConfig::update(dir.path(), &first, |c| c.forget("D:\\A"));
        assert_eq!(after.recent.len(), 1);
        // A file that can't be read just then: this window's copy is kept, not emptied.
        std::fs::write(dir.path().join(FILE), b"{ zepsu").unwrap();
        let kept = AppConfig::update(dir.path(), &after, |c| c.remember(entry("D:\\D")));
        assert_eq!(kept.recent.len(), 2);
        assert_eq!(AppConfig::load(dir.path()).recent.len(), 2);
        // A file from before 0.5.0: its entries are archives.
        std::fs::write(dir.path().join(FILE), br#"{"recent": [{"path": "D:\\C", "name": "C", "openedAt": "", "people": 3}]}"#).unwrap();
        let old = AppConfig::load(dir.path());
        assert_eq!((old.recent[0].kind.as_str(), old.recent[0].archives), ("archive", None));
    }

    #[test]
    fn damaged_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), b"{not json").unwrap();
        assert_eq!(AppConfig::load(dir.path()).appearance, Appearance::default());
    }

    #[test]
    fn update_checks_are_off_unless_switched_on() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!AppConfig::load(dir.path()).updates.check, "a new computer");
        std::fs::write(dir.path().join(FILE), br#"{"recent": []}"#).unwrap();
        assert!(!AppConfig::load(dir.path()).updates.check, "a file from before 0.4.0");
        std::fs::write(dir.path().join(FILE), br#"{"updates": {}}"#).unwrap();
        assert!(!AppConfig::load(dir.path()).updates.check, "a file without the key");
        let mut config = AppConfig::default();
        config.updates.check = true;
        config.save(dir.path()).unwrap();
        assert!(AppConfig::load(dir.path()).updates.check);
    }
}
