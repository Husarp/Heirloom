//! Moving files (design 0.7.0 B.6): switching the layout, „Przenieś do folderów osób” and unpacking a folder.
//! Every move goes through `fsx::move_no_replace` and is written to a journal (`.heirloom/porzadkowanie/<ts>.jsonl`,
//! append-only), so a run that is cut off can be finished or taken back, and so can a finished one. The FILE values of
//! the moved files change in ONE save (kind `layout`); if that save fails twice, every file goes back and the archive is
//! exactly as before.

use super::{Folders, Layout, MEDIA, Tree, split_media, walk};
use crate::fsx::{self, name_key};
use crate::gedwrite::file_uri;
use crate::progress::Progress;
use crate::{ApiError, ApiResult, Session};
use heirloom_core::{Edit, SaveOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

const JOURNALS: &str = "porzadkowanie";
const KEEP_JOURNALS: usize = 5;
const MAX_PATH: usize = 240;
pub const NOTE_DONE: &str = "Uporządkowano pliki";
pub const NOTE_UNDONE: &str = "Cofnięto porządkowanie";

// ---------- test hooks ----------

thread_local! {
    static STOP_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
    static FAIL_SAVES: Cell<u8> = const { Cell::new(0) };
    static BEFORE_MOVE: RefCell<Option<Box<dyn Fn(usize, &Path)>>> = const { RefCell::new(None) };
}

/// Tests: stop as if the program had been closed after this many moves of a run.
#[doc(hidden)]
pub fn stop_after(moves: Option<usize>) {
    STOP_AFTER.with(|s| s.set(moves));
}

/// Tests: the next `n` saves of a run fail with a conflict, as if another program had changed the data file.
#[doc(hidden)]
pub fn fail_saves(n: u8) {
    FAIL_SAVES.with(|s| s.set(n));
}

/// Tests: called with the number and the target path of a move just before it is made.
#[doc(hidden)]
pub fn before_move(hook: Option<Box<dyn Fn(usize, &Path)>>) {
    BEFORE_MOVE.with(|h| *h.borrow_mut() = hook);
}

pub fn overlay_mode() -> ApiError {
    ApiError::new("overlay_mode", "W trybie „Nie ruszaj oryginału” pliki w archiwum zostają na miejscu — możesz je tylko przypisać do osób.")
}

// ---------- the journal ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub objects: Vec<String>,
    #[serde(default)]
    pub unknown: bool,
    /// The file as it was when the plan was made. A move keeps size and time, so a cut-off run finds its file again by
    /// them (never by the name alone: another file may have taken it).
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileEdit {
    before: String,
    after: String,
}

#[derive(Debug, Clone)]
struct Journal {
    path: PathBuf,
    /// The file name without `.jsonl`: names the journal in the commands.
    name: String,
    ts: String,
    batch: String,
    moves: Vec<Move>,
    edits: BTreeMap<String, FileEdit>,
    /// The layout the run sets when it is done, and the one it had before.
    layout: Option<String>,
    layout_before: Option<String>,
    /// Move number → where the file really went.
    done: BTreeMap<usize, String>,
    /// `saved`, `rolledBack`, `undone`; none while the run is not over.
    state: Option<String>,
}

fn journal_dir(sidecar: &Path) -> PathBuf {
    sidecar.join(JOURNALS)
}

fn append_line(path: &Path, line: &Value) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(format!("{line}\n").as_bytes())?;
    file.sync_data()
}

fn read_journal(path: &Path) -> Option<Journal> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    // A line cut off by a crash is not JSON and is ignored.
    let header: Value = serde_json::from_str(lines.next()?).ok()?;
    let moves: Vec<Move> = serde_json::from_value(header.get("moves")?.clone()).ok()?;
    let edits: BTreeMap<String, FileEdit> = serde_json::from_value(header.get("fileEdits")?.clone()).unwrap_or_default();
    let mut journal = Journal {
        path: path.to_path_buf(),
        name: path.file_stem()?.to_string_lossy().into_owned(),
        ts: header.get("ts").and_then(Value::as_str).unwrap_or_default().to_string(),
        batch: header.get("batch")?.as_str()?.to_string(),
        moves,
        edits,
        layout: header.get("layout").and_then(Value::as_str).map(str::to_string),
        layout_before: header.get("layoutBefore").and_then(Value::as_str).map(str::to_string),
        done: BTreeMap::new(),
        state: None,
    };
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        if let Some(i) = v.get("done").and_then(Value::as_u64) {
            let planned = journal.moves.get(i as usize).map(|m| m.to.clone());
            if let Some(to) = v.get("to").and_then(Value::as_str).map(str::to_string).or(planned) {
                journal.done.insert(i as usize, to);
            }
        } else if let Some(state) = v.get("state").and_then(Value::as_str) {
            journal.state = Some(state.to_string());
        }
    }
    Some(journal)
}

/// Every journal, oldest first.
fn journals(sidecar: &Path) -> Vec<Journal> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(journal_dir(sidecar)).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "jsonl")).collect();
    paths.sort();
    paths.iter().filter_map(|p| read_journal(p)).collect()
}

/// A run that was cut off (no end line): `{ journal, done, total, layout }`.
pub fn interrupted(sidecar: &Path) -> Option<Value> {
    journals(sidecar).into_iter().rev().find(|j| j.state.is_none()).map(|j| json!({ "journal": j.name, "done": j.done.len(), "total": j.moves.len(), "layout": j.layout }))
}

/// The last run that is saved or still open: `{ name, ts, state, moved, total }` (Settings › „Ostatnie porządkowanie”).
pub fn last_journal_summary(sidecar: &Path) -> Option<Value> {
    journals(sidecar)
        .into_iter()
        .rev()
        .find(|j| matches!(j.state.as_deref(), None | Some("saved")))
        .map(|j| json!({ "name": j.name, "ts": j.ts, "state": j.state.clone().unwrap_or_else(|| "interrupted".into()), "moved": j.done.len(), "total": j.moves.len() }))
}

/// Keeps the newest five journals; only finished ones are ever deleted.
fn rotate(sidecar: &Path) {
    let all = journals(sidecar);
    if all.len() <= KEEP_JOURNALS {
        return;
    }
    for old in all.iter().take(all.len() - KEEP_JOURNALS).filter(|j| j.state.is_some()) {
        let _ = std::fs::remove_file(&old.path);
    }
}

// ---------- the plan (B.6) ----------

pub struct PlanOptions {
    pub to: Layout,
    /// Subfolders of `media/` ticked to be taken along (compared names, without `media/`).
    pub folders: Vec<String>,
    /// Single files (paths inside the archive, compared) taken along.
    pub files: Vec<String>,
    /// Only the ticked folders and files: not every person folder.
    pub only: bool,
}

pub fn parse_options(args: &Value) -> Result<PlanOptions, ApiError> {
    let to = args.get("to").and_then(Value::as_str).and_then(Layout::parse).ok_or_else(|| ApiError::bad_args("to"))?;
    let list = |key: &str| -> Vec<String> { args.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|x| x.replace('\\', "/")).collect()).unwrap_or_default() };
    let folders = list("folders").into_iter().map(|f| name_key(f.trim_matches('/').strip_prefix("media/").unwrap_or(f.trim_matches('/')))).collect();
    let files = list("files").iter().map(|f| name_key(f)).collect();
    Ok(PlanOptions { to, folders, files, only: args.get("only").and_then(Value::as_bool).unwrap_or(false) })
}

pub struct Plan {
    pub to: Layout,
    pub moves: Vec<Move>,
    bytes: u64,
    /// Object id → the FILE value now and after the move.
    edits: BTreeMap<String, FileEdit>,
    /// Files in the media root that belong to nobody (source scans): they stay.
    unassigned: usize,
    /// Files of several people (they go to the folder of one of them).
    group_files: usize,
    /// Files several records point at (moved once).
    shared: usize,
    already: usize,
    new_folders: Vec<String>,
    shortened: Vec<Value>,
    long_paths: usize,
}

impl Plan {
    pub fn json(&self) -> Value {
        json!({
            "to": self.to.code(),
            "count": self.moves.len(),
            "bytes": self.bytes,
            "moves": self.moves.iter().take(300).collect::<Vec<_>>(),
            "unknownMoves": self.moves.iter().filter(|m| m.unknown).count(),
            "unassigned": self.unassigned,
            "groupFiles": self.group_files,
            "shared": self.shared,
            "alreadyInPlace": self.already,
            "newFolders": self.new_folders,
            "shortened": self.shortened,
            "longPaths": self.long_paths,
        })
    }
}

fn join_rel(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}

/// Splits a path inside the archive into its folder and its file name.
fn split_path(rel: &str) -> (&str, &str) {
    rel.rsplit_once('/').unwrap_or(("", rel))
}

fn path_of(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|p| !p.is_empty()).fold(root.to_path_buf(), |p, part| p.join(part))
}

fn names_in(root: &Path, dir: &str) -> HashSet<String> {
    std::fs::read_dir(path_of(root, dir)).into_iter().flatten().flatten().map(|e| name_key(&e.file_name().to_string_lossy())).collect()
}

fn name_parts(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_string()),
        _ => (name.to_string(), String::new()),
    }
}

/// Works out what a run would move (media.restructurePreview). Nothing is changed.
fn build_plan(s: &mut Session, opts: &PlanOptions) -> Result<Plan, ApiError> {
    s.ensure();
    let layers = s.layers();
    let root = layers[0].clone();
    let t = Tree::build(&s.archive.doc, s.derived.as_ref().expect("ensured"), &layers);
    let include = t.linked_persons();
    let folders = Folders::build(&t, &layers, &include);
    let (files, _) = walk(&layers, &[]);
    let mut plan = Plan { to: opts.to, moves: Vec::new(), bytes: 0, edits: BTreeMap::new(), unassigned: 0, group_files: 0, shared: 0, already: 0, new_folders: Vec::new(), shortened: Vec::new(), long_paths: 0 };
    let mut taken: HashMap<String, HashSet<String>> = HashMap::new();
    for f in &files {
        let Some((top, name)) = split_media(&f.rel) else { continue };
        let key = f.key();
        // A file written inside a person is never moved: its path is part of the person's record.
        if t.embedded.contains(&key) {
            continue;
        }
        let objs = t.by_key.get(&key);
        let explicit = opts.files.contains(&key);
        let ticked = top.is_some_and(|x| opts.folders.contains(&name_key(x)));
        let target_dir: String = match opts.to {
            Layout::PerPerson => {
                if !(explicit || ticked || (!opts.only && top.is_none())) {
                    continue;
                }
                let Some(objs) = objs else { continue };
                let Some(owner) = t.file_owner(objs) else {
                    plan.unassigned += 1;
                    continue;
                };
                let linkers = t.file_linkers(objs);
                let in_place = top.is_some_and(|x| linkers.iter().any(|p| folders.by_person.get(p).is_some_and(|n| name_key(n) == name_key(x))));
                if in_place {
                    plan.already += 1;
                    continue;
                }
                if linkers.len() > 1 {
                    plan.group_files += 1;
                }
                format!("{MEDIA}/{}", folders.by_person[&owner])
            }
            Layout::Flat => {
                let Some(top) = top else { continue };
                let person_folder = folders.owner_of.contains_key(&name_key(top));
                if !(explicit || ticked || (!opts.only && person_folder)) {
                    continue;
                }
                // Files nothing references stay in a person folder; in a ticked folder they come along.
                if objs.is_none() && !(explicit || ticked) {
                    continue;
                }
                MEDIA.to_string()
            }
        };
        if name_key(split_path(&f.rel).0) == name_key(&target_dir) {
            continue;
        }
        let used = taken.entry(name_key(&target_dir)).or_insert_with(|| names_in(&root, &target_dir));
        let (stem, ext) = name_parts(name);
        let free = fsx::free_name(&|n| used.contains(n), &stem, &ext);
        used.insert(name_key(&free));
        let to = join_rel(&target_dir, &free);
        let object_ids: Vec<String> = objs.map(|o| o.iter().map(|&i| t.objs[i].id.clone()).collect()).unwrap_or_default();
        if object_ids.len() > 1 {
            plan.shared += 1;
        }
        for &i in objs.into_iter().flatten() {
            plan.edits.insert(t.objs[i].id.clone(), FileEdit { before: t.objs[i].raw.clone(), after: file_uri(&to) });
        }
        plan.bytes += f.size;
        plan.moves.push(Move { from: f.rel.clone(), to, objects: object_ids, unknown: objs.is_none(), size: f.size, mtime: f.mtime });
    }
    shorten_long_paths(&root, &folders, &mut plan);
    let mut new_folders: Vec<String> = Vec::new();
    for m in &plan.moves {
        let dir = split_path(&m.to).0.to_string();
        if dir != MEDIA && !path_of(&root, &dir).is_dir() && !new_folders.contains(&dir) {
            new_folders.push(dir);
        }
    }
    plan.new_folders = new_folders;
    Ok(plan)
}

/// Full paths over 240 characters fail in Explorer and other genealogy programs: a NEW folder's name is shortened so
/// that the longest file in it fits. A path that is still too long (a very long file name) is counted.
fn shorten_long_paths(root: &Path, folders: &Folders, plan: &mut Plan) {
    let base = root.to_string_lossy().chars().count() + 1;
    let mut longest: BTreeMap<String, usize> = BTreeMap::new();
    for m in &plan.moves {
        let e = longest.entry(split_path(&m.to).0.to_string()).or_default();
        *e = (*e).max(base + m.to.chars().count());
    }
    let mut used: HashSet<String> = folders.existing.keys().cloned().collect();
    used.extend(longest.keys().map(|d| name_key(split_path(d).1)));
    for (dir, len) in longest {
        if len <= MAX_PATH {
            continue;
        }
        let (parent, folder) = split_path(&dir);
        let is_new = !folders.existing.contains_key(&name_key(folder));
        if parent != MEDIA || !is_new {
            plan.long_paths += plan.moves.iter().filter(|m| split_path(&m.to).0 == dir && base + m.to.chars().count() > MAX_PATH).count();
            continue;
        }
        let excess = len - MAX_PATH;
        let keep = folder.chars().count().saturating_sub(excess).max(12);
        let cut: String = folder.chars().take(keep).collect::<String>().trim_end_matches(['.', ' ']).to_string();
        let new = fsx::free_name(&|n| used.contains(n), &cut, "");
        used.insert(name_key(&new));
        let new_dir = format!("{MEDIA}/{new}");
        plan.shortened.push(json!({ "from": folder, "to": new }));
        for m in plan.moves.iter_mut().filter(|m| split_path(&m.to).0 == dir) {
            let name = split_path(&m.to).1.to_string();
            m.to = join_rel(&new_dir, &name);
            for id in &m.objects {
                if let Some(e) = plan.edits.get_mut(id) {
                    e.after = file_uri(&m.to);
                }
            }
            if base + m.to.chars().count() > MAX_PATH {
                plan.long_paths += 1;
            }
        }
    }
}

// ---------- preconditions ----------

/// The same for a run, finishing one and undoing one: A2, writable, nothing unsaved, no data conflict, the foreign file
/// confirmed. Checked before a single file moves.
fn can_move(s: &Session, allow_foreign: bool) -> Result<(), ApiError> {
    if s.overlay.is_some() {
        return Err(overlay_mode());
    }
    if s.archive.settings().read_only {
        return Err(ApiError::new("read_only", "To archiwum jest tylko do odczytu (Ustawienia › Archiwum)."));
    }
    if s.archive.has_unsaved_changes() {
        return Err(ApiError::new("unsaved", "Najpierw zapisz zmiany w archiwum — dopiero potem Heirloom przeniesie pliki."));
    }
    if s.archive.is_foreign() && !s.archive.settings().foreign_save_confirmed && !allow_foreign {
        return Err(heirloom_core::Error::ForeignNeedsConfirmation(s.archive.origin().unwrap_or("nieznany program").to_string()).into());
    }
    if s.archive.changed_on_disk().unwrap_or(true) {
        return Err(ApiError::new("conflict", "Plik z danymi został zmieniony przez inny program. Wczytaj go ponownie i spróbuj jeszcze raz."));
    }
    Ok(())
}

/// `media.restructurePreview { to, folders?, files?, only? }`: what a run would do. Nothing changes.
pub fn preview(s: &mut Session, args: &Value) -> ApiResult {
    if s.overlay.is_some() {
        return Err(overlay_mode());
    }
    let opts = parse_options(args)?;
    Ok(build_plan(s, &opts)?.json())
}

// ---------- running ----------

struct Job<'a>(&'a Progress);

impl Drop for Job<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

struct RunOptions {
    author: String,
    allow_foreign: bool,
    /// Stored as `display.mediaLayout` when the run is over.
    set_layout: Option<Layout>,
    /// Unknown files that came along get assigned to this person (unpacking a folder named after them).
    assign_to: Option<String>,
}

fn author_of(args: &Value) -> Result<String, ApiError> {
    args.get("author").and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args("author"))
}

/// `media.restructure { to, author, folders?, files?, only?, setLayout?, assignTo?, allowForeign? }`.
pub fn restructure(s: &mut Session, progress: &Progress, args: &Value) -> ApiResult {
    let author = author_of(args)?;
    let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
    let opts = parse_options(args)?;
    can_move(s, allow_foreign)?;
    if s.archive.upgrade_to_v7() {
        s.changed();
    }
    let plan = build_plan(s, &opts)?;
    let set_layout = args.get("setLayout").and_then(Value::as_bool).unwrap_or(!opts.only).then_some(opts.to);
    let options = RunOptions { author, allow_foreign, set_layout, assign_to: args.get("assignTo").and_then(Value::as_str).map(str::to_string) };
    if plan.moves.is_empty() {
        if let Some(layout) = options.set_layout {
            super::store_setting(s, layout);
        }
        return Ok(json!({ "moved": 0, "skipped": [], "held": 0, "journal": null, "layout": options.set_layout.map(Layout::code), "removedFolders": [], "unknownMoved": [], "assigned": 0 }));
    }
    let io = |e: std::io::Error| ApiError::new("io", format!("Nie można zapisać dziennika porządkowania: {e}"));
    let sidecar = s.archive.sidecar_path().to_path_buf();
    std::fs::create_dir_all(journal_dir(&sidecar)).map_err(io)?;
    let (stamp, path) = loop {
        let stamp = heirloom_core::archive::compact_timestamp();
        let path = journal_dir(&sidecar).join(format!("{stamp}.jsonl"));
        if !path.exists() {
            break (stamp, path);
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    let header = json!({
        "ts": heirloom_core::history::now(),
        "batch": format!("porzadkowanie-{stamp}"),
        "dataHashBefore": s.archive.fingerprint_hex(),
        "moves": plan.moves,
        "fileEdits": plan.edits,
        "layout": options.set_layout.map(Layout::code),
        "layoutBefore": super::setting(s).map(Layout::code),
    });
    // The journal is on the disk before the first file moves.
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(io)?;
    file.write_all(format!("{header}\n").as_bytes()).and_then(|()| file.sync_all()).map_err(io)?;
    drop(file);
    let journal = read_journal(&path).ok_or_else(|| ApiError::new("io", "Nie można odczytać dziennika porządkowania."))?;
    execute(s, progress, journal, &options)
}

/// `media.finishRestructure { author, journal?, allowForeign? }`: carries on with a run that was cut off.
pub fn finish(s: &mut Session, progress: &Progress, args: &Value) -> ApiResult {
    let author = author_of(args)?;
    let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
    can_move(s, allow_foreign)?;
    let sidecar = s.archive.sidecar_path().to_path_buf();
    let wanted = args.get("journal").and_then(Value::as_str);
    let mut journal = journals(&sidecar)
        .into_iter()
        .rev()
        .find(|j| j.state.is_none() && wanted.is_none_or(|w| w == j.name))
        .ok_or_else(|| ApiError::new("nothing_to_finish", "Nie ma przerwanego przenoszenia plików."))?;
    if s.archive.upgrade_to_v7() {
        s.changed();
    }
    // The run may have been cut off between a move and its line in the journal.
    find_unrecorded(s.archive.root(), &mut journal);
    let options = RunOptions { author, allow_foreign, set_layout: journal.layout.as_deref().and_then(Layout::parse), assign_to: None };
    execute(s, progress, journal, &options)
}

fn mtime_of(path: &Path) -> i64 {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64)
}

/// A run can be cut off between a move and its line in the journal. Such a file is found again by what a move keeps —
/// its size and time — in the folder it was going to: at its planned name, or at the next free name when the planned one
/// had been taken. A file of the same name but other contents is NOT the moved file. Nothing is changed.
fn find_unrecorded(root: &Path, j: &mut Journal) {
    for i in 0..j.moves.len() {
        if j.done.contains_key(&i) {
            continue;
        }
        let m = &j.moves[i];
        if m.mtime == 0 || path_of(root, &m.from).exists() {
            continue;
        }
        let (dir, name) = split_path(&m.to);
        let (stem, ext) = name_parts(name);
        let mut found: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(path_of(root, dir)).into_iter().flatten().flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            let (fstem, fext) = name_parts(&file);
            // The planned name, or "name (2)", "name (3)" …
            let numbered = fstem.strip_prefix(stem.as_str()).is_some_and(|rest| rest.is_empty() || (rest.starts_with(" (") && rest.ends_with(')') && rest[2..rest.len() - 1].bytes().all(|b| b.is_ascii_digit())));
            if !numbered || name_key(&fext) != name_key(&ext) {
                continue;
            }
            let path = entry.path();
            if entry.metadata().is_ok_and(|md| md.is_file() && md.len() == m.size) && mtime_of(&path) == m.mtime {
                found.push(join_rel(dir, &file));
            }
        }
        // Several files that look alike: not decided here (the moved map keeps the photo visible).
        if let [only] = found.as_slice() {
            j.done.insert(i, only.clone());
        }
    }
}

/// The next free name for a file whose planned name has been taken.
fn find_free(root: &Path, wanted: &str, tried: &mut HashSet<String>) -> String {
    let (dir, name) = split_path(wanted);
    let (stem, ext) = name_parts(name);
    let existing = names_in(root, dir);
    let free = fsx::free_name(&|n| existing.contains(n) || tried.contains(&name_key(&join_rel(dir, n))), &stem, &ext);
    let rel = join_rel(dir, &free);
    tried.insert(name_key(&rel));
    rel
}

/// How a move that did not happen is told to the user.
fn why_skipped(e: &std::io::Error) -> &'static str {
    match e.raw_os_error() {
        Some(32 | 33) => "held",
        _ if e.kind() == std::io::ErrorKind::PermissionDenied => "locked",
        _ if e.kind() == std::io::ErrorKind::NotFound => "missing",
        _ => "error",
    }
}

/// The moves, the FILE changes and the save; shared by a new run and by finishing a cut-off one.
fn execute(s: &mut Session, progress: &Progress, mut journal: Journal, options: &RunOptions) -> ApiResult {
    let root = s.archive.root().to_path_buf();
    let sidecar = s.archive.sidecar_path().to_path_buf();
    let total = journal.moves.len();
    progress.start("Przenoszę pliki…", total as u64);
    let _job = Job(progress);
    let mut skipped: Vec<Value> = Vec::new();
    let mut done_now = 0usize;
    for i in 0..total {
        progress.step(i as u64);
        if journal.done.contains_key(&i) {
            continue;
        }
        let (from, planned) = (journal.moves[i].from.clone(), journal.moves[i].to.clone());
        let from_abs = path_of(&root, &from);
        if !from_abs.is_file() {
            skipped.push(json!({ "from": from, "reason": "missing" }));
            continue;
        }
        let mut tried: HashSet<String> = HashSet::new();
        let mut target = planned;
        // The planned name may have been taken since the plan (a file made meanwhile): the next free name is used.
        for _ in 0..100 {
            let to_abs = path_of(&root, &target);
            BEFORE_MOVE.with(|h| {
                if let Some(hook) = h.borrow().as_ref() {
                    hook(i, &to_abs);
                }
            });
            if let Some(parent) = to_abs.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    skipped.push(json!({ "from": from, "reason": why_skipped(&e) }));
                    break;
                }
            }
            match fsx::with_retry(|| fsx::move_no_replace(&from_abs, &to_abs)) {
                Ok(()) => {
                    // The line is written after the move: a run cut off in between is found out by looking at the files.
                    let _ = append_line(&journal.path, &json!({ "done": i, "to": target }));
                    journal.done.insert(i, target.clone());
                    done_now += 1;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    tried.insert(name_key(&target));
                    target = find_free(&root, &target, &mut tried);
                }
                Err(e) => {
                    skipped.push(json!({ "from": from, "reason": why_skipped(&e) }));
                    break;
                }
            }
        }
        if STOP_AFTER.with(|s| s.get()).is_some_and(|n| done_now >= n) {
            s.layout_interrupted = interrupted(&sidecar);
            return Err(ApiError::new("stopped_for_test", "stopped"));
        }
    }
    progress.step(total as u64);

    // Nothing moved (every file was held): nothing to save, and nothing to take back later.
    if journal.done.is_empty() {
        let _ = append_line(&journal.path, &json!({ "state": "rolledBack", "moved": 0 }));
        if let Some(layout) = options.set_layout {
            super::store_setting(s, layout);
        }
        s.layout_interrupted = interrupted(&sidecar);
        return Ok(json!({ "moved": 0, "skipped": skipped, "held": skipped.iter().filter(|x| x["reason"] != "missing").count(), "journal": null, "layout": options.set_layout.map(Layout::code), "removedFolders": [], "unknownMoved": [], "assigned": 0 }));
    }
    // The FILE values of the files that moved, in one save.
    let batch = journal.batch.clone();
    if let Err(error) = apply_and_save(s, &journal, &options.author, options.allow_foreign, &batch, NOTE_DONE, &file_edits_forward, &HashSet::new()) {
        // The save failed twice: every file goes back, and the archive is as it was.
        let (_, failed) = move_back(&root, &journal);
        let _ = append_line(&journal.path, &json!({ "state": "rolledBack" }));
        let _ = s.archive.reload();
        s.changed();
        remove_empty_parents(&root, journal.done.values().map(String::as_str));
        s.layout_interrupted = interrupted(&sidecar);
        let message = if failed == 0 {
            format!("Nie udało się zapisać archiwum, więc pliki wróciły na swoje miejsca. Nic się nie zmieniło. ({})", error.message)
        } else {
            format!("Nie udało się zapisać archiwum, a {} nie wróciło na miejsce — wszystkie zdjęcia są nadal widoczne. ({})", crate::count_pl(failed, "plik", "pliki", "plików"), error.message)
        };
        return Err(ApiError::new("rolled_back", message));
    }
    let removed = remove_empty_parents(&root, journal.moves.iter().enumerate().filter(|(i, _)| journal.done.contains_key(i)).map(|(_, m)| m.from.as_str()));
    let _ = append_line(&journal.path, &json!({ "state": "saved", "moved": journal.done.len() }));
    journal.state = Some("saved".into());
    if let Some(layout) = options.set_layout {
        super::store_setting(s, layout);
    }
    rotate(&sidecar);
    s.layout_interrupted = interrupted(&sidecar);
    let unknown_moved: Vec<String> = journal.moves.iter().enumerate().filter(|(i, m)| m.unknown && journal.done.contains_key(i)).map(|(i, _)| journal.done[&i].clone()).collect();
    let mut assigned = 0;
    if let (Some(person), false) = (&options.assign_to, unknown_moved.is_empty()) {
        let items: Vec<Value> = unknown_moved.iter().map(|p| json!({ "path": p, "person": person })).collect();
        assigned = super::assign(s, &json!({ "items": items }))?["assigned"].as_array().map_or(0, Vec::len);
    }
    Ok(json!({
        "moved": journal.done.len(),
        "skipped": skipped,
        "held": skipped.iter().filter(|x| x["reason"] != "missing").count(),
        "journal": journal.name,
        "batch": journal.batch,
        "layout": options.set_layout.map(Layout::code),
        "removedFolders": removed,
        "unknownMoved": unknown_moved,
        "assigned": assigned,
    }))
}

/// The FILE changes of the moves done: an object's FILE value changes where it still is the value from before the run.
fn file_edits_forward(j: &Journal, raw: &HashMap<String, String>, _: &HashSet<usize>) -> Vec<(String, String)> {
    let mut edits = Vec::new();
    for (i, m) in j.moves.iter().enumerate() {
        let Some(to) = j.done.get(&i) else { continue };
        for id in &m.objects {
            if j.edits.get(id).is_some_and(|e| raw.get(id) == Some(&e.before)) {
                edits.push((id.clone(), file_uri(to)));
            }
        }
    }
    edits
}

/// The FILE changes of an undo: an object's FILE value goes back where it still is what the run made it, for the files
/// that stand at their old place now.
fn file_edits_back(j: &Journal, raw: &HashMap<String, String>, at_old_place: &HashSet<usize>) -> Vec<(String, String)> {
    let mut edits = Vec::new();
    for (i, m) in j.moves.iter().enumerate() {
        if !at_old_place.contains(&i) {
            continue;
        }
        let to = j.done.get(&i).unwrap_or(&m.to);
        for id in &m.objects {
            if let Some(e) = j.edits.get(id).filter(|_| raw.get(id) == Some(&file_uri(to))) {
                edits.push((id.clone(), e.before.clone()));
            }
        }
    }
    edits
}

type MakeEdits<'a> = &'a dyn Fn(&Journal, &HashMap<String, String>, &HashSet<usize>) -> Vec<(String, String)>;

/// Applies FILE changes to the records and saves them (kind `layout`): once more after a conflict, with the file read
/// again. On failure the open data is as the disk has it.
#[allow(clippy::too_many_arguments)]
fn apply_and_save(s: &mut Session, j: &Journal, author: &str, allow_foreign: bool, batch: &str, note: &str, make: MakeEdits, picked: &HashSet<usize>) -> Result<bool, ApiError> {
    let mut attempt = 0;
    loop {
        let raw: HashMap<String, String> = j.edits.keys().filter_map(|id| Some((id.clone(), super::file_value(s.archive.doc.record(id)?)?.to_string()))).collect();
        let mut edits: Vec<Edit> = Vec::new();
        for (id, value) in make(j, &raw, picked) {
            if let Some(mut node) = s.archive.doc.record(&id).cloned() {
                if let Some(file) = node.child_mut("FILE") {
                    file.value = Some(value);
                    edits.push(Edit::Replace(node));
                }
            }
        }
        if edits.is_empty() {
            return Ok(false);
        }
        s.archive.apply_all(edits)?;
        s.changed();
        let outcome = if FAIL_SAVES.with(|f| f.get()) > 0 {
            FAIL_SAVES.with(|f| f.set(f.get() - 1));
            Err(heirloom_core::Error::Conflict)
        } else {
            s.archive.save(&SaveOptions { author, batch: Some(batch), note: Some(note), allow_foreign, kind: Some("layout") }).map(|_| ())
        };
        match outcome {
            Ok(()) => {
                // Saved: nothing may undo the edit in memory (the files would no longer match their paths).
                s.archive.reload()?;
                s.changed();
                return Ok(true);
            }
            Err(heirloom_core::Error::Conflict) if attempt == 0 => {
                attempt += 1;
                s.archive.reload()?;
                s.changed();
            }
            Err(e) => {
                let _ = s.archive.reload();
                s.changed();
                return Err(e.into());
            }
        }
    }
}

/// Moves files back from where the journal says they went. Only a file still at its new place, with its old place free.
/// Returns the moves put back and how many could not be.
fn move_back(root: &Path, j: &Journal) -> (HashSet<usize>, usize) {
    let mut back = HashSet::new();
    let mut failed = 0;
    for i in (0..j.moves.len()).rev() {
        let to = j.done.get(&i).unwrap_or(&j.moves[i].to);
        let (to_abs, from_abs) = (path_of(root, to), path_of(root, &j.moves[i].from));
        if !to_abs.is_file() {
            continue;
        }
        if from_abs.exists() {
            failed += 1;
            continue;
        }
        if let Some(parent) = from_abs.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match fsx::with_retry(|| fsx::move_no_replace(&to_abs, &from_abs)) {
            Ok(()) => {
                back.insert(i);
            }
            Err(_) => failed += 1,
        }
    }
    (back, failed)
}

/// The two files Windows puts in folders by itself; only these are deleted with an emptied folder (a „~$…” or
/// `.tmp-123` file may be somebody's work in progress).
fn is_leftover(name: &str) -> bool {
    matches!(name.to_lowercase().as_str(), "thumbs.db" | "desktop.ini")
}

/// Removes the folders the moved files left, when they are empty (Thumbs.db and desktop.ini go too, but only when they
/// are the last entries). Never `media/` itself, never a folder with anything else in it. Returns the folders removed.
fn remove_empty_parents<'a>(root: &Path, files: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut dirs: Vec<String> = Vec::new();
    for rel in files {
        let mut dir = split_path(rel).0;
        while dir.len() > MEDIA.len() && dir.contains('/') {
            if !dirs.iter().any(|d| d == dir) {
                dirs.push(dir.to_string());
            }
            dir = split_path(dir).0;
        }
    }
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    let mut removed = Vec::new();
    for dir in dirs {
        let path = path_of(root, &dir);
        let Ok(entries) = std::fs::read_dir(&path) else { continue };
        let names: Vec<(String, bool)> = entries.flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), e.file_type().is_ok_and(|k| k.is_file()))).collect();
        if names.iter().any(|(n, file)| !file || !is_leftover(n)) {
            continue;
        }
        for (n, _) in &names {
            let _ = fsx::remove_own_file(&path.join(n));
        }
        if std::fs::remove_dir(&path).is_ok() {
            removed.push(dir);
        }
    }
    removed
}

/// `media.undoRestructure { journal, author, allowForeign? }`: runs the journal backwards, file by file. A file goes back
/// only if it is still at its new place and the old place is free; a FILE value is reverted only where it still is what
/// the run made it. Safe to run again.
pub fn undo(s: &mut Session, progress: &Progress, args: &Value) -> ApiResult {
    let author = author_of(args)?;
    let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
    let name = args.get("journal").and_then(Value::as_str).ok_or_else(|| ApiError::bad_args("journal"))?;
    can_move(s, allow_foreign)?;
    let sidecar = s.archive.sidecar_path().to_path_buf();
    let mut journal = journals(&sidecar).into_iter().find(|j| j.name == name).ok_or_else(|| ApiError::new("no_journal", "Nie ma takiego porządkowania."))?;
    if matches!(journal.state.as_deref(), Some("undone" | "rolledBack")) {
        return Err(ApiError::new("cannot_undo", "Tego porządkowania nie da się już cofnąć."));
    }
    if s.archive.upgrade_to_v7() {
        s.changed();
    }
    let root = s.archive.root().to_path_buf();
    // A file whose line is missing (cut off) is found by its size and time (see `find_unrecorded`).
    find_unrecorded(&root, &mut journal);
    progress.start("Cofam przenoszenie plików…", journal.moves.len() as u64);
    let _job = Job(progress);
    let (back, failed) = move_back(&root, &journal);
    // Whatever stands at its old place now (moved back now, or put there by hand) gets its FILE value back.
    let at_old: HashSet<usize> = (0..journal.moves.len()).filter(|&i| path_of(&root, &journal.moves[i].from).is_file() && !path_of(&root, journal.done.get(&i).unwrap_or(&journal.moves[i].to)).is_file()).collect();
    let batch = format!("{}-cofniecie", journal.batch);
    if let Err(error) = apply_and_save(s, &journal, &author, allow_foreign, &batch, NOTE_UNDONE, &file_edits_back, &at_old) {
        // The files go forward again, so the files and the FILE values still agree.
        for &i in &back {
            let to = journal.done.get(&i).unwrap_or(&journal.moves[i].to);
            let (to_abs, from_abs) = (path_of(&root, to), path_of(&root, &journal.moves[i].from));
            if let Some(parent) = to_abs.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = fsx::move_no_replace(&from_abs, &to_abs);
        }
        let _ = s.archive.reload();
        s.changed();
        return Err(ApiError::new("rolled_back", format!("Nie udało się zapisać archiwum, więc pliki zostały, gdzie były. ({})", error.message)));
    }
    let removed = remove_empty_parents(&root, journal.moves.iter().enumerate().filter(|(i, _)| back.contains(i)).map(|(i, m)| journal.done.get(&i).map_or(m.to.as_str(), String::as_str)));
    let _ = append_line(&journal.path, &json!({ "state": "undone" }));
    // The layout goes back to what it was before the run (none: the next scan guesses again).
    if journal.state.as_deref() == Some("saved") && journal.layout.is_some() {
        match journal.layout_before.as_deref().and_then(Layout::parse) {
            Some(before) => super::store_setting(s, before),
            None => {
                s.archive.settings_mut().display.remove(super::SETTING);
                let _ = s.archive.save_settings();
            }
        }
    }
    s.layout_interrupted = interrupted(&sidecar);
    Ok(json!({ "movedBack": back.len(), "notBack": failed, "journal": journal.name, "removedFolders": removed }))
}
