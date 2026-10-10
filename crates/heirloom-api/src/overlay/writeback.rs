//! „Przenieś zmiany do archiwum” (A.8): the user's new files are copied into the original, then its data file is
//! replaced by the user's version in one atomic write, then the history goes along. A journal in
//! `.heirloom/zapisy/<czas>.jsonl` records each step, so an interrupted run (a pendrive pulled out) is finished or
//! taken back at the next open, and the last run can be undone exactly while nobody else has changed the archive.
//! Changes made meanwhile in the original are merged in first ([`rebase`]).

use super::merge::{self, Side};
use super::{OverlayInfo, hash_hex, io_error, read_manifest, sidecar, write_manifest};
use crate::gedwrite::file_uri;
use crate::progress::Progress;
use crate::tools::{decode_file, for_each_file};
use crate::{ApiError, ApiResult, Session, count_pl, fsx};
use heirloom_core::archive::{MEDIA_DIR, compact_timestamp, write_atomically};
use heirloom_core::gedcom::{Document, Node, Version};
use heirloom_core::history::{self, Entry};
use heirloom_core::{Archive, Edit, SaveOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const JOURNALS: &str = "zapisy";
const AI_DIR: &str = "zrodla-ai";
const KEEP_JOURNALS: usize = 5;
pub const FROM_ARCHIVE: &str = "fromArchive";
/// Who „made” the changes taken from the archive, in the history of „Twoje zmiany”.
const ARCHIVE_AUTHOR: &str = "Archiwum";

thread_local! {
    /// Tests: stop as if the program had been closed right after this step (`header`, `copy`, `written`).
    static STOP_AFTER: Cell<Option<&'static str>> = const { Cell::new(None) };
}

#[doc(hidden)]
pub fn stop_after(step: Option<&'static str>) {
    STOP_AFTER.with(|s| s.set(step));
}

fn checkpoint(step: &'static str) -> Result<(), ApiError> {
    if STOP_AFTER.with(|s| s.get()) == Some(step) {
        return Err(ApiError::new("stopped_for_test", step));
    }
    Ok(())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Header {
    pub ts: String,
    pub original_data_path: String,
    pub before_hash: String,
    /// The original's `historia.jsonl` size before (None: it has no `.heirloom`, nothing goes there).
    pub history_bytes_before: Option<u64>,
    /// `synced.historyLen` of the manifest before, for the undo.
    pub synced_history_len: usize,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileEntry {
    pub rel: String,
    pub size: u64,
    pub hash: String,
}

/// A journal as read back.
#[derive(Debug, Default)]
pub struct Journal {
    pub path: PathBuf,
    pub header: Header,
    /// Files now in the original: (path, created by this run).
    pub copied: Vec<(String, bool)>,
    /// Folders this run created in the original.
    pub dirs: Vec<String>,
    /// The copy of the original's data file before the write (in `.heirloom/kopie` of „Twoje zmiany”).
    pub pre_copy: Option<String>,
    pub written: Option<String>,
    pub history_bytes_after: Option<u64>,
    /// `done`, `rolledBack`, `undone`, `settled`; None while it runs (or was interrupted).
    pub state: Option<String>,
}

impl Journal {
    fn line(&self, value: Value) -> Result<(), ApiError> {
        let mut file = fs::OpenOptions::new().append(true).open(&self.path).map_err(io_error)?;
        let mut text = value.to_string();
        text.push('\n');
        file.write_all(text.as_bytes()).and_then(|()| file.sync_all()).map_err(io_error)
    }

    fn set_state(&mut self, state: &str) -> Result<(), ApiError> {
        self.line(json!({ "state": state }))?;
        self.state = Some(state.to_string());
        Ok(())
    }

    /// The run finished and can still be undone or settled.
    fn open_for_undo(&self) -> bool {
        self.state.as_deref() == Some("done")
    }

    fn unfinished(&self) -> bool {
        self.state.is_none()
    }
}

fn read_journal(path: &Path) -> Option<Journal> {
    let text = fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let header: Header = serde_json::from_str(lines.next()?).ok()?;
    let mut journal = Journal { path: path.to_path_buf(), header, ..Journal::default() };
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(rel) = value.get("copied").and_then(Value::as_str) {
            journal.copied.push((rel.to_string(), value.get("created").and_then(Value::as_bool).unwrap_or(false)));
        }
        if let Some(dir) = value.get("dir").and_then(Value::as_str) {
            journal.dirs.push(dir.to_string());
        }
        if let Some(copy) = value.get("preCopy").and_then(Value::as_str) {
            journal.pre_copy = Some(copy.to_string());
        }
        if let Some(hash) = value.get("written").and_then(Value::as_str) {
            journal.written = Some(hash.to_string());
        }
        if let Some(n) = value.get("historyBytesAfter").and_then(Value::as_u64) {
            journal.history_bytes_after = Some(n);
        }
        if let Some(state) = value.get("state").and_then(Value::as_str) {
            journal.state = Some(state.to_string());
        }
    }
    Some(journal)
}

fn journal_dir(root: &Path) -> PathBuf {
    sidecar(root).join(JOURNALS)
}

/// The journals, oldest first.
fn journals(root: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = fs::read_dir(journal_dir(root))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    list.sort();
    list
}

pub fn last_journal(root: &Path) -> Option<Journal> {
    journals(root).last().and_then(|p| read_journal(p))
}

fn original_history(original_root: &Path) -> Option<PathBuf> {
    let side = original_root.join(heirloom_core::archive::SIDECAR_DIR);
    side.is_dir().then(|| side.join("historia.jsonl"))
}

fn file_len(path: &Path) -> u64 {
    fs::metadata(path).map_or(0, |m| m.len())
}

/// The files of „Twoje zmiany” (`media/`, `zrodla-ai/`) the original doesn't have with the same content at the same
/// place.
pub fn new_files(root: &Path, original_root: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    for folder in [MEDIA_DIR, AI_DIR] {
        for (rel, path) in fsx::walk(&root.join(folder)) {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if fsx::is_noise(&name) {
                continue;
            }
            let rel = format!("{folder}/{rel}");
            let size = file_len(&path);
            let target = original_root.join(&rel);
            let Ok(hash) = fsx::hash_file(&path) else { continue };
            if file_len(&target) == size && target.is_file() && fsx::hash_file(&target).ok() == Some(hash) {
                continue;
            }
            out.push(FileEntry { rel, size, hash: hash.to_hex().to_string() });
        }
    }
    out
}

/// The FILE values of a data file, as archive-relative paths with `/`.
fn referenced(bytes: &[u8]) -> HashSet<String> {
    let (mut doc, _) = Document::from_bytes(bytes);
    let version = doc.version;
    let mut out = HashSet::new();
    for_each_file(&mut doc.records, false, &mut |file| {
        if let Some(v) = file.value.as_deref() {
            out.insert(fsx::name_key(&decode_file(v, version).replace('\\', "/")));
        }
    });
    out
}

/// Points every FILE that names `from` at `to` (archive-relative paths); the records changed.
fn relink_files(doc: &Document, renames: &HashMap<String, String>) -> Vec<Edit> {
    let version = doc.version;
    let mut edits = Vec::new();
    for record in doc.records.iter().filter(|r| r.xref.is_some()) {
        let mut copy = record.clone();
        let mut changed = false;
        for_each_file(std::slice::from_mut(&mut copy), false, &mut |file| {
            let Some(path) = file.value.as_deref().map(|v| decode_file(v, version).replace('\\', "/")) else { return };
            if let Some(to) = renames.get(&fsx::name_key(&path)) {
                file.value = Some(file_uri(to));
                changed = true;
            }
        });
        if changed {
            edits.push(Edit::Replace(copy));
        }
    }
    edits
}

fn not_overlay() -> ApiError {
    ApiError::new("not_overlay", "To archiwum nie jest otwarte w trybie „Nie ruszaj oryginału”.")
}

fn unsaved() -> ApiError {
    ApiError::new("unsaved", "Najpierw zapisz albo odrzuć niezapisane zmiany.")
}

/// Whether something can be written into the original's folder: the data file opened for writing (nothing written)
/// and a probe file made and removed.
pub fn writable(root: &Path, data: &Path) -> bool {
    if fs::OpenOptions::new().append(true).open(data).is_err() {
        return false;
    }
    let probe = root.join(format!(".heirloom-test-{}", std::process::id()));
    match fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// The same question without writing anything (for what is only shown): the data file opened for writing, and the
/// folder's read-only mark.
pub fn writable_quiet(root: &Path, data: &Path) -> bool {
    fs::OpenOptions::new().append(true).open(data).is_ok() && !(cfg!(unix) && fs::metadata(root).is_ok_and(|m| m.permissions().readonly()))
}

/// What „Przenieś zmiany do archiwum” would do (step 2 of the flow).
pub fn preview(s: &Session) -> ApiResult {
    let info = s.overlay.as_ref().ok_or_else(not_overlay)?;
    let root = s.archive.root();
    let missing = info.original_missing || !info.original_data_path.is_file();
    let pending = info.pending(&s.archive);
    let base = info.base_as(s.archive.doc.version);
    let people = history::diff(&base, s.archive.saved(), "", None).iter().filter(|e| e.tag == "INDI").count();
    let files = if missing { Vec::new() } else { new_files(root, &info.original_root) };
    let original = (!missing).then(|| Archive::open(&info.original_data_path).ok()).flatten();
    let foreign = original.as_ref().filter(|o| o.is_foreign() && !o.settings().foreign_save_confirmed && !info.manifest.original.foreign_confirmed).map(|o| {
        json!({
            "origin": o.origin().unwrap_or("nieznany program"),
            "version": o.doc.head().and_then(|h| h.child("GEDC")).and_then(|g| g.child_value("VERS")).map(str::trim),
            "dataFile": o.settings().data_file,
        })
    });
    Ok(json!({
        "changes": pending,
        "people": people,
        "files": files.len(),
        "bytes": files.iter().map(|f| f.size).sum::<u64>(),
        "originalChanged": info.original_changed(),
        "foreign": foreign,
        "lockedOriginal": original.as_ref().is_some_and(|o| o.settings().read_only),
        "writable": !missing && writable_quiet(&info.original_root, &info.original_data_path),
        "originalMissing": missing,
        "root": info.original_root.display().to_string(),
        "name": s.archive.settings().name,
    }))
}

/// „Przenieś zmiany do archiwum”.
pub fn write_back(s: &mut Session, author: &str, allow_foreign: bool, progress: &Progress) -> ApiResult {
    let info = s.overlay.as_ref().ok_or_else(not_overlay)?;
    if s.archive.has_unsaved_changes() {
        return Err(unsaved());
    }
    let (original_root, original_data) = (info.original_root.clone(), info.original_data_path.clone());
    if info.original_missing || !original_data.is_file() {
        return Err(ApiError::new("original_missing", format!("Nie można otworzyć archiwum ({}) — może pendrive jest odłączony.", original_root.display())));
    }
    let root = s.archive.root().to_path_buf();
    if let Some(open) = last_journal(&root).filter(Journal::unfinished) {
        recover(s, Some(open))?;
        return Err(ApiError::new("interrupted", "Poprzednie przenoszenie nie zostało dokończone — sprawdź stan i spróbuj ponownie."));
    }
    let original = Archive::open(&original_data)?;
    if original.settings().read_only {
        return Err(ApiError::new("locked_original", "Właściciel archiwum zablokował zmiany (Tylko do odczytu). Twoje zmiany zostają u Ciebie."));
    }
    if !writable(&original_root, &original_data) {
        return Err(ApiError::new("not_writable", "W archiwum nie da się nic zapisać (np. płyta lub zablokowany pendrive). Twoje zmiany zostają u Ciebie."));
    }
    let manifest = info.manifest.clone();
    let pending = info.pending(&s.archive);
    if original.is_foreign() && !original.settings().foreign_save_confirmed && !manifest.original.foreign_confirmed && !allow_foreign {
        return Err(heirloom_core::Error::ForeignNeedsConfirmation(original.origin().unwrap_or("nieznany program").to_string()).into());
    }
    let before = fs::read(&original_data).map_err(io_error)?;
    let before_hash = hash_hex(&before);
    if before_hash != manifest.base.hash {
        return Err(ApiError::new("original_changed", "Ktoś zmienił archiwum od ostatniego razu — najpierw połącz jego zmiany z Twoimi."));
    }
    // The previous write-back can no longer be undone from here on.
    if let Some(mut previous) = last_journal(&root).filter(Journal::open_for_undo) {
        settle(&root, &original_root, &mut previous);
    }

    // A file the archive meanwhile got under the same name (with other content) moves aside in „Twoje zmiany” first.
    let mut renames: HashMap<String, String> = HashMap::new();
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    for f in new_files(&root, &original_root) {
        if !original_root.join(&f.rel).exists() {
            continue;
        }
        let (folder, name) = f.rel.rsplit_once('/').unwrap_or(("", f.rel.as_str()));
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s, e),
            _ => (name, ""),
        };
        let taken = |key: &str| [&root, &original_root].iter().any(|layer| fs::read_dir(layer.join(folder)).into_iter().flatten().flatten().any(|e| fsx::name_key(&e.file_name().to_string_lossy()) == key));
        let free = fsx::free_name(&taken, stem, ext);
        let to = if folder.is_empty() { free } else { format!("{folder}/{free}") };
        let (from_path, to_path) = (root.join(&f.rel), root.join(&to));
        fsx::move_no_replace(&from_path, &to_path).map_err(|e| ApiError::new("io", format!("Nie można zmienić nazwy pliku {}: {e}", f.rel)))?;
        moved.push((to_path, from_path));
        renames.insert(fsx::name_key(&f.rel), to);
    }
    if !renames.is_empty() {
        let edits = relink_files(&s.archive.doc, &renames);
        let saved = s.archive.apply_all(edits).map_err(ApiError::from).and_then(|()| {
            s.archive
                .save(&SaveOptions { author, batch: None, note: Some("Zmieniono nazwy plików przy przenoszeniu do archiwum"), allow_foreign: true, kind: None })
                .map_err(ApiError::from)
        });
        s.changed();
        if let Err(e) = saved {
            s.archive.discard_last();
            for (now, was) in moved.iter().rev() {
                let _ = fsx::move_no_replace(now, was);
            }
            return Err(e);
        }
    }

    let files = new_files(&root, &original_root);
    let history_path = original_history(&original_root);
    let header = Header {
        ts: history::now(),
        original_data_path: original_data.display().to_string(),
        before_hash: before_hash.clone(),
        history_bytes_before: history_path.as_deref().map(file_len),
        synced_history_len: manifest.synced.history_len,
        files: files.clone(),
    };
    fs::create_dir_all(journal_dir(&root)).map_err(io_error)?;
    let path = journal_dir(&root).join(format!("{}.jsonl", compact_timestamp()));
    {
        // create_new: a second window can't start the same run.
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(io_error)?;
        let line = serde_json::to_string(&header).map_err(io_error)? + "\n";
        file.write_all(line.as_bytes()).and_then(|()| file.sync_all()).map_err(io_error)?;
    }
    let mut journal = Journal { path, header, ..Journal::default() };
    checkpoint("header")?;

    let copied = copy_files(&root, &original_root, &mut journal, progress).and_then(|()| {
        // The original as it was, kept by „Twoje zmiany” (and by the archive itself when it keeps copies).
        let stem = Path::new(&manifest.original.data_file).file_stem().map_or_else(|| "dane".into(), |s| s.to_string_lossy().into_owned());
        let kopie = sidecar(&root).join("kopie");
        fs::create_dir_all(&kopie).map_err(io_error)?;
        let name = format!("{stem}-oryginal-{}.ged", compact_timestamp());
        fs::write(kopie.join(&name), &before).map_err(io_error)?;
        journal.line(json!({ "preCopy": name }))?;
        journal.pre_copy = Some(name);
        if let Some(history_path) = &history_path {
            if let Some(dir) = history_path.parent().map(|d| d.join("kopie")) {
                if fs::create_dir_all(&dir).is_ok() {
                    let _ = fs::write(dir.join(format!("{stem}-{}.ged", compact_timestamp())), &before);
                }
            }
        }
        Ok(())
    });
    if let Err(e) = copied {
        if e.code == "stopped_for_test" {
            return Err(e);
        }
        rollback(&original_root, &mut journal, None);
        return Err(ApiError::new(e.code, format!("Nic w archiwum się nie zmieniło. {}", e.message)));
    }
    // Someone saved into the archive meanwhile: nothing is overwritten.
    let now = fs::read(&original_data).map_err(io_error)?;
    if hash_hex(&now) != before_hash {
        rollback(&original_root, &mut journal, None);
        return Err(ApiError::new("original_changed", "Nic w archiwum się nie zmieniło: ktoś właśnie je zmienił. Połącz najpierw jego zmiany z Twoimi."));
    }
    let bytes = fs::read(s.archive.data_path()).map_err(io_error)?;
    let after_hash = hash_hex(&bytes);
    if let Err(e) = write_atomically(&original_data, &bytes) {
        rollback(&original_root, &mut journal, None);
        return Err(ApiError::new("io", format!("Nic w archiwum się nie zmieniło. Nie można zapisać pliku danych: {e}")));
    }
    journal.line(json!({ "written": after_hash }))?;
    journal.written = Some(after_hash);
    checkpoint("written")?;
    finish(s, &mut journal)?;
    Ok(json!({ "changes": pending, "files": files.len(), "bytes": files.iter().map(|f| f.size).sum::<u64>() }))
}

fn copy_files(root: &Path, original_root: &Path, journal: &mut Journal, progress: &Progress) -> Result<(), ApiError> {
    let files = journal.header.files.clone();
    progress.start("Przenoszę pliki do archiwum…", files.len() as u64);
    let result = (|| {
        for (n, f) in files.iter().enumerate() {
            progress.step(n as u64);
            let target = original_root.join(&f.rel);
            if let Some(parent) = target.parent() {
                // Folders this run makes (media\Kowalski Józef) go again with a rollback.
                let mut missing = Vec::new();
                let mut dir = parent.to_path_buf();
                while !dir.exists() && dir.starts_with(original_root) {
                    missing.push(dir.clone());
                    dir = dir.parent().map(Path::to_path_buf).unwrap_or_default();
                }
                fs::create_dir_all(parent).map_err(io_error)?;
                for dir in missing.iter().rev() {
                    if let Ok(rel) = dir.strip_prefix(original_root) {
                        let rel = rel.to_string_lossy().replace('\\', "/");
                        journal.line(json!({ "dir": rel }))?;
                        journal.dirs.push(rel);
                    }
                }
            }
            if target.exists() {
                // Already there with the same content (a rerun): nothing to copy.
                if fsx::hash_file(&target).map(|h| h.to_hex().to_string()).ok().as_deref() == Some(f.hash.as_str()) {
                    journal.line(json!({ "copied": f.rel, "created": false }))?;
                    journal.copied.push((f.rel.clone(), false));
                    continue;
                }
                return Err(ApiError::new("conflict", format!("W archiwum jest już inny plik {}.", f.rel)));
            }
            let hash = fsx::with_retry(|| fsx::copy_no_replace(&root.join(&f.rel), &target))
                .map_err(|e| ApiError::new("io", format!("Nie można skopiować pliku {}: {e}", f.rel)))?;
            if hash.to_hex().to_string() != f.hash || file_len(&target) != f.size {
                let _ = fsx::remove_own_file(&target);
                return Err(ApiError::new("io", format!("Plik {} zmienił się podczas kopiowania.", f.rel)));
            }
            journal.line(json!({ "copied": f.rel, "created": true }))?;
            journal.copied.push((f.rel.clone(), true));
            checkpoint("copy")?;
        }
        Ok(())
    })();
    progress.finish();
    result
}

/// Takes back the files a run copied (only where they are still what it copied and nothing points at them).
fn rollback(original_root: &Path, journal: &mut Journal, keep: Option<&HashSet<String>>) {
    let hashes: HashMap<&str, &str> = journal.header.files.iter().map(|f| (f.rel.as_str(), f.hash.as_str())).collect();
    for (rel, created) in journal.copied.iter().rev() {
        if !created || keep.is_some_and(|k| k.contains(&fsx::name_key(rel))) {
            continue;
        }
        let path = original_root.join(rel);
        if fsx::hash_file(&path).map(|h| h.to_hex().to_string()).ok().as_deref() == hashes.get(rel.as_str()).copied() {
            let _ = fsx::remove_own_file(&path);
        }
    }
    for dir in journal.dirs.iter().rev() {
        let _ = fs::remove_dir(original_root.join(dir));
    }
    let _ = journal.set_state("rolledBack");
}

/// Steps 6–7: the history and the editors go to the original (when it has `.heirloom`), the base becomes what was
/// written, and the run is done.
fn finish(s: &mut Session, journal: &mut Journal) -> Result<(), ApiError> {
    let root = s.archive.root().to_path_buf();
    let info = s.overlay.as_mut().ok_or_else(not_overlay)?;
    let original_root = info.original_root.clone();
    let mut manifest = read_manifest(&root).unwrap_or_else(|| info.manifest.clone());
    let overlay_history = history::read(&s.archive.history_path()).unwrap_or_default();
    if let (Some(path), None) = (original_history(&original_root), journal.history_bytes_after) {
        let new: Vec<Entry> = overlay_history.iter().skip(journal.header.synced_history_len).filter(|e| e.kind.as_deref() != Some(FROM_ARCHIVE)).cloned().collect();
        // A run interrupted after appending already has them (the size tells).
        let appended = journal.header.history_bytes_before.is_some_and(|b| file_len(&path) != b);
        if !new.is_empty() && !appended {
            let _ = history::append(&path, &new);
            if let Ok(mut original) = Archive::open(&info.original_data_path) {
                if super::has_own_settings(&original) {
                    let mut newest: HashMap<&str, &str> = HashMap::new();
                    for e in &new {
                        let ts = newest.entry(e.author.as_str()).or_insert(e.ts.as_str());
                        if e.ts.as_str() > *ts {
                            *ts = e.ts.as_str();
                        }
                    }
                    let settings = original.settings_mut();
                    for (author, ts) in newest {
                        match settings.editors.iter_mut().find(|e| e.name == author) {
                            Some(editor) => editor.last_edited = Some(ts.to_string()),
                            None => settings.editors.push(heirloom_core::archive::EditorEntry { name: author.to_string(), last_edited: Some(ts.to_string()) }),
                        }
                    }
                    let _ = original.save_settings();
                }
            }
        }
        let after = file_len(&path);
        journal.line(json!({ "historyBytesAfter": after }))?;
        journal.history_bytes_after = Some(after);
    }
    let written = fs::read(&info.original_data_path).map_err(io_error)?;
    write_atomically(&sidecar(&root).join(super::BASE), &written).map_err(io_error)?;
    manifest.base = super::Base { hash: hash_hex(&written), len: written.len() as u64, at: history::now() };
    manifest.synced.history_len = overlay_history.len();
    manifest.synced.last_write_back = Some(history::now());
    manifest.original.foreign_confirmed = true;
    write_manifest(&root, &manifest)?;
    info.reload_base(&root);
    info.interrupted = None;
    journal.set_state("done")?;
    prune(&root);
    Ok(())
}

/// Keeps the newest journals.
fn prune(root: &Path) {
    let list = journals(root);
    if list.len() > KEEP_JOURNALS {
        for old in &list[..list.len() - KEEP_JOURNALS] {
            let _ = fs::remove_file(old);
        }
    }
}

/// The last write-back can no longer be undone: the copies in „Twoje zmiany” of files the archive now has
/// identically go (the archive keeps them).
pub fn settle(root: &Path, original_root: &Path, journal: &mut Journal) {
    for (rel, _) in &journal.copied {
        let (mine, theirs) = (root.join(rel), original_root.join(rel));
        if mine.is_file() && theirs.is_file() && file_len(&mine) == file_len(&theirs) && fsx::hash_file(&mine).ok() == fsx::hash_file(&theirs).ok() {
            let _ = fsx::remove_own_file(&mine);
            // Folders it leaves empty (media\Kowalski Józef), never media itself.
            let mut dir = mine.parent().map(Path::to_path_buf);
            while let Some(d) = dir.filter(|d| d.starts_with(root.join(MEDIA_DIR)) && *d != root.join(MEDIA_DIR) || d.starts_with(root.join(AI_DIR)) && *d != root.join(AI_DIR)) {
                if fs::remove_dir(&d).is_err() {
                    break;
                }
                dir = d.parent().map(Path::to_path_buf);
            }
        }
    }
    let _ = journal.set_state("settled");
}

/// Settles the last write-back when someone has changed the archive since (it can't be undone any more).
pub fn settle_if_stale(root: &Path, info: &OverlayInfo) {
    let Some(mut journal) = last_journal(root).filter(Journal::open_for_undo) else { return };
    if info.original_hash().is_some_and(|h| Some(h) != journal.written) {
        settle(root, &info.original_root, &mut journal);
    }
}

/// Whether the last write-back can be undone now: done, not settled, and the archive still as it left it.
pub fn can_undo(root: &Path, info: &OverlayInfo) -> bool {
    last_journal(root).filter(Journal::open_for_undo).is_some_and(|j| j.written.is_some() && info.original_hash() == j.written && j.pre_copy.is_some())
}

/// „Cofnij” of the last write-back: the archive goes back to the bytes it had before, the files the run added go,
/// the history it appended goes; the user's changes are pending again (their files are still in „Twoje zmiany”).
pub fn undo(s: &mut Session) -> ApiResult {
    let root = s.archive.root().to_path_buf();
    let info = s.overlay.as_ref().ok_or_else(not_overlay)?;
    let Some(mut journal) = last_journal(&root).filter(Journal::open_for_undo) else {
        return Err(ApiError::new("cannot_undo", "Nie ma przeniesienia, które można cofnąć."));
    };
    if !can_undo(&root, info) {
        return Err(ApiError::new(
            "cannot_undo",
            "Tego przeniesienia nie da się już cofnąć: ktoś zmienił archiwum po nim. Wcześniejszą wersję masz w kopii zapasowej.",
        ));
    }
    let original_root = info.original_root.clone();
    let original_data = info.original_data_path.clone();
    let copy = sidecar(&root).join("kopie").join(journal.pre_copy.clone().unwrap_or_default());
    let before = fs::read(&copy).map_err(|e| ApiError::new("io", format!("Nie można odczytać kopii archiwum sprzed przeniesienia: {e}")))?;
    if hash_hex(&before) != journal.header.before_hash {
        return Err(ApiError::new("cannot_undo", "Kopia archiwum sprzed przeniesienia jest uszkodzona — przywróć je z kopii zapasowej."));
    }
    write_atomically(&original_data, &before).map_err(io_error)?;
    let keep = referenced(&before);
    let mut files = Journal { copied: journal.copied.clone(), dirs: journal.dirs.clone(), header: journal.header.clone(), path: journal.path.clone(), ..Journal::default() };
    rollback(&original_root, &mut files, Some(&keep));
    if let (Some(path), Some(before_len), Some(after_len)) = (original_history(&original_root), journal.header.history_bytes_before, journal.history_bytes_after) {
        if file_len(&path) == after_len {
            if let Ok(file) = fs::OpenOptions::new().write(true).open(&path) {
                let _ = file.set_len(before_len);
            }
        }
    }
    write_atomically(&sidecar(&root).join(super::BASE), &before).map_err(io_error)?;
    let mut manifest = read_manifest(&root).unwrap_or_else(|| info.manifest.clone());
    manifest.base = super::Base { hash: journal.header.before_hash.clone(), len: before.len() as u64, at: history::now() };
    manifest.synced.history_len = journal.header.synced_history_len;
    write_manifest(&root, &manifest)?;
    journal.set_state("undone")?;
    if let Some(info) = s.overlay.as_mut() {
        info.reload_base(&root);
    }
    Ok(Value::Null)
}

/// At open: a write-back the program didn't finish is finished (the archive already has the new data) or taken back
/// (it doesn't). Returns a message for the user.
pub fn recover(s: &mut Session, journal: Option<Journal>) -> Result<Option<String>, ApiError> {
    let root = s.archive.root().to_path_buf();
    let Some(mut journal) = journal.or_else(|| last_journal(&root)).filter(Journal::unfinished) else { return Ok(None) };
    let Some(info) = s.overlay.as_mut() else { return Ok(None) };
    if info.original_missing {
        return Ok(None);
    }
    let original_root = info.original_root.clone();
    let Some(now) = fs::read(&info.original_data_path).ok() else { return Ok(None) };
    let hash = hash_hex(&now);
    if journal.written.as_deref() == Some(hash.as_str()) || (journal.written.is_none() && Some(&hash) != Some(&journal.header.before_hash) && hash == hash_hex(&fs::read(s.archive.data_path()).unwrap_or_default())) {
        if journal.written.is_none() {
            journal.line(json!({ "written": hash }))?;
            journal.written = Some(hash);
        }
        finish(s, &mut journal)?;
        return Ok(Some("Przenoszenie zmian do archiwum zostało teraz dokończone.".into()));
    }
    let keep = referenced(&now);
    rollback(&original_root, &mut journal, Some(&keep));
    if hash == journal.header.before_hash {
        info.interrupted = Some(
            "Ostatnie przenoszenie zmian do archiwum nie zostało dokończone (np. odłączono pendrive). Archiwum jest takie jak przed nim, a Twoje zmiany są bezpieczne.".into(),
        );
    }
    Ok(None)
}

/// Applies `target` (a whole list of records) to the open archive as one change group; returns how many records changed.
pub fn apply_records(archive: &mut Archive, target: &[Node]) -> Result<usize, ApiError> {
    let current: HashMap<&str, &Node> = archive.doc.records.iter().filter_map(|r| Some((r.xref.as_deref()?, r))).collect();
    let wanted: HashSet<&str> = target.iter().filter_map(|r| r.xref.as_deref()).collect();
    let mut edits = Vec::new();
    if let (Some(head), Some(now)) = (target.iter().find(|r| r.tag == "HEAD"), archive.doc.head()) {
        if head != now {
            edits.push(Edit::Replace(head.clone()));
        }
    }
    for record in target {
        let Some(xref) = record.xref.as_deref() else { continue };
        match current.get(xref) {
            Some(now) if *now == record => {}
            Some(_) => edits.push(Edit::Replace(record.clone())),
            None => edits.push(Edit::Add(record.clone())),
        }
    }
    for xref in current.keys().filter(|x| !wanted.contains(*x)) {
        edits.push(Edit::Remove(xref.to_string()));
    }
    let n = edits.len();
    if n > 0 {
        archive.apply_all(edits)?;
    }
    Ok(n)
}

/// „Połącz” (A.9): the archive's changes since the last sync merged into „Twoje zmiany”. Without `choices`, a merge with
/// conflicts changes nothing and returns them; with them, it is applied and saved as one change („Zmiany z archiwum”),
/// and the archive as it is now becomes the base.
pub fn rebase(s: &mut Session, choices: Option<HashMap<String, Side>>) -> ApiResult {
    let root = s.archive.root().to_path_buf();
    if s.archive.has_unsaved_changes() {
        return Err(unsaved());
    }
    let info = s.overlay.as_ref().ok_or_else(not_overlay)?;
    if info.original_missing {
        return Err(ApiError::new("original_missing", "Archiwum jest teraz niedostępne."));
    }
    let original_root = info.original_root.clone();
    let theirs_bytes = fs::read(&info.original_data_path).map_err(io_error)?;
    let (mut theirs, _) = Document::from_bytes(&theirs_bytes);
    // One form for all three: GEDCOM 7 when either side is.
    if theirs.version == Version::V7 && s.archive.doc.version != Version::V7 {
        s.archive.upgrade_to_v7();
        s.changed();
    }
    let info = s.overlay.as_ref().ok_or_else(not_overlay)?;
    let version = s.archive.doc.version;
    if version == Version::V7 {
        theirs.upgrade_to_v7();
    }
    let base = info.base_as(version);
    let mine = s.archive.doc.records.clone();
    let fast_forward = heirloom_core::history::diff(&base, &mine, "", None).iter().all(|e| e.tag == "HEAD");
    let merged = if fast_forward {
        merge::Merge { records: theirs.records.clone(), ..merge::Merge::default() }
    } else {
        merge::merge3(&base, &mine, &theirs.records, choices.as_ref().unwrap_or(&HashMap::new())).map_err(|t| {
            ApiError::new(
                "too_different",
                format!("Archiwum wygląda teraz na zupełnie inne (wspólnych osób: {}%). Twoich zmian nie łączymy z nim automatycznie.", t.shared_percent),
            )
        })?
    };
    if choices.is_none() && !merged.conflicts.is_empty() {
        let mine_pending = info.pending(&s.archive);
        let d = s.derived();
        let conflicts: Vec<Value> = merged
            .conflicts
            .iter()
            .map(|c| {
                let name = c.mine.as_ref().or(c.theirs.as_ref()).and_then(|n| n.child_value("NAME")).map(|n| n.replace('/', "").trim().to_string());
                let name = name.or_else(|| d.index(&c.key).map(|i| d.info[i].name.clone())).unwrap_or_else(|| c.key.clone());
                let side = |n: &Option<Node>| n.as_ref().map(heirloom_core::history::record_text);
                json!({
                    "key": c.key,
                    "tag": c.tag,
                    "name": name,
                    "default": if c.default == Side::Mine { "mine" } else { "theirs" },
                    "mine": side(&c.mine),
                    "theirs": side(&c.theirs),
                    "base": side(&c.base),
                })
            })
            .collect();
        return Ok(json!({ "merged": false, "conflicts": conflicts, "theirChanges": their_changes(&base, &theirs.records), "myChanges": mine_pending }));
    }
    let mut records = merged.records;
    // A file of „Twoje zmiany” whose name the archive now uses for another file moves aside, so it hides nothing.
    let mut renames: HashMap<String, String> = HashMap::new();
    for f in new_files(&root, &original_root) {
        if !original_root.join(&f.rel).is_file() {
            continue;
        }
        let (folder, name) = f.rel.rsplit_once('/').unwrap_or(("", f.rel.as_str()));
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s, e),
            _ => (name, ""),
        };
        let taken = |key: &str| [&root, &original_root].iter().any(|layer| fs::read_dir(layer.join(folder)).into_iter().flatten().flatten().any(|e| fsx::name_key(&e.file_name().to_string_lossy()) == key));
        let to = match folder {
            "" => fsx::free_name(&taken, stem, ext),
            folder => format!("{folder}/{}", fsx::free_name(&taken, stem, ext)),
        };
        if fsx::move_no_replace(&root.join(&f.rel), &root.join(&to)).is_ok() {
            renames.insert(fsx::name_key(&f.rel), to);
        }
    }
    if !renames.is_empty() {
        let doc = Document { records, ..s.archive.doc.clone() };
        let edits = relink_files(&doc, &renames);
        records = doc.records;
        for edit in edits {
            if let Edit::Replace(node) = edit {
                if let Some(slot) = records.iter_mut().find(|r| r.xref == node.xref) {
                    *slot = node;
                }
            }
        }
    }
    // The user's own history lines follow a renumbered record (before the merge is saved: its lines are numbered right).
    if !merged.renumbered.is_empty() {
        renumber_history(&s.archive.history_path(), s.overlay.as_ref().map_or(0, |i| i.manifest.synced.history_len), &merged.renumbered);
    }
    let editors = s.archive.settings().editors.clone();
    let changed = apply_records(&mut s.archive, &records)?;
    s.changed();
    if changed > 0 {
        let note = format!("Zmiany z archiwum {}", original_root.display());
        s.archive.save(&SaveOptions { author: ARCHIVE_AUTHOR, batch: None, note: Some(&note), allow_foreign: true, kind: Some(FROM_ARCHIVE) })?;
        // „Archiwum” isn't someone who edits.
        s.archive.settings_mut().editors = editors;
        let _ = s.archive.save_settings();
        s.changed();
    }
    write_atomically(&sidecar(&root).join(super::BASE), &theirs_bytes).map_err(io_error)?;
    let mut manifest = read_manifest(&root).unwrap_or_default();
    manifest.base = super::Base { hash: hash_hex(&theirs_bytes), len: theirs_bytes.len() as u64, at: history::now() };
    write_manifest(&root, &manifest)?;
    if let Some(info) = s.overlay.as_mut() {
        info.reload_base(&root);
    }
    Ok(json!({ "merged": true, "fastForward": fast_forward, "changes": changed, "conflicts": [] }))
}

/// How many records the archive changed since the base.
fn their_changes(base: &[Node], theirs: &[Node]) -> usize {
    heirloom_core::history::diff(base, theirs, "", None).iter().filter(|e| e.tag != "HEAD").count()
}

/// The user's own history lines (after the synced part) follow a renumbered record, so „Cofnij” still finds it.
fn renumber_history(path: &Path, from: usize, map: &[(String, String)]) {
    let Ok(mut entries) = history::read(path) else { return };
    let lookup: HashMap<&str, &str> = map.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let fix = |text: &mut Option<String>| {
        if let Some(t) = text.as_mut() {
            let mut doc = Document::from_bytes(t.as_bytes()).0;
            for node in &mut doc.records {
                merge::rewrite_pointers(node, &lookup);
            }
            *t = doc.records.iter().map(heirloom_core::history::record_text).collect::<Vec<_>>().join("\n");
        }
    };
    for e in entries.iter_mut().skip(from) {
        if let Some(new) = lookup.get(e.record.as_str()) {
            e.record = new.to_string();
        }
        fix(&mut e.before);
        fix(&mut e.after);
    }
    let mut text = String::new();
    for e in &entries {
        if let Ok(line) = serde_json::to_string(e) {
            text.push_str(&line);
            text.push('\n');
        }
    }
    let _ = write_atomically(path, text.as_bytes());
}

/// „37 zmian (12 osób), 12 nowych plików (48 MB)”.
pub fn summary(changes: usize, people: usize, files: usize) -> String {
    let mut text = count_pl(changes, "zmiana", "zmiany", "zmian");
    if people > 0 {
        text.push_str(&format!(" ({})", count_pl(people, "osoba", "osoby", "osób")));
    }
    if files > 0 {
        text.push_str(&format!(", {}", count_pl(files, "nowy plik", "nowe pliki", "nowych plików")));
    }
    text
}
