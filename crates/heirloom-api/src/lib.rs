//! The app's commands, shared by the Tauri window (`src-tauri`) and the browser bridge (`heirloom-bridge`).
//! Every command takes JSON and returns JSON, so the UI's `api.ts` works the same over both.

pub mod activity;
pub mod backup;
pub mod combined;
pub mod config;
pub mod edit;
pub mod gedwrite;
pub mod import;
pub mod media_edit;
pub mod derive;
pub mod fsx;
pub mod kin;
pub mod layout;
pub mod lists;
pub mod tree;
pub mod media;
pub mod overlay;
pub mod people;
pub mod progress;
pub mod running;
pub mod text;
pub(crate) mod tools;
pub mod update;

use config::{ACCENT_COLOURS, AppConfig, COLOUR_SCHEMES, IN_PLACE, OVERLAY, RecentArchive, data_key};
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

/// "1,2 GB", "48 MB", "350 kB".
pub fn count_bytes(n: u64) -> String {
    let (value, unit) = match n {
        n if n >= 1_000_000_000 => (n as f64 / 1e9, "GB"),
        n if n >= 1_000_000 => (n as f64 / 1e6, "MB"),
        n => (n as f64 / 1e3, "kB"),
    };
    let text = if value >= 10.0 { format!("{value:.0}") } else { format!("{value:.1}") };
    format!("{} {unit}", text.replace('.', ","))
}

/// The open archive and what is worked out from it.
pub struct Session {
    /// In „Nie ruszaj oryginału” the folder of the user's changes (a normal archive folder), never the original.
    pub archive: Archive,
    /// Rebuilt lazily after every change.
    pub(crate) derived: Option<Derived>,
    /// The display settings it was built with; changing them (name order, date format, surname joins) rebuilds it.
    display_seen: Option<serde_json::Map<String, Value>>,
    pub last_saved: Option<String>,
    /// How this person works with the archive on this computer: [`IN_PLACE`], [`OVERLAY`], or None until asked.
    pub work_mode: Option<&'static str>,
    /// „Nie ruszaj oryginału”: the original archive behind `archive`.
    pub overlay: Option<overlay::OverlayInfo>,
    /// The original's path as it was opened (the folder or the `.ged`).
    pub opened: String,
    /// Heirloom saved into it before, or it was in the recent list before this open („Do tej pory…”).
    used_before: bool,
    /// Files someone moved in Explorer, found by the last `media.scan`: served after the folders miss (Part B).
    pub(crate) moved: media::Moved,
    /// A „Uporządkuj” run that was cut off (`{ journal, done, total }`), found at open and after each run.
    pub(crate) layout_interrupted: Option<Value>,
}

impl Session {
    pub(crate) fn new(archive: Archive) -> Session {
        let layout_interrupted = layout::engine::interrupted(archive.sidecar_path());
        Session { archive, derived: None, display_seen: None, last_saved: None, work_mode: None, overlay: None, opened: String::new(), used_before: false, moved: media::Moved::default(), layout_interrupted }
    }

    /// Where the archive's files are, the one written into first: the folder of the user's changes and the original,
    /// or the archive folder alone.
    pub fn layers(&self) -> Vec<PathBuf> {
        let root = self.archive.root().to_path_buf();
        match &self.overlay {
            Some(info) => vec![root, info.original_root.clone()],
            None => vec![root],
        }
    }

    pub fn file_roots(&self) -> media_edit::FileRoots {
        media_edit::FileRoots::Layers(self.layers(), self.moved.clone())
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
        if let Some(info) = &self.overlay {
            info.forget_pending();
        }
    }

    pub fn history(&self) -> Vec<heirloom_core::history::Entry> {
        heirloom_core::history::read(&self.archive.history_path()).unwrap_or_default()
    }
}

/// Commands that write into the archive (or prepare a write) and wait for the first-open question.
const GUARDED: &[&str] = &["archive.save", "archive.saveAsCopy", "archive.setSettings", "settings.import", "import.commit", "history.undo"];

pub struct Api {
    config_dir: Option<PathBuf>,
    cache_root: Option<PathBuf>,
    config: AppConfig,
    session: Option<Session>,
    /// The import being reviewed (steps 1–5); dropped when another archive is opened.
    import: Option<import::Draft>,
    /// Where file URLs are served from; shared with the media handler, which runs outside the command lock.
    media: media::MediaRoots,
    /// Archives opened together („Otwórz razem…”), read-only; never open at the same time as `session`.
    combined: Option<combined::Combined>,
    /// What this window was started with (`--open <path> [--person <xref>]`), given to the interface once.
    start: Option<(String, Option<String>)>,
    /// How far a long job has got; read without the command lock (`job.progress`).
    progress: progress::Progress,
    /// Tests and the browser bridge: a folder standing in for the user's home (`Dokumenty`, `AppData\Local`).
    home: Option<PathBuf>,
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
        Api { config_dir, cache_root, config, session: None, import: None, media: media::MediaRoots::default(), combined: None, start: None, progress: progress::Progress::default(), home: None }
    }

    /// The handle the window reads long jobs' progress through, outside the command lock.
    pub fn progress(&self) -> progress::Progress {
        self.progress.clone()
    }

    /// Tests and the browser bridge: `<home>/Dokumenty` and `<home>/Lokalne` stand for the user's folders.
    #[doc(hidden)]
    pub fn set_home(&mut self, home: PathBuf) {
        self.home = Some(home);
    }

    /// The user's Documents folder (wherever Windows keeps it: redirected, in OneDrive).
    fn documents(&self) -> Option<PathBuf> {
        match &self.home {
            Some(home) => Some(home.join("Dokumenty")),
            None => dirs::document_dir(),
        }
    }

    fn local_dir(&self) -> Option<PathBuf> {
        match &self.home {
            Some(home) => Some(home.join("Lokalne")),
            None => dirs::data_local_dir(),
        }
    }

    /// Where new folders of „Twoje zmiany” go.
    fn changes_parent(&self) -> PathBuf {
        if let Some(folder) = &self.config.changes_folder {
            return PathBuf::from(folder);
        }
        self.default_changes_parent().unwrap_or_else(|| PathBuf::from("Moje zmiany"))
    }

    fn default_changes_parent(&self) -> Option<PathBuf> {
        self.documents().or_else(|| self.local_dir()).map(|d| d.join("Heirloom").join("Moje zmiany"))
    }

    /// Where „Twoje zmiany” go when Documents can't be written (e.g. Controlled Folder Access).
    fn local_changes_parent(&self) -> Option<PathBuf> {
        self.local_dir().map(|d| d.join("Heirloom").join("Moje zmiany"))
    }

    /// Where the automatic backups go.
    fn backup_parent(&self) -> PathBuf {
        if let Some(folder) = &self.config.backup_folder {
            return PathBuf::from(folder);
        }
        self.documents().or_else(|| self.local_dir()).map_or_else(|| PathBuf::from("Kopie zapasowe"), |d| d.join("Heirloom").join("Kopie zapasowe"))
    }

    /// Archives worked on in „Nie ruszaj oryginału” here: data key → folder of the changes (for archives opened together).
    fn overlays(&self) -> std::collections::HashMap<String, PathBuf> {
        self.config
            .archives
            .iter()
            .filter(|(_, u)| u.mode == OVERLAY)
            .filter_map(|(k, u)| Some((k.clone(), PathBuf::from(u.overlay.as_ref()?))))
            .filter(|(_, folder)| overlay::read_manifest(folder).is_some_and(|m| !m.retired))
            .collect()
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

    /// What the read screens show: the open archive, or the archives opened together.
    fn view(&mut self) -> Result<&Derived, ApiError> {
        if self.combined.is_some() {
            return Ok(self.combined.as_mut().expect("checked").derived());
        }
        Ok(self.session()?.derived())
    }

    /// A person's id from the interface as this view knows them: in archives opened together a linked person's
    /// other records' ids (from before the link) lead to the person.
    fn canonical(&self, id: String) -> String {
        match &self.combined {
            Some(c) => c.canonical(&id),
            None => id,
        }
    }

    /// `--open <path> [--person <xref>]` from the command line: `app.takeStart` gives it to the interface once.
    pub fn set_start(&mut self, path: String, person: Option<String>) {
        self.start = Some((path, person));
    }

    /// Opens an archive (folder or GEDCOM file) or a set of archives (`.heirloom-zestaw`).
    pub fn open_path(&mut self, path: &str) -> ApiResult {
        let method = if combined::is_set_path(path) { "set.open" } else { "archive.open" };
        self.call(method, json!({ "path": path }))
    }

    /// Runs one command. `args` is a JSON object (or null when the command takes nothing).
    pub fn call(&mut self, method: &str, args: Value) -> ApiResult {
        // Read without the lock in the window (src-tauri); here for the browser bridge.
        if method == "job.progress" || method == "job.cancel" {
            return Ok(self.progress.call(method));
        }
        // Archives opened together are only for browsing (decision 1a): nothing that could change an archive runs.
        if self.combined.is_some() && !combined::allowed(method) {
            return Err(combined::read_only());
        }
        // Until the user chose how to work with the archive (the question on first open), nothing is written into it.
        if GUARDED.contains(&method) {
            self.mode_chosen()?;
        }
        match method {
            "app.state" => Ok(self.app_state()),
            "app.takeStart" => Ok(self.start.take().map_or(Value::Null, |(path, person)| json!({ "path": path, "person": person }))),
            "set.create" => {
                let archives: Vec<String> = args.get("archives").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                let replace = args.get("replace").and_then(Value::as_bool).unwrap_or(false);
                let path = str_arg(&args, "path")?;
                // The set file never goes into an archive worked on in „Nie ruszaj oryginału”.
                if self.inside_untouched_original(Path::new(&path)) {
                    return Err(ApiError::new("inside_archive", "Wybierz miejsce poza folderem archiwum, którego nie zmieniasz („Nie ruszaj oryginału”)."));
                }
                let set = combined::Combined::create(Path::new(&path), &str_arg(&args, "name")?, &archives, replace, self.overlays())?;
                self.install_set(set)
            }
            "set.open" => {
                let set = combined::Combined::open(Path::new(&str_arg(&args, "path")?), self.overlays())?;
                self.install_set(set)
            }
            m if m.starts_with("set.") => {
                let c = self.combined.as_mut().ok_or_else(combined::no_set)?;
                let result = c.call(m, &args);
                let roots = c.media_roots(self.cache_root.as_deref());
                self.media.set_many(roots);
                result
            }
            "app.setAppearance" => self.set_appearance(&args),
            "app.setPlace" => {
                // The archive is named by the UI: a place sent late must not land in an archive opened since.
                let archive_id = str_arg(&args, "archiveId")?;
                let part = |key: &str| args.get(key).filter(|v| !v.is_null()).cloned();
                let viewed = args.get("viewed").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
                let (route, tree) = (part("route"), part("tree"));
                self.update_config(|c| c.set_place(&archive_id, route, tree, viewed, heirloom_core::history::now()));
                Ok(Value::Null)
            }
            "app.setUpdates" => {
                let check = args.get("check").and_then(Value::as_bool).ok_or_else(|| ApiError::bad_args("check"))?;
                self.update_config(|c| c.updates.check = check);
                Ok(self.app_state())
            }
            "recent.setSort" => {
                let sort = str_arg(&args, "sort")?;
                if !["opened", "edited", "name"].contains(&sort.as_str()) {
                    return Err(ApiError::bad_args("sort"));
                }
                self.update_config(|c| c.recent_sort = Some(sort));
                Ok(self.app_state())
            }
            "recent.forget" => {
                let path = str_arg(&args, "path")?;
                self.update_config(|c| c.forget(&path));
                Ok(self.app_state())
            }
            "archive.open" => self.open_archive(&str_arg(&args, "path")?),
            "archive.create" => self.create_archive(&str_arg(&args, "folder")?, &str_arg(&args, "name")?),
            "archive.close" => {
                self.session = None;
                self.combined = None;
                self.import = None;
                self.media.set(None);
                Ok(Value::Null)
            }
            "archive.status" => self.archive_status(),
            "archive.save" => self.save(&args),
            "archive.reload" if self.combined.is_some() => {
                self.call("set.refresh", Value::Null)?;
                self.archive_status()
            }
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
            "archive.modeInfo" => self.mode_info(),
            "archive.setWorkMode" => self.set_work_mode(&args),
            "archive.overlayInfo" => self.overlay_info(),
            "archive.writeBackPreview" => overlay::writeback::preview(self.session()?),
            "archive.writeBack" => {
                let author = str_arg(&args, "author")?;
                let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
                let progress = self.progress.clone();
                let result = overlay::writeback::write_back(self.session()?, &author, allow_foreign, &progress)?;
                self.update_config(|c| c.set_editor(author));
                self.touch_edited();
                Ok(json!({ "result": result, "status": self.archive_status()? }))
            }
            "archive.undoWriteBack" => {
                overlay::writeback::undo(self.session()?)?;
                self.archive_status()
            }
            "archive.rebase" => {
                let choices = args.get("choices").and_then(Value::as_object).map(|c| {
                    c.iter()
                        .map(|(k, v)| (k.clone(), if v.as_str() == Some("theirs") { overlay::merge::Side::Theirs } else { overlay::merge::Side::Mine }))
                        .collect()
                });
                // `preview`: only says what a merge would do (the conflicts, the counts); nothing changes.
                let preview = args.get("preview").and_then(Value::as_bool).unwrap_or(false);
                let s = self.session()?;
                let result = overlay::writeback::rebase(s, choices, preview)?;
                Ok(json!({ "result": result, "status": self.archive_status()? }))
            }
            "archive.discardOverlay" => self.discard_overlay(),
            "archive.relocateOriginal" => self.relocate_original(&str_arg(&args, "path")?, args.get("force").and_then(Value::as_bool).unwrap_or(false)),
            "archive.backupInfo" | "archive.backupAuto" | "archive.restore" | "app.setBackupFolder" => backup::call(self, method, &args),
            "archive.setEditor" => {
                // Remembered for "Kto edytuje?" next time; the name is added to the archive's list on the first save.
                let name = str_arg(&args, "name")?;
                self.update_config(|c| c.set_editor(name));
                Ok(Value::Null)
            }
            "people.list" => Ok(people::list(self.view()?)),
            "people.search" => {
                let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
                let query = str_arg(&args, "q")?;
                Ok(people::search(self.view()?, &query, limit))
            }
            "person.get" if self.combined.is_some() => {
                let id = self.canonical(str_arg(&args, "id")?);
                let c = self.combined.as_mut().expect("checked");
                // No history here: each archive keeps its own.
                let mut profile = people::profile(c.derived(), &id, &[])?;
                profile["brokenLinks"] = people::broken_links(c.doc(), &id);
                profile["combined"] = c.person_info(&id);
                // The UID as the archive has it (the merged document puts the archive's key in front).
                if let Some(uid) = profile["uid"].as_str().and_then(|u| u.split_once('~')).map(|(_, u)| u.to_string()) {
                    profile["uid"] = json!(uid);
                }
                Ok(profile)
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
                let id = self.canonical(str_arg(&args, "id")?);
                people::panel(self.view()?, &id)
            }
            "person.relations" => {
                let id = self.canonical(str_arg(&args, "id")?);
                people::relations(self.view()?, &id)
            }
            "date.parse" => Ok(date_feedback(&str_arg(&args, "text")?)),
            "person.editData" => {
                let id = str_arg(&args, "id")?;
                people::edit_data(self.session()?.derived(), &id)
            }
            "person.hover" => {
                let id = self.canonical(str_arg(&args, "id")?);
                let from = opt_str(&args, "from").map(|f| self.canonical(f));
                people::hover(self.view()?, &id, from.as_deref())
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
                if m == "import.commit" && result.is_ok() {
                    self.touch_edited();
                }
                self.media.set_imports(self.import.as_ref().map(|d| d.files.iter().map(|f| f.path.clone()).collect()).unwrap_or_default());
                result
            }
            "tree.graph" => {
                let id = self.canonical(str_arg(&args, "id")?);
                let up = args.get("up").and_then(Value::as_u64).unwrap_or(2) as usize;
                let down = args.get("down").and_then(Value::as_u64).unwrap_or(2) as usize;
                let expand: Vec<String> = args.get("expand").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|x| self.canonical(x.to_string())).collect()).unwrap_or_default();
                // Rodzina's „+N” pill: [{id, depth}] unfolds that person's descendants `depth` generations down (null: all).
                let unfold: Vec<tree::Unfold> = args
                    .get("unfold")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|u| Some((self.canonical(u.get("id")?.as_str()?.to_string()), u.get("depth").and_then(Value::as_u64).map(|n| n as usize)))).collect())
                    .unwrap_or_default();
                // Przodkowie asks for the whole line (spec §4.2), so `up` may go further than `down`.
                tree::graph(self.view()?, &id, up.min(64), down.min(12), &expand, &unfold)
            }
            "tree.overview" => {
                let focus = opt_str(&args, "focus").map(|f| self.canonical(f));
                Ok(tree::overview(self.view()?, focus.as_deref()))
            }
            "start.data" if self.combined.is_some() => {
                let c = self.combined.as_mut().expect("checked");
                let title = c.set.name.clone();
                Ok(lists::start(c.derived(), &[], &title))
            }
            "start.data" => {
                let s = self.session()?;
                let history = s.history();
                let title = s.archive.settings().name.clone();
                Ok(lists::start(s.derived(), &history, &title))
            }
            "archive.firstOpen" => {
                let s = self.session()?;
                let roots = s.file_roots();
                Ok(lists::first_open(s.derived(), &roots))
            }
            "surnames.list" => Ok(lists::surnames(self.view()?)),
            "surname.get" => {
                let key = str_arg(&args, "key")?;
                lists::surname(self.view()?, &key)
            }
            "places.list" => Ok(lists::places(self.view()?)),
            "place.get" => {
                let path: Vec<String> = args.get("path").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
                lists::place(self.view()?, &path)
            }
            "stories.list" => Ok(lists::stories(self.view()?)),
            "story.get" => {
                let id = str_arg(&args, "id")?;
                lists::story(self.view()?, &id)
            }
            // Own file categories: the archive's list and the names found on files (archives opened together have only
            // the latter).
            "media.categories" => {
                let saved = match &self.combined {
                    Some(_) => Vec::new(),
                    None => lists::saved_categories(&self.session()?.archive.settings().display),
                };
                Ok(lists::media_categories(self.view()?, &saved))
            }
            "media.list" | "media.get" | "media.missing" | "media.findMissing" => {
                let roots = match &self.combined {
                    Some(c) => c.file_roots(),
                    None => self.session()?.file_roots(),
                };
                let d = self.view()?;
                match method {
                    "media.list" => Ok(lists::media_list(d, &roots)),
                    "media.get" => lists::media_get(d, &str_arg(&args, "id")?, &roots),
                    "media.missing" => Ok(lists::missing_list(d, &roots)),
                    _ => Ok(lists::find_missing(d, &roots, Path::new(&str_arg(&args, "folder")?))),
                }
            }
            // Part B: where the photos lie in media/. The scan and the preview only read.
            "media.scan" => {
                let s = self.session()?;
                let mut report = layout::run_scan(s);
                let key = match &s.overlay {
                    Some(info) => info.manifest.original.data_path.to_lowercase(),
                    None => data_key(&s.archive.data_path()),
                };
                // „Nie teraz” was said to exactly these findings (on this computer).
                let seen = self.config.archives.get(&key).and_then(|u| u.tidy_seen.as_ref());
                report["seen"] = json!(seen.is_some_and(|f| report["fingerprint"].as_str() == Some(f.as_str())));
                Ok(report)
            }
            "media.restructurePreview" => layout::engine::preview(self.session()?, &args),
            "media.fixMoved" | "media.assign" | "media.leaveAlone" | "media.setLayout" | "media.restructure" | "media.finishRestructure" | "media.undoRestructure" | "media.tidySeen" => {
                self.mode_chosen()?;
                self.layout_command(method, &args)
            }
            "sources.list" => Ok(lists::sources(self.view()?)),
            "source.get" => {
                let id = str_arg(&args, "id")?;
                lists::source(self.view()?, &id)
            }
            m if ["person.", "relation.", "family.", "fact.", "text.", "media.", "source.", "citation.", "place."].iter().any(|p| m.starts_with(p)) => {
                // media.add copies files into the archive at once, before any save.
                if !m.ends_with("Preview") {
                    self.mode_chosen()?;
                }
                edit::call(self.session()?, m, &args)
            }
            _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
        }
    }

    /// The Part B commands that change something (the files in `media/`, or what the data says about them).
    fn layout_command(&mut self, method: &str, args: &Value) -> ApiResult {
        if method == "media.tidySeen" {
            // „Nie teraz”: the banner stays away until the findings change (per computer).
            let fingerprint = str_arg(args, "fingerprint")?;
            let s = self.session()?;
            let key = match &s.overlay {
                Some(info) => info.manifest.original.data_path.to_lowercase(),
                None => data_key(&s.archive.data_path()),
            };
            self.update_config(|c| {
                if let Some(record) = c.archives.get_mut(&key) {
                    record.tidy_seen = Some(fingerprint);
                }
            });
            return Ok(Value::Null);
        }
        let progress = self.progress.clone();
        let s = self.session()?;
        let moves_files = matches!(method, "media.restructure" | "media.finishRestructure" | "media.undoRestructure");
        let mut result = match method {
            "media.fixMoved" => layout::fix_moved(s)?,
            "media.assign" => layout::assign(s, args)?,
            "media.leaveAlone" => layout::leave_alone(s, args)?,
            "media.setLayout" => layout::set_layout(s, args)?,
            "media.restructure" => layout::engine::restructure(s, &progress, args)?,
            "media.finishRestructure" => layout::engine::finish(s, &progress, args)?,
            _ => layout::engine::undo(s, &progress, args)?,
        };
        if moves_files {
            // The data file was saved: the registry's hash follows (A.2), and the moved map is looked up again.
            let author = str_arg(args, "author")?;
            let record = s.overlay.is_none().then(|| (data_key(&s.archive.data_path()), s.archive.fingerprint_hex()));
            result["report"] = layout::run_scan(s);
            self.update_config(|c| {
                c.set_editor(author);
                if let Some((u, hash)) = record.and_then(|(key, hash)| c.archives.get_mut(&key).filter(|u| u.mode == IN_PLACE).map(|u| (u, hash))) {
                    u.hash = Some(hash);
                }
            });
            self.touch_edited();
            result["status"] = self.archive_status()?;
        }
        Ok(result)
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
        // Moving files (Part B) has its own button: undoing its lines one by one would unlink the files it moved.
        if targets.iter().any(|e| e.kind.as_deref() == Some("layout")) {
            return Err(ApiError::new("layout_entry", "Przenoszenie plików cofniesz przyciskiem „Cofnij porządkowanie”."));
        }
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
        let (archive, id) = match (&self.combined, &self.session) {
            (Some(c), _) => (Some(c.status()), Some(c.set.id.clone())),
            (None, Some(s)) => (Some(status_of(s, self.documents().as_deref())), Some(s.archive.settings().archive_id.clone())),
            (None, None) => (None, None),
        };
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "recent": self.config.recent,
            "recentSort": self.config.recent_sort,
            "appearance": self.config.appearance,
            "lastEditor": self.config.last_editor,
            "editorNames": self.config.editor_names,
            "updates": self.config.updates,
            "archive": archive,
            // Where the open archive (or set) was left on this computer.
            "place": id.and_then(|id| self.config.places.get(&id)),
        })
    }

    fn set_appearance(&mut self, args: &Value) -> ApiResult {
        // Only the choices given change, on top of what another window chose meanwhile.
        let theme = args.get("theme").and_then(Value::as_str);
        if theme.is_some_and(|t| !["system", "light", "dark"].contains(&t)) {
            return Err(ApiError::bad_args("theme"));
        }
        let text_size = args.get("textSize").and_then(Value::as_u64);
        let density = args.get("density").and_then(Value::as_str);
        if density.is_some_and(|d| !["comfortable", "compact"].contains(&d)) {
            return Err(ApiError::bad_args("density"));
        }
        let animations = args.get("animations").and_then(Value::as_bool);
        let start_in = args.get("startIn").and_then(Value::as_str);
        if start_in.is_some_and(|s| !["start", "last"].contains(&s)) {
            return Err(ApiError::bad_args("startIn"));
        }
        let scheme = args.get("scheme").and_then(Value::as_str);
        if scheme.is_some_and(|s| !COLOUR_SCHEMES.contains(&s)) {
            return Err(ApiError::bad_args("scheme"));
        }
        let accent = args.get("accent").and_then(Value::as_str);
        if accent.is_some_and(|a| !ACCENT_COLOURS.contains(&a)) {
            return Err(ApiError::bad_args("accent"));
        }
        self.update_config(|c| {
            let appearance = &mut c.appearance;
            if let Some(theme) = theme {
                appearance.theme = theme.into();
            }
            if let Some(size) = text_size {
                appearance.text_size = (size as u32).clamp(100, 150);
            }
            if let Some(density) = density {
                appearance.density = density.into();
            }
            if let Some(animations) = animations {
                appearance.animations = animations;
            }
            if let Some(start_in) = start_in {
                appearance.start_in = start_in.into();
            }
            if let Some(scheme) = scheme {
                appearance.scheme = scheme.into();
            }
            if let Some(accent) = accent {
                appearance.accent = accent.into();
            }
        });
        Ok(self.app_state())
    }

    /// Opens an archive (A.4): in „Nie ruszaj oryginału” the folder of the user's changes instead; the first time on
    /// this computer without a mode (the question follows); a folder of „Twoje zmiany” opened directly as its original.
    fn open_archive(&mut self, path: &str) -> ApiResult {
        let archive = match Archive::open(Path::new(path)) {
            Ok(archive) => archive,
            Err(e) => {
                // The original is gone (a pendrive unplugged): the user's version still opens.
                if let Some((folder, manifest)) = self.overlay_of_missing(path) {
                    let archive = Archive::open(&folder)?;
                    return self.install_overlay(archive, manifest, true, Vec::new());
                }
                return Err(e.into());
            }
        };
        if let Some(manifest) = overlay::read_manifest(archive.root()) {
            if manifest.retired {
                // Earlier, set-aside changes (renamed after a switch, a write-back or „Porzuć moje zmiany…”): opened so the
                // user can look at them, like any archive of its own. Never registered over the original's record and
                // never attached to the original again.
                self.install(archive, path)?;
                if let Some(s) = self.session.as_mut() {
                    s.work_mode = Some(IN_PLACE);
                }
                let mut status = self.archive_status()?;
                status["notice"] = json!("To są wcześniejsze, odłożone zmiany. Archiwum, z którego pochodzą, nie zostało przez to zmienione.");
                return Ok(status);
            }
            let missing = !Path::new(&manifest.original.data_path).is_file();
            return self.install_overlay(archive, manifest, missing, Vec::new());
        }
        let key = data_key(&archive.data_path());
        match self.config.archives.get(&key).cloned() {
            Some(u) if u.mode == IN_PLACE => return self.install(archive, path),
            Some(u) if u.mode == OVERLAY => {
                let found = u.overlay.as_deref().map(PathBuf::from).and_then(|f| Some((overlay::read_manifest(&f).filter(|m| !m.retired)?, f)));
                if let Some((manifest, folder)) = found {
                    let mine = Archive::open(&folder)?;
                    return self.install_overlay(mine, manifest, false, Vec::new());
                }
                // The folder of the changes is gone: asked again.
                self.update_config(|c| {
                    c.archives.remove(&key);
                });
            }
            _ => {}
        }
        if let Some((folder, manifest)) = self.reattach(&archive, path, &key) {
            let mine = Archive::open(&folder)?;
            let info = overlay::OverlayInfo::new(&folder, manifest.clone(), false);
            let pending = info.pending(&mine);
            let notice = format!(
                "Znaleziono Twoje wcześniejsze zmiany w tym archiwum ({}). Pracujesz dalej w trybie „Nie ruszaj oryginału”.",
                count_pl(pending, "zmiana", "zmiany", "zmian")
            );
            return self.install_overlay(mine, manifest, false, vec![notice]);
        }
        self.install(archive, path)
    }

    fn create_archive(&mut self, folder: &str, name: &str) -> ApiResult {
        let name = name.trim();
        if name.is_empty() {
            return Err(ApiError::new("bad_args", "Podaj nazwę archiwum."));
        }
        let archive = Archive::create(Path::new(folder), name)?;
        // Made here by Heirloom: worked on in place, nobody is asked.
        self.record_in_place(&archive, folder);
        self.install(archive, folder)
    }

    /// Remembers on this computer that this archive is worked on in place.
    pub(crate) fn record_in_place(&mut self, archive: &Archive, opened: &str) {
        self.update_config(in_place(archive, opened));
    }

    /// Makes `archive` the open one and remembers it in the recent list; its mode is what this computer recorded for
    /// it (none: the question).
    pub(crate) fn install(&mut self, archive: Archive, opened_path: &str) -> ApiResult {
        let key = data_key(&archive.data_path());
        let mode = self.config.archives.get(&key).filter(|u| u.mode == IN_PLACE).map(|_| IN_PLACE);
        if mode.is_some() {
            // The hash finds the archive again by its content when it has no settings of its own (A.4 c).
            let hash = archive.fingerprint_hex();
            if self.config.archives.get(&key).and_then(|u| u.hash.as_ref()) != Some(&hash) {
                self.update_config(|c| {
                    if let Some(record) = c.archives.get_mut(&key) {
                        record.hash = Some(hash);
                    }
                });
            }
        }
        let moved = media::Moved::default();
        self.media.set(Some(media::Roots {
            archive: archive.root().to_path_buf(),
            fallback: None,
            thumbs: self.cache_root.as_ref().map(|c| c.join(&archive.settings().archive_id).join("miniatury")),
            key: None,
            moved: moved.clone(),
        }));
        let used_before = archive.history_path().is_file() || self.config.seen(opened_path);
        let entry = RecentArchive {
            path: opened_path.to_string(),
            name: archive.settings().name.clone(),
            opened_at: heirloom_core::history::now(),
            people: archive.doc.records.iter().filter(|r| r.tag == "INDI").count(),
            kind: "archive".into(),
            archives: None,
            edited_at: None,
        };
        self.update_config(|c| c.remember(entry));
        let mut session = Session::new(archive);
        session.work_mode = mode;
        session.moved = moved;
        session.opened = opened_path.to_string();
        session.used_before = used_before;
        self.session = Some(session);
        self.combined = None;
        self.import = None;
        self.archive_status()
    }

    /// „Nie ruszaj oryginału”: the folder of the user's changes becomes the open archive, its files backed by the
    /// original's. A write-back the program didn't finish is finished or taken back, and changes made in the archive
    /// meanwhile are taken in at once when the user has none of their own.
    fn install_overlay(&mut self, archive: Archive, manifest: overlay::Manifest, original_missing: bool, mut notices: Vec<String>) -> ApiResult {
        let root = archive.root().to_path_buf();
        let original = manifest.original.clone();
        let key = original.data_path.to_lowercase();
        let base_hash = manifest.base.hash.clone();
        self.update_config(|c| {
            let record = c.archives.entry(key).or_default();
            record.mode = OVERLAY.into();
            record.overlay = Some(root.display().to_string());
            record.opened = original.opened.clone();
            record.archive_id = original.archive_id.clone();
            record.data_file = original.data_file.clone();
            record.hash = Some(base_hash);
            if record.decided_at.is_empty() {
                record.decided_at = heirloom_core::history::now();
            }
        });
        let moved = media::Moved::default();
        self.media.set(Some(media::Roots {
            archive: root.clone(),
            fallback: Some(PathBuf::from(&original.root)),
            thumbs: self.cache_root.as_ref().map(|c| c.join(&archive.settings().archive_id).join("miniatury")),
            key: None,
            moved: moved.clone(),
        }));
        // One entry in the recent list, the original's, whichever folder was opened.
        let entry = RecentArchive {
            path: original.opened.clone(),
            name: archive.settings().name.clone(),
            opened_at: heirloom_core::history::now(),
            people: archive.doc.records.iter().filter(|r| r.tag == "INDI").count(),
            kind: "archive".into(),
            archives: None,
            edited_at: None,
        };
        self.update_config(|c| c.remember(entry));
        let mut session = Session::new(archive);
        session.work_mode = Some(OVERLAY);
        session.moved = moved;
        session.opened = original.opened.clone();
        session.overlay = Some(overlay::OverlayInfo::new(&root, manifest, original_missing));
        self.session = Some(session);
        self.combined = None;
        self.import = None;
        if !original_missing {
            let s = self.session.as_mut().expect("just set");
            if let Ok(Some(text)) = overlay::writeback::recover(s, None) {
                notices.push(text);
            }
            if let Some(info) = &s.overlay {
                overlay::writeback::settle_if_stale(&root, info);
            }
            let fast_forward = s.overlay.as_ref().is_some_and(|i| i.interrupted.is_none() && i.original_changed() && i.pending(&s.archive) == 0);
            if fast_forward && overlay::writeback::rebase(s, None, false).is_ok_and(|r| r["merged"] == true) {
                notices.push("Wczytano nowe zmiany z archiwum.".into());
            }
        }
        let mut status = self.archive_status()?;
        if !notices.is_empty() {
            status["notice"] = json!(notices.join(" "));
        }
        Ok(status)
    }

    /// A path that can't be opened now, worked on here in „Nie ruszaj oryginału”: the user's version (A.4 step 2).
    fn overlay_of_missing(&self, path: &str) -> Option<(PathBuf, overlay::Manifest)> {
        let wanted = overlay::absolute(path).to_lowercase();
        self.config.archives.values().filter(|u| u.mode == OVERLAY && u.opened.to_lowercase() == wanted).find_map(|u| {
            let folder = PathBuf::from(u.overlay.as_ref()?);
            Some((folder.clone(), overlay::read_manifest(&folder).filter(|m| !m.retired)?))
        })
    }

    /// The folders of „Twoje zmiany” on this computer, in every place they may be.
    fn overlay_folders(&self) -> Vec<PathBuf> {
        let mut parents: Vec<PathBuf> = self.config.changes_folder.iter().map(PathBuf::from).collect();
        parents.extend(self.default_changes_parent());
        parents.extend(self.local_changes_parent());
        parents.dedup();
        parents.iter().flat_map(|p| std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path())).filter(|p| p.is_dir()).collect()
    }

    /// An archive with no record here, whose changes are still waiting (A.4 c): the record lost (a damaged
    /// aplikacja.json), or the archive moved (another drive letter). Only when the recorded original is gone: a copied
    /// folder or a restored zip has the same id and must not take the changes over.
    fn reattach(&mut self, a: &Archive, opened: &str, key: &str) -> Option<(PathBuf, overlay::Manifest)> {
        let own = overlay::has_own_settings(a);
        let (id, data_file, hash) = (a.settings().archive_id.clone(), a.settings().data_file.to_lowercase(), a.fingerprint_hex());
        let same = |archive_id: Option<&str>, file: &str, h: Option<&str>| if own { archive_id == Some(id.as_str()) } else { file.to_lowercase() == data_file && h == Some(hash.as_str()) };
        let mut hit: Option<(PathBuf, overlay::Manifest)> = None;
        for (k, u) in &self.config.archives {
            if u.mode != OVERLAY || k == key || Path::new(&u.opened).exists() || !same(u.archive_id.as_deref(), &u.data_file, u.hash.as_deref()) {
                continue;
            }
            let Some(folder) = u.overlay.as_ref().map(PathBuf::from) else { continue };
            if let Some(m) = overlay::read_manifest(&folder).filter(|m| !m.retired) {
                hit = Some((folder, m));
                break;
            }
        }
        if hit.is_none() {
            for folder in self.overlay_folders() {
                let Some(m) = overlay::read_manifest(&folder).filter(|m| !m.retired) else { continue };
                let gone = !Path::new(&m.original.opened).exists() && !Path::new(&m.original.data_path).exists();
                if m.original.data_path.to_lowercase() == key || (gone && same(m.original.archive_id.as_deref(), &m.original.data_file, Some(&m.base.hash))) {
                    hit = Some((folder, m));
                    break;
                }
            }
        }
        let (folder, mut manifest) = hit?;
        // The record follows the archive to where it is now.
        manifest.original.opened = overlay::absolute(opened);
        manifest.original.root = a.root().display().to_string();
        manifest.original.data_path = std::fs::canonicalize(a.data_path()).unwrap_or_else(|_| a.data_path()).display().to_string();
        overlay::write_manifest(&folder, &manifest).ok()?;
        let folder_text = folder.display().to_string();
        self.update_config(|c| c.archives.retain(|_, u| u.overlay.as_deref() != Some(folder_text.as_str())));
        Some((folder, manifest))
    }

    /// Whether `path` is inside an archive worked on in „Nie ruszaj oryginału” on this computer.
    fn inside_untouched_original(&self, path: &Path) -> bool {
        let target = path.parent().and_then(|p| std::fs::canonicalize(p).ok()).unwrap_or_else(|| path.to_path_buf());
        self.config.archives.values().filter(|u| u.mode == OVERLAY).filter_map(|u| u.overlay.as_ref()).filter_map(|f| overlay::read_manifest(Path::new(f))).any(|m| {
            let root = std::fs::canonicalize(&m.original.root).unwrap_or_else(|_| PathBuf::from(&m.original.root));
            target.starts_with(root)
        })
    }

    fn mode_chosen(&self) -> Result<(), ApiError> {
        match &self.session {
            Some(s) if s.work_mode.is_none() && self.combined.is_none() => Err(ApiError::new("mode_unset", "Najpierw wybierz, jak pracować z tym archiwum.")),
            _ => Ok(()),
        }
    }

    /// What the question „Jak chcesz pracować z tym archiwum?” shows (A.5).
    fn mode_info(&mut self) -> ApiResult {
        let documents = self.documents();
        let parent = self.changes_parent();
        let last_editor = self.config.last_editor.clone();
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        let archive = &s.archive;
        let (root, data) = match &s.overlay {
            Some(info) => (info.original_root.clone(), info.original_data_path.clone()),
            None => (archive.root().to_path_buf(), archive.data_path()),
        };
        // Nothing is written to find out: the data file opened for writing, the folder's read-only mark, and whether
        // the settings had to live inside the app.
        let writable = overlay::writeback::writable_quiet(&root, &data) && (s.overlay.is_some() || archive.sidecar_path().starts_with(&root));
        let used_before = s.used_before;
        let editors: Vec<Value> = archive
            .settings()
            .editors
            .iter()
            .filter(|e| Some(&e.name) != last_editor.as_ref())
            .map(|e| json!({ "name": e.name, "lastEdited": e.last_edited }))
            .collect();
        let foreign = archive.is_foreign().then(|| archive.origin().unwrap_or("nieznany program").to_string());
        let folder = overlay::free_folder(&parent, &archive.settings().name);
        let suggested = if !writable || foreign.is_some() && !used_before { OVERLAY } else { IN_PLACE };
        Ok(json!({
            "writable": writable,
            "foreign": foreign,
            "usedBefore": used_before,
            "editors": editors,
            "folder": { "path": folder.display().to_string(), "label": overlay::label(&folder, documents.as_deref()) },
            "suggested": suggested,
        }))
    }

    /// The answer to the question, or a switch in Ustawienia › Archiwum › Sposób pracy.
    fn set_work_mode(&mut self, args: &Value) -> ApiResult {
        let mode = str_arg(args, "mode")?;
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        match mode.as_str() {
            "inPlace" => {
                if let Some(info) = &s.overlay {
                    // A1 → A2: only with nothing waiting, else the user chooses (write back, or keep as an archive).
                    let pending = info.pending(&s.archive);
                    if pending > 0 || s.archive.has_unsaved_changes() {
                        return Err(ApiError::new("pending_changes", format!("Masz {}, których nie ma w archiwum.", count_pl(pending.max(1), "zmianę", "zmiany", "zmian"))));
                    }
                    if info.original_missing || !info.original_data_path.is_file() {
                        return Err(ApiError::new("original_missing", "Archiwum jest teraz niedostępne."));
                    }
                    if !overlay::writeback::writable(&info.original_root, &info.original_data_path) {
                        return Err(ApiError::new("not_writable", NOT_WRITABLE));
                    }
                    let (root, data, opened) = (s.archive.root().to_path_buf(), info.original_data_path.clone(), s.opened.clone());
                    if let Some(mut journal) = overlay::writeback::last_journal(&root) {
                        if journal.state.as_deref() == Some("done") {
                            overlay::writeback::settle(&root, &info.original_root, &mut journal);
                        }
                    }
                    let original = Archive::open(&data)?;
                    self.session = None;
                    overlay::retire(&root, &format!("przeniesione do archiwum {} – można usunąć", overlay::today()));
                    self.record_in_place(&original, &opened);
                    let mut status = self.install(original, &opened)?;
                    status["notice"] = json!("Od teraz zmiany zapisują się w archiwum.");
                    return Ok(status);
                }
                if !overlay::writeback::writable(s.archive.root(), &s.archive.data_path()) || !s.archive.sidecar_path().starts_with(s.archive.root()) {
                    return Err(ApiError::new("not_writable", NOT_WRITABLE));
                }
                self.update_config(in_place(&s.archive, &s.opened.clone()));
                if let Some(s) = self.session.as_mut() {
                    s.work_mode = Some(IN_PLACE);
                }
                self.archive_status()
            }
            "overlay" => {
                if s.overlay.is_some() {
                    return self.archive_status();
                }
                if s.archive.has_unsaved_changes() {
                    return Err(ApiError::new("unsaved", "Najpierw zapisz albo odrzuć niezapisane zmiany."));
                }
                let chosen = opt_str(args, "folder").map(PathBuf::from);
                let parent = chosen.clone().unwrap_or_else(|| self.changes_parent());
                let root = std::fs::canonicalize(s.archive.root()).unwrap_or_else(|_| s.archive.root().to_path_buf());
                let parent_full = std::path::absolute(&parent).unwrap_or_else(|_| parent.clone());
                if std::fs::canonicalize(&parent_full).unwrap_or_else(|_| parent_full.clone()).starts_with(&root) || parent_full.starts_with(s.archive.root()) {
                    return Err(ApiError::new("inside_archive", "Wybierz miejsce poza folderem archiwum."));
                }
                let opened = s.opened.clone();
                let mut notices = Vec::new();
                let folder = match overlay::create(&s.archive, &opened, &parent) {
                    Ok(folder) => folder,
                    // Documents can't be written (e.g. Windows' protection against ransomware): the app's own folder.
                    Err(e) if e.code == "permission" && chosen.is_none() && self.config.changes_folder.is_none() => {
                        let local = self.local_changes_parent().ok_or(e)?;
                        let folder = overlay::create(&s.archive, &opened, &local)?;
                        notices.push(format!("Folder Dokumenty jest zablokowany (np. przez ochronę przed ransomware). Twoje zmiany będą w: {}", folder.display()));
                        folder
                    }
                    Err(e) => return Err(e),
                };
                if let Some(chosen) = chosen {
                    self.update_config(|c| c.changes_folder = Some(chosen.display().to_string()));
                }
                let manifest = overlay::read_manifest(&folder).ok_or_else(|| ApiError::new("io", "Nie można odczytać folderu Twoich zmian."))?;
                let mine = Archive::open(&folder)?;
                self.install_overlay(mine, manifest, false, notices)
            }
            _ => Err(ApiError::bad_args("mode")),
        }
    }

    /// Ustawienia › Archiwum › Twoje zmiany.
    fn overlay_info(&mut self) -> ApiResult {
        let documents = self.documents();
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        let info = s.overlay.as_ref().ok_or_else(|| ApiError::new("not_overlay", "To archiwum nie jest otwarte w trybie „Nie ruszaj oryginału”."))?;
        let root = s.archive.root();
        let base = info.base_as(s.archive.doc.version);
        let people = heirloom_core::history::diff(&base, s.archive.saved(), "", None).iter().filter(|e| e.tag == "INDI").count();
        let files = if info.original_missing { Vec::new() } else { overlay::writeback::new_files(root, &info.original_root) };
        let original = (!info.original_missing).then(|| Archive::open(&info.original_data_path).ok()).flatten();
        Ok(json!({
            "folder": root.display().to_string(),
            "folderLabel": overlay::label(root, documents.as_deref()),
            "pending": info.pending(&s.archive),
            "people": people,
            "newFiles": files.len(),
            "newBytes": files.iter().map(|f| f.size).sum::<u64>(),
            "writable": !info.original_missing && overlay::writeback::writable_quiet(&info.original_root, &info.original_data_path),
            "lockedOriginal": original.as_ref().is_some_and(|o| o.settings().read_only),
            "lastWriteBack": info.manifest.synced.last_write_back,
            "canUndoWriteBack": overlay::writeback::can_undo(root, info),
            "originalChanged": info.original_changed(),
            "originalMissing": info.original_missing,
        }))
    }

    /// „Porzuć moje zmiany…”: the folder is renamed (never deleted) and a fresh one starts from the archive as it is now.
    fn discard_overlay(&mut self) -> ApiResult {
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        let info = s.overlay.as_ref().ok_or_else(|| ApiError::new("not_overlay", "To archiwum nie jest otwarte w trybie „Nie ruszaj oryginału”."))?;
        if info.original_missing || !info.original_data_path.is_file() {
            return Err(ApiError::new("original_missing", "Archiwum jest teraz niedostępne."));
        }
        let (root, opened, data) = (s.archive.root().to_path_buf(), s.opened.clone(), info.original_data_path.clone());
        let parent = root.parent().map(Path::to_path_buf).unwrap_or_else(|| self.changes_parent());
        let original = Archive::open(&data)?;
        self.session = None;
        let kept = overlay::retire(&root, &format!("porzucone {}", overlay::today()));
        let folder = overlay::create(&original, &opened, &parent)?;
        let manifest = overlay::read_manifest(&folder).ok_or_else(|| ApiError::new("io", "Nie można odczytać folderu Twoich zmian."))?;
        let mine = Archive::open(&folder)?;
        let name = kept.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.install_overlay(mine, manifest, false, vec![format!("Porzucone zmiany są w folderze „{name}”.")])
    }

    /// „Wskaż, gdzie jest teraz…”: the original found in another place.
    fn relocate_original(&mut self, path: &str, force: bool) -> ApiResult {
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        let info = s.overlay.as_ref().ok_or_else(|| ApiError::new("not_overlay", "To archiwum nie jest otwarte w trybie „Nie ruszaj oryginału”."))?;
        let found = Archive::open(Path::new(path))?;
        let mut manifest = info.manifest.clone();
        let same = match &manifest.original.archive_id {
            Some(id) => overlay::has_own_settings(&found) && found.settings().archive_id == *id,
            None => found.settings().data_file.eq_ignore_ascii_case(&manifest.original.data_file) && found.fingerprint_hex() == manifest.base.hash,
        };
        if !same && !force {
            return Err(ApiError::new("other_archive", "To inne archiwum niż to, z którego pochodzą Twoje zmiany. Użyć go mimo to?"));
        }
        let root = s.archive.root().to_path_buf();
        let old_key = manifest.original.data_path.to_lowercase();
        manifest.original.opened = overlay::absolute(path);
        manifest.original.root = found.root().display().to_string();
        manifest.original.data_path = std::fs::canonicalize(found.data_path()).unwrap_or_else(|_| found.data_path()).display().to_string();
        overlay::write_manifest(&root, &manifest)?;
        self.update_config(|c| {
            c.archives.remove(&old_key);
        });
        self.session = None;
        let mine = Archive::open(&root)?;
        self.install_overlay(mine, manifest, false, Vec::new())
    }

    /// Makes `set` the open view (closing the archive and any import) and remembers it in the recent list.
    fn install_set(&mut self, set: combined::Combined) -> ApiResult {
        self.media.set_many(set.media_roots(self.cache_root.as_deref()));
        self.media.set_imports(Vec::new());
        let entry = RecentArchive {
            path: set.path.display().to_string(),
            name: set.set.name.clone(),
            opened_at: heirloom_core::history::now(),
            people: set.people(),
            kind: "set".into(),
            archives: Some(set.parts.len()),
            edited_at: None,
        };
        self.update_config(|c| c.remember(entry));
        self.session = None;
        self.import = None;
        self.combined = Some(set);
        self.archive_status()
    }

    fn archive_status(&self) -> ApiResult {
        if let Some(c) = &self.combined {
            return Ok(c.status());
        }
        let s = self.session.as_ref().ok_or_else(ApiError::no_archive)?;
        Ok(status_of(s, self.documents().as_deref()))
    }

    fn save(&mut self, args: &Value) -> ApiResult {
        let author = str_arg(args, "author")?;
        let note = opt_str(args, "note");
        let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
        let s = self.session()?;
        let report = s.archive.save(&SaveOptions { author: &author, batch: None, note: note.as_deref(), allow_foreign, kind: None })?;
        // What names this save for „Cofnij zapis”; nothing when its history couldn't be written.
        let history = report.history_ts.as_ref().filter(|_| report.history_error.is_none()).map(|ts| json!({ "ts": ts, "author": author }));
        let saved_at = heirloom_core::history::now();
        s.last_saved = Some(saved_at.clone());
        s.changed();
        // The data file's hash on this computer (A.2), for finding the archive again by its content.
        let record = s.overlay.is_none().then(|| (data_key(&s.archive.data_path()), s.archive.fingerprint_hex()));
        self.update_config(|c| {
            c.set_editor(author);
            if let Some(r) = record.and_then(|(key, hash)| c.archives.get_mut(&key).filter(|u| u.mode == IN_PLACE).map(|u| (u, hash))) {
                r.0.hash = Some(r.1);
            }
        });
        self.touch_edited();
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

    /// Marks the open archive (by the path it was opened from) as edited by Heirloom just now, for sorting the recent list.
    fn touch_edited(&mut self) {
        let Some(path) = self.session.as_ref().map(|s| s.opened.clone()).filter(|p| !p.is_empty()) else { return };
        let now = heirloom_core::history::now();
        self.update_config(|c| c.touch_edited(&path, now));
    }

    /// One change to `aplikacja.json`, made on the file as it is now: another window (another process) may have
    /// written it since this one read it. Only conveniences (recent list, appearance, update checks, last places) live
    /// there; failing to write them must not break the app.
    fn update_config(&mut self, change: impl FnOnce(&mut AppConfig)) {
        match &self.config_dir {
            Some(dir) => self.config = AppConfig::update(dir, &self.config, change),
            None => change(&mut self.config),
        }
    }
}

/// The change to `aplikacja.json` that records `archive` as worked on in place.
fn in_place(archive: &Archive, opened: &str) -> impl FnOnce(&mut AppConfig) + use<> {
    let key = data_key(&archive.data_path());
    let (id, data_file, hash) = (overlay::has_own_settings(archive).then(|| archive.settings().archive_id.clone()), archive.settings().data_file.clone(), archive.fingerprint_hex());
    let opened = overlay::absolute(opened);
    move |c: &mut AppConfig| {
        let record = c.archives.entry(key).or_default();
        record.mode = IN_PLACE.into();
        record.overlay = None;
        record.opened = opened;
        record.archive_id = id;
        record.data_file = data_file;
        record.hash = Some(hash);
        record.decided_at = heirloom_core::history::now();
    }
}

const NOT_WRITABLE: &str = "W tym folderze nie da się nic zapisać (np. płyta, zablokowany pendrive albo folder tylko do odczytu), więc zostaje „Nie ruszaj oryginału”.";

fn status_of(s: &Session, documents: Option<&Path>) -> Value {
    let archive = &s.archive;
    let settings = archive.settings();
    // In „Nie ruszaj oryginału” the archive is the original: the screens compare these paths.
    let (root, data_path) = match &s.overlay {
        Some(info) => (info.original_root.clone(), info.original_data_path.clone()),
        None => (archive.root().to_path_buf(), archive.data_path()),
    };
    let overlay = s.overlay.as_ref().map(|info| {
        json!({
            "folder": archive.root().display().to_string(),
            "folderLabel": overlay::label(archive.root(), documents),
            "dataPath": info.original_data_path.display().to_string(),
            "pending": info.pending(archive),
            "originalChanged": info.original_changed(),
            "originalMissing": info.original_missing,
            "lastWriteBack": info.manifest.synced.last_write_back,
            "canUndoWriteBack": !info.original_missing && overlay::writeback::can_undo(archive.root(), info),
            "interrupted": info.interrupted,
            "sameDrive": tools::disk_name(archive.root()) == tools::disk_name(&info.original_root),
        })
    });
    json!({
        "archiveId": settings.archive_id,
        "name": settings.name,
        "root": root.display().to_string(),
        "dataFile": settings.data_file,
        "dataPath": data_path.display().to_string(),
        "workMode": s.work_mode,
        "overlay": overlay,
        "origin": archive.origin(),
        "foreign": archive.is_foreign(),
        "foreignConfirmed": settings.foreign_save_confirmed,
        "readOnly": settings.read_only,
        "layoutInterrupted": s.layout_interrupted,
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
    fn the_colour_scheme_and_accent_are_chosen_for_the_whole_app() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let mut api = Api::new(Some(config.clone()), None);
        let state = api.call("app.state", Value::Null).unwrap();
        assert_eq!((&state["appearance"]["scheme"], &state["appearance"]["accent"]), (&json!("museum"), &json!("default")));
        assert_eq!(api.call("app.setAppearance", json!({ "scheme": "neon" })).unwrap_err().code, "bad_args");
        assert_eq!(api.call("app.setAppearance", json!({ "accent": "neon" })).unwrap_err().code, "bad_args");
        // One at a time: the other choices stay.
        api.call("app.setAppearance", json!({ "theme": "dark" })).unwrap();
        api.call("app.setAppearance", json!({ "scheme": "terakota" })).unwrap();
        let state = api.call("app.setAppearance", json!({ "accent": "plum" })).unwrap();
        assert_eq!((&state["appearance"]["scheme"], &state["appearance"]["accent"], &state["appearance"]["theme"]), (&json!("terakota"), &json!("plum"), &json!("dark")));
        // After a restart they are back (they belong to this computer, not to an archive).
        let mut api = Api::new(Some(config), None);
        let state = api.call("app.state", Value::Null).unwrap();
        assert_eq!((&state["appearance"]["scheme"], &state["appearance"]["accent"]), (&json!("terakota"), &json!("plum")));
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
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
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
    #[test]
    fn saving_marks_the_archive_edited_and_the_sort_is_stored() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        assert!(api.call("app.state", Value::Null).unwrap()["recent"][0].get("editedAt").is_none());
        api.call("person.create", json!({ "given": "Jan" })).unwrap();
        api.call("archive.save", json!({ "author": "Ewa" })).unwrap();
        assert!(api.call("app.state", Value::Null).unwrap()["recent"][0]["editedAt"].is_string());
        assert!(api.call("recent.setSort", json!({ "sort": "bogus" })).is_err());
        assert_eq!(api.call("recent.setSort", json!({ "sort": "name" })).unwrap()["recentSort"], "name");
    }

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
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
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

    /// `tree.graph` takes `unfold: [{id, depth}]` (null depth: all) and says what it unfolded.
    #[test]
    fn tree_graph_unfolds_a_child_by_the_args() {
        let dir = tempfile::tempdir().unwrap();
        let mut api = Api::new(None, None);
        api.call("archive.create", json!({ "folder": dir.path().join("a").to_str().unwrap(), "name": "A" })).unwrap();
        let a = api.call("person.create", json!({ "given": "A" })).unwrap()["id"].as_str().unwrap().to_string();
        let mut line = vec![a.clone()];
        for given in ["B", "C", "D"] {
            let of = line.last().unwrap().clone();
            line.push(api.call("person.create", json!({ "given": given, "relation": { "kind": "child", "of": of } })).unwrap()["id"].as_str().unwrap().to_string());
        }
        let (b, d) = (line[1].clone(), line[3].clone());
        let plain = api.call("tree.graph", json!({ "id": a, "up": 1, "down": 1 })).unwrap();
        assert_eq!(plain["unfold"], json!([]));
        let unfolded = api.call("tree.graph", json!({ "id": a, "up": 1, "down": 1, "unfold": [{ "id": b, "depth": null }, { "id": 7 }] })).unwrap();
        assert_eq!(unfolded["unfold"], json!([{ "id": b, "depth": null }]));
        assert!(unfolded["people"].get(&d).is_some() && plain["people"].get(&d).is_none(), "the grandchild's child comes with the unfold");
        let two = api.call("tree.graph", json!({ "id": a, "up": 1, "down": 1, "unfold": [{ "id": b, "depth": 1 }] })).unwrap();
        assert!(two["people"].get(&line[2]).is_some() && two["people"].get(&d).is_none());
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
        api.call("archive.setWorkMode", json!({ "mode": "inPlace" })).unwrap();
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
