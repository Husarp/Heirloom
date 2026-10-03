//! The family archive: a normal folder with the data in GEDCOM, the media files, and Heirloom's own small
//! files in `.heirloom/` (PLAN.md §11.2).
//!
//! Saving is safe: a backup copy of the data file is made first, the new version is written next to it and
//! swapped in, changes made meanwhile by another program are detected, and the first save into a file that
//! came from another program must be confirmed.

use crate::gedcom::{Document, Node, Warning};
use crate::{Error, PRODUCT_ID, Result, history};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SIDECAR_DIR: &str = ".heirloom";
pub const DEFAULT_DATA_FILE: &str = "rodzina.ged";
pub const MEDIA_DIR: &str = "media";
const SETTINGS_FILE: &str = "ustawienia.json";
const HISTORY_FILE: &str = "historia.jsonl";
const BACKUP_DIR: &str = "kopie";

/// `.heirloom/ustawienia.json`: settings that belong to this archive, not to the computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub format: String,
    pub version: u32,
    /// Names the cache folder, so the cache follows the archive when the folder is moved.
    pub archive_id: String,
    /// Shown on Start and in the archive picker.
    pub name: String,
    pub data_file: String,
    pub read_only: bool,
    /// The first save into another program's file was confirmed.
    pub foreign_save_confirmed: bool,
    pub backups_to_keep: usize,
    /// Copy the data file into `.heirloom/kopie/` before each save (Ustawienia › Kopie zapasowe).
    pub backup_before_save: bool,
    /// The names offered by "Kto edytuje?" (Ustawienia › Osoby edytujące).
    pub editors: Vec<EditorEntry>,
    /// Display choices the app reads (tree, names, dates). Kept as a free-form object, so keys written by a newer
    /// version of the app survive.
    pub display: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorEntry {
    pub name: String,
    /// When this person last saved a change (RFC 3339).
    pub last_edited: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            format: "heirloom-settings".into(),
            version: 1,
            archive_id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            data_file: DEFAULT_DATA_FILE.into(),
            read_only: false,
            foreign_save_confirmed: false,
            backups_to_keep: 20,
            backup_before_save: true,
            editors: Vec::new(),
            display: serde_json::Map::new(),
        }
    }
}

/// A change to whole records. Records are identified by their xref (`@I1@`).
#[derive(Debug, Clone)]
pub enum Edit {
    Replace(Node),
    Add(Node),
    Remove(String),
}

pub struct SaveOptions<'a> {
    /// The name chosen in "Kto edytuje?"; goes into the history.
    pub author: &'a str,
    /// Set by the Import, so a whole import can be undone later.
    pub batch: Option<&'a str>,
    /// A note for the history ("akty z Łęcznej, od cioci Heleny").
    pub note: Option<&'a str>,
    /// The user confirmed saving into a file that came from another program.
    pub allow_foreign: bool,
}

#[derive(Debug)]
pub struct SaveReport {
    pub backup: Option<PathBuf>,
    pub history_entries: usize,
    /// The time every history entry of this save carries: it names the save for „Cofnij zapis”.
    pub history_ts: Option<String>,
    /// The data was saved, but the change history couldn't be written (e.g. `historia.jsonl` locked).
    pub history_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified: Option<SystemTime>,
    hash: blake3::Hash,
}

struct Change {
    index: usize,
    before: Option<Node>,
    after: Option<Node>,
}

pub struct Archive {
    root: PathBuf,
    sidecar: PathBuf,
    settings: Settings,
    settings_changed: bool,
    /// `ustawienia.json` couldn't be read; it is kept aside (not overwritten) when the settings are next saved.
    settings_damaged: bool,
    pub doc: Document,
    /// Problems found while reading the data file.
    pub warnings: Vec<Warning>,
    /// The records as they are on disk: for the history and "unsaved changes".
    saved: Vec<Node>,
    fingerprint: Fingerprint,
    /// Each step is a group of record changes made by one action.
    undo_stack: Vec<Vec<Change>>,
    redo_stack: Vec<Vec<Change>>,
}

impl Archive {
    /// Creates a new, empty archive (GEDCOM 7) in `root`.
    pub fn create(root: &Path, name: &str) -> Result<Archive> {
        Archive::create_from_document(root, name, Document::new_v7())
    }

    /// Creates an archive around an existing document (used by the test-family generator).
    pub fn create_from_document(root: &Path, name: &str, doc: Document) -> Result<Archive> {
        let data = root.join(DEFAULT_DATA_FILE);
        if data.exists() {
            return Err(Error::AlreadyExists(data));
        }
        // An archive whose data file has another name: its settings (id, name, confirmations) must not be
        // overwritten by the new, empty archive's.
        let settings = root.join(SIDECAR_DIR).join(SETTINGS_FILE);
        if settings.exists() {
            return Err(Error::AlreadyExists(settings));
        }
        fs::create_dir_all(root.join(MEDIA_DIR))?;
        let bytes = doc.to_bytes();
        write_atomically(&data, &bytes)?;
        let mut archive = Archive {
            root: root.to_path_buf(),
            sidecar: root.join(SIDECAR_DIR),
            settings: Settings { name: name.to_string(), ..Settings::default() },
            settings_changed: true,
            settings_damaged: false,
            saved: doc.records.clone(),
            fingerprint: fingerprint(&data, &bytes)?,
            doc,
            warnings: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        };
        archive.save_settings()?;
        Ok(archive)
    }

    /// Opens a folder (finds its GEDCOM file) or a `.ged` file directly. Reading never changes anything.
    pub fn open(path: &Path) -> Result<Archive> {
        let (root, opened_file) = if path.is_file() {
            let root = path.parent().map(Path::to_path_buf).unwrap_or_default();
            (root, path.file_name().map(|n| n.to_string_lossy().into_owned()))
        } else {
            (path.to_path_buf(), None)
        };
        let mut sidecar = root.join(SIDECAR_DIR);
        // The settings are only display information (PLAN §11.2 rule 2): if they are damaged, the archive still
        // opens, with defaults.
        let (mut stored, settings_damaged) = match read_settings(&sidecar.join(SETTINGS_FILE)) {
            Ok(stored) => (stored, false),
            Err(Error::Json(_)) => (None, true),
            Err(e) => return Err(e),
        };
        if let Some(s) = &mut stored {
            // Both are used as file names; a shared folder must not point them anywhere else.
            if !is_plain_name(&s.archive_id) {
                s.archive_id = uuid::Uuid::new_v4().to_string();
            }
            if !is_plain_name(&s.data_file) {
                s.data_file.clear();
            }
        }
        let stored_file = stored.as_ref().map(|s| s.data_file.clone()).filter(|f| !f.is_empty());
        let data_file = match (&opened_file, &stored_file) {
            (Some(file), _) => file.clone(),
            (None, Some(file)) => file.clone(),
            (None, None) => find_data_file(&root)?,
        };
        if stored.is_none() && !settings_damaged {
            // A read-only folder may have its settings kept inside the app instead (PLAN §11.2 rule 6).
            if let Some(fallback) = fallback_sidecar(&root.join(&data_file)) {
                if let Ok(Some(s)) = read_settings(&fallback.join(SETTINGS_FILE)) {
                    stored = Some(s);
                    sidecar = fallback;
                }
            }
        }
        let settings_changed = stored.as_ref().is_none_or(|s| s.data_file != data_file);
        let mut settings = stored.unwrap_or_else(|| Settings { name: folder_name(&root), ..Settings::default() });
        if settings.data_file != data_file {
            // Saving into another program's file was confirmed for the archive's previous data file, not this one.
            settings.foreign_save_confirmed = false;
        }
        settings.data_file = data_file;

        let data = root.join(&settings.data_file);
        let bytes = fs::read(&data)?;
        let (doc, mut warnings) = Document::from_bytes(&bytes);
        if settings_damaged {
            warnings.insert(0, Warning {
                line: 0,
                message: format!(
                    "Plik ustawień {SIDECAR_DIR}/{SETTINGS_FILE} jest uszkodzony — użyto ustawień domyślnych. \
                     Przy zapisie zostanie zachowany jako {SETTINGS_FILE}.uszkodzony."
                ),
            });
        }
        Ok(Archive {
            fingerprint: fingerprint(&data, &bytes)?,
            root,
            sidecar,
            settings,
            settings_changed,
            settings_damaged,
            saved: doc.records.clone(),
            doc,
            warnings,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn data_path(&self) -> PathBuf {
        self.root.join(&self.settings.data_file)
    }

    pub fn sidecar_path(&self) -> &Path {
        &self.sidecar
    }

    pub fn history_path(&self) -> PathBuf {
        self.sidecar.join(HISTORY_FILE)
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.sidecar.join(BACKUP_DIR)
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Settings to change; they are written with [`Archive::save_settings`] or the next save.
    pub fn settings_mut(&mut self) -> &mut Settings {
        self.settings_changed = true;
        &mut self.settings
    }

    /// Identifies the data file's content as last read or written (the cache uses it to know it is fresh).
    pub fn fingerprint_hex(&self) -> String {
        self.fingerprint.hash.to_hex().to_string()
    }

    /// The program that wrote the file (HEAD.SOUR).
    pub fn origin(&self) -> Option<&str> {
        self.doc.source_program()
    }

    /// The file came from another program: the first save into it asks first (PLAN §11.2 rule 4).
    pub fn is_foreign(&self) -> bool {
        self.origin() != Some(PRODUCT_ID)
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.doc.records != self.saved
    }

    /// How many records differ from the file on disk ("3 niezapisane zmiany").
    pub fn unsaved_records(&self) -> usize {
        if !self.has_unsaved_changes() {
            return 0;
        }
        // Records that only moved still count as one change, and so does the header alone; next to other changes the
        // header isn't one of its own (Heirloom declares its extension tags there with the first edit).
        history::diff(&self.saved, &self.doc.records, "", None).iter().filter(|e| e.tag != "HEAD").count().max(1)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// How many steps can be undone: a profile section remembers it when opened, and „Anuluj” undoes back to it.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Converts a GEDCOM 5.5.1 file to GEDCOM 7 in memory before the first edit, so everything Heirloom adds is
    /// written one way ([`Document::upgrade_to_v7`]). The on-disk records are compared in the same form, so only
    /// real edits count as changes; the file becomes 7.0 at the next save, after a backup copy of the original.
    /// Returns false when there was nothing to convert.
    pub fn upgrade_to_v7(&mut self) -> bool {
        let from = self.doc.version;
        if !self.doc.upgrade_to_v7() {
            return false;
        }
        self.saved = crate::gedcom::upgrade_records(&self.saved, from);
        self.undo_stack.clear();
        self.redo_stack.clear();
        true
    }

    /// Changes whole records in memory; [`Archive::save`] writes them. Every change can be undone.
    pub fn apply(&mut self, edit: Edit) -> Result<()> {
        self.apply_all(vec![edit])
    }

    /// Several record changes that belong together (e.g. a new child, its family and its parent) as one undo step.
    /// If one of them fails, the ones already made are rolled back and nothing changes.
    pub fn apply_all(&mut self, edits: Vec<Edit>) -> Result<()> {
        let mut group = Vec::with_capacity(edits.len());
        for edit in edits {
            match self.apply_one(edit) {
                Ok(change) => group.push(change),
                Err(e) => {
                    for change in group.iter().rev() {
                        self.revert(change);
                    }
                    return Err(e);
                }
            }
        }
        if !group.is_empty() {
            self.undo_stack.push(group);
            self.redo_stack.clear();
        }
        Ok(())
    }

    fn apply_one(&mut self, edit: Edit) -> Result<Change> {
        Ok(match edit {
            // The header has no xref; it is the one record found by its tag.
            Edit::Replace(node) if node.xref.is_none() && node.tag == "HEAD" => {
                let index = self.doc.records.iter().position(|r| r.tag == "HEAD").ok_or(Error::NoSuchRecord("HEAD".into()))?;
                let before = std::mem::replace(&mut self.doc.records[index], node.clone());
                Change { index, before: Some(before), after: Some(node) }
            }
            Edit::Replace(node) => {
                let xref = node.xref.clone().ok_or(Error::MissingXref)?;
                let index = self.doc.position(&xref).ok_or(Error::NoSuchRecord(xref))?;
                let before = std::mem::replace(&mut self.doc.records[index], node.clone());
                Change { index, before: Some(before), after: Some(node) }
            }
            Edit::Add(node) => {
                let xref = node.xref.clone().ok_or(Error::MissingXref)?;
                if self.doc.position(&xref).is_some() {
                    return Err(Error::DuplicateRecord(xref));
                }
                let records = &self.doc.records;
                let index = records.iter().rposition(|r| r.tag == "TRLR").unwrap_or(records.len());
                self.doc.records.insert(index, node.clone());
                Change { index, before: None, after: Some(node) }
            }
            Edit::Remove(xref) => {
                let index = self.doc.position(&xref).ok_or(Error::NoSuchRecord(xref))?;
                let before = self.doc.records.remove(index);
                Change { index, before: Some(before), after: None }
            }
        })
    }

    fn revert(&mut self, change: &Change) {
        match (&change.before, &change.after) {
            (Some(before), Some(_)) => self.doc.records[change.index] = before.clone(),
            (None, Some(_)) => {
                self.doc.records.remove(change.index);
            }
            (Some(before), None) => self.doc.records.insert(change.index, before.clone()),
            (None, None) => {}
        }
    }

    fn reapply(&mut self, change: &Change) {
        match (&change.before, &change.after) {
            (Some(_), Some(after)) => self.doc.records[change.index] = after.clone(),
            (None, Some(after)) => self.doc.records.insert(change.index, after.clone()),
            (Some(_), None) => {
                self.doc.records.remove(change.index);
            }
            (None, None) => {}
        }
    }

    /// Undoes the last change (a whole group); returns false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(group) = self.undo_stack.pop() else { return false };
        for change in group.iter().rev() {
            self.revert(change);
        }
        self.redo_stack.push(group);
        true
    }

    /// Takes the last change group back without offering it for Redo: a change that couldn't be saved and is made
    /// again from the start (the Import). Returns false when there is nothing to take back.
    pub fn discard_last(&mut self) -> bool {
        let Some(group) = self.undo_stack.pop() else { return false };
        for change in group.iter().rev() {
            self.revert(change);
        }
        true
    }

    /// Redoes the last undone change; returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(group) = self.redo_stack.pop() else { return false };
        for change in &group {
            self.reapply(change);
        }
        self.undo_stack.push(group);
        true
    }

    /// Whether another program changed or removed the data file since it was read or saved. Quick, for
    /// polling: an unchanged size and time count as unchanged ([`Archive::save`] also compares the contents).
    pub fn changed_on_disk(&self) -> Result<bool> {
        let data = self.data_path();
        let meta = match fs::metadata(&data) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(e) => return Err(e.into()),
        };
        if meta.len() == self.fingerprint.len && meta.modified().ok() == self.fingerprint.modified {
            return Ok(false);
        }
        Ok(blake3::hash(&fs::read(&data)?) != self.fingerprint.hash)
    }

    /// Like [`Archive::changed_on_disk`], but always compares the contents.
    fn changed_on_disk_exact(&self) -> Result<bool> {
        match fs::read(self.data_path()) {
            Ok(bytes) => Ok(blake3::hash(&bytes) != self.fingerprint.hash),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
            Err(e) => Err(e.into()),
        }
    }

    /// Writes the changes: backup copy, then the new file swapped in, then the history.
    pub fn save(&mut self, options: &SaveOptions) -> Result<SaveReport> {
        if self.settings.read_only {
            return Err(Error::ReadOnly);
        }
        // Remembered as confirmed only once the file is written: until then the next attempt is still the first save
        // and still keeps a copy of the original.
        let first_foreign_save = self.is_foreign() && !self.settings.foreign_save_confirmed;
        if first_foreign_save && !options.allow_foreign {
            return Err(Error::ForeignNeedsConfirmation(self.origin().unwrap_or("nieznany program").to_string()));
        }
        // Saving copies the whole file anyway, so its contents are compared, not only its size and time: a
        // same-size edit can keep the old time (2-second steps on FAT32 USB sticks, tools that preserve it).
        if self.changed_on_disk_exact()? {
            return Err(Error::Conflict);
        }
        let mut entries = history::diff(&self.saved, &self.doc.records, options.author, options.batch);
        if let Some(note) = options.note.filter(|n| !n.trim().is_empty()) {
            for entry in &mut entries {
                entry.note = Some(note.trim().to_string());
            }
        }
        // Records only moved (removed and added back unchanged) are still written, or "unsaved changes" would
        // never clear.
        if entries.is_empty() && self.doc.records == self.saved {
            self.save_settings()?;
            return Ok(SaveReport { backup: None, history_entries: 0, history_ts: None, history_error: None });
        }
        self.ensure_sidecar()?;
        // The first save into another program's file always keeps the original: its confirmation promises that.
        let backup = if self.settings.backup_before_save || first_foreign_save { self.backup()? } else { None };
        let data = self.data_path();
        let bytes = self.doc.to_bytes();
        write_atomically(&data, &bytes)?;
        // From here on the save has happened: nothing below may report it as failed (the Import would add its
        // batch a second time).
        let hash = blake3::hash(&bytes);
        self.fingerprint = fingerprint(&data, &bytes).unwrap_or(Fingerprint { len: bytes.len() as u64, modified: None, hash });
        self.saved = self.doc.records.clone();
        if first_foreign_save {
            self.settings.foreign_save_confirmed = true;
            self.settings_changed = true;
        }
        // The data is safely on disk now; a history that can't be written is reported, not treated as a failed save.
        let history_error = history::append(&self.history_path(), &entries).err().map(|e| e.to_string());
        if !entries.is_empty() && !options.author.is_empty() {
            let now = history::now();
            match self.settings.editors.iter_mut().find(|e| e.name == options.author) {
                Some(editor) => editor.last_edited = Some(now),
                None => self.settings.editors.push(EditorEntry { name: options.author.to_string(), last_edited: Some(now) }),
            }
            self.settings_changed = true;
        }
        // Settings that can't be written now (the file open in a sync program) stay marked as changed and are
        // written with the next save.
        let _ = self.save_settings();
        let history_ts = entries.first().map(|e| e.ts.clone());
        Ok(SaveReport { backup, history_entries: entries.len(), history_ts, history_error })
    }

    /// "Zapisz moje zmiany jako kopię": writes the current state to a new file next to the data file.
    pub fn save_as_copy(&self) -> Result<PathBuf> {
        let path = self.root.join(format!("{} (kopia {}).ged", self.data_stem(), compact_timestamp()));
        write_atomically(&path, &self.doc.to_bytes())?;
        Ok(path)
    }

    /// "Wczytaj nową wersję": drops unsaved changes and reads the file again.
    pub fn reload(&mut self) -> Result<()> {
        let data = self.data_path();
        let bytes = fs::read(&data)?;
        let (doc, warnings) = Document::from_bytes(&bytes);
        self.fingerprint = fingerprint(&data, &bytes)?;
        self.saved = doc.records.clone();
        self.doc = doc;
        self.warnings = warnings;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    /// Writes `.heirloom/ustawienia.json` if the settings changed.
    pub fn save_settings(&mut self) -> Result<()> {
        if !self.settings_changed {
            return Ok(());
        }
        self.ensure_sidecar()?;
        let path = self.sidecar.join(SETTINGS_FILE);
        if self.settings_damaged {
            // Keep the damaged file for inspection instead of overwriting it.
            let _ = fs::rename(&path, self.sidecar.join(format!("{SETTINGS_FILE}.uszkodzony")));
            self.settings_damaged = false;
        }
        let json = serde_json::to_string_pretty(&self.settings)?;
        write_atomically(&path, json.as_bytes())?;
        self.settings_changed = false;
        Ok(())
    }

    /// Creates `.heirloom/`, or — if the archive folder can't be written — a folder inside the app.
    fn ensure_sidecar(&mut self) -> Result<()> {
        match fs::create_dir_all(&self.sidecar) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                let fallback = fallback_sidecar(&self.data_path()).ok_or(e)?;
                fs::create_dir_all(&fallback)?;
                self.sidecar = fallback;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn data_stem(&self) -> String {
        Path::new(&self.settings.data_file).file_stem().map_or_else(|| "dane".into(), |s| s.to_string_lossy().into_owned())
    }

    /// Copies the data file into `.heirloom/kopie/` and keeps only the newest copies.
    fn backup(&self) -> Result<Option<PathBuf>> {
        let data = self.data_path();
        if !data.exists() {
            return Ok(None);
        }
        let dir = self.backup_dir();
        fs::create_dir_all(&dir)?;
        let stem = self.data_stem();
        let target = dir.join(format!("{stem}-{}.ged", compact_timestamp()));
        fs::copy(&data, &target)?;
        // Only this data file's own copies, never the one just made: copies of another data file
        // ("rodzina-stara-…" also starts with "rodzina-") and files put here by hand are not ours to delete.
        let mut copies: Vec<PathBuf> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| *p != target && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| is_backup_of(n, &stem)))
            .collect();
        copies.sort();
        let keep = self.settings.backups_to_keep.max(1) - 1;
        if copies.len() > keep {
            for old in &copies[..copies.len() - keep] {
                // Tidying up must not stop the save (an old copy may be open in another program).
                let _ = fs::remove_file(old);
            }
        }
        Ok(Some(target))
    }
}

/// A file name without folders or `..` (settings values used as names must not point anywhere else).
fn is_plain_name(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', ':']) && !name.starts_with("..")
}

/// `<stem>-20260928-164005-123.ged`: a copy made by [`Archive::backup`] (see [`compact_timestamp`]).
fn is_backup_of(name: &str, stem: &str) -> bool {
    let stamp = name.strip_prefix(stem).and_then(|n| n.strip_prefix('-')).and_then(|n| n.strip_suffix(".ged"));
    stamp.is_some_and(|s| {
        s.len() == 19 && s.bytes().enumerate().all(|(i, b)| if i == 8 || i == 15 { b == b'-' } else { b.is_ascii_digit() })
    })
}

/// Writes a new file next to the target and swaps it in, so a crash never leaves a half-written file.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().map_or_else(|| "plik".into(), |n| n.to_string_lossy().into_owned());
    let temp = path.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })
}

fn fingerprint(path: &Path, bytes: &[u8]) -> Result<Fingerprint> {
    let meta = fs::metadata(path)?;
    Ok(Fingerprint { len: meta.len(), modified: meta.modified().ok(), hash: blake3::hash(bytes) })
}

fn read_settings(path: &Path) -> Result<Option<Settings>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Where the `.heirloom` files go when the archive folder is read-only: inside the app, keyed by the data file.
fn fallback_sidecar(data_file: &Path) -> Option<PathBuf> {
    let absolute = fs::canonicalize(data_file).unwrap_or_else(|_| data_file.to_path_buf());
    let key = blake3::hash(absolute.to_string_lossy().as_bytes()).to_hex();
    Some(dirs::data_local_dir()?.join("Heirloom").join("archives").join(&key[..16]))
}

fn find_data_file(root: &Path) -> Result<String> {
    let mut found: Vec<String> = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.to_ascii_lowercase().ends_with(".ged"))
        .collect();
    found.sort();
    if found.iter().any(|n| n == DEFAULT_DATA_FILE) {
        return Ok(DEFAULT_DATA_FILE.to_string());
    }
    match found.len() {
        0 => Err(Error::NoDataFile),
        1 => Ok(found.remove(0)),
        _ => Err(Error::SeveralDataFiles(found)),
    }
}

fn folder_name(root: &Path) -> String {
    root.file_name().map_or_else(|| "Archiwum".into(), |n| n.to_string_lossy().into_owned())
}

/// "20260928-164005-123" (UTC): sorts in time order, safe in file names.
fn compact_timestamp() -> String {
    let format = time::macros::format_description!("[year][month][day]-[hour][minute][second]-[subsecond digits:3]");
    time::OffsetDateTime::now_utc().format(&format).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(xref: &str, name: &str) -> Node {
        Node::new("INDI").with_xref(xref).with_child(Node::with_value("NAME", name))
    }

    fn options() -> SaveOptions<'static> {
        SaveOptions { author: "Ewa", batch: None, note: None, allow_foreign: false }
    }

    #[test]
    fn create_edit_save_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Rodzina Kowalskich");
        let mut archive = Archive::create(&root, "Rodzina Kowalskich").unwrap();
        assert!(root.join("rodzina.ged").is_file() && root.join("media").is_dir());
        assert!(root.join(".heirloom").join("ustawienia.json").is_file());
        assert!(!archive.is_foreign() && !archive.has_unsaved_changes());

        archive.apply(Edit::Add(person("@I1@", "Józef /Kowalski/"))).unwrap();
        assert!(archive.has_unsaved_changes());
        let report = archive.save(&options()).unwrap();
        assert_eq!(report.history_entries, 1);
        assert!(report.backup.unwrap().is_file());
        assert!(!archive.has_unsaved_changes());

        let reopened = Archive::open(&root).unwrap();
        assert_eq!(reopened.settings().name, "Rodzina Kowalskich");
        assert_eq!(reopened.settings().archive_id, archive.settings().archive_id);
        assert!(reopened.doc.record("@I1@").is_some());
        assert_eq!(reopened.doc.records.last().unwrap().tag, "TRLR", "new records go before the trailer");
        let history = history::read(&reopened.history_path()).unwrap();
        assert_eq!((history[0].author.as_str(), history[0].record.as_str()), ("Ewa", "@I1@"));
    }

    #[test]
    fn undo_and_redo() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        let original = archive.doc.records.clone();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        archive.apply(Edit::Replace(person("@I1@", "Jan /Nowakowski/"))).unwrap();
        archive.apply(Edit::Remove("@I1@".into())).unwrap();
        assert!(archive.doc.record("@I1@").is_none());
        assert!(archive.undo() && archive.undo());
        assert_eq!(archive.doc.record("@I1@").unwrap().child_value("NAME"), Some("Jan /Nowak/"));
        assert!(archive.redo());
        assert_eq!(archive.doc.record("@I1@").unwrap().child_value("NAME"), Some("Jan /Nowakowski/"));
        while archive.undo() {}
        assert_eq!(archive.doc.records, original);
        assert!(!archive.has_unsaved_changes(), "undoing everything means nothing to save");
        assert!(matches!(archive.apply(Edit::Remove("@I9@".into())), Err(Error::NoSuchRecord(_))));
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(matches!(archive.apply(Edit::Add(person("@I1@", "X /Y/"))), Err(Error::DuplicateRecord(_))));
    }

    #[test]
    fn detects_changes_made_by_another_program() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        let data = archive.data_path();
        let mut other = fs::read_to_string(&data).unwrap();
        other.push_str("0 @I9@ INDI\r\n");
        fs::write(&data, other).unwrap();
        assert!(archive.changed_on_disk().unwrap());
        assert!(matches!(archive.save(&options()), Err(Error::Conflict)));
        let copy = archive.save_as_copy().unwrap();
        assert!(fs::read_to_string(copy).unwrap().contains("@I1@"));
        archive.reload().unwrap();
        assert!(archive.doc.record("@I9@").is_some() && archive.doc.record("@I1@").is_none());
        assert!(!archive.changed_on_disk().unwrap());
    }

    #[test]
    fn other_programs_files_need_confirmation_once() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("drzewo z MyHeritage.ged");
        fs::write(&file, "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 TRLR\n").unwrap();
        let mut archive = Archive::open(&file).unwrap();
        assert!(archive.is_foreign());
        assert_eq!(archive.origin(), Some("MYHERITAGE"));
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(matches!(archive.save(&options()), Err(Error::ForeignNeedsConfirmation(p)) if p == "MYHERITAGE"));
        archive.save(&SaveOptions { allow_foreign: true, ..options() }).unwrap();
        archive.apply(Edit::Remove("@I1@".into())).unwrap();
        archive.save(&options()).expect("confirmed once for this archive");
        let reopened = Archive::open(dir.path()).unwrap();
        assert!(reopened.settings().foreign_save_confirmed);
        assert_eq!(reopened.settings().data_file, "drzewo z MyHeritage.ged");
        assert!(fs::read_to_string(&file).unwrap().starts_with("0 HEAD\n1 SOUR MYHERITAGE\n"), "LF endings kept");
    }

    #[test]
    fn read_only_archives_refuse_to_save() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.settings_mut().read_only = true;
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(matches!(archive.save(&options()), Err(Error::ReadOnly)));
    }

    #[test]
    fn keeps_only_the_newest_backups() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.settings_mut().backups_to_keep = 3;
        for i in 0..6 {
            archive.apply(Edit::Add(person(&format!("@I{i}@"), "Jan /Nowak/"))).unwrap();
            archive.save(&options()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(fs::read_dir(archive.backup_dir()).unwrap().count(), 3);
        assert_eq!(history::read(&archive.history_path()).unwrap().len(), 6);
    }

    #[test]
    fn undo_and_redo_stay_in_step_over_long_mixed_sequences() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        // Every state the document went through; `at` is the current one.
        let mut states = vec![archive.doc.records.clone()];
        let mut at = 0usize;
        let mut seed = 7u64;
        for step in 0..3000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let xref = format!("@I{}@", (seed >> 40) % 6);
            let edit = match (seed >> 33) % 10 {
                0..=2 => Some(Edit::Add(person(&xref, &format!("Dodany{step} /X/")))),
                3 | 4 => Some(Edit::Replace(person(&xref, &format!("Zmieniony{step} /X/")))),
                5 => Some(Edit::Remove(xref)),
                6 | 7 => {
                    assert_eq!(archive.undo(), at > 0);
                    at = at.saturating_sub(1);
                    None
                }
                _ => {
                    assert_eq!(archive.redo(), at + 1 < states.len());
                    at = (at + 1).min(states.len() - 1);
                    None
                }
            };
            if let Some(edit) = edit
                && archive.apply(edit).is_ok()
            {
                states.truncate(at + 1);
                states.push(archive.doc.records.clone());
                at += 1;
            }
            assert_eq!(archive.doc.records, states[at], "step {step}");
        }
    }

    #[test]
    fn a_record_removed_and_added_back_can_be_saved() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        archive.apply(Edit::Add(person("@I2@", "Anna /Nowak/"))).unwrap();
        archive.save(&options()).unwrap();
        archive.apply(Edit::Remove("@I1@".into())).unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(archive.has_unsaved_changes(), "the order changed");
        assert_eq!(archive.save(&options()).unwrap().history_entries, 0, "no record changed");
        assert!(!archive.has_unsaved_changes(), "before the fix nothing was written and this never cleared");
    }

    #[test]
    fn a_same_size_edit_that_keeps_the_old_time_is_still_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        archive.save(&options()).unwrap();
        let data = archive.data_path();
        let time = fs::metadata(&data).unwrap().modified().unwrap();
        fs::write(&data, fs::read_to_string(&data).unwrap().replace("Nowak", "Nowik")).unwrap();
        fs::File::options().write(true).open(&data).unwrap().set_modified(time).unwrap();
        archive.apply(Edit::Add(person("@I2@", "Anna /Nowak/"))).unwrap();
        assert!(matches!(archive.save(&options()), Err(Error::Conflict)));
        assert!(fs::read_to_string(&data).unwrap().contains("Nowik"), "the other program's edit is kept");
    }

    #[test]
    fn creating_where_an_archive_already_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("drzewo.ged");
        fs::write(&file, "0 HEAD\n1 SOUR MYHERITAGE\n0 TRLR\n").unwrap();
        let mut existing = Archive::open(&file).unwrap();
        existing.save_settings().unwrap();
        assert!(matches!(Archive::create(dir.path(), "Nowe"), Err(Error::AlreadyExists(_))));
        let reopened = Archive::open(dir.path()).unwrap();
        assert_eq!(reopened.settings().archive_id, existing.settings().archive_id, "settings not overwritten");
        assert!(!dir.path().join(DEFAULT_DATA_FILE).exists());
    }

    #[test]
    fn confirming_one_foreign_file_does_not_cover_another() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("z MyHeritage.ged");
        let second = dir.path().join("z Ancestry.ged");
        fs::write(&first, "0 HEAD\n1 SOUR MYHERITAGE\n0 TRLR\n").unwrap();
        fs::write(&second, "0 HEAD\n1 SOUR ANCESTRY\n0 TRLR\n").unwrap();
        let mut archive = Archive::open(&first).unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        archive.save(&SaveOptions { allow_foreign: true, ..options() }).unwrap();
        let mut other = Archive::open(&second).unwrap();
        assert!(!other.settings().foreign_save_confirmed);
        other.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(matches!(other.save(&options()), Err(Error::ForeignNeedsConfirmation(p)) if p == "ANCESTRY"));
    }

    #[test]
    fn backup_rotation_never_deletes_the_new_copy_or_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.settings_mut().backups_to_keep = 2;
        let kopie = archive.backup_dir();
        fs::create_dir_all(&kopie).unwrap();
        let others = ["rodzina-stara-20260101-120000-000.ged", "rodzina-1999.ged", "rodzina-przed importem.ged"];
        for name in others {
            fs::write(kopie.join(name), "0 HEAD\n0 TRLR\n").unwrap();
        }
        // Copies dated in the future (the clock was wrong once) sort after the new one.
        for name in ["rodzina-20990101-120000-000.ged", "rodzina-20990102-120000-000.ged"] {
            fs::write(kopie.join(name), "0 HEAD\n0 TRLR\n").unwrap();
        }
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        let backup = archive.save(&options()).unwrap().backup.unwrap();
        assert!(backup.is_file(), "the copy just made survives the rotation");
        for name in others {
            assert!(kopie.join(name).is_file(), "{name} is not a copy of rodzina.ged and must stay");
        }
        let own: Vec<_> = fs::read_dir(&kopie)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| is_backup_of(n, "rodzina"))
            .collect();
        assert_eq!(own.len(), 2, "{own:?}");
    }

    #[cfg(windows)]
    #[test]
    fn an_old_backup_held_open_elsewhere_does_not_stop_saving() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.settings_mut().backups_to_keep = 1;
        fs::create_dir_all(archive.backup_dir()).unwrap();
        let old = archive.backup_dir().join("rodzina-20200101-120000-000.ged");
        fs::write(&old, "0 HEAD\n0 TRLR\n").unwrap();
        // Opened without FILE_SHARE_DELETE, as most programs do, so it can't be deleted meanwhile.
        let held = fs::File::options().read(true).share_mode(1).open(&old).unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        archive.save(&options()).expect("the save itself succeeds");
        assert!(!archive.has_unsaved_changes());
        drop(held);
    }

    #[cfg(windows)]
    #[test]
    fn saving_over_a_file_open_in_another_program_fails_cleanly() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        let before = fs::read(archive.data_path()).unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        let held = fs::File::options().read(true).share_mode(1).open(archive.data_path()).unwrap();
        assert!(matches!(archive.save(&options()), Err(Error::Io(_))), "Windows can't replace it");
        drop(held);
        assert_eq!(fs::read(archive.data_path()).unwrap(), before, "the old file is intact");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 3, "no temporary file left behind: {names:?}");
        assert!(archive.has_unsaved_changes());
        archive.save(&options()).expect("saving again works once the file is free");
    }

    #[cfg(windows)]
    #[test]
    fn settings_that_cant_be_written_dont_fail_a_save_already_on_disk() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        let settings = archive.sidecar_path().join(SETTINGS_FILE);
        let held = fs::File::options().read(true).share_mode(1).open(&settings).unwrap();
        archive.save(&options()).expect("the data file is written, so the save succeeded");
        assert!(fs::read_to_string(archive.data_path()).unwrap().contains("@I1@") && !archive.has_unsaved_changes());
        drop(held);
        archive.save_settings().unwrap();
        assert_eq!(Archive::open(dir.path()).unwrap().settings().editors[0].name, "Ewa", "written the next time");
    }

    #[test]
    fn a_failed_first_save_into_a_foreign_file_still_keeps_the_original_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("z MyHeritage.ged");
        fs::write(&file, "0 HEAD\n1 SOUR MYHERITAGE\n0 TRLR\n").unwrap();
        let mut archive = Archive::open(&file).unwrap();
        archive.settings_mut().backup_before_save = false;
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        fs::write(&file, "0 HEAD\n1 SOUR MYHERITAGE\n0 @I9@ INDI\n0 TRLR\n").unwrap();
        let confirmed = SaveOptions { allow_foreign: true, ..options() };
        assert!(matches!(archive.save(&confirmed), Err(Error::Conflict)));
        assert!(!archive.settings().foreign_save_confirmed, "nothing was saved yet");
        archive.reload().unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(archive.save(&confirmed).unwrap().backup.is_some(), "the first real save copies the original");
        assert!(archive.settings().foreign_save_confirmed);
    }

    #[test]
    fn the_last_change_can_be_taken_back_without_a_redo() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(dir.path(), "Test").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        let before = archive.doc.records.clone();
        archive.apply_all(vec![Edit::Add(person("@I2@", "Anna /Nowak/")), Edit::Remove("@I1@".into())]).unwrap();
        assert!(archive.discard_last());
        assert_eq!(archive.doc.records, before);
        assert!(archive.can_undo() && !archive.can_redo());
    }

    #[test]
    fn finding_the_data_file_in_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(Archive::open(dir.path()), Err(Error::NoDataFile)));
        fs::write(dir.path().join("a.ged"), "0 HEAD\n0 TRLR\n").unwrap();
        fs::write(dir.path().join("b.GED"), "0 HEAD\n0 TRLR\n").unwrap();
        assert!(matches!(Archive::open(dir.path()), Err(Error::SeveralDataFiles(list)) if list.len() == 2));
        fs::remove_file(dir.path().join("b.GED")).unwrap();
        let archive = Archive::open(dir.path()).unwrap();
        assert_eq!(archive.settings().data_file, "a.ged");
        assert!(!dir.path().join(".heirloom").exists(), "opening writes nothing");
    }

    #[test]
    fn a_group_of_changes_is_one_undo_step_and_all_or_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(&dir.path().join("a"), "A").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Józef /Kowalski/"))).unwrap();
        let family = Node::new("FAM").with_xref("@F1@").with_child(Node::with_value("HUSB", "@I1@"));
        let child = person("@I2@", "Jan /Kowalski/");
        archive.apply_all(vec![Edit::Add(family), Edit::Add(child)]).unwrap();
        assert!(archive.doc.record("@F1@").is_some() && archive.doc.record("@I2@").is_some());
        assert_eq!(archive.unsaved_records(), 3);
        archive.undo();
        assert!(archive.doc.record("@F1@").is_none() && archive.doc.record("@I2@").is_none());
        assert!(archive.doc.record("@I1@").is_some(), "only the last group is undone");
        archive.redo();
        assert!(archive.doc.record("@F1@").is_some() && archive.doc.record("@I2@").is_some());

        let before = archive.doc.records.clone();
        let result = archive.apply_all(vec![Edit::Add(person("@I3@", "Anna /Nowak/")), Edit::Remove("@I99@".into())]);
        assert!(matches!(result, Err(Error::NoSuchRecord(_))));
        assert_eq!(archive.doc.records, before, "a failed group changes nothing");
    }

    #[test]
    fn damaged_settings_do_not_stop_the_archive_from_opening() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let mut archive = Archive::create(&root, "A").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Józef /Kowalski/"))).unwrap();
        archive.save(&options()).unwrap();
        fs::write(root.join(".heirloom").join("ustawienia.json"), b"{ zepsute").unwrap();

        let mut reopened = Archive::open(&root).unwrap();
        assert!(reopened.doc.record("@I1@").is_some());
        assert!(reopened.warnings[0].message.contains("uszkodzony"));
        reopened.settings_mut().name = "B".into();
        reopened.save_settings().unwrap();
        assert_eq!(fs::read(root.join(".heirloom").join("ustawienia.json.uszkodzony")).unwrap(), b"{ zepsute");
        assert_eq!(Archive::open(&root).unwrap().settings().name, "B");
    }

    #[test]
    fn settings_cannot_point_outside_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        Archive::create(&root, "A").unwrap();
        let path = root.join(".heirloom").join("ustawienia.json");
        let text = fs::read_to_string(&path).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
        json["archiveId"] = "..\\..\\Windows".into();
        json["dataFile"] = "..\\inny.ged".into();
        fs::write(&path, json.to_string()).unwrap();
        let archive = Archive::open(&root).unwrap();
        assert!(is_plain_name(&archive.settings().archive_id));
        assert_eq!(archive.settings().data_file, "rodzina.ged");
    }

    #[test]
    fn the_backup_before_each_save_can_be_turned_off() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let mut archive = Archive::create(&root, "A").unwrap();
        // Settings written before the option existed still load, with the backups on.
        let path = root.join(".heirloom").join("ustawienia.json");
        let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("backupBeforeSave");
        fs::write(&path, json.to_string()).unwrap();
        assert!(Archive::open(&root).unwrap().settings().backup_before_save);

        archive.settings_mut().backup_before_save = false;
        archive.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(archive.save(&options()).unwrap().backup.is_none());
        assert!(!archive.backup_dir().exists());
        assert!(!Archive::open(&root).unwrap().settings().backup_before_save);

        // The first save into another program's file keeps its original anyway.
        let file = dir.path().join("b").join("z MyHeritage.ged");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "0 HEAD\n1 SOUR MYHERITAGE\n0 TRLR\n").unwrap();
        let mut foreign = Archive::open(&file).unwrap();
        foreign.settings_mut().backup_before_save = false;
        foreign.apply(Edit::Add(person("@I1@", "Jan /Nowak/"))).unwrap();
        assert!(foreign.save(&SaveOptions { allow_foreign: true, ..options() }).unwrap().backup.is_some());
        foreign.apply(Edit::Remove("@I1@".into())).unwrap();
        assert!(foreign.save(&options()).unwrap().backup.is_none());
    }

    #[test]
    fn saving_remembers_who_edited() {
        let dir = tempfile::tempdir().unwrap();
        let mut archive = Archive::create(&dir.path().join("a"), "A").unwrap();
        archive.apply(Edit::Add(person("@I1@", "Józef /Kowalski/"))).unwrap();
        archive.save(&SaveOptions { note: Some("akty z Łęcznej"), ..options() }).unwrap();
        assert_eq!(archive.settings().editors[0].name, "Ewa");
        assert!(archive.settings().editors[0].last_edited.is_some());
        let history = history::read(&archive.history_path()).unwrap();
        assert_eq!(history[0].note.as_deref(), Some("akty z Łęcznej"));
    }
}
