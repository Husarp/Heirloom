//! The Settings screen's tools (spec §4.34): the space the archive takes, a backup (.zip), the GEDZIP export, the
//! archive check, the cache, the settings file and who edits. They only read the archive folder: they write where
//! the user chose and clear the app's own cache, never anything inside the archive.

use crate::derive::Derived;
use crate::gedwrite::file_uri;
use crate::media_edit::resolve;
use crate::{Api, ApiError, ApiResult, Session, str_arg};
use heirloom_core::archive::{MEDIA_DIR, SIDECAR_DIR};
use heirloom_core::gedcom::date::{Calendar, DateValue, Qualifier, Ymd};
use heirloom_core::gedcom::model::Sex;
use heirloom_core::gedcom::view::{Certainty, MediaKind, PersonDetails, media_kind_for};
use heirloom_core::gedcom::{Document, Node, Version};
use heirloom_core::polish;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf, Prefix};
use std::time::SystemTime;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// The researcher's AI files, kept as delivered (PLAN §11.2).
const AI_DIR: &str = "zrodla-ai";
/// This archive's thumbnails in the app's cache (as `Api::install` names the folder).
const THUMBS_DIR: &str = "miniatury";
const SETTINGS_FORMAT: &str = "heirloom-ustawienia";
/// Already compressed: stored in a zip as they are (deflating them again only costs time).
const COMPRESSED: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "avif", "mp4", "mov", "m4v", "mp3", "m4a", "ogg", "pdf", "zip", "7z",
    "rar", "gz", "docx", "xlsx", "pptx", "odt", "ods",
];

pub fn call(api: &mut Api, method: &str, args: &Value) -> ApiResult {
    match method {
        "archive.storage" => storage(api),
        "archive.backup" => backup(api.session()?, &target(args)?),
        "archive.exportGedzip" => export_gedzip(api.session()?, &target(args)?),
        "archive.check" => Ok(check(api.session()?)),
        "archive.rebuildCache" => rebuild_cache(api),
        "archive.editors" => Ok(editors(api.session()?)),
        "archive.saveAsNew" => save_as_new(api, args),
        "settings.export" => export_settings(api, &target(args)?),
        "settings.import" => import_settings(api, Path::new(&str_arg(args, "path")?)),
        "app.about" => Ok(json!({ "version": env!("CARGO_PKG_VERSION"), "buildDate": env!("HEIRLOOM_BUILD_DATE") })),
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

/// Where to write: a full path chosen in a save dialog.
fn target(args: &Value) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(str_arg(args, "path")?);
    if !path.is_absolute() || path.file_name().is_none() {
        return Err(ApiError::bad_args("path"));
    }
    Ok(path)
}

fn write_failed(e: impl Display) -> ApiError {
    ApiError::new("io", format!("Nie można zapisać pliku: {e}"))
}

// ---------- Storage ----------

/// "Zajęte miejsce": bytes per kind (photos and other files in `media/`, the AI files count as documents), the data
/// file's copies, this archive's cache, and the free space on its disk.
fn storage(api: &mut Api) -> ApiResult {
    let cache_root = api.cache_root.clone();
    let s = api.session()?;
    let root = s.archive.root();
    let (mut photos, mut documents) = (0, 0);
    for (path, len) in files_in(&root.join(MEDIA_DIR)) {
        if media_kind_for(None, None, path.to_str()) == MediaKind::Photo {
            photos += len;
        } else {
            documents += len;
        }
    }
    documents += total_size(&root.join(AI_DIR));
    let backups = total_size(&s.archive.backup_dir());
    let cache = cache_root.map_or(0, |c| total_size(&c.join(&s.archive.settings().archive_id)));
    let data = fs::metadata(s.archive.data_path()).map_or(0, |m| m.len());
    Ok(json!({
        "photos": photos,
        "documents": documents,
        "backups": backups,
        "cache": cache,
        "data": data,
        "total": photos + documents + backups + cache + data,
        "disk": { "name": disk_name(root), "free": fs4::available_space(root).ok() },
    }))
}

/// Every file under `dir` with its size, in path order (links are not followed).
fn files_in(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => stack.push(entry.path()),
                Ok(kind) if kind.is_file() => files.push((entry.path(), entry.metadata().map_or(0, |m| m.len()))),
                _ => {}
            }
        }
    }
    files.sort();
    files
}

fn total_size(dir: &Path) -> u64 {
    files_in(dir).iter().map(|(_, len)| len).sum()
}

/// "D:" for a drive, `\\serwer\udział` for a network folder.
fn disk_name(root: &Path) -> Option<String> {
    let absolute = fs::canonicalize(root).ok()?;
    let Some(Component::Prefix(prefix)) = absolute.components().next() else { return None };
    match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => Some(format!("{}:", char::from(letter))),
        Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
            Some(format!(r"\\{}\{}", server.to_string_lossy(), share.to_string_lossy()))
        }
        _ => None,
    }
}

/// When the newest copy in `.heirloom/kopie` was made (RFC 3339), for "Ostatnia: dziś 11:04".
pub(crate) fn last_backup(dir: &Path) -> Option<String> {
    let newest = fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        // A copy keeps the data file's modification time; its creation time is when the copy was made.
        .filter_map(|m| m.created().ok().max(m.modified().ok()))
        .max()?;
    time::OffsetDateTime::from(newest).format(&time::format_description::well_known::Rfc3339).ok()
}

// ---------- Backup and GEDZIP ----------

/// "Utwórz kopię (.zip)": the data file, `media/`, `zrodla-ai/` and Heirloom's own files from `.heirloom/` (not the
/// copies of the data file in `.heirloom/kopie`), at their places in the archive folder.
fn backup(s: &Session, target: &Path) -> ApiResult {
    let root = s.archive.root();
    outside_archive(root, target)?;
    let mut entries = vec![(s.archive.settings().data_file.clone(), s.archive.data_path())];
    for folder in [MEDIA_DIR, AI_DIR] {
        for (path, _) in files_in(&root.join(folder)) {
            if let Ok(relative) = path.strip_prefix(root) {
                entries.push((zip_name(relative), path));
            }
        }
    }
    if let Ok(list) = fs::read_dir(s.archive.sidecar_path()) {
        for entry in list.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Files only (`kopie/` is a folder); a `.tmp-` file is a write in progress.
            if entry.file_type().is_ok_and(|t| t.is_file()) && !name.contains(".tmp-") {
                entries.push((format!("{SIDECAR_DIR}/{name}"), entry.path()));
            }
        }
    }
    let bytes = write_zip(target, None, &entries)?;
    Ok(json!({ "path": target.display().to_string(), "bytes": bytes, "files": entries.len() }))
}

/// GEDZIP (GEDCOM 7): `gedcom.ged` at the root, and every local file it points to at the path its FILE value names
/// (percent-encoded, as `file_uri` writes it). Files kept elsewhere on the disk are packed into `media/`, and their
/// FILE values changed in the exported copy only. The saved file is exported, not unsaved changes.
fn export_gedzip(s: &Session, target: &Path) -> ApiResult {
    let root = s.archive.root();
    outside_archive(root, target)?;
    let saved = fs::read(s.archive.data_path()).map_err(|e| ApiError::new("io", format!("Nie można odczytać pliku danych: {e}")))?;
    let (mut doc, _) = Document::from_bytes(&saved);
    let written_as = doc.version;
    doc.upgrade_to_v7();
    doc.bom = true;
    // Paths inside the archive keep their names; files from elsewhere get free names in media/.
    let mut taken = HashSet::from(["gedcom.ged".to_string()]);
    for_each_file(&mut doc.records, false, &mut |file| {
        if let Some(name) = file.value.as_deref().and_then(|v| inside_name(&decode_file(v, written_as))) {
            taken.insert(name.to_lowercase());
        }
    });
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    let mut packed: HashMap<PathBuf, String> = HashMap::new();
    let mut missing: Vec<String> = Vec::new();
    for_each_file(&mut doc.records, false, &mut |file| {
        let Some(path) = file.value.as_deref().map(|v| decode_file(v, written_as)) else { return };
        if path.is_empty() || is_url(&path) {
            return;
        }
        let source = resolve(root, &path);
        if !source.is_file() {
            if !missing.contains(&path) {
                missing.push(path);
            }
            return;
        }
        let name = packed.entry(source.clone()).or_insert_with(|| {
            let name = inside_name(&path).unwrap_or_else(|| free_name(&mut taken, &source));
            entries.push((name.clone(), source));
            name
        });
        file.value = Some(file_uri(name));
    });
    let bytes = write_zip(target, Some(&doc.to_bytes()), &entries)?;
    Ok(json!({ "path": target.display().to_string(), "bytes": bytes, "files": entries.len(), "missing": missing }))
}

/// „Zapisz jako nowe archiwum” (design 17b, the first save into another program's file): what is open now, unsaved
/// changes included, becomes a new GEDCOM 7 archive in an empty folder, with the files it points to copied in (files
/// from elsewhere on the disk into `media/`, as the GEDZIP export packs them). The other program's file is never
/// written. Files are copied first and a link is changed only for a file that arrived; a file that couldn't be
/// copied keeps pointing at where it is. The new archive is opened in its place.
fn save_as_new(api: &mut Api, args: &Value) -> ApiResult {
    let folder = PathBuf::from(str_arg(args, "folder")?);
    if !folder.is_absolute() {
        return Err(ApiError::bad_args("folder"));
    }
    let s = api.session()?;
    let root = s.archive.root().to_path_buf();
    let same = match (fs::canonicalize(&folder), fs::canonicalize(&root)) {
        (Ok(f), Ok(r)) => f == r,
        _ => folder == root,
    };
    if same {
        return Err(ApiError::new("inside_archive", "Wybierz inny folder niż ten, w którym jest obecny plik."));
    }
    if fs::read_dir(&folder).is_ok_and(|mut d| d.next().is_some()) {
        return Err(ApiError::new("not_empty", "Wybierz pusty folder — nowe archiwum powstanie w nim."));
    }
    // The name of the chosen folder, unless one is given („Rodzina Kowalskich”, not the old folder's „Pobrane”).
    let name = args
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .or_else(|| folder.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| s.archive.settings().name.clone());
    let mut doc = s.archive.doc.clone();
    let written_as = doc.version;
    doc.upgrade_to_v7();
    doc.bom = true;
    // Written by Heirloom from now on: saving into it asks nothing.
    if let Some(head) = doc.head_mut() {
        head.children.retain(|c| c.tag != "SOUR");
        let at = head.children.iter().position(|c| c.tag == "GEDC").map_or(0, |p| p + 1);
        let source = Node::with_value("SOUR", heirloom_core::PRODUCT_ID)
            .with_child(Node::with_value("NAME", "Heirloom"))
            .with_child(Node::with_value("VERS", env!("CARGO_PKG_VERSION")));
        head.children.insert(at, source);
    }
    // Where each file goes. The archive's own names (the data file, `.heirloom/`) are never a file's place.
    let own = |name: &str| name.eq_ignore_ascii_case(heirloom_core::archive::DEFAULT_DATA_FILE) || name.to_lowercase().starts_with(&format!("{SIDECAR_DIR}/"));
    let place = |path: &str| inside_name(path).filter(|n| !own(n));
    let mut taken = HashSet::from([heirloom_core::archive::DEFAULT_DATA_FILE.to_lowercase()]);
    for_each_file(&mut doc.records, false, &mut |file| {
        if let Some(name) = file.value.as_deref().and_then(|v| place(&decode_file(v, written_as))) {
            taken.insert(name.to_lowercase());
        }
    });
    let mut plan: HashMap<PathBuf, String> = HashMap::new();
    let mut missing = 0usize;
    for_each_file(&mut doc.records, false, &mut |file| {
        let Some(path) = file.value.as_deref().map(|v| decode_file(v, written_as)) else { return };
        if path.is_empty() || is_url(&path) {
            return;
        }
        let source = resolve(&root, &path);
        if !source.is_file() {
            missing += 1;
            return;
        }
        plan.entry(source.clone()).or_insert_with(|| place(&path).unwrap_or_else(|| free_name(&mut taken, &source)));
    });
    // Enough room for the copies (the data file is small next to them).
    let needed: u64 = plan.keys().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();
    fs::create_dir_all(&folder).map_err(write_failed)?;
    if let Ok(free) = fs4::available_space(&folder) {
        if needed > free {
            return Err(ApiError::new(
                "no_space",
                format!("Za mało miejsca na dysku: pliki zajmą {} MB, a wolne jest {} MB.", needed.div_ceil(1_000_000), free / 1_000_000),
            ));
        }
    }
    let mut copied: HashMap<PathBuf, String> = HashMap::new();
    let mut failed = 0usize;
    for (source, name) in &plan {
        let target = folder.join(name.replace('/', std::path::MAIN_SEPARATOR_STR));
        let done = target.parent().is_none_or(|p| fs::create_dir_all(p).is_ok()) && fs::copy(source, &target).is_ok();
        if done {
            copied.insert(source.clone(), name.clone());
        } else {
            failed += 1;
        }
    }
    // Links now: to the copy, or (the copy failed) to the file where it is.
    for_each_file(&mut doc.records, false, &mut |file| {
        let Some(path) = file.value.as_deref().map(|v| decode_file(v, written_as)) else { return };
        if path.is_empty() || is_url(&path) {
            return;
        }
        let source = resolve(&root, &path);
        if let Some(name) = copied.get(&source) {
            file.value = Some(file_uri(name));
        } else if plan.contains_key(&source) {
            file.value = Some(format!("file:///{}", file_uri(&source.display().to_string())));
        }
    });
    let archive = heirloom_core::Archive::create_from_document(&folder, &name, doc)?;
    // The same family goes on in the new folder (same records, same ids), and so does an import in progress.
    let import = api.import.take();
    let status = api.install(archive, &folder.display().to_string())?;
    api.import = import;
    Ok(json!({ "status": status, "copied": copied.len(), "failed": failed, "missing": missing }))
}

/// Backups and exports are written outside the archive folder, so nothing in it is ever replaced (and a backup never
/// ends up inside what it protects).
fn outside_archive(root: &Path, target: &Path) -> Result<(), ApiError> {
    let folder = target
        .parent()
        .and_then(|p| fs::canonicalize(p).ok())
        .ok_or_else(|| ApiError::new("not_found", "Nie ma folderu, w którym ma powstać plik."))?;
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if folder.starts_with(&root) {
        return Err(ApiError::new("inside_archive", "Wybierz miejsce poza folderem archiwum, np. Dokumenty albo pendrive."));
    }
    Ok(())
}

/// A path inside a zip: `media/Józef 1904.jpg`.
fn zip_name(relative: &Path) -> String {
    relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

/// Calls `f` with every FILE of a multimedia object: `0 @O1@ OBJE` records and objects written inside other records
/// (GEDCOM 5.5.1). The header's FILE (in 5.5.1 the file's own name) is not a media file.
fn for_each_file(nodes: &mut [Node], in_object: bool, f: &mut dyn FnMut(&mut Node)) {
    for node in nodes {
        if in_object && node.tag == "FILE" {
            f(node);
        } else {
            let object = node.tag == "OBJE" && node.pointer().is_none();
            for_each_file(&mut node.children, object, f);
        }
    }
}

/// A FILE value as a path, the way the app reads it (heirloom-core `view.rs`): GEDCOM 7 values are URIs.
fn decode_file(value: &str, version: Version) -> String {
    let value = value.trim();
    let value = value.strip_prefix("file:///").or_else(|| value.strip_prefix("file://")).unwrap_or(value);
    if version == Version::V7 || value.contains("%20") { percent_decode(value) } else { value.to_string() }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match hex {
            Some(b) => {
                out.push(b);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// `https://…`: a file on the web, not on the disk (a drive letter is one character before the colon).
fn is_url(path: &str) -> bool {
    path.split_once(':').is_some_and(|(scheme, _)| scheme.len() > 1 && scheme.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)))
}

/// `media/Józef 1904.jpg` for a path written relative to the archive folder; None for absolute paths and paths that
/// climb out of it (they are packed under a new name).
fn inside_name(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    if normalized.starts_with('/') || Path::new(path).is_absolute() {
        return None;
    }
    let parts: Vec<&str> = normalized.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    if parts.is_empty() || parts.iter().any(|p| *p == ".." || p.contains(':')) {
        return None;
    }
    let name = parts.join("/");
    (!name.eq_ignore_ascii_case("gedcom.ged")).then_some(name)
}

/// `media/<file name>`, or `media/<name> (2).<ext>` when that name is taken.
fn free_name(taken: &mut HashSet<String>, source: &Path) -> String {
    let file = source.file_name().map_or_else(|| "plik".to_string(), |n| n.to_string_lossy().into_owned());
    let (stem, ext) = match file.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (file.clone(), String::new()),
    };
    let mut name = format!("{MEDIA_DIR}/{file}");
    let mut n = 2;
    while !taken.insert(name.to_lowercase()) {
        name = format!("{MEDIA_DIR}/{stem} ({n}){ext}");
        n += 1;
    }
    name
}

/// Writes the zip next to `target` under a temporary name and moves it into place when complete, so a failed run
/// never leaves a half-written file that looks like a backup. Returns the zip's size.
fn write_zip(target: &Path, gedcom: Option<&[u8]>, entries: &[(String, PathBuf)]) -> Result<u64, ApiError> {
    let name = target.file_name().map_or_else(|| "kopia.zip".to_string(), |n| n.to_string_lossy().into_owned());
    let temp = target.with_file_name(format!("{name}.tmp-{}", std::process::id()));
    let written = fill_zip(&temp, gedcom, entries).and_then(|()| fs::rename(&temp, target).map_err(write_failed));
    if let Err(e) = written {
        let _ = fs::remove_file(&temp);
        return Err(e);
    }
    Ok(fs::metadata(target).map_or(0, |m| m.len()))
}

fn fill_zip(path: &Path, gedcom: Option<&[u8]>, entries: &[(String, PathBuf)]) -> Result<(), ApiError> {
    let mut zip = ZipWriter::new(std::io::BufWriter::new(fs::File::create(path).map_err(write_failed)?));
    if let Some(bytes) = gedcom {
        zip.start_file("gedcom.ged", entry_options("gedcom.ged", bytes.len() as u64, SystemTime::now())).map_err(write_failed)?;
        zip.write_all(bytes).map_err(write_failed)?;
    }
    for (name, source) in entries {
        let unreadable = |e: std::io::Error| ApiError::new("io", format!("Nie można odczytać pliku {name}: {e}"));
        let mut file = fs::File::open(source).map_err(unreadable)?;
        let meta = file.metadata().map_err(unreadable)?;
        let modified = meta.modified().unwrap_or_else(|_| SystemTime::now());
        zip.start_file(name.as_str(), entry_options(name, meta.len(), modified)).map_err(write_failed)?;
        std::io::copy(&mut file, &mut zip).map_err(|e| ApiError::new("io", format!("Nie można skopiować pliku {name}: {e}")))?;
    }
    let file = zip.finish().map_err(write_failed)?.into_inner().map_err(write_failed)?;
    file.sync_all().map_err(write_failed)
}

fn entry_options(name: &str, size: u64, modified: SystemTime) -> SimpleFileOptions {
    let extension = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let method = if COMPRESSED.contains(&extension.as_str()) { CompressionMethod::Stored } else { CompressionMethod::Deflated };
    SimpleFileOptions::default().compression_method(method).last_modified_time(zip_time(modified)).large_file(size >= 0xF000_0000)
}

/// Zip entries carry local time (the format has no time zone).
fn zip_time(t: SystemTime) -> zip::DateTime {
    let utc = time::OffsetDateTime::from(t);
    let local = time::UtcOffset::local_offset_at(utc).map_or(utc, |offset| utc.to_offset(offset));
    u16::try_from(local.year())
        .ok()
        .and_then(|year| {
            zip::DateTime::from_date_and_time(year, u8::from(local.month()), local.day(), local.hour(), local.minute(), local.second()).ok()
        })
        .unwrap_or_default()
}

// ---------- Check ----------

fn problem(d: &Derived, kind: &str, message: String, person: Option<usize>) -> Value {
    json!({ "kind": kind, "message": message, "id": person.map(|i| d.xref(i)), "name": person.map(|i| &d.info[i].name) })
}

/// "Sprawdź archiwum": what the data itself shows is wrong, grouped by kind (in this order): links to records that
/// aren't in the file, missing files, people without a name, children born before a parent or after a parent died,
/// someone who is their own ancestor, families without anyone. Nothing is changed.
fn check(s: &mut Session) -> Value {
    let root = s.archive.root().to_path_buf();
    s.ensure();
    let d = s.derived.as_ref().expect("ensured");
    let doc = &s.archive.doc;
    let mut problems = Vec::new();

    let xrefs: HashSet<&str> = doc.records.iter().filter_map(|r| r.xref.as_deref()).collect();
    for record in &doc.records {
        let mut links = Vec::new();
        pointers(&record.children, &mut links);
        for (tag, target) in links.into_iter().filter(|(_, t)| *t != "@VOID@" && !xrefs.contains(t)) {
            let (subject, person) = link_owner(d, record);
            let text = format!("wskazuje na rekord {target} ({tag}), którego nie ma w pliku.");
            let message = match subject {
                Some(subject) => format!("{subject} {text}"),
                None => format!("W{}", &text[1..]),
            };
            problems.push(problem(d, "brokenLink", message, person));
        }
    }

    let mut owner_of: HashMap<&str, usize> = HashMap::new();
    for (i, p) in d.view.people.iter().enumerate() {
        for link in &p.media {
            owner_of.entry(link.object.as_str()).or_insert(i);
        }
    }
    for id in crate::lists::missing_files(d, &root) {
        let file = d.view.media.get(&id).and_then(|m| m.file.clone()).unwrap_or_default();
        problems.push(problem(d, "missingFile", format!("Nie ma pliku {file}."), owner_of.get(id.as_str()).copied()));
    }

    for (i, p) in d.view.model.persons.iter().enumerate() {
        if p.display_name().is_empty() {
            problems.push(problem(d, "noName", "Osoba bez imienia i nazwiska.".into(), Some(i)));
        }
    }

    for family in &d.view.model.families {
        let parents: Vec<usize> = family.partners.iter().filter_map(|x| d.index(x)).collect();
        for child in family.children.iter().filter_map(|x| d.index(x)) {
            // Birth parents only: an adoptive or foster family is another story.
            if d.view.people[child].pedigree.iter().any(|(f, _)| *f == family.xref) {
                continue;
            }
            let Some(born) = known_date(&d.view.people[child], "BIRT") else { continue };
            for &parent in &parents {
                let name = &d.info[parent].name;
                if let Some(parent_born) = known_date(&d.view.people[parent], "BIRT")
                    && compare(born.start, parent_born.start) == Ordering::Less
                {
                    let (a, b) = (polish::format_date_short(&born), polish::format_date_short(&parent_born));
                    problems.push(problem(d, "dates", format!("Urodzenie ({a}) przed urodzeniem rodzica: {name} ({b})."), Some(child)));
                }
                if let Some(died) = known_date(&d.view.people[parent], "DEAT") {
                    // A father may die before his child is born, up to about nine months.
                    let grace = if d.view.model.persons[parent].sex == Sex::Female { 0 } else { 1 };
                    let late = match (born.start.month, died.start.month) {
                        (Some(_), Some(_)) => months(died.start, born.start) > grace * 10,
                        _ => born.start.year - died.start.year > grace,
                    };
                    if late {
                        let (a, b) = (polish::format_date_short(&born), polish::format_date_short(&died));
                        problems.push(problem(d, "dates", format!("Urodzenie ({a}) po śmierci rodzica: {name} (zm. {b})."), Some(child)));
                    }
                }
            }
        }
    }

    // Following the parents leads back to the same person.
    let n = d.info.len();
    let mut state = vec![0u8; n]; // 0 not seen, 1 on the current path, 2 done
    let mut in_loop = Vec::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        let mut path = vec![(start, 0usize)];
        while let Some(&(i, next)) = path.last() {
            match d.info[i].parents.get(next) {
                Some(&parent) => {
                    if let Some(last) = path.last_mut() {
                        last.1 += 1;
                    }
                    match state[parent] {
                        0 => {
                            state[parent] = 1;
                            path.push((parent, 0));
                        }
                        1 if !in_loop.contains(&parent) => in_loop.push(parent),
                        _ => {}
                    }
                }
                None => {
                    state[i] = 2;
                    path.pop();
                }
            }
        }
    }
    for i in in_loop {
        problems.push(problem(d, "ownAncestor", "Jest swoim własnym przodkiem: rodzice i dzieci tworzą pętlę.".into(), Some(i)));
    }

    for family in &d.view.model.families {
        if family.partners.iter().chain(&family.children).all(|x| d.index(x).is_none()) {
            problems.push(problem(d, "emptyFamily", format!("Rodzina {} nie ma żadnych osób.", family.xref), None));
        }
    }
    Value::Array(problems)
}

/// Every pointer (`@I1@`) under these nodes, with its tag, in file order.
fn pointers<'a>(nodes: &'a [Node], out: &mut Vec<(&'a str, &'a str)>) {
    for node in nodes {
        if let Some(target) = node.pointer() {
            out.push((&node.tag, target));
        }
        pointers(&node.children, out);
    }
}

/// Who a broken link belongs to: a person (shown as the link), or a family (with a member to open), or another record.
fn link_owner(d: &Derived, record: &Node) -> (Option<String>, Option<usize>) {
    let xref = record.xref.as_deref().unwrap_or("");
    match record.tag.as_str() {
        "INDI" => (None, d.index(xref)),
        "FAM" => {
            let member = d.view.family(xref).and_then(|(f, _)| f.partners.iter().chain(&f.children).find_map(|x| d.index(x)));
            (Some(format!("Rodzina {xref}")), member)
        }
        "HEAD" => (Some("Nagłówek pliku".into()), None),
        tag => {
            let word = match tag {
                "SOUR" => "Źródło",
                "OBJE" => "Plik",
                "SNOTE" | "NOTE" => "Notatka",
                "REPO" => "Archiwum",
                "SUBM" => "Autor",
                _ => "Rekord",
            };
            (Some(format!("{word} {xref}")), None)
        }
    }
}

/// A date good enough to compare: exact (a year alone is fine), Gregorian or Julian, not marked uncertain.
fn known_date(details: &PersonDetails, tag: &str) -> Option<DateValue> {
    let fact = details.facts.iter().find(|f| f.tag == tag && f.date.is_some())?;
    let date = fact.date?;
    let sure = !matches!(fact.certainty, Some(Certainty::Low | Certainty::Medium));
    (sure && date.qualifier == Qualifier::Exact && matches!(date.calendar, Calendar::Gregorian | Calendar::Julian)).then_some(date)
}

/// Compares two dates at the precision both have: "1878" is neither before nor after "12 MAR 1878".
fn compare(a: Ymd, b: Ymd) -> Ordering {
    let month = a.month.is_some() && b.month.is_some();
    let day = month && a.day.is_some() && b.day.is_some();
    let key = |d: Ymd| (d.year, if month { d.month } else { None }, if day { d.day } else { None });
    key(a).cmp(&key(b))
}

/// Whole months from `a` to `b`.
fn months(a: Ymd, b: Ymd) -> i32 {
    (b.year - a.year) * 12 + i32::from(b.month.unwrap_or(1)) - i32::from(a.month.unwrap_or(1))
}

// ---------- Cache ----------

/// "Odbuduj dane podręczne": empties this archive's thumbnail folder in the app's cache (never anything in the
/// archive folder) and works out the view of the data again. Thumbnails are made again when they are shown.
fn rebuild_cache(api: &mut Api) -> ApiResult {
    let cache_root = api.cache_root.clone();
    let s = api.session()?;
    let (mut files, mut bytes) = (0u64, 0u64);
    let thumbs = cache_root.map(|c| c.join(&s.archive.settings().archive_id).join(THUMBS_DIR));
    if let Some(dir) = thumbs.filter(|dir| !is_inside(dir, s.archive.root())) {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let len = entry.metadata().map_or(0, |m| m.len());
            if entry.file_type().is_ok_and(|t| t.is_file()) && fs::remove_file(entry.path()).is_ok() {
                files += 1;
                bytes += len;
            }
        }
    }
    s.changed();
    s.ensure();
    Ok(json!({ "files": files, "bytes": bytes }))
}

fn is_inside(path: &Path, folder: &Path) -> bool {
    match (fs::canonicalize(path), fs::canonicalize(folder)) {
        (Ok(path), Ok(folder)) => path.starts_with(folder),
        _ => false,
    }
}

// ---------- Settings file ----------

/// "Eksportuj do pliku": this archive's display choices and this computer's appearance, as one small JSON file.
fn export_settings(api: &mut Api, target: &Path) -> ApiResult {
    let s = api.session()?;
    let (root, display) = (s.archive.root().to_path_buf(), s.archive.settings().display.clone());
    outside_archive(&root, target)?;
    let file = json!({ "format": SETTINGS_FORMAT, "version": 1, "display": display, "appearance": api.config.appearance });
    let text = serde_json::to_string_pretty(&file).map_err(write_failed)?;
    fs::write(target, text).map_err(write_failed)?;
    Ok(json!({ "path": target.display().to_string() }))
}

/// "Wczytaj z pliku…": applies the file's appearance and merges its display choices into this archive's settings
/// (keys the file doesn't have stay). Returns the new app state.
fn import_settings(api: &mut Api, source: &Path) -> ApiResult {
    let bytes = fs::read(source).map_err(|e| ApiError::new("io", format!("Nie można odczytać pliku: {e}")))?;
    let file: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if file.get("format").and_then(Value::as_str) != Some(SETTINGS_FORMAT) {
        return Err(ApiError::new("bad_file", "To nie jest plik ustawień Heirlooma."));
    }
    if let Some(appearance) = file.get("appearance").filter(|a| a.is_object()) {
        api.set_appearance(appearance)?;
    }
    if let Some(display) = file.get("display").and_then(Value::as_object) {
        let s = api.session()?;
        let settings = s.archive.settings_mut();
        for (key, value) in display {
            settings.display.insert(key.clone(), value.clone());
        }
        s.archive.save_settings()?;
    }
    Ok(api.app_state())
}

// ---------- Editors ----------

/// "Osoby edytujące": the names from the settings, with how many changes each saved and when they last did (from
/// the change history, which keeps the names as they were).
fn editors(s: &Session) -> Value {
    let history = s.history();
    let mut stats: HashMap<&str, (usize, &str)> = HashMap::new();
    // The header's entries are bookkeeping (the extension tags declared with the first edit), not changes.
    for entry in history.iter().filter(|e| e.tag != "HEAD") {
        let (count, last) = stats.entry(entry.author.as_str()).or_insert((0, ""));
        *count += 1;
        if entry.ts.as_str() > *last {
            *last = entry.ts.as_str();
        }
    }
    let list = s
        .archive
        .settings()
        .editors
        .iter()
        .map(|e| {
            let (changes, last) = stats.get(e.name.as_str()).copied().unwrap_or((0, ""));
            let last_edited = [e.last_edited.as_deref(), Some(last).filter(|l| !l.is_empty())].into_iter().flatten().max();
            json!({ "name": e.name, "lastEdited": last_edited, "changes": changes })
        })
        .collect();
    Value::Array(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::Edit;
    use std::io::Read;

    /// A generated family opened in an `Api` whose cache is in `dir/cache`.
    fn open(dir: &Path, media: usize) -> (Api, PathBuf) {
        let root = dir.join("Rodzina");
        let options = heirloom_gen::Options { people: 60, media, seed: 11, name: "Rodzina Kowalskich".into() };
        heirloom_gen::write_archive(&root, &options).unwrap();
        let mut api = Api::new(None, Some(dir.join("cache")));
        api.call("archive.open", json!({ "path": root.to_str().unwrap() })).unwrap();
        (api, root)
    }

    fn add_and_save(api: &mut Api, xref: &str, author: &str) {
        let person = Node::new("INDI").with_xref(xref).with_child(Node::with_value("NAME", "Jan /Nowak/"));
        api.session.as_mut().unwrap().archive.apply(Edit::Add(person)).unwrap();
        api.call("archive.save", json!({ "author": author })).unwrap();
    }

    /// Every file under `root` with its contents, to show that a tool changed nothing.
    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        files_in(root).into_iter().map(|(p, _)| (p.clone(), fs::read(p).unwrap())).collect()
    }

    fn zip_names(path: &Path) -> Vec<String> {
        let zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut names: Vec<String> = zip.file_names().map(str::to_string).collect();
        names.sort();
        names
    }

    fn zip_file(path: &Path, name: &str) -> (CompressionMethod, Vec<u8>) {
        let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut entry = zip.by_name(name).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        (entry.compression(), bytes)
    }

    fn path_arg(path: &Path) -> Value {
        json!({ "path": path.to_str().unwrap() })
    }

    #[test]
    fn storage_counts_each_kind_of_file() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, root) = open(dir.path(), 3);
        fs::write(root.join("media").join("akt.pdf"), vec![0u8; 1000]).unwrap();
        fs::create_dir_all(root.join("zrodla-ai").join("paczka")).unwrap();
        fs::write(root.join("zrodla-ai").join("paczka").join("odpowiedz.txt"), vec![b'a'; 500]).unwrap();
        let id = api.call("archive.status", Value::Null).unwrap()["archiveId"].as_str().unwrap().to_string();
        let thumbs = dir.path().join("cache").join(id).join("miniatury");
        fs::create_dir_all(&thumbs).unwrap();
        fs::write(thumbs.join("abc-128.jpg"), vec![0u8; 300]).unwrap();
        add_and_save(&mut api, "@I9001@", "Ewa");

        let s = api.call("archive.storage", Value::Null).unwrap();
        let photos: u64 = ["M0001.png", "M0002.png", "M0003.png"].iter().map(|f| fs::metadata(root.join("media").join(f)).unwrap().len()).sum();
        assert_eq!(s["photos"], photos);
        assert_eq!(s["documents"], 1500, "the PDF and the AI answer");
        assert_eq!(s["cache"], 300);
        assert!(s["backups"].as_u64().unwrap() > 0, "the copy made before the save");
        assert_eq!(s["data"], fs::metadata(root.join("rodzina.ged")).unwrap().len());
        let parts: u64 = ["photos", "documents", "backups", "cache", "data"].iter().map(|k| s[k].as_u64().unwrap()).sum();
        assert_eq!(s["total"], parts);
        assert!(s["disk"]["free"].as_u64().is_some_and(|free| free > 0));
        if cfg!(windows) {
            assert!(s["disk"]["name"].as_str().unwrap().ends_with(':'), "{}", s["disk"]);
        }
    }

    #[test]
    fn a_backup_holds_the_archive_but_not_the_data_file_copies() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, root) = open(dir.path(), 2);
        add_and_save(&mut api, "@I9001@", "Ewa");
        fs::create_dir_all(root.join("zrodla-ai")).unwrap();
        fs::write(root.join("zrodla-ai").join("odpowiedź 1.txt"), "tekst").unwrap();
        fs::write(root.join("notatki.txt"), "nie należy do archiwum").unwrap();
        let before = snapshot(&root);

        let target = dir.path().join("Kopia.zip");
        let result = api.call("archive.backup", path_arg(&target)).unwrap();
        assert_eq!(snapshot(&root), before, "nothing in the archive changes");
        let names = zip_names(&target);
        for name in ["rodzina.ged", "media/M0001.png", "media/M0002.png", "zrodla-ai/odpowiedź 1.txt", ".heirloom/ustawienia.json", ".heirloom/historia.jsonl"] {
            assert!(names.contains(&name.to_string()), "{name} missing from {names:?}");
        }
        assert!(!names.iter().any(|n| n.contains("kopie") || n == "notatki.txt"), "{names:?}");
        assert_eq!(result["files"], names.len());
        assert_eq!(result["bytes"], fs::metadata(&target).unwrap().len());
        let (method, bytes) = zip_file(&target, "rodzina.ged");
        assert_eq!((method, bytes), (CompressionMethod::Deflated, fs::read(root.join("rodzina.ged")).unwrap()));
        assert_eq!(zip_file(&target, "media/M0001.png").0, CompressionMethod::Stored, "photos are stored as they are");
        let leftovers = fs::read_dir(dir.path()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().contains(".tmp-")).count();
        assert_eq!(leftovers, 0);

        let inside = root.join("media").join("kopia.zip");
        assert_eq!(api.call("archive.backup", path_arg(&inside)).unwrap_err().code, "inside_archive");
        assert!(!inside.exists());
        assert_eq!(api.call("archive.backup", json!({ "path": "kopia.zip" })).unwrap_err().code, "bad_args");
    }

    #[test]
    fn gedzip_holds_gedcom_ged_and_the_files_it_points_to() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        fs::create_dir_all(root.join("media")).unwrap();
        fs::write(root.join("media").join("Józef 1904.jpg"), b"w archiwum").unwrap();
        let elsewhere = dir.path().join("zdjęcia").join("Józef 1904.jpg");
        fs::create_dir_all(elsewhere.parent().unwrap()).unwrap();
        fs::write(&elsewhere, b"gdzie indziej").unwrap();
        let text = format!(
            "0 HEAD\n1 GEDC\n2 VERS 7.0\n1 SOUR HEIRLOOM\n0 @I1@ INDI\n1 NAME Józef /Kowalski/\n1 OBJE @O1@\n\
0 @O1@ OBJE\n1 FILE media/J%C3%B3zef%201904.jpg\n2 FORM image/jpeg\n\
0 @O2@ OBJE\n1 FILE {}\n2 FORM image/jpeg\n\
0 @O3@ OBJE\n1 FILE media/brak.jpg\n2 FORM image/jpeg\n\
0 @O4@ OBJE\n1 FILE https://example.org/x.jpg\n2 FORM image/jpeg\n\
0 @O5@ OBJE\n1 FILE media/J%C3%B3zef%201904.jpg\n2 FORM image/jpeg\n0 TRLR\n",
            file_uri(elsewhere.to_str().unwrap())
        );
        fs::write(root.join("rodzina.ged"), &text).unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", path_arg(&root)).unwrap();
        let before = snapshot(&root);

        let target = dir.path().join("Kowalscy.gdz");
        let result = api.call("archive.exportGedzip", path_arg(&target)).unwrap();
        assert_eq!(snapshot(&root), before, "nothing in the archive changes");
        assert_eq!(result["missing"], json!(["media/brak.jpg"]));
        assert_eq!(result["files"], 2, "each file once, even when two objects point to it");
        assert_eq!(zip_names(&target), ["gedcom.ged", "media/Józef 1904 (2).jpg", "media/Józef 1904.jpg"]);
        assert_eq!(zip_file(&target, "media/Józef 1904.jpg").1, b"w archiwum");
        assert_eq!(zip_file(&target, "media/Józef 1904 (2).jpg").1, b"gdzie indziej");
        let (doc, _) = Document::from_bytes(&zip_file(&target, "gedcom.ged").1);
        assert_eq!(doc.version, Version::V7);
        let file_of = |xref: &str| doc.record(xref).unwrap().child_value("FILE").unwrap().to_string();
        assert_eq!(file_of("@O1@"), "media/Józef%201904.jpg");
        assert_eq!(file_of("@O2@"), "media/Józef%201904%20(2).jpg", "a file from elsewhere is packed and pointed to");
        assert_eq!(file_of("@O3@"), "media/brak.jpg");
        assert_eq!(file_of("@O4@"), "https://example.org/x.jpg");
        assert_eq!(api.call("archive.exportGedzip", path_arg(&root.join("x.gdz"))).unwrap_err().code, "inside_archive");
    }

    #[test]
    fn gedzip_of_a_551_file_is_gedcom_7() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        fs::create_dir_all(root.join("zdjecia")).unwrap();
        fs::write(root.join("zdjecia").join("Jan i Anna.jpg"), b"jpg").unwrap();
        let text = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n2 FORM LINEAGE-LINKED\n1 CHAR UTF-8\n1 SOUR PAF\n1 FILE drzewo.ged\n\
0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 OBJE\n2 FILE zdjecia\\Jan i Anna.jpg\n3 FORM jpg\n0 TRLR\n";
        fs::write(root.join("drzewo.ged"), text).unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", path_arg(&root)).unwrap();
        let target = dir.path().join("drzewo.gdz");
        api.call("archive.exportGedzip", path_arg(&target)).unwrap();
        assert_eq!(zip_names(&target), ["gedcom.ged", "zdjecia/Jan i Anna.jpg"]);
        let (doc, _) = Document::from_bytes(&zip_file(&target, "gedcom.ged").1);
        assert_eq!(doc.head().unwrap().child("GEDC").unwrap().child_value("VERS"), Some("7.0"));
        assert_eq!(doc.head().unwrap().child_value("FILE"), Some("drzewo.ged"), "the header's FILE is not a media file");
        let object = doc.record("@I1@").unwrap().child("OBJE").unwrap();
        assert_eq!(object.child_value("FILE"), Some("zdjecia/Jan%20i%20Anna.jpg"));
    }

    #[test]
    fn the_check_finds_what_the_data_shows_is_wrong() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 7.0\n1 SOUR HEIRLOOM\n\
0 @I1@ INDI\n1 NAME Jan /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1900\n1 DEAT\n2 DATE 1920\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Anna /Nowak/\n1 SEX F\n1 BIRT\n2 DATE 1902\n1 DEAT\n2 DATE 12 MAR 1925\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Piotr /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1899\n1 FAMC @F1@\n\
0 @I4@ INDI\n1 NAME Ewa /Kowalska/\n1 SEX F\n1 BIRT\n2 DATE 5 JUN 1925\n1 FAMC @F1@\n\
0 @I5@ INDI\n1 SEX U\n\
0 @I6@ INDI\n1 NAME Adam /Pętla/\n1 FAMC @F2@\n1 FAMS @F2@\n\
0 @I7@ INDI\n1 NAME Józef /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1921\n1 FAMC @F1@\n\
0 @I8@ INDI\n1 NAME Zofia /Kowalska/\n1 SEX F\n1 BIRT\n2 DATE ABT 1890\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 CHIL @I4@\n1 CHIL @I7@\n1 CHIL @I8@\n1 CHIL @I99@\n\
0 @F2@ FAM\n1 HUSB @I6@\n1 CHIL @I6@\n\
0 @F3@ FAM\n\
0 @O1@ OBJE\n1 FILE media/brak.jpg\n2 FORM image/jpeg\n0 TRLR\n";
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("rodzina.ged"), text).unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", path_arg(dir.path())).unwrap();
        let problems = api.call("archive.check", Value::Null).unwrap();
        let problems = problems.as_array().unwrap();
        let count = |kind: &str| problems.iter().filter(|p| p["kind"] == kind).count();
        assert_eq!(count("brokenLink"), 1);
        assert_eq!(count("missingFile"), 1);
        assert_eq!(count("noName"), 1);
        assert_eq!(count("dates"), 4, "{problems:#?}");
        assert_eq!(count("ownAncestor"), 1);
        assert_eq!(count("emptyFamily"), 1);
        let broken = problems.iter().find(|p| p["kind"] == "brokenLink").unwrap();
        assert_eq!(broken["message"], "Rodzina @F1@ wskazuje na rekord @I99@ (CHIL), którego nie ma w pliku.");
        assert_eq!(broken["id"], "@I1@", "a family's problem opens one of its people");
        let ids = |kind: &str| problems.iter().filter(|p| p["kind"] == kind).map(|p| p["id"].as_str().unwrap_or("")).collect::<Vec<_>>();
        assert_eq!(ids("dates"), ["@I3@", "@I3@", "@I4@", "@I4@"], "a child after the father's death (1921) and an approximate date are fine");
        assert_eq!(ids("ownAncestor"), ["@I6@"]);
        assert_eq!(ids("noName"), ["@I5@"]);

        let dir = tempfile::tempdir().unwrap();
        let (mut api, _) = open(dir.path(), 2);
        let problems = api.call("archive.check", Value::Null).unwrap();
        assert!(!problems.as_array().unwrap().iter().any(|p| ["brokenLink", "missingFile", "ownAncestor"].contains(&p["kind"].as_str().unwrap())), "{problems:#?}");
    }

    #[test]
    fn rebuilding_the_cache_only_empties_the_thumbnail_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, root) = open(dir.path(), 1);
        let id = api.call("archive.status", Value::Null).unwrap()["archiveId"].as_str().unwrap().to_string();
        let thumbs = dir.path().join("cache").join(id).join("miniatury");
        fs::create_dir_all(&thumbs).unwrap();
        fs::write(thumbs.join("a-128.jpg"), vec![0u8; 100]).unwrap();
        fs::write(thumbs.join("a-256.jpg"), vec![0u8; 200]).unwrap();
        let before = snapshot(&root);
        let result = api.call("archive.rebuildCache", Value::Null).unwrap();
        assert_eq!((result["files"].as_u64(), result["bytes"].as_u64()), (Some(2), Some(300)));
        assert_eq!(fs::read_dir(&thumbs).unwrap().count(), 0);
        assert_eq!(snapshot(&root), before);
        assert!(api.call("people.list", Value::Null).is_ok(), "the view is there again");
    }

    #[test]
    fn settings_travel_in_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, root) = open(dir.path(), 0);
        api.call("archive.setSettings", json!({ "display": { "treeColor": "generation", "cardStyle": "plain" } })).unwrap();
        api.call("app.setAppearance", json!({ "theme": "dark", "textSize": 125 })).unwrap();
        let file = dir.path().join("ustawienia Heirlooma.json");
        api.call("settings.export", path_arg(&file)).unwrap();
        let written: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!((written["format"].as_str(), written["version"].as_u64()), (Some("heirloom-ustawienia"), Some(1)));
        assert_eq!((written["display"]["treeColor"].as_str(), written["appearance"]["textSize"].as_u64()), (Some("generation"), Some(125)));
        assert_eq!(api.call("settings.export", path_arg(&root.join("u.json"))).unwrap_err().code, "inside_archive");

        // Another archive on another computer: the file's choices are merged in.
        let other = tempfile::tempdir().unwrap();
        let (mut api2, _) = open(other.path(), 0);
        api2.call("archive.setSettings", json!({ "display": { "nameOrder": "surname", "treeColor": "none" } })).unwrap();
        let state = api2.call("settings.import", path_arg(&file)).unwrap();
        assert_eq!((state["appearance"]["theme"].as_str(), state["appearance"]["textSize"].as_u64()), (Some("dark"), Some(125)));
        let display = &state["archive"]["display"];
        assert_eq!((display["treeColor"].as_str(), display["nameOrder"].as_str()), (Some("generation"), Some("surname")));
        let not_ours = other.path().join("inny.json");
        fs::write(&not_ours, "{\"theme\": \"dark\"}").unwrap();
        assert_eq!(api2.call("settings.import", path_arg(&not_ours)).unwrap_err().code, "bad_file");
    }

    #[test]
    fn editors_come_with_their_changes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, _) = open(dir.path(), 0);
        add_and_save(&mut api, "@I9001@", "Ewa");
        add_and_save(&mut api, "@I9002@", "Ewa");
        add_and_save(&mut api, "@I9003@", "Adam");
        api.call("archive.setSettings", json!({ "editors": ["Adam", "Ewa", "Ciocia Helena"] })).unwrap();
        let list = api.call("archive.editors", Value::Null).unwrap();
        let summary: Vec<(&str, u64, bool)> =
            list.as_array().unwrap().iter().map(|e| (e["name"].as_str().unwrap(), e["changes"].as_u64().unwrap(), e["lastEdited"].is_string())).collect();
        assert_eq!(summary, [("Adam", 1, true), ("Ewa", 2, true), ("Ciocia Helena", 0, false)]);
    }

    #[test]
    fn the_status_tells_about_the_data_file_and_its_copies() {
        let dir = tempfile::tempdir().unwrap();
        let (mut api, root) = open(dir.path(), 0);
        let status = api.call("archive.status", Value::Null).unwrap();
        assert_eq!((status["backupBeforeSave"].as_bool(), status["gedcomVersion"].as_str()), (Some(true), Some("7.0")));
        assert!(status["lastBackup"].is_null());
        assert_eq!(status["dataBytes"], fs::metadata(root.join("rodzina.ged")).unwrap().len());
        add_and_save(&mut api, "@I9001@", "Ewa");
        let status = api.call("archive.status", Value::Null).unwrap();
        assert!(status["lastBackup"].as_str().is_some_and(|t| t.starts_with("20")), "{}", status["lastBackup"]);

        let status = api.call("archive.setSettings", json!({ "backupBeforeSave": false })).unwrap();
        assert_eq!(status["backupBeforeSave"], false);
        let copies = || fs::read_dir(root.join(".heirloom").join("kopie")).unwrap().count();
        let before = copies();
        add_and_save(&mut api, "@I9002@", "Ewa");
        assert_eq!(copies(), before, "no copy when it is turned off");
    }

    #[test]
    fn about_gives_the_version_and_build_date() {
        let about = Api::new(None, None).call("app.about", Value::Null).unwrap();
        assert_eq!(about["version"], env!("CARGO_PKG_VERSION"));
        let date = about["buildDate"].as_str().unwrap();
        assert!(date.len() == 10 && date.starts_with("20") && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-', "{date}");
    }
}
