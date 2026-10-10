//! „Nie ruszaj oryginału”: the user's changes live in a folder of their own („Moje zmiany”), which is itself a normal
//! archive folder — a copy of the data file, its own `media/` (only the files added here), `zrodla-ai/` and
//! `.heirloom/` — so every write of the app lands there unchanged; the original archive is only read. Reads look in
//! that folder first and then in the original (`Session::layers`). The changes reach the original only when the user
//! moves them there („Przenieś zmiany do archiwum”, [`writeback`]); changes made in the original meanwhile are merged
//! in first ([`merge`]).
//!
//! `.heirloom/oryginal.json` (the manifest) names the original; `.heirloom/baza.ged` is the original's data file as
//! of the last sync, the base for telling the user's changes from the archive's.

pub mod merge;
pub mod writeback;

use crate::ApiError;
use crate::fsx;
use heirloom_core::archive::{SIDECAR_DIR, write_atomically};
use heirloom_core::gedcom::{Document, Node, Version};
use heirloom_core::{Archive, Settings};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MANIFEST: &str = "oryginal.json";
pub const BASE: &str = "baza.ged";
pub const README: &str = "CO TO JEST.txt";
const FORMAT: &str = "heirloom-moje-zmiany";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub original: Original,
    pub base: Base,
    pub synced: Synced,
    pub created_at: String,
    /// The changes are in the archive and the folder is kept only for safety (its rename failed): never re-attached.
    pub retired: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Original {
    /// The folder or `.ged` as it was opened.
    pub opened: String,
    pub root: String,
    /// The data file's full path (as the registry keys it).
    pub data_path: String,
    pub data_file: String,
    /// Only when the original has its own settings file.
    pub archive_id: Option<String>,
    /// The first write into another program's file was confirmed (by the write-back).
    pub foreign_confirmed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Base {
    pub hash: String,
    pub len: u64,
    pub at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Synced {
    /// How many entries of the overlay's history the original already has (the history is copied at the start).
    pub history_len: usize,
    pub last_write_back: Option<String>,
}

pub fn sidecar(root: &Path) -> PathBuf {
    root.join(SIDECAR_DIR)
}

/// The manifest of a folder of „Twoje zmiany”; None for any other folder.
pub fn read_manifest(root: &Path) -> Option<Manifest> {
    let bytes = fs::read(sidecar(root).join(MANIFEST)).ok()?;
    serde_json::from_slice::<Manifest>(&bytes).ok().filter(|m| m.format == FORMAT)
}

pub fn write_manifest(root: &Path, manifest: &Manifest) -> Result<(), ApiError> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|e| ApiError::new("io", e.to_string()))?;
    write_atomically(&sidecar(root).join(MANIFEST), &json).map_err(io_error)
}

pub fn io_error(e: impl std::fmt::Display) -> ApiError {
    ApiError::new("io", format!("Nie można zapisać pliku: {e}"))
}

pub fn hash_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// The archive has its own `ustawienia.json` (not one made up for another program's file).
pub fn has_own_settings(archive: &Archive) -> bool {
    archive.sidecar_path().join("ustawienia.json").is_file()
}

/// „Dokumenty › Heirloom › Moje zmiany › Rodzina Kowalskich” for a path under the Documents folder.
pub fn label(path: &Path, documents: Option<&Path>) -> String {
    if let Some(rest) = documents.and_then(|d| path.strip_prefix(d).ok()) {
        let parts: Vec<String> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        return std::iter::once("Dokumenty".to_string()).chain(parts).collect::<Vec<_>>().join(" › ");
    }
    path.display().to_string()
}

/// The folder a new overlay of `name` gets under `parent`: `Rodzina Kowalskich`, or `Rodzina Kowalskich (2)` when taken.
pub fn free_folder(parent: &Path, name: &str) -> PathBuf {
    let base = fsx::folder_name(name);
    let mut folder = parent.join(&base);
    let mut n = 2;
    while fs::symlink_metadata(&folder).is_ok() {
        folder = parent.join(format!("{base} ({n})"));
        n += 1;
    }
    folder
}

/// Makes a new overlay of `original` under `parent` (A.5): the working copy and the base from one read of the data
/// file, the settings (unlocked, saving confirmed), the history, the manifest and a note for whoever finds the folder.
/// A half-made folder is removed again.
pub fn create(original: &Archive, opened: &str, parent: &Path) -> Result<PathBuf, ApiError> {
    let bytes = fs::read(original.data_path()).map_err(|e| ApiError::new("io", format!("Nie można odczytać pliku danych: {e}")))?;
    fs::create_dir_all(parent).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => ApiError::new("permission", format!("Nie można utworzyć folderu {}: {e}", parent.display())),
        _ => io_error(e),
    })?;
    let folder = free_folder(parent, &original.settings().name);
    fs::create_dir(&folder).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => ApiError::new("permission", format!("Nie można utworzyć folderu {}: {e}", folder.display())),
        _ => io_error(e),
    })?;
    match fill(original, opened, &folder, &bytes) {
        Ok(()) => Ok(folder),
        Err(e) => {
            // The folder is new and ours: nothing of it may stay half-made.
            let _ = fs::remove_dir_all(&folder);
            Err(e)
        }
    }
}

fn fill(original: &Archive, opened: &str, folder: &Path, bytes: &[u8]) -> Result<(), ApiError> {
    let data_file = original.settings().data_file.clone();
    let side = sidecar(folder);
    fs::create_dir_all(&side).map_err(io_error)?;
    write_atomically(&folder.join(&data_file), bytes).map_err(io_error)?;
    write_atomically(&side.join(BASE), bytes).map_err(io_error)?;
    let own = has_own_settings(original);
    let settings = Settings {
        read_only: false,
        foreign_save_confirmed: true,
        archive_id: if own { original.settings().archive_id.clone() } else { uuid::Uuid::new_v4().to_string() },
        data_file: data_file.clone(),
        ..original.settings().clone()
    };
    let json = serde_json::to_vec_pretty(&settings).map_err(|e| ApiError::new("io", e.to_string()))?;
    write_atomically(&side.join("ustawienia.json"), &json).map_err(io_error)?;
    // The history from wherever the original keeps it (its `.heirloom`, or the app's folder for a read-only one).
    let history = original.history_path();
    let history_len = if history.is_file() {
        fs::copy(&history, side.join("historia.jsonl")).map_err(io_error)?;
        heirloom_core::history::read(&history).map(|e| e.len()).unwrap_or(0)
    } else {
        0
    };
    let now = heirloom_core::history::now();
    let manifest = Manifest {
        format: FORMAT.into(),
        version: 1,
        original: Original {
            opened: absolute(opened),
            root: original.root().display().to_string(),
            data_path: fs::canonicalize(original.data_path()).unwrap_or_else(|_| original.data_path()).display().to_string(),
            data_file,
            archive_id: own.then(|| original.settings().archive_id.clone()),
            foreign_confirmed: original.settings().foreign_save_confirmed,
        },
        base: Base { hash: hash_hex(bytes), len: bytes.len() as u64, at: now.clone() },
        synced: Synced { history_len, last_write_back: None },
        created_at: now,
        retired: false,
    };
    write_manifest(folder, &manifest)?;
    let readme = format!(
        "Ten folder zrobił program Heirloom. Są w nim Twoje zmiany w archiwum „{}” ({}), których archiwum jeszcze nie ma — \
         samo archiwum zostaje bez zmian. Nie usuwaj tego folderu, dopóki nie przeniesiesz zmian do archiwum (Heirloom › \
         Ustawienia › Archiwum › Twoje zmiany › „Przenieś zmiany do archiwum”). W folderze media są tylko pliki dodane przez \
         Ciebie; reszta jest w archiwum.\r\n",
        original.settings().name,
        original.root().display()
    );
    fs::write(folder.join(README), readme).map_err(io_error)?;
    // The copy is what was read.
    let copy = fs::read(folder.join(&manifest.original.data_file)).map_err(io_error)?;
    if hash_hex(&copy) != manifest.base.hash {
        return Err(ApiError::new("io", "Kopia pliku danych różni się od oryginału — spróbuj jeszcze raz."));
    }
    Ok(())
}

pub fn absolute(path: &str) -> String {
    std::path::absolute(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.to_string())
}

/// The base: the original's data file as of the last sync.
pub fn read_base(root: &Path) -> Option<Document> {
    let bytes = fs::read(sidecar(root).join(BASE)).ok()?;
    Some(Document::from_bytes(&bytes).0)
}

/// A finished overlay is renamed so it is never taken for current changes; when the rename fails (the folder open in
/// Explorer), it is marked in its manifest instead. Returns the folder's new place.
pub fn retire(root: &Path, words: &str) -> PathBuf {
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let parent = root.parent().unwrap_or(Path::new(""));
    // Not through free_folder: its 60-character cut would take the date and „można usunąć” off the end.
    let base = format!("{name} ({words})");
    let mut target = parent.join(&base);
    for n in 2.. {
        if fs::symlink_metadata(&target).is_err() {
            break;
        }
        target = parent.join(format!("{base} ({n})"));
    }
    if fsx::move_no_replace(root, &target).is_ok() {
        return target;
    }
    if let Some(mut manifest) = read_manifest(root) {
        manifest.retired = true;
        let _ = write_manifest(root, &manifest);
    }
    root.to_path_buf()
}

/// `2026-10-10` in local time.
pub fn today() -> String {
    let now = time::OffsetDateTime::now_utc();
    let local = time::UtcOffset::local_offset_at(now).map_or(now, |offset| now.to_offset(offset));
    local.format(time::macros::format_description!("[year]-[month]-[day]")).unwrap_or_default()
}

/// The data file's size and time.
pub type Stamp = (u64, Option<SystemTime>);

pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// What a session in „Nie ruszaj oryginału” knows about its original.
pub struct OverlayInfo {
    pub original_root: PathBuf,
    pub original_data_path: PathBuf,
    pub manifest: Manifest,
    /// The base's records and version (a 5.5.1 base is compared after conversion when the overlay is GEDCOM 7).
    pub base_records: Vec<Node>,
    pub base_version: Version,
    pub original_missing: bool,
    /// The original's hash as last read, with the size and time it had then.
    original: RefCell<Option<(Stamp, String)>>,
    pending: Cell<Option<usize>>,
    /// A write-back that didn't finish (`interrupted` in the status), until it is tried again.
    pub interrupted: Option<String>,
}

impl OverlayInfo {
    pub fn new(root: &Path, manifest: Manifest, original_missing: bool) -> OverlayInfo {
        let base = read_base(root).unwrap_or_else(|| Document { records: Vec::new(), ..Document::new_v7() });
        OverlayInfo {
            original_root: PathBuf::from(&manifest.original.root),
            original_data_path: PathBuf::from(&manifest.original.data_path),
            base_records: base.records,
            base_version: base.version,
            manifest,
            original_missing,
            original: RefCell::new(None),
            pending: Cell::new(None),
            interrupted: None,
        }
    }

    /// After the base changed on disk (a write-back, a merge).
    pub fn reload_base(&mut self, root: &Path) {
        if let Some(base) = read_base(root) {
            self.base_records = base.records;
            self.base_version = base.version;
        }
        if let Some(manifest) = read_manifest(root) {
            self.manifest = manifest;
        }
        self.pending.set(None);
        *self.original.borrow_mut() = None;
    }

    pub fn forget_pending(&self) {
        self.pending.set(None);
    }

    /// The base in the form of `version` (the overlay's).
    pub fn base_as(&self, version: Version) -> Vec<Node> {
        if version == Version::V7 && self.base_version != Version::V7 {
            heirloom_core::gedcom::upgrade_records(&self.base_records, self.base_version)
        } else {
            self.base_records.clone()
        }
    }

    /// How many records the user's saved version has that the archive doesn't (the header aside).
    pub fn pending(&self, archive: &Archive) -> usize {
        if let Some(n) = self.pending.get() {
            return n;
        }
        let base = self.base_as(archive.doc.version);
        let n = heirloom_core::history::diff(&base, archive.saved(), "", None).iter().filter(|e| e.tag != "HEAD").count();
        self.pending.set(Some(n));
        n
    }

    /// The original's data file hash now (None when it can't be read); hashed only when its size or time changed.
    pub fn original_hash(&self) -> Option<String> {
        let now = stamp(&self.original_data_path)?;
        if let Some((seen, hash)) = self.original.borrow().as_ref() {
            if *seen == now {
                return Some(hash.clone());
            }
        }
        let bytes = fs::read(&self.original_data_path).ok()?;
        let hash = hash_hex(&bytes);
        *self.original.borrow_mut() = Some((now, hash.clone()));
        Some(hash)
    }

    /// Someone changed the archive since the last sync.
    pub fn original_changed(&self) -> bool {
        !self.original_missing && self.original_hash().is_some_and(|h| h != self.manifest.base.hash)
    }
}
