//! Backups of the whole archive as a .zip (A.7): made with one click into `<Dokumenty>\Heirloom\Kopie zapasowe\<nazwa>`
//! (the computer's own disk: real redundancy for a pendrive, network or cloud archive), the newest five of them kept;
//! and restored into a new folder, never over anything.

use crate::config::{ArchiveUse, ZipInfo, data_key};
use crate::tools::{decode_file, disk_name, for_each_file, inside_name, is_url, outside_archive, write_zip};
use crate::{Api, ApiError, ApiResult, Session, fsx, opt_str, overlay, str_arg};
use heirloom_core::Archive;
use heirloom_core::archive::{MEDIA_DIR, SIDECAR_DIR};
use heirloom_core::gedcom::Document;
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const AI_DIR: &str = "zrodla-ai";
const KEEP_ZIPS: usize = 5;
/// Heirloom's own files of „Twoje zmiany” that a standalone archive doesn't have.
const OVERLAY_ONLY: &[&str] = &[overlay::MANIFEST, overlay::BASE];

thread_local! {
    /// Tests: the free space a disk reports.
    static AVAILABLE: Cell<Option<u64>> = const { Cell::new(None) };
}

#[doc(hidden)]
pub fn set_available_space(bytes: Option<u64>) {
    AVAILABLE.with(|a| a.set(bytes));
}

fn available(path: &Path) -> Option<u64> {
    AVAILABLE.with(|a| a.get()).or_else(|| fs4::available_space(path).ok())
}

pub fn call(api: &mut Api, method: &str, args: &Value) -> ApiResult {
    match method {
        "archive.backupInfo" => info(api, &scope(args)?),
        "archive.backupAuto" => auto(api, &scope(args)?),
        "archive.restore" => restore(api, args),
        "app.setBackupFolder" => {
            let folder = str_arg(args, "folder")?;
            if !Path::new(&folder).is_absolute() {
                return Err(ApiError::bad_args("folder"));
            }
            api.update_config(|c| c.backup_folder = Some(folder));
            Ok(Value::Null)
        }
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

/// `archive`: the archive as the user sees it (in „Nie ruszaj oryginału” their version, as a standalone archive);
/// `original`: the original archive itself (before the write-back).
fn scope(args: &Value) -> Result<String, ApiError> {
    let scope = opt_str(args, "scope").unwrap_or_else(|| "archive".into());
    if scope != "archive" && scope != "original" {
        return Err(ApiError::bad_args("scope"));
    }
    Ok(scope)
}

/// The files of a backup, at their places in the archive folder: the data file, `media/`, `zrodla-ai/`, Heirloom's own
/// files from `.heirloom/` (not the data file's copies in `kopie/`), and every other file the data points to inside
/// the archive folder (a 5.5.1 archive's `zdjecia\jan.jpg`). In „Twoje zmiany” both folders give their files (the
/// user's first) and the zip restores as a normal archive.
pub fn selection(s: &Session, scope: &str) -> Result<Vec<(String, PathBuf)>, ApiError> {
    match (scope, &s.overlay) {
        ("original", Some(info)) => {
            let original = Archive::open(&info.original_data_path)?;
            Ok(select(&[original.root().to_path_buf()], &original.settings().data_file, &original.data_path(), original.sidecar_path()))
        }
        _ => Ok(select(&s.layers(), &s.archive.settings().data_file, &s.archive.data_path(), s.archive.sidecar_path())),
    }
}

fn select(layers: &[PathBuf], data_file: &str, data_path: &Path, sidecar: &Path) -> Vec<(String, PathBuf)> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = vec![(data_file.to_string(), data_path.to_path_buf())];
    seen.insert(fsx::name_key(data_file));
    for layer in layers {
        for folder in [MEDIA_DIR, AI_DIR] {
            for (rel, path) in fsx::walk(&layer.join(folder)) {
                let name = format!("{folder}/{rel}");
                if !rel.contains(".tmp-") && seen.insert(fsx::name_key(&name)) {
                    entries.push((name, path));
                }
            }
        }
    }
    if let Ok(list) = fs::read_dir(sidecar) {
        for entry in list.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Files only (`kopie/` is a folder); a `.tmp-` file is a write in progress.
            if entry.file_type().is_ok_and(|t| t.is_file()) && !name.contains(".tmp-") && !OVERLAY_ONLY.contains(&name.as_str()) {
                entries.push((format!("{SIDECAR_DIR}/{name}"), entry.path()));
            }
        }
    }
    // Files the data points to elsewhere inside the archive folder.
    if let Ok(bytes) = fs::read(data_path) {
        let (mut doc, _) = Document::from_bytes(&bytes);
        let version = doc.version;
        let own = |name: &str| name.eq_ignore_ascii_case(data_file) || name.to_lowercase().starts_with(&format!("{SIDECAR_DIR}/"));
        let mut extra = Vec::new();
        for_each_file(&mut doc.records, false, &mut |file| {
            let Some(path) = file.value.as_deref().map(|v| decode_file(v, version).replace('\\', "/")) else { return };
            if path.is_empty() || is_url(&path) {
                return;
            }
            let Some(name) = inside_name(&path).filter(|n| !own(n)) else { return };
            if seen.insert(fsx::name_key(&name)) {
                extra.push(name);
            }
        });
        for name in extra {
            let source = crate::media_edit::resolve_in(layers, &name);
            if source.is_file() {
                entries.push((name, source));
            }
        }
    }
    entries
}

/// `Heirloom-<nazwa>-2026-10-10-1405.zip` in local time.
fn zip_file_name(name: &str) -> String {
    let now = time::OffsetDateTime::now_utc();
    let local = time::UtcOffset::local_offset_at(now).map_or(now, |offset| now.to_offset(offset));
    let stamp = local.format(time::macros::format_description!("[year]-[month]-[day]-[hour][minute]")).unwrap_or_default();
    format!("Heirloom-{}-{stamp}", fsx::folder_name(name))
}

fn is_auto_name(path: &Path) -> bool {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).is_some_and(|n| n.starts_with("Heirloom-") && n.to_lowercase().ends_with(".zip"))
}

/// The registry key the backups of the open archive are recorded under (the original's, in „Twoje zmiany”).
fn key_of(s: &Session) -> String {
    match &s.overlay {
        Some(info) => data_key(&info.original_data_path),
        None => data_key(&s.archive.data_path()),
    }
}

/// The automatic backups' folder for this archive.
fn folder_for(api: &Api, s: &Session) -> PathBuf {
    api.backup_parent().join(fsx::folder_name(&s.archive.settings().name))
}

/// What the files are now: a zip with the same data hash and signature needs no successor.
fn signature(entries: &[(String, PathBuf)]) -> (String, String) {
    let data = entries.first().and_then(|(_, p)| fsx::hash_file(p).ok()).map(|h| h.to_hex().to_string()).unwrap_or_default();
    let mut hasher = blake3::Hasher::new();
    for (name, path) in entries.iter().skip(1) {
        let meta = fs::metadata(path).ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
        hasher.update(format!("{name}|{}|{modified}\n", meta.map_or(0, |m| m.len())).as_bytes());
    }
    (data, hasher.finalize().to_hex().to_string())
}

/// Files of OneDrive that are only in the cloud (zipping them downloads them).
fn cloud_only(entries: &[(String, PathBuf)]) -> (usize, u64) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
        let mut found = (0, 0);
        for (_, path) in entries {
            if let Ok(meta) = fs::metadata(path) {
                if meta.file_attributes() & RECALL_ON_DATA_ACCESS != 0 {
                    found.0 += 1;
                    found.1 += meta.len();
                }
            }
        }
        found
    }
    #[cfg(not(windows))]
    {
        let _ = entries;
        (0, 0)
    }
}

fn in_one_drive(path: &Path) -> bool {
    ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"]
        .iter()
        .filter_map(|v| std::env::var_os(v))
        .filter(|v| !v.is_empty())
        .any(|v| path.starts_with(Path::new(&v)))
}

/// Room the zip needs: photos are stored as they are.
fn needed(bytes: u64) -> u64 {
    bytes + bytes / 20 + 50_000_000
}

/// „Kopia trafi do: … · ok. 1,2 GB (wolne: 35 GB)” and what to warn about.
fn info(api: &mut Api, scope: &str) -> ApiResult {
    let documents = api.documents();
    let s = api.session.as_ref().ok_or_else(ApiError::no_archive)?;
    let entries = selection(s, scope)?;
    let folder = folder_for(api, s);
    let bytes: u64 = entries.iter().filter_map(|(_, p)| fs::metadata(p).ok()).map(|m| m.len()).sum();
    let existing = folder.ancestors().find(|p| p.exists()).map(Path::to_path_buf).unwrap_or_else(|| folder.clone());
    let record = api.config.archives.get(&key_of(s));
    let last = record.and_then(|r| r.zips.last());
    let (data_hash, media_sig) = signature(&entries);
    let current = last.is_some_and(|z| z.data_hash == data_hash && z.media_sig == media_sig && Path::new(&z.path).is_file());
    let archive_disk = disk_name(s.overlay.as_ref().map_or(s.archive.root(), |i| &i.original_root));
    let (cloud_files, cloud_bytes) = cloud_only(&entries);
    Ok(json!({
        "folder": folder.display().to_string(),
        "folderLabel": overlay::label(&folder, documents.as_deref()),
        "files": entries.len(),
        "bytes": bytes,
        "needed": needed(bytes),
        "free": available(&existing),
        "lastZip": last.map(|z| json!({ "path": z.path, "at": z.at, "bytes": z.bytes, "exists": Path::new(&z.path).is_file() })),
        "current": current,
        "inOneDrive": in_one_drive(&folder),
        "sameDisk": archive_disk.is_some() && archive_disk == disk_name(&existing),
        "cloudOnly": { "files": cloud_files, "bytes": cloud_bytes },
    }))
}

/// „Zrób kopię zapasową”: the zip goes into the backup folder under a new name, is recorded, and only the newest five
/// automatic backups of this archive are kept (files Heirloom didn't make are never touched).
fn auto(api: &mut Api, scope: &str) -> ApiResult {
    let progress = api.progress.clone();
    let s = api.session.as_ref().ok_or_else(ApiError::no_archive)?;
    let entries = selection(s, scope)?;
    let folder = folder_for(api, s);
    fs::create_dir_all(&folder).map_err(|e| ApiError::new("io", format!("Nie można utworzyć folderu {}: {e}", folder.display())))?;
    let mut layers = s.layers();
    if let Some(info) = &s.overlay {
        layers.push(info.original_root.clone());
    }
    let stem = zip_file_name(&s.archive.settings().name);
    let name = fsx::free_name(&|n| folder.join(n).exists() || fs::read_dir(&folder).into_iter().flatten().flatten().any(|e| fsx::name_key(&e.file_name().to_string_lossy()) == n), &stem, "zip");
    let target = folder.join(&name);
    for layer in &layers {
        outside_archive(layer, &target)?;
    }
    let bytes: u64 = entries.iter().filter_map(|(_, p)| fs::metadata(p).ok()).map(|m| m.len()).sum();
    if let Some(free) = available(&folder) {
        if needed(bytes) > free {
            let disk = disk_name(&folder).map(|d| format!(" {d}")).unwrap_or_default();
            return Err(ApiError::new(
                "no_space",
                format!(
                    "Za mało miejsca na dysku{disk} — kopia zajmie ok. {}, a wolne jest {}. Wybierz inne miejsce, np. pendrive.",
                    crate::count_bytes(needed(bytes)),
                    crate::count_bytes(free)
                ),
            ));
        }
    }
    let (data_hash, media_sig) = signature(&entries);
    let key = key_of(s);
    progress.start("Tworzę kopię zapasową…", entries.len() as u64);
    let written = write_zip(&target, None, &entries, Some(&progress));
    progress.finish();
    let size = written?;
    let zip = ZipInfo { path: target.display().to_string(), at: heirloom_core::history::now(), bytes: size, data_hash, media_sig };
    let data_file = s.archive.settings().data_file.clone();
    let mut removed: Vec<PathBuf> = Vec::new();
    api.update_config(|c| {
        let record = c.archives.entry(key).or_insert_with(|| ArchiveUse { data_file, ..ArchiveUse::default() });
        record.zips.push(zip);
        // The newest five automatic backups stay; older ones made here go.
        let ours: Vec<usize> = (0..record.zips.len()).filter(|&i| is_auto_name(Path::new(&record.zips[i].path))).collect();
        if ours.len() > KEEP_ZIPS {
            let old: HashSet<usize> = ours[..ours.len() - KEEP_ZIPS].iter().copied().collect();
            removed = old.iter().map(|&i| PathBuf::from(&record.zips[i].path)).collect();
            let mut i = 0;
            record.zips.retain(|_| {
                let keep = !old.contains(&i);
                i += 1;
                keep
            });
        }
    });
    for path in removed {
        let _ = fs::remove_file(path);
    }
    Ok(json!({ "path": target.display().to_string(), "bytes": size, "files": entries.len() }))
}

/// One entry name of a zip as a safe relative path: no `..`, no absolute paths, no drives or streams, no device names.
fn safe_parts(name: &str) -> Option<Vec<String>> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }
    let parts: Vec<String> = normalized.split('/').filter(|p| !p.is_empty() && *p != ".").map(str::to_string).collect();
    if parts.is_empty() || parts.iter().any(|p| p == ".." || p.contains(':') || fsx::is_device_name(p)) {
        return None;
    }
    Some(parts)
}

/// „Przywróć z kopii…”: the zip is unpacked into a new folder „<nazwa> (z kopii 2026-10-10)” (next to the open
/// archive unless another folder is given) and opened there. Nothing existing is ever overwritten.
fn restore(api: &mut Api, args: &Value) -> ApiResult {
    let zip_path = PathBuf::from(str_arg(args, "zip")?);
    let parent = match opt_str(args, "folder") {
        Some(f) => PathBuf::from(f),
        None => {
            let s = api.session.as_ref().ok_or_else(|| ApiError::bad_args("folder"))?;
            let root = s.overlay.as_ref().map_or(s.archive.root(), |i| &i.original_root);
            root.parent().map(Path::to_path_buf).ok_or_else(|| ApiError::bad_args("folder"))?
        }
    };
    let file = fs::File::open(&zip_path).map_err(|e| ApiError::new("io", format!("Nie można odczytać pliku: {e}")))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| ApiError::new("bad_file", "W tym pliku nie ma archiwum Heirlooma."))?;
    let mut plan: Vec<(usize, Vec<String>)> = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| ApiError::new("bad_file", e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let parts = safe_parts(entry.name()).ok_or_else(|| ApiError::new("bad_file", format!("Ten plik zawiera niebezpieczną ścieżkę: {}", entry.name())))?;
        plan.push((i, parts));
    }
    if !plan.iter().any(|(_, p)| p.len() == 1 && p[0].to_lowercase().ends_with(".ged")) {
        return Err(ApiError::new("bad_file", "W tym pliku nie ma archiwum Heirlooma."));
    }
    // The archive's name from its settings, else the zip's.
    let mut name = zip_path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Archiwum".into());
    if let Ok(mut settings) = zip.by_name(&format!("{SIDECAR_DIR}/ustawienia.json")) {
        let mut text = String::new();
        if std::io::Read::read_to_string(&mut settings, &mut text).is_ok() {
            if let Some(n) = serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("name").and_then(Value::as_str).map(str::to_string)).filter(|n| !n.trim().is_empty()) {
                name = n;
            }
        }
    }
    let date = zip_path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .and_then(|stem| stem.rsplit_once('-').map(|(rest, _)| rest.to_string()))
        .and_then(|rest| rest.get(rest.len().saturating_sub(10)..).map(str::to_string))
        .filter(|d| d.len() == 10 && d.as_bytes()[4] == b'-' && d.as_bytes()[7] == b'-')
        .unwrap_or_else(overlay::today);
    fs::create_dir_all(&parent).map_err(|e| ApiError::new("permission", format!("Nie można utworzyć folderu {}: {e}", parent.display())))?;
    let target = overlay::free_folder(&parent, &format!("{name} (z kopii {date})"));
    fs::create_dir(&target).map_err(|e| ApiError::new("permission", format!("Nie można utworzyć folderu {}: {e}", target.display())))?;
    let unpacked = (|| {
        for (i, parts) in &plan {
            let mut entry = zip.by_index(*i).map_err(|e| ApiError::new("bad_file", e.to_string()))?;
            let dest = parts.iter().fold(target.clone(), |p, part| p.join(part));
            if let Some(dir) = dest.parent() {
                fs::create_dir_all(dir).map_err(overlay::io_error)?;
            }
            let mut out = fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(overlay::io_error)?;
            std::io::copy(&mut entry, &mut out).map_err(overlay::io_error)?;
        }
        Ok::<_, ApiError>(())
    })();
    if let Err(e) = unpacked {
        let _ = fs::remove_dir_all(&target);
        return Err(e);
    }
    let archive = Archive::open(&target)?;
    api.record_in_place(&archive, &target.display().to_string());
    let folder_name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut status = api.install(archive, &target.display().to_string())?;
    status["notice"] = json!(format!("Przywrócono archiwum do folderu „{folder_name}”. Poprzednie archiwum zostało bez zmian."));
    Ok(status)
}
