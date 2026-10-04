//! The set file (`Rodzina razem.heirloom-zestaw`): which archives are opened together and which of their people are the
//! same person. It is the only file the combined view writes; the archives themselves are never touched.
//!
//! Every change re-reads the file first and applies only that one change (read-modify-write), so two windows, or two
//! people on a shared folder, deciding at the same time don't erase each other's decisions. Keys this version doesn't
//! know are kept.

use crate::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Component, Path, PathBuf};

pub const FORMAT: &str = "heirloom-zestaw";
pub const VERSION: u32 = 1;
pub const EXTENSION: &str = "heirloom-zestaw";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct SetFile {
    pub format: String,
    pub version: u32,
    /// Names the set's last place on this computer (as an archive id does).
    pub id: String,
    pub name: String,
    pub created: String,
    pub archives: Vec<Entry>,
    pub links: Vec<Link>,
    pub rejected: Vec<Link>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// One archive of the set.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    /// `a`, `b`, … : how links and the combined view's ids (`@b~I12@`) name the archive.
    pub key: String,
    /// The archive's id from its `.heirloom/ustawienia.json`; empty for a GEDCOM file Heirloom never saved (it gets a
    /// new id on every open), so a moved archive can be recognised when there is one.
    pub id: String,
    pub name: String,
    /// Relative to the set file's folder where possible (`../Nowakowie/drzewo.ged`), else absolute.
    pub path: String,
    /// Absolute, as last opened.
    pub last_path: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Two people of different archives: the same person (`links`) or not (`rejected`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Link {
    pub a: PersonRef,
    pub b: PersonRef,
    /// `suggested`, `manual` or `bulk`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub how: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
    pub by: String,
    pub at: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PersonRef {
    /// The archive's key in the set.
    pub archive: String,
    pub xref: String,
    /// Found first when given: it survives renumbering by other programs.
    pub uid: Option<String>,
    /// Only a label, for a link whose person can't be found any more; never used for matching.
    pub name: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl PersonRef {
    pub fn same(&self, other: &PersonRef) -> bool {
        self.archive == other.archive && self.xref == other.xref
    }
}

impl Link {
    /// The same two people, in either order.
    pub fn joins(&self, a: &PersonRef, b: &PersonRef) -> bool {
        (self.a.same(a) && self.b.same(b)) || (self.a.same(b) && self.b.same(a))
    }

    pub fn touches(&self, p: &PersonRef) -> bool {
        self.a.same(p) || self.b.same(p)
    }
}

impl SetFile {
    pub fn new(name: &str) -> SetFile {
        SetFile {
            format: FORMAT.into(),
            version: VERSION,
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            created: heirloom_core::history::now(),
            ..SetFile::default()
        }
    }

    /// A key no archive of the set has: `a`, `b`, … `z`, then `aa`, `ab`, ….
    pub fn next_key(&self) -> String {
        let letters = |mut n: usize| {
            let mut key = String::new();
            loop {
                key.insert(0, (b'a' + (n % 26) as u8) as char);
                if n < 26 {
                    break key;
                }
                n = n / 26 - 1;
            }
        };
        (0..).map(letters).find(|k| self.archives.iter().all(|a| &a.key != k)).expect("endless")
    }

    pub fn entry(&self, key: &str) -> Option<&Entry> {
        self.archives.iter().find(|a| a.key == key)
    }
}

fn plain_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 8 && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn damaged() -> ApiError {
    ApiError::new("set_damaged", "Plik zestawu jest uszkodzony. Archiwa nie zostały zmienione.")
}

pub fn read(path: &Path) -> Result<SetFile, ApiError> {
    let bytes = std::fs::read(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ApiError::new("not_found", format!("Nie ma pliku zestawu {}.", path.display())),
        _ => ApiError::new("io", format!("Nie udało się odczytać zestawu: {e}")),
    })?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(ApiError::new("not_a_set", "To nie jest plik zestawu Heirloom."));
    }
    if value.get("version").and_then(Value::as_u64).is_some_and(|v| v > u64::from(VERSION)) {
        return Err(ApiError::new("set_newer", "Ten zestaw zapisała nowsza wersja Heirloom. Zaktualizuj program, żeby go otworzyć."));
    }
    let set: SetFile = serde_json::from_value(value).map_err(|_| damaged())?;
    // Keys become parts of ids and file URLs: anything else than a short plain key means the file was damaged.
    let mut keys = std::collections::HashSet::new();
    if !set.archives.iter().all(|a| plain_key(&a.key) && keys.insert(a.key.as_str())) {
        return Err(damaged());
    }
    Ok(set)
}

/// Written next to the file (a name of this process's own, so two windows saving at once don't write into one
/// temporary file), flushed to disk, then swapped in, so a failed write leaves the old file whole.
pub fn write(path: &Path, set: &SetFile) -> Result<(), ApiError> {
    use std::io::Write;
    let failed = |e: std::io::Error| ApiError::new("set_write", format!("Nie udało się zapisać zestawu: {e}"));
    let json = serde_json::to_vec_pretty(set).map_err(|e| failed(std::io::Error::other(e)))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    let written = std::fs::File::create(&tmp).and_then(|mut file| {
        file.write_all(&json)?;
        file.sync_all()
    });
    written.and_then(|()| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        failed(e)
    })
}

/// Re-reads the file, applies one change and writes it back. Nothing is written when the change fails.
pub fn modify<T>(path: &Path, change: impl FnOnce(&mut SetFile) -> Result<T, ApiError>) -> Result<(SetFile, T), ApiError> {
    let mut set = read(path)?;
    let out = change(&mut set)?;
    write(path, &set)?;
    Ok((set, out))
}

/// `target` as written in the set file in `dir`: relative where both are on one drive (`../Nowakowie`), else absolute.
/// Always with `/`, so a set made on Windows reads the same elsewhere.
pub fn relative_path(dir: &Path, target: &Path) -> String {
    let (from, to): (Vec<Component>, Vec<Component>) = (dir.components().collect(), target.components().collect());
    let root = |c: &[Component]| c.iter().take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir)).count();
    let same_root = root(&from) > 0 && from.get(..root(&from)) == to.get(..root(&from)) && root(&from) == root(&to);
    if !same_root || from.iter().chain(&to).any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return target.display().to_string();
    }
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".into(); from.len() - common];
    parts.extend(to[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

/// Where an archive of the set is now: its path (relative to the set file's folder), else where it was last opened.
pub fn locate(dir: &Path, entry: &Entry) -> Option<PathBuf> {
    let written = (!entry.path.is_empty()).then(|| {
        let p = Path::new(&entry.path);
        if p.is_absolute() { p.to_path_buf() } else { dir.join(entry.path.replace('/', std::path::MAIN_SEPARATOR_STR)) }
    });
    let last = (!entry.last_path.is_empty()).then(|| PathBuf::from(&entry.last_path));
    written.into_iter().chain(last).find(|p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_written_relative_to_the_set() {
        let base = std::env::temp_dir().join("rodzina");
        assert_eq!(relative_path(&base, &base.join("Kowalscy")), "Kowalscy");
        assert_eq!(relative_path(&base, &base.parent().unwrap().join("Nowakowie z Ciechanek").join("drzewo.ged")), "../Nowakowie z Ciechanek/drzewo.ged");
        assert_eq!(relative_path(&base, &base), ".");
        assert_eq!(relative_path(Path::new("relative"), &base), base.display().to_string(), "only absolute paths are compared");
        #[cfg(windows)]
        assert_eq!(relative_path(Path::new("C:\\Rodzina"), Path::new("D:\\Nowakowie")), "D:\\Nowakowie", "another drive");
    }

    #[test]
    fn unknown_keys_survive_and_newer_or_damaged_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Rodzina razem.heirloom-zestaw");
        let text = r#"{ "format": "heirloom-zestaw", "version": 1, "id": "x", "name": "R", "colours": { "a": 3 },
            "archives": [ { "key": "a", "name": "K", "path": "K", "pinned": true } ],
            "links": [ { "a": { "archive": "a", "xref": "@I1@", "note": "n" }, "b": { "archive": "b", "xref": "@I2@" }, "how": "manual", "by": "Ewa", "at": "t", "checked": 1 } ] }"#;
        std::fs::write(&path, text).unwrap();
        let (set, ()) = modify(&path, |s| {
            s.name = "Rodzina razem".into();
            Ok(())
        })
        .unwrap();
        assert_eq!(set.archives[0].extra["pinned"], true);
        let again: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!((&again["colours"]["a"], &again["archives"][0]["pinned"], &again["links"][0]["checked"], &again["links"][0]["a"]["note"]), (&json(3), &json(true), &json(1), &json("n")));
        assert_eq!(again["name"], "Rodzina razem");

        std::fs::write(&path, r#"{ "format": "heirloom-zestaw", "version": 2, "archives": [] }"#).unwrap();
        assert_eq!(read(&path).unwrap_err().code, "set_newer");
        std::fs::write(&path, r#"{ "format": "heirloom-settings", "version": 1 }"#).unwrap();
        assert_eq!(read(&path).unwrap_err().code, "not_a_set");
        std::fs::write(&path, b"{ \"format\": \"heirloom-zestaw\", ").unwrap();
        assert_eq!(modify(&path, |_| Ok(())).unwrap_err().code, "set_damaged");
        assert_eq!(std::fs::read(&path).unwrap(), b"{ \"format\": \"heirloom-zestaw\", ", "a damaged file is never overwritten");
        std::fs::write(&path, r#"{ "format": "heirloom-zestaw", "version": 1, "archives": [ { "key": "../x" } ] }"#).unwrap();
        assert_eq!(read(&path).unwrap_err().code, "set_damaged");
    }

    fn json<T: Serialize>(v: T) -> Value {
        serde_json::to_value(v).unwrap()
    }

    #[test]
    fn keys_go_through_the_alphabet() {
        let mut set = SetFile::new("R");
        for _ in 0..28 {
            let key = set.next_key();
            set.archives.push(Entry { key, ..Entry::default() });
        }
        let keys: Vec<&str> = set.archives.iter().map(|a| a.key.as_str()).collect();
        assert_eq!((keys[0], keys[25], keys[26], keys[27]), ("a", "z", "aa", "ab"));
        set.archives.remove(1);
        assert_eq!(set.next_key(), "b");
    }
}
