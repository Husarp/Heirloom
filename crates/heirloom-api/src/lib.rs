//! The app's commands, shared by the Tauri window (`src-tauri`) and the browser bridge (`heirloom-bridge`).
//! Every command takes JSON and returns JSON, so the UI's `api.ts` works the same over both.

pub mod activity;
pub mod config;
pub mod edit;
pub mod gedwrite;
pub mod import;
pub mod media_edit;
pub mod derive;
pub mod kin;
pub mod lists;
pub mod tree;
pub mod media;
pub mod people;
pub mod text;
mod tools;
pub mod update;

use config::{AppConfig, RecentArchive};
use derive::Derived;
use heirloom_core::{Archive, Error as CoreError, SaveOptions};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// An error for the UI: a stable code to branch on and a Polish message to show.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &'static str, message: impl Into<String>) -> ApiError {
        ApiError { code, message: message.into() }
    }

    fn no_archive() -> ApiError {
        ApiError::new("no_archive", "Żadne archiwum nie jest otwarte.")
    }

    pub fn bad_args(detail: impl std::fmt::Display) -> ApiError {
        ApiError::new("bad_args", format!("Błędne dane polecenia: {detail}"))
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> ApiError {
        let code = match &e {
            CoreError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => "not_found",
            CoreError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => "permission",
            CoreError::Io(_) => "io",
            CoreError::Json(_) => "settings_damaged",
            CoreError::Cache(_) => "cache",
            CoreError::NoDataFile => "no_data_file",
            CoreError::SeveralDataFiles(_) => "several_data_files",
            CoreError::AlreadyExists(_) => "already_exists",
            CoreError::ReadOnly => "read_only",
            CoreError::ForeignNeedsConfirmation(_) => "foreign",
            CoreError::Conflict => "conflict",
            CoreError::NoSuchRecord(_) => "no_record",
            CoreError::DuplicateRecord(_) | CoreError::MissingXref => "invalid",
        };
        ApiError::new(code, e.to_string())
    }
}

pub type ApiResult = Result<Value, ApiError>;

/// "3 zmiany", "1 plik", "5 osób".
pub fn count_pl(n: usize, one: &str, few: &str, many: &str) -> String {
    let word = if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    };
    format!("{n} {word}")
}

/// The open archive and what is worked out from it.
pub struct Session {
    pub archive: Archive,
    /// Rebuilt lazily after every change.
    pub(crate) derived: Option<Derived>,
    /// The display settings it was built with; changing them (name order, date format, surname joins) rebuilds it.
    display_seen: Option<serde_json::Map<String, Value>>,
    pub last_saved: Option<String>,
}

impl Session {
    pub(crate) fn new(archive: Archive) -> Session {
        Session { archive, derived: None, display_seen: None, last_saved: None }
    }

    pub(crate) fn ensure(&mut self) {
        if self.display_seen.as_ref() != Some(&self.archive.settings().display) {
            self.derived = None;
        }
        if self.derived.is_none() {
            let display = self.archive.settings().display.clone();
            let options = derive::Display::from_settings(&display);
            heirloom_core::polish::set_numeric_dates(options.numeric_dates);
            self.derived = Some(derive::build_with(&self.archive.doc, &options));
            self.display_seen = Some(display);
        }
    }

    pub fn derived(&mut self) -> &Derived {
        self.ensure();
        self.derived.as_ref().expect("built above")
    }

    /// After any change to the records.
    pub fn changed(&mut self) {
        self.derived = None;
    }

    pub fn history(&self) -> Vec<heirloom_core::history::Entry> {
        heirloom_core::history::read(&self.archive.history_path()).unwrap_or_default()
    }
}

pub struct Api {
    config_dir: Option<PathBuf>,
    cache_root: Option<PathBuf>,
    config: AppConfig,
    session: Option<Session>,
    /// The import being reviewed (steps 1–5); dropped when another archive is opened.
    import: Option<import::Draft>,
    /// Where file URLs are served from; shared with the media handler, which runs outside the command lock.
    media: media::MediaRoots,
}

fn str_arg(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

fn opt_str(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_string)
}

impl Api {
    /// `config_dir` holds `aplikacja.json` (None: nothing is remembered, for tests); `cache_root` holds the
    /// per-archive caches (thumbnails).
    pub fn new(config_dir: Option<PathBuf>, cache_root: Option<PathBuf>) -> Api {
        let config = config_dir.as_deref().map(AppConfig::load).unwrap_or_default();
        Api { config_dir, cache_root, config, session: None, import: None, media: media::MediaRoots::default() }
    }

    /// The standard locations: `%APPDATA%\Heirloom` and `%LOCALAPPDATA%\Heirloom\cache`.
    pub fn with_default_dirs() -> Api {
        Api::new(AppConfig::default_dir(), heirloom_core::Cache::default_root())
    }

    pub fn media_roots(&self) -> media::MediaRoots {
        self.media.clone()
    }

    fn session(&mut self) -> Result<&mut Session, ApiError> {
        self.session.as_mut().ok_or_else(ApiError::no_archive)
    }

    /// Runs one command. `args` is a JSON object (or null when the command takes nothing).
    pub fn call(&mut self, method: &str, args: Value) -> ApiResult {
        match method {
            "app.state" => Ok(self.app_state()),
            "app.setAppearance" => self.set_appearance(&args),
            "app.setPlace" => {
                // The archive is named by the UI: a place sent late must not land in an archive opened since.
                let archive_id = str_arg(&args, "archiveId")?;
                let part = |key: &str| args.get(key).filter(|v| !v.is_null()).cloned();
                let viewed = args.get("viewed").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
                self.config.set_place(&archive_id, part("route"), part("tree"), viewed, heirloom_core::history::now());
                self.save_config();
                Ok(Value::Null)
            }
            "app.setUpdates" => {
                self.config.updates.check = args.get("check").and_then(Value::as_bool).ok_or_else(|| ApiError::bad_args("check"))?;
                self.save_config();
                Ok(self.app_state())
            }
            "recent.forget" => {
                self.config.forget(&str_arg(&args, "path")?);
                self.save_config();
                Ok(self.app_state())
            }
            "archive.open" => self.open_archive(&str_arg(&args, "path")?),
            "archive.create" => self.create_archive(&str_arg(&args, "folder")?, &str_arg(&args, "name")?),
            "archive.close" => {
                self.session = None;
                self.import = None;
                self.media.set(None);
                Ok(Value::Null)
            }
            "archive.status" => self.archive_status(),
            "archive.save" => self.save(&args),
            "archive.reload" => {
                let s = self.session()?;
                s.archive.reload()?;
                s.changed();
                self.archive_status()
            }
            "archive.saveAsCopy" => {
                let path = self.session()?.archive.save_as_copy()?;
                Ok(json!({ "path": path.display().to_string() }))
            }
            "archive.undo" => {
                let s = self.session()?;
                if s.archive.undo() {
                    s.changed();
                }
                self.archive_status()
            }
            "archive.redo" => {
                let s = self.session()?;
                if s.archive.redo() {
                    s.changed();
                }
                self.archive_status()
            }
            "archive.setSettings" => self.set_archive_settings(&args),
            "archive.storage" | "archive.backup" | "archive.exportGedzip" | "archive.check" | "archive.rebuildCache" | "archive.editors"
            | "archive.saveAsNew" | "settings.export" | "settings.import" | "app.about" => tools::call(self, method, &args),
            "archive.setEditor" => {
                // Remembered for "Kto edytuje?" next time; the name is added to the archive's list on the first save.
                self.config.last_editor = Some(str_arg(&args, "name")?);
                self.save_config();
                Ok(Value::Null)
            }
            "people.list" => Ok(people::list(self.session()?.derived())),
            "people.search" => {
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
                let query = str_arg(&args, "q")?;
                Ok(people::search(self.session()?.derived(), &query, limit))
            }
            "person.get" => {
                let id = str_arg(&args, "id")?;
                let s = self.session()?;
                let history = s.history();
                let mut profile = people::profile(s.derived(), &id, &history)?;
                profile["brokenLinks"] = people::broken_links(&s.archive.doc, &id);
                Ok(profile)
            }
            "person.panel" => {
                let id = str_arg(&args, "id")?;
                people::panel(self.session()?.derived(), &id)
            }
            "person.relations" => {
                let id = str_arg(&args, "id")?;
                people::relations(self.session()?.derived(), &id)
            }
            "date.parse" => Ok(date_feedback(&str_arg(&args, "text")?)),
            "person.editData" => {
                let id = str_arg(&args, "id")?;
                people::edit_data(self.session()?.derived(), &id)
            }
            "person.hover" => {
                let id = str_arg(&args, "id")?;
                let from = opt_str(&args, "from");
                people::hover(self.session()?.derived(), &id, from.as_deref())
            }
            "history.feed" => {
                let filter = opt_str(&args, "filter").unwrap_or_else(|| "all".into());
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(200) as usize;
                let s = self.session()?;
                let entries = s.history();
                s.ensure();
                let d = s.derived.as_ref().expect("ensured");
                Ok(activity::feed(d, &entries, &s.archive.doc.records, &filter, limit))
            }
            "history.undo" => self.undo_history(&args),
            m if m.starts_with("import.") => {
                let session = self.session.as_mut().ok_or_else(ApiError::no_archive)?;
                let result = import::call(session, &mut self.import, m, &args);
                self.media.set_imports(self.import.as_ref().map(|d| d.files.iter().map(|f| f.path.clone()).collect()).unwrap_or_default());
                result
            }
            "tree.graph" => {
                let id = str_arg(&args, "id")?;
                let up = args.get("up").and_then(Value::as_u64).unwrap_or(2) as usize;
                let down = args.get("down").and_then(Value::as_u64).unwrap_or(2) as usize;
                let expand: Vec<String> = args.get("expand").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                tree::graph(self.session()?.derived(), &id, up.min(12), down.min(12), &expand)
            }
            "tree.overview" => {
                let focus = opt_str(&args, "focus");
                Ok(tree::overview(self.session()?.derived(), focus.as_deref()))
            }
            "start.data" => {
                let s = self.session()?;
                let history = s.history();
                let title = s.archive.settings().name.clone();
                Ok(lists::start(s.derived(), &history, &title))
            }
            "archive.firstOpen" => {
                let s = self.session()?;
                let root = s.archive.root().to_path_buf();
                Ok(lists::first_open(s.derived(), &root))
            }
            "surnames.list" => Ok(lists::surnames(self.session()?.derived())),
            "surname.get" => {
                let key = str_arg(&args, "key")?;
                lists::surname(self.session()?.derived(), &key)
            }
            "places.list" => Ok(lists::places(self.session()?.derived())),
            "place.get" => {
                let path: Vec<String> = args.get("path").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                lists::place(self.session()?.derived(), &path)
            }
            "stories.list" => Ok(lists::stories(self.session()?.derived())),
            "story.get" => {
                let id = str_arg(&args, "id")?;
                lists::story(self.session()?.derived(), &id)
            }
            "media.list" | "media.get" | "media.missing" | "media.findMissing" => {
                let s = self.session()?;
                let root = s.archive.root().to_path_buf();
                let d = s.derived();
                match method {
                    "media.list" => Ok(lists::media_list(d, &root)),
                    "media.get" => lists::media_get(d, &str_arg(&args, "id")?, &root),
                    "media.missing" => Ok(lists::missing_list(d, &root)),
                    _ => Ok(lists::find_missing(d, &root, Path::new(&str_arg(&args, "folder")?))),
                }
            }
            "sources.list" => Ok(lists::sources(self.session()?.derived())),
            "source.get" => {
                let id = str_arg(&args, "id")?;
                lists::source(self.session()?.derived(), &id)
            }
            m if ["person.", "relation.", "family.", "text.", "media.", "source.", "citation.", "place."].iter().any(|p| m.starts_with(p)) => {
                edit::call(self.session()?, m, &args)
            }
            _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
        }
    }

    /// "Cofnij" in the history: the record goes back to how it was before that change (or a whole import batch
    /// is taken back). It is a new change like any other: it shows as unsaved and can itself be undone.
    fn undo_history(&mut self, args: &Value) -> ApiResult {
        let s = self.session()?;
        if s.archive.settings().read_only {
            return Err(ApiError::new("read_only", "To archiwum jest tylko do odczytu (Ustawienia › Archiwum)."));
        }
        let entries = s.history();
        let targets: Vec<&heirloom_core::history::Entry> = match (opt_str(args, "batch"), opt_str(args, "ts")) {
            (Some(batch), _) => entries.iter().filter(|e| e.batch.as_deref() == Some(batch.as_str())).collect(),
            // „Cofnij zapis”: every entry one save wrote, named by the time and author they share (`archive.save`
            // returns them) — never by position, which an unreadable history or another computer could shift.
            (None, Some(ts)) => {
                let author = str_arg(args, "author")?;
                entries.iter().filter(|e| e.ts == ts && e.author == author && e.batch.is_none()).collect()
            }
            (None, None) => {
                let index = args.get("index").and_then(Value::as_u64).ok_or_else(|| ApiError::bad_args("index"))? as usize;
                vec![entries.get(index).ok_or_else(|| ApiError::bad_args("index"))?]
            }
        };
        let parse = |text: &str| {
            heirloom_core::gedcom::Document::from_bytes(text.as_bytes()).0.records.into_iter().find(|r| r.tag != "HEAD" && r.tag != "TRLR")
        };
        let mut edits = Vec::new();
        // Each record as it is now, then as the undo leaves it (a record can come twice in a batch).
        let mut state: std::collections::HashMap<String, Option<String>> = std::collections::HashMap::new();
        let mut skipped = 0usize;
        for entry in targets.iter().rev().filter(|e| e.tag != "HEAD") {
            let now = state
                .entry(entry.record.clone())
                .or_insert_with(|| s.archive.doc.record(&entry.record).map(heirloom_core::history::record_text))
                .clone();
            // Changed since (a later edit, import or undo): it stays as it is now, or that change would be lost.
            if now != entry.after {
                skipped += 1;
                continue;
            }
            match (entry.before.as_deref().and_then(parse), now.is_some()) {
                (Some(before), true) => edits.push(heirloom_core::Edit::Replace(before)),
                (Some(before), false) => edits.push(heirloom_core::Edit::Add(before)),
                (None, true) => edits.push(heirloom_core::Edit::Remove(entry.record.clone())),
                (None, false) => {}
            }
            state.insert(entry.record.clone(), entry.before.clone());
        }
        if !edits.is_empty() {
            s.archive.apply_all(edits)?;
            s.changed();
        }
        let mut status = self.archive_status()?;
        status["undoSkipped"] = json!(skipped);
        Ok(status)
    }

    fn app_state(&self) -> Value {
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "recent": self.config.recent,
            "appearance": self.config.appearance,
            "lastEditor": self.config.last_editor,
            "updates": self.config.updates,
            "archive": self.session.as_ref().map(|s| status_of(s)),
            // Where the open archive was left on this computer.
            "place": self.session.as_ref().and_then(|s| self.config.places.get(&s.archive.settings().archive_id)),
        })
    }

    fn set_appearance(&mut self, args: &Value) -> ApiResult {
        let mut appearance = self.config.appearance.clone();
        if let Some(theme) = args.get("theme").and_then(Value::as_str) {
            if !["system", "light", "dark"].contains(&theme) {
                return Err(ApiError::bad_args("theme"));
            }
            appearance.theme = theme.into();
        }
        if let Some(size) = args.get("textSize").and_then(Value::as_u64) {
            appearance.text_size = (size as u32).clamp(100, 150);
        }
        if let Some(density) = args.get("density").and_then(Value::as_str) {
            if !["comfortable", "compact"].contains(&density) {
                return Err(ApiError::bad_args("density"));
            }
            appearance.density = density.into();
        }
        if let Some(animations) = args.get("animations").and_then(Value::as_bool) {
            appearance.animations = animations;
        }
        if let Some(start_in) = args.get("startIn").and_then(Value::as_str) {
            if !["start", "last"].contains(&start_in) {
                return Err(ApiError::bad_args("startIn"));
            }
            appearance.start_in = start_in.into();
        }
        self.config.appearance = appearance;
        self.save_config();
        Ok(self.app_state())
    }

    fn open_archive(&mut self, path: &str) -> ApiResult {
        let archive = Archive::open(Path::new(path))?;
        self.install(archive, path)
    }

    fn create_archive(&mut self, folder: &str, name: &str) -> ApiResult {
        let name = name.trim();
        if name.is_empty() {
            return Err(ApiError::new("bad_args", "Podaj nazwę archiwum."));
        }
        let archive = Archive::create(Path::new(folder), name)?;
        self.install(archive, folder)
    }

    /// Makes `archive` the open one and remembers it in the recent list.
    pub(crate) fn install(&mut self, archive: Archive, opened_path: &str) -> ApiResult {
        self.media.set(Some(media::Roots {
            archive: archive.root().to_path_buf(),
            thumbs: self.cache_root.as_ref().map(|c| c.join(&archive.settings().archive_id).join("miniatury")),
        }));
        self.config.remember(RecentArchive {
            path: opened_path.to_string(),
            name: archive.settings().name.clone(),
            opened_at: heirloom_core::history::now(),
            people: archive.doc.records.iter().filter(|r| r.tag == "INDI").count(),
        });
        self.save_config();
        self.session = Some(Session::new(archive));
        self.import = None;
        self.archive_status()
    }

    fn archive_status(&self) -> ApiResult {
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        Ok(status_of(s))
    }

    fn save(&mut self, args: &Value) -> ApiResult {
        let author = str_arg(args, "author")?;
        let note = opt_str(args, "note");
        let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
        let s = self.session()?;
        let report = s.archive.save(&SaveOptions { author: &author, batch: None, note: note.as_deref(), allow_foreign })?;
        // What names this save for „Cofnij zapis”; nothing when its history couldn't be written.
        let history = report.history_ts.as_ref().filter(|_| report.history_error.is_none()).map(|ts| json!({ "ts": ts, "author": author }));
        let saved_at = heirloom_core::history::now();
        s.last_saved = Some(saved_at.clone());
        s.changed();
        self.config.last_editor = Some(author);
        self.save_config();
        Ok(json!({
            "backup": report.backup.map(|p| p.display().to_string()),
            "changes": report.history_entries,
            "historyError": report.history_error,
            "savedAt": saved_at,
            "history": history,
            "status": self.archive_status()?,
        }))
    }

    fn set_archive_settings(&mut self, args: &Value) -> ApiResult {
        let s = self.session()?;
        let settings = s.archive.settings_mut();
        if let Some(name) = args.get("name").and_then(Value::as_str) {
            let name = name.trim();
            if name.is_empty() {
                return Err(ApiError::new("bad_args", "Nazwa archiwum nie może być pusta."));
            }
            settings.name = name.to_string();
        }
        if let Some(read_only) = args.get("readOnly").and_then(Value::as_bool) {
            settings.read_only = read_only;
        }
        if let Some(keep) = args.get("backupsToKeep").and_then(Value::as_u64) {
            settings.backups_to_keep = (keep as usize).clamp(1, 1000);
        }
        if let Some(backup) = args.get("backupBeforeSave").and_then(Value::as_bool) {
            settings.backup_before_save = backup;
        }
        if let Some(editors) = args.get("editors").and_then(Value::as_array) {
            let names: Vec<String> = editors.iter().filter_map(Value::as_str).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect();
            let old = std::mem::take(&mut settings.editors);
            settings.editors = names
                .into_iter()
                .map(|name| old.iter().find(|e| e.name == name).cloned().unwrap_or(heirloom_core::archive::EditorEntry { name, last_edited: None }))
                .collect();
        }
        if let Some(display) = args.get("display").and_then(Value::as_object) {
            for (k, v) in display {
                settings.display.insert(k.clone(), v.clone());
            }
        }
        s.archive.save_settings()?;
        self.archive_status()
    }

    fn save_config(&self) {
        if let Some(dir) = &self.config_dir {
            // Only conveniences (recent list, appearance, update checks, last places) live here; failing to write them must not break the app.
            let _ = self.config.save(dir);
        }
    }
}

fn status_of(s: &Session) -> Value {
    let archive = &s.archive;
    let settings = archive.settings();
    json!({
        "archiveId": settings.archive_id,
        "name": settings.name,
        "root": archive.root().display().to_string(),
        "dataFile": settings.data_file,
        "dataPath": archive.data_path().display().to_string(),
        "origin": archive.origin(),
        "foreign": archive.is_foreign(),
        "foreignConfirmed": settings.foreign_save_confirmed,
        "readOnly": settings.read_only,
        "unsavedChanges": archive.unsaved_records(),
        "canUndo": archive.can_undo(),
        "undoDepth": archive.undo_depth(),
        "canRedo": archive.can_redo(),
        "changedOnDisk": archive.changed_on_disk().unwrap_or(false),
        "warnings": archive.warnings.iter().map(|w| json!({ "line": w.line, "message": w.message })).collect::<Vec<_>>(),
        "encoding": archive.doc.source_encoding.label(),
        "people": archive.doc.records.iter().filter(|r| r.tag == "INDI").count(),
        "families": archive.doc.records.iter().filter(|r| r.tag == "FAM").count(),
        "editors": settings.editors.iter().map(|e| json!({ "name": e.name, "lastEdited": e.last_edited })).collect::<Vec<_>>(),
        "display": settings.display,
        "lastSaved": s.last_saved,
        "backupsToKeep": settings.backups_to_keep,
        "backupBeforeSave": settings.backup_before_save,
        "lastBackup": tools::last_backup(&archive.backups()),
        "dataBytes": std::fs::metadata(archive.data_path()).ok().map(|m| m.len()),
        "gedcomVersion": archive.doc.head().and_then(|h| h.child("GEDC")).and_then(|g| g.child_value("VERS")).map(str::trim),
        "sidecar": archive.sidecar_path().display().to_string(),
    })
}

/// What the smart date field shows under the input (spec §5.11): how the typed text was read.
fn date_feedback(text: &str) -> Value {
    use heirloom_core::gedcom::date::{self, Calendar, Qualifier};
    use heirloom_core::polish;
    if text.trim().is_empty() {
        return json!({ "qualifier": "empty" });
    }
    let Some(gedcom) = polish::parse_date_input(text) else {
        return json!({ "qualifier": "text", "text": text.trim() });
    };
    let Some(value) = date::parse(&gedcom) else { return json!({ "qualifier": "text", "text": text.trim() }) };
    let qualifier = match value.qualifier {
        Qualifier::Exact | Qualifier::Interpreted if value.start.day.is_some() => "exact",
        Qualifier::Exact | Qualifier::Interpreted if value.start.month.is_some() => "month",
        Qualifier::Exact | Qualifier::Interpreted => "year",
        Qualifier::About | Qualifier::Estimated | Qualifier::Calculated => "about",
        Qualifier::Before | Qualifier::To => "before",
        Qualifier::After | Qualifier::From => "after",
        Qualifier::Between | Qualifier::FromTo => "range",
    };
    // "12 marca 1878 (wt.)": the weekday helps to check a date copied from a record (worked out for the Gregorian
    // calendar only).
    let weekday = match (value.start.day, value.start.month) {
        (Some(d), Some(m)) if qualifier == "exact" && value.calendar == Calendar::Gregorian && (1..=12).contains(&m) => {
            let (y, m, d) = (i64::from(value.start.year), i64::from(m), i64::from(d));
            let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
            let y = if m < 3 { y - 1 } else { y };
            let dow = (y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d).rem_euclid(7);
            Some(["niedz.", "pon.", "wt.", "śr.", "czw.", "pt.", "sob."][dow as usize])
        }
        _ => None,
    };
    let long = polish::format_date(&value);
    json!({
        "qualifier": qualifier,
        "gedcom": gedcom,
        "text": match weekday { Some(w) => format!("{long} ({w})"), None => long },
        "short": polish::format_date_short(&value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_feedback_reads_polish_input() {
        let exact = date_feedback("12.03.1878");
        assert_eq!((exact["qualifier"].as_str(), exact["text"].as_str()), (Some("exact"), Some("12 marca 1878 (wt.)")));
        assert_eq!(date_feedback("ok. 14.03.1878")["qualifier"], "about");
        assert_eq!(date_feedback("między 1850 a 1855")["text"], "między 1850 a 1855");
        assert_eq!(date_feedback("zimą 1915")["qualifier"], "text");
    }

    #[test]
    fn the_last_place_is_remembered_per_archive_on_this_computer() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let mut api = Api::new(Some(config.clone()), None);
        let a = dir.path().join("a");
        let id = api.call("archive.create", json!({ "folder": a.to_str().unwrap(), "name": "A" })).unwrap()["archiveId"].as_str().unwrap().to_string();
        assert!(api.call("app.state", Value::Null).unwrap()["place"].is_null());
        assert_eq!(api.call("app.setAppearance", json!({ "startIn": "elsewhere" })).unwrap_err().code, "bad_args");
        api.call("app.setAppearance", json!({ "startIn": "last" })).unwrap();
        let route = json!({ "name": "tree", "view": "family", "person": "@I1@", "cam": { "zoom": 0.5, "x": 10.0, "y": -20.0 } });
        api.call("app.setPlace", json!({ "archiveId": id, "route": route, "tree": route, "viewed": ["@I1@"] })).unwrap();
        api.call("app.setPlace", json!({ "archiveId": id, "route": { "name": "people" } })).unwrap();

        // After a restart: the choice and the place are back, and nothing was written into the archive folder.
        let mut api = Api::new(Some(config), None);
        api.call("archive.open", json!({ "path": a.to_str().unwrap() })).unwrap();
        let state = api.call("app.state", Value::Null).unwrap();
        assert_eq!(state["appearance"]["startIn"], "last");
        assert_eq!((&state["place"]["route"], &state["place"]["tree"], &state["place"]["viewed"]), (&json!({ "name": "people" }), &route, &json!(["@I1@"])));
        let settings = std::fs::read_to_string(a.join(".heirloom").join("ustawienia.json")).unwrap();
        assert!(!settings.contains("people") && !settings.contains("startPerson"));
    }

    #[test]
    fn taking_back_an_import_leaves_an_earlier_one_of_the_same_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        let json = import::check::extract_blocks(include_str!("../../../docs/IMPORT_FORMAT.md"), "doc").into_iter().next().unwrap().text;
        let answer = json!({ "texts": [format!("```json\n{json}\n```")] });
        api.call("import.load", answer.clone()).unwrap();
        let first = api.call("import.commit", json!({ "author": "Ewa" })).unwrap();
        // The same answer imported again, its people added as new ones.
        let state = api.call("import.load", answer).unwrap();
        for p in state["persons"].as_array().unwrap() {
            api.call("import.decide", json!({ "person": p["id"], "kind": "new" })).unwrap();
        }
        let second = api.call("import.commit", json!({ "author": "Ewa" })).unwrap();
        assert_ne!(first["batch"], second["batch"], "each import is taken back on its own");
        let status = api.call("history.undo", json!({ "batch": second["batch"] })).unwrap();
        assert_eq!(status["people"], 4, "the first import's people stay");
        // „N importów · każdy można cofnąć” counts only the imports still there.
        let history = api.call("import.history", Value::Null).unwrap();
        let active: Vec<(&str, bool)> = history.as_array().unwrap().iter().map(|h| (h["name"].as_str().unwrap(), h["active"].as_bool().unwrap())).collect();
        assert_eq!(active, [(second["batch"].as_str().unwrap(), false), (first["batch"].as_str().unwrap(), true)]);
        let undone: Vec<bool> = history.as_array().unwrap().iter().map(|h| h["undone"].as_bool().unwrap()).collect();
        assert_eq!(undone, [true, false]);
    }

    #[test]
    fn an_import_in_progress_goes_on_in_the_new_archive() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("od-programu");
        std::fs::create_dir_all(&old).unwrap();
        let ged = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n0 TRLR\n";
        std::fs::write(old.join("drzewo.ged"), ged).unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", json!({ "path": old.join("drzewo.ged").to_str().unwrap() })).unwrap();
        let json = import::check::extract_blocks(include_str!("../../../docs/IMPORT_FORMAT.md"), "doc").into_iter().next().unwrap().text;
        let state = api.call("import.load", json!({ "texts": [format!("```json\n{json}\n```")] })).unwrap();
        for p in state["persons"].as_array().unwrap() {
            api.call("import.decide", json!({ "person": p["id"], "kind": "new" })).unwrap();
        }
        // „Zapisz import” saves the earlier edits first, and that first save into another program's file can make a
        // new archive: the same family goes on there, and so does the import.
        api.call("archive.saveAsNew", json!({ "folder": old.join("Nowakowie").to_str().unwrap() })).unwrap();
        api.call("import.commit", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(api.call("archive.status", Value::Null).unwrap()["people"], 5);
    }

    /// A file held by another program isn't copied, and its link keeps pointing at the file where it is.
    #[cfg(windows)]
    #[test]
    fn a_file_that_cannot_be_copied_keeps_its_link() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("od-programu");
        std::fs::create_dir_all(&old).unwrap();
        let photo = old.join("jan.jpg");
        std::fs::write(&photo, b"jpg").unwrap();
        let ged = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 OBJE\n2 FILE jan.jpg\n0 TRLR\n";
        std::fs::write(old.join("drzewo.ged"), ged).unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", json!({ "path": old.join("drzewo.ged").to_str().unwrap() })).unwrap();
        let _held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&photo).unwrap();
        let new = old.join("Nowakowie");
        let result = api.call("archive.saveAsNew", json!({ "folder": new.to_str().unwrap() })).unwrap();
        assert_eq!((result["copied"].as_u64(), result["failed"].as_u64()), (Some(0), Some(1)));
        let saved = std::fs::read_to_string(new.join("rodzina.ged")).unwrap();
        assert!(saved.contains("file:///") && saved.contains("od-programu/jan.jpg"), "the link points at the file where it is");
    }

    /// A save whose history couldn't be written has nothing that names it, so „Cofnij zapis” isn't offered for it.
    #[cfg(windows)]
    #[test]
    fn a_save_without_its_history_offers_no_undo() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("a");
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": folder.to_str().unwrap(), "name": "A" })).unwrap();
        api.call("person.create", json!({ "given": "Jan" })).unwrap();
        api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        api.call("person.create", json!({ "given": "Anna" })).unwrap();
        let _held = std::fs::OpenOptions::new().read(true).share_mode(0).open(folder.join(".heirloom").join("historia.jsonl")).unwrap();
        let saved = api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        assert!(saved["history"].is_null() && saved["historyError"].is_string(), "{saved}");
    }

    #[test]
    fn the_first_person_of_a_new_archive_is_one_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        api.call("archive.setSettings", json!({ "editors": ["Ewa"] })).unwrap();
        // The first edit also declares Heirloom's extension tags in the file's header: bookkeeping, not a change.
        api.call("person.create", json!({ "given": "Józef" })).unwrap();
        assert_eq!(api.call("archive.status", Value::Null).unwrap()["unsavedChanges"], 1);
        api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(api.call("archive.editors", Value::Null).unwrap()[0]["changes"], 1);
    }

    #[test]
    fn a_save_can_be_taken_back_as_a_whole() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        let jan = api.call("person.create", json!({ "given": "Jan", "surname": "Nowak" })).unwrap()["id"].as_str().unwrap().to_string();
        let first = api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(first["history"]["author"], "Ewa");
        api.call("person.update", json!({ "id": jan, "given": "Jan Paweł", "surname": "Nowak", "events": { "birth": { "date": "1901", "certainty": "low" } } })).unwrap();
        api.call("person.create", json!({ "given": "Anna", "surname": "Nowak" })).unwrap();
        let second = api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        assert_ne!(first["history"]["ts"], second["history"]["ts"], "each save has its own time");
        let edit = api.call("person.editData", json!({ "id": jan })).unwrap();
        assert_eq!(edit["events"]["birth"]["certainty"], "low", "the certainty chosen in the profile is kept");
        // „Cofnij zapis”: the second save comes back as unsaved changes, the first stays.
        let status = api.call("history.undo", json!(second["history"])).unwrap();
        assert_eq!((status["people"].as_u64(), status["unsavedChanges"].as_u64(), status["undoSkipped"].as_u64()), (Some(1), Some(2), Some(0)));
        assert_eq!(api.call("person.get", json!({ "id": jan })).unwrap()["person"]["name"], "Jan Nowak");
    }

    #[test]
    fn taking_back_a_save_leaves_what_was_changed_after_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        let jan = api.call("person.create", json!({ "given": "Jan", "surname": "Nowak" })).unwrap()["id"].as_str().unwrap().to_string();
        let anna = api.call("person.create", json!({ "given": "Anna", "surname": "Nowak" })).unwrap()["id"].as_str().unwrap().to_string();
        api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        api.call("person.update", json!({ "id": jan, "given": "Jan Paweł", "surname": "Nowak" })).unwrap();
        api.call("person.update", json!({ "id": anna, "given": "Anna Maria", "surname": "Nowak" })).unwrap();
        let saved = api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        // Anna is changed again later (another save): „Cofnij zapis” of the earlier save must not undo that.
        api.call("person.update", json!({ "id": anna, "given": "Anna Zofia", "surname": "Nowak" })).unwrap();
        api.call("archive.save", json!({ "author": "Adam" })).unwrap();
        let status = api.call("history.undo", saved["history"].clone()).unwrap();
        assert_eq!(status["undoSkipped"], 1);
        assert_eq!(api.call("person.get", json!({ "id": jan })).unwrap()["person"]["name"], "Jan Nowak", "Jan goes back");
        assert_eq!(api.call("person.get", json!({ "id": anna })).unwrap()["person"]["name"], "Anna Zofia Nowak", "Anna keeps the later change");
    }

    #[test]
    fn certainty_alone_adds_no_event_and_clearing_it_takes_the_mark_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        let id = api.call("person.create", json!({ "given": "Jan", "surname": "Nowak" })).unwrap()["id"].as_str().unwrap().to_string();
        api.call("person.update", json!({ "id": id, "events": { "death": { "certainty": "high" } } })).unwrap();
        assert!(api.call("person.editData", json!({ "id": id })).unwrap()["events"]["death"].is_null(), "no death without a date or place");
        api.call("person.update", json!({ "id": id, "events": { "birth": { "date": "1901", "place": "Lublin", "certainty": "medium" } } })).unwrap();
        api.call("person.update", json!({ "id": id, "events": { "birth": { "certainty": "" } } })).unwrap();
        let birth = &api.call("person.editData", json!({ "id": id })).unwrap()["events"]["birth"];
        assert_eq!((birth["date"].as_str(), birth["certainty"].as_str()), (Some("1901"), None));
        // Emptied date and place: the event goes, its mark too.
        api.call("person.update", json!({ "id": id, "events": { "birth": { "date": "", "place": "", "certainty": "high" } } })).unwrap();
        assert!(api.call("person.editData", json!({ "id": id })).unwrap()["events"]["birth"].is_null());
    }

    #[test]
    fn a_file_from_another_program_can_become_a_new_archive() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("od-programu");
        std::fs::create_dir_all(old.join("zdjecia")).unwrap();
        std::fs::write(old.join("zdjecia").join("jan.jpg"), b"jpg").unwrap();
        let outside = dir.path().join("gdzie-indziej.jpg");
        std::fs::write(&outside, b"jpg2").unwrap();
        let ged = format!(
            "0 HEAD
1 SOUR MYHERITAGE
1 GEDC
2 VERS 5.5.1
1 CHAR UTF-8
0 @I1@ INDI
1 NAME Jan /Nowak/
1 OBJE
2 FILE zdjecia/jan.jpg
1 OBJE
2 FILE {}
1 OBJE
2 FILE rodzina.ged
0 TRLR
",
            outside.display()
        );
        std::fs::write(old.join("drzewo.ged"), ged).unwrap();
        // A stray file with the name the new archive's data file will have.
        std::fs::write(old.join("rodzina.ged"), b"stray").unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.open", json!({ "path": old.join("drzewo.ged").to_str().unwrap() })).unwrap();
        let id = api.call("people.list", json!({})).unwrap()["people"][0]["id"].as_str().unwrap().to_string();
        api.call("person.update", json!({ "id": id, "given": "Jan Paweł", "surname": "Nowak" })).unwrap();
        let same = api.call("archive.saveAsNew", json!({ "folder": old.to_str().unwrap() }));
        assert_eq!(same.unwrap_err().code, "inside_archive");
        // A folder next to the file (Pobrane\Nowakowie): allowed.
        let new = old.join("Nowakowie");
        let result = api.call("archive.saveAsNew", json!({ "folder": new.to_str().unwrap() })).unwrap();
        assert_eq!((result["copied"].as_u64(), result["failed"].as_u64(), result["missing"].as_u64()), (Some(3), Some(0), Some(0)));
        let status = &result["status"];
        assert_eq!((status["foreign"].as_bool(), status["unsavedChanges"].as_u64(), status["name"].as_str()), (Some(false), Some(0), Some("Nowakowie")), "named after the chosen folder");
        assert!(new.join("zdjecia").join("jan.jpg").is_file() && new.join("media").join("gdzie-indziej.jpg").is_file());
        assert_eq!(std::fs::read(new.join("media").join("rodzina.ged")).unwrap(), b"stray", "the stray file goes to media/, not over the data");
        let text = std::fs::read_to_string(new.join("rodzina.ged")).unwrap();
        assert!(text.contains("Jan Paweł") && text.contains("2 VERS 7.0") && text.contains("FILE media/gdzie-indziej.jpg"), "{text}");
        assert!(std::fs::read_to_string(old.join("drzewo.ged")).unwrap().contains("1 NAME Jan /Nowak/"), "the other program's file is untouched");
        let full = dir.path().join("pelny");
        std::fs::create_dir_all(&full).unwrap();
        std::fs::write(full.join("cos.txt"), b"x").unwrap();
        let taken = api.call("archive.saveAsNew", json!({ "folder": full.to_str().unwrap() }));
        assert_eq!(taken.unwrap_err().code, "not_empty");
    }

    #[test]
    fn date_feedback_gives_weekdays_only_for_gregorian_dates() {
        // The 13th month of the French calendar used to index past the weekday table.
        assert_eq!(date_feedback("FRENCH_R 3 COMP 2")["qualifier"], "exact");
        assert_eq!(date_feedback("JULIAN 12 MAR 1878")["text"], "12 marca 1878 (kal. juliański)", "a Gregorian weekday would be wrong");
    }
}
