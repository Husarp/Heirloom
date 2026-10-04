//! „Otwórz razem…”: several archives shown as one, for browsing only (decision 1a). The archives are read, never
//! written; the list of archives and the people found to be the same person live in a set file next to them
//! (`setfile`, decision 2a). The view is one merged document in memory (`merge`) that every read screen uses as it
//! uses one archive; „To ta sama osoba?” comes from `pairs`.

pub mod merge;
pub mod pairs;
pub mod setfile;

use crate::derive::{self, Derived};
use crate::media_edit::FileRoots;
use crate::{ApiError, ApiResult, media, opt_str, str_arg};
use heirloom_core::Archive;
use heirloom_core::archive::EditorEntry;
use heirloom_core::gedcom::{Document, Node, Version, model};
use merge::{combined_id, split_id};
use serde_json::{Map, Value, json};
use setfile::{Entry, Link, PersonRef, SetFile};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Commands that only read: the combined view answers them as an archive does. Everything else that isn't `set.*` or
/// `app.*` is refused (see [`read_only`]), whatever the interface shows.
const ALLOWED: &[&str] = &[
    "recent.forget", "archive.open", "archive.create", "archive.close", "archive.status", "archive.reload", "date.parse",
    "people.list", "people.search", "person.get", "person.panel", "person.relations", "person.hover", "tree.graph",
    "tree.overview", "start.data", "surnames.list", "surname.get", "places.list", "place.get", "stories.list",
    "story.get", "sources.list", "source.get", "media.list", "media.get", "media.missing",
];

pub fn allowed(method: &str) -> bool {
    method.starts_with("set.") || method.starts_with("app.") || ALLOWED.contains(&method)
}

pub fn read_only() -> ApiError {
    ApiError::new("combined_read_only", "Widok razem służy tylko do przeglądania. Osobę zmienisz w jej archiwum („Edytuj w jego archiwum”).")
}

pub fn no_set() -> ApiError {
    ApiError::new("no_set", "Żaden zestaw archiwów nie jest otwarty.")
}

fn same_archive() -> ApiError {
    ApiError::new("same_archive", "To dwie osoby z jednego archiwum — połącz je w tym archiwum.")
}

/// A `.heirloom-zestaw` file (opened with `set.open`, not `archive.open`).
pub fn is_set_path(path: &str) -> bool {
    Path::new(path).extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(setfile::EXTENSION))
}

/// The data file's size and time, to notice a save made in another window.
type Stamp = (u64, Option<SystemTime>);

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

/// One archive of the set as it was read.
pub struct Part {
    pub key: String,
    pub name: String,
    /// Its id from `.heirloom/ustawienia.json`; empty when it has no settings file (another program's GEDCOM).
    pub id: String,
    /// Where it was found: the folder or the `.ged` file, as given to the set („Edytuj w jego archiwum” opens it).
    pub opened: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub data_path: Option<PathBuf>,
    /// `ok`, `missing` (not found), `unreadable`, or `other` (found, but its id isn't the one the set remembers).
    pub state: &'static str,
    pub error: Option<String>,
    stamp: Option<Stamp>,
    hash: String,
    /// GEDCOM 7 in memory (another program's 5.5.1 file is converted here, never on disk).
    doc: Option<Document>,
    display: Map<String, Value>,
    editors: Vec<EditorEntry>,
    uids: HashMap<String, String>,
    people: usize,
    families: usize,
}

impl Part {
    fn missing(entry: &Entry) -> Part {
        Part {
            key: entry.key.clone(),
            name: entry.name.clone(),
            id: entry.id.clone(),
            opened: None,
            root: None,
            data_path: None,
            state: "missing",
            error: None,
            stamp: None,
            hash: String::new(),
            doc: None,
            display: Map::new(),
            editors: Vec::new(),
            uids: HashMap::new(),
            people: 0,
            families: 0,
        }
    }

    /// Reads an archive (`Archive::open` never writes anything), keeps its records and drops the rest: the combined
    /// view holds no `Archive`, so there is nothing it could save.
    fn read(key: &str, path: &Path) -> Result<Part, ApiError> {
        let archive = Archive::open(path)?;
        let has_settings = archive.sidecar_path().join("ustawienia.json").is_file();
        let settings = archive.settings();
        let mut doc = archive.doc.clone();
        doc.upgrade_to_v7();
        let uids = doc
            .records
            .iter()
            .filter(|r| r.tag == "INDI")
            .filter_map(|r| Some((r.child_value("UID").or_else(|| r.child_value("_UID"))?.trim().to_string(), r.xref.clone()?)))
            .collect();
        Ok(Part {
            key: key.to_string(),
            name: settings.name.clone(),
            id: if has_settings { settings.archive_id.clone() } else { String::new() },
            opened: Some(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())),
            root: Some(archive.root().to_path_buf()),
            data_path: Some(archive.data_path()),
            state: "ok",
            error: None,
            stamp: stamp(&archive.data_path()),
            hash: archive.fingerprint_hex(),
            people: doc.records.iter().filter(|r| r.tag == "INDI").count(),
            families: doc.records.iter().filter(|r| r.tag == "FAM").count(),
            display: settings.display.clone(),
            editors: settings.editors.clone(),
            uids,
            doc: Some(doc),
        })
    }

    /// An entry of the set file, found where it was saved (or last opened).
    fn load(dir: &Path, entry: &Entry) -> Part {
        let Some(path) = setfile::locate(dir, entry) else { return Part::missing(entry) };
        match Part::read(&entry.key, &path) {
            Ok(mut part) => {
                if !entry.id.is_empty() && !part.id.is_empty() && part.id != entry.id {
                    part.state = "other";
                }
                part
            }
            Err(e) => Part { state: "unreadable", error: Some(e.message), opened: Some(path), ..Part::missing(entry) },
        }
    }

    fn same_data(&self, other: &Part) -> bool {
        let canon = |p: &Option<PathBuf>| p.as_ref().map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()));
        self.data_path.is_some() && canon(&self.data_path) == canon(&other.data_path)
    }

    /// The person's record in this archive (its own id).
    fn person(&self, xref: &str) -> Option<&Node> {
        self.doc.as_ref()?.record(xref).filter(|r| r.tag == "INDI")
    }
}

/// The merged document and what is worked out from it; rebuilt lazily after a decision.
struct Built {
    doc: Document,
    members: HashMap<String, Vec<String>>,
    member_records: HashMap<String, Node>,
    derived: Derived,
}

/// A link of the set file whose people can't both be found, or which would put two people of one archive together.
struct Lost {
    link: Link,
    reason: &'static str,
}

pub struct Combined {
    pub path: PathBuf,
    pub set: SetFile,
    pub parts: Vec<Part>,
    /// Every linked person's id that isn't the person's own (the first archive's record) → that id.
    alias: HashMap<String, String>,
    /// Per link of the set file (same order): the two ids it joins in this view, when both were found.
    resolved: Vec<Option<(String, String)>>,
    lost: Vec<Lost>,
    built: Option<Built>,
    /// „Do sprawdzenia”, worked out on the first `set.pairs`, then kept up to date by the decisions.
    pairs: Option<Vec<pairs::Pair>>,
    /// Families as last counted (the merged count once the view was built), while it waits for a rebuild.
    families: usize,
}

fn now() -> String {
    heirloom_core::history::now()
}

impl Combined {
    pub fn open(path: &Path) -> Result<Combined, ApiError> {
        let set = setfile::read(path)?;
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let parts: Vec<Part> = set.archives.iter().map(|e| Part::load(&dir, e)).collect();
        if !parts.iter().any(|p| p.doc.is_some()) {
            return Err(ApiError::new("set_empty", "Żadne archiwum z tego zestawu nie jest dostępne."));
        }
        Ok(Combined::new(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()), set, parts))
    }

    /// „Otwórz razem…”: writes a new set file of these archives (folders or GEDCOM files) and opens it. An existing set
    /// file (with its decisions) is replaced only when `replace` says the user confirmed that for this very file.
    pub fn create(path: &Path, name: &str, archives: &[String], replace: bool) -> Result<Combined, ApiError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(ApiError::new("bad_args", "Podaj nazwę zestawu."));
        }
        if archives.len() < 2 {
            return Err(ApiError::new("bad_args", "Wybierz co najmniej dwa archiwa."));
        }
        let mut path = std::path::absolute(path).map_err(|e| ApiError::new("io", e.to_string()))?;
        let named = is_set_path(&path.to_string_lossy());
        if !named {
            let file = format!("{}.{}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), setfile::EXTENSION);
            path.set_file_name(file);
        }
        if path.exists() && !(replace && named) {
            return Err(ApiError::new(
                "set_exists",
                format!("W tym miejscu jest już zestaw „{}”. Otwórz go z listy ostatnich albo wybierz inną nazwę lub miejsce.", path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()),
            ));
        }
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        let mut set = SetFile::new(name);
        let mut parts: Vec<Part> = Vec::new();
        for archive in archives {
            let key = set.next_key();
            let part = add_part(&dir, &mut set, &parts, &key, Path::new(archive))?;
            parts.push(part);
        }
        setfile::write(&path, &set)?;
        Ok(Combined::new(path, set, parts))
    }

    fn new(path: PathBuf, set: SetFile, parts: Vec<Part>) -> Combined {
        let families = parts.iter().map(|p| p.families).sum();
        let mut c = Combined { path, set, parts, alias: HashMap::new(), resolved: Vec::new(), lost: Vec::new(), built: None, pairs: None, families };
        c.resolve();
        c
    }

    fn dir(&self) -> PathBuf {
        self.path.parent().unwrap_or(Path::new("")).to_path_buf()
    }

    fn part(&self, key: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.key == key)
    }

    fn rank(&self, key: &str) -> usize {
        self.parts.iter().position(|p| p.key == key).unwrap_or(usize::MAX)
    }

    /// A person reference of the set file as an id of this view: UID first (it survives renumbering), else the record
    /// id, unless that record now has another UID (another program renumbered the file).
    fn find(&self, r: &PersonRef) -> Option<String> {
        let part = self.part(&r.archive)?;
        part.doc.as_ref()?;
        if let Some(xref) = r.uid.as_deref().and_then(|u| part.uids.get(u)) {
            return Some(combined_id(&r.archive, xref));
        }
        let record = part.person(&r.xref)?;
        let uid = record.child_value("UID").or_else(|| record.child_value("_UID")).map(str::trim);
        if r.uid.is_some() && uid.is_some() && uid != r.uid.as_deref() {
            return None;
        }
        Some(combined_id(&r.archive, &r.xref))
    }

    fn available(&self, r: &PersonRef) -> bool {
        self.part(&r.archive).is_some_and(|p| p.doc.is_some())
    }

    /// Works out who is one person from the set file's links (union-find); the view is rebuilt on the next read.
    fn resolve(&mut self) {
        let mut parent: HashMap<String, String> = HashMap::new();
        fn root(parent: &HashMap<String, String>, mut id: String) -> String {
            while let Some(p) = parent.get(&id) {
                if *p == id {
                    break;
                }
                id = p.clone();
            }
            id
        }
        // Each class's archives, by its root.
        let mut archives: HashMap<String, HashSet<String>> = HashMap::new();
        self.lost.clear();
        self.resolved.clear();
        for link in &self.set.links {
            // A link to an archive that isn't here (missing) waits; it isn't lost.
            if !self.available(&link.a) || !self.available(&link.b) {
                self.resolved.push(None);
                continue;
            }
            let (Some(a), Some(b)) = (self.find(&link.a), self.find(&link.b)) else {
                self.lost.push(Lost { link: link.clone(), reason: "not_found" });
                self.resolved.push(None);
                continue;
            };
            let (ra, rb) = (root(&parent, a.clone()), root(&parent, b.clone()));
            if ra == rb {
                self.resolved.push(Some((a, b)));
                continue;
            }
            let of = |r: &String, id: &String| archives.get(r).cloned().unwrap_or_else(|| HashSet::from([split_id(id).map(|(k, _)| k.to_string()).unwrap_or_default()]));
            let (sa, sb) = (of(&ra, &a), of(&rb, &b));
            if !sa.is_disjoint(&sb) {
                // Two people of one archive would be one: the archive was edited since. The later link waits.
                self.lost.push(Lost { link: link.clone(), reason: "same_archive" });
                self.resolved.push(None);
                continue;
            }
            parent.insert(rb.clone(), ra.clone());
            archives.remove(&rb);
            archives.insert(ra, sa.union(&sb).cloned().collect());
            self.resolved.push(Some((a, b)));
        }
        // Each class's own id: the member of the archive that comes first in the set.
        let mut classes: HashMap<String, Vec<String>> = HashMap::new();
        for id in parent.keys().chain(parent.values()).cloned().collect::<HashSet<_>>() {
            classes.entry(root(&parent, id.clone())).or_default().push(id);
        }
        self.alias.clear();
        for members in classes.values() {
            let first = members.iter().min_by_key(|id| (split_id(id).map_or(usize::MAX, |(k, _)| self.rank(k)), (*id).clone())).cloned().unwrap_or_default();
            for m in members.iter().filter(|m| **m != first) {
                self.alias.insert(m.clone(), first.clone());
            }
        }
        self.built = None;
    }

    /// Any member's id → the person's id in this view (ids from before a link keep working).
    pub fn canonical(&self, id: &str) -> String {
        self.alias.get(id).cloned().unwrap_or_else(|| id.to_string())
    }

    fn ensure(&mut self) {
        if self.built.is_some() {
            return;
        }
        let inputs: Vec<merge::Input> = self.parts.iter().filter_map(|p| Some(merge::Input { key: &p.key, doc: p.doc.as_ref()? })).collect();
        let keys: Vec<String> = inputs.iter().map(|i| i.key.to_string()).collect();
        let merged = merge::merge(&inputs, &self.alias);
        let options = derive::Display::from_settings(&self.display());
        heirloom_core::polish::set_numeric_dates(options.numeric_dates);
        let mut derived = derive::build_with(&merged.doc, &options);
        derived.origins = derived.view.model.persons.iter().map(|p| merged.origins.get(&p.xref).cloned().unwrap_or_default()).collect();
        derived.archive_keys = keys;
        self.families = derived.view.model.families.len();
        self.built = Some(Built { doc: merged.doc, members: merged.members, member_records: merged.member_records, derived });
    }

    pub fn derived(&mut self) -> &Derived {
        self.ensure();
        &self.built.as_ref().expect("built above").derived
    }

    pub fn doc(&mut self) -> &Document {
        self.ensure();
        &self.built.as_ref().expect("built above").doc
    }

    /// The first archive's display settings, with every archive's surname joins. Never written anywhere.
    fn display(&self) -> Map<String, Value> {
        let mut display = self.parts.iter().find(|p| p.doc.is_some()).map(|p| p.display.clone()).unwrap_or_default();
        let mut joins = Map::new();
        for p in self.parts.iter().filter(|p| p.doc.is_some()) {
            if let Some(Value::Object(j)) = p.display.get("surnameJoins") {
                for (k, v) in j {
                    joins.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }
        display.insert("surnameJoins".into(), Value::Object(joins));
        display
    }

    pub fn media_roots(&self, cache_root: Option<&Path>) -> Vec<media::Roots> {
        self.parts
            .iter()
            .filter_map(|p| {
                let root = p.root.clone().filter(|_| p.doc.is_some())?;
                // Thumbnails per archive, as when it is opened alone (an archive without settings: per set and key).
                let id = if p.id.is_empty() { format!("{}-{}", self.set.id, p.key) } else { p.id.clone() };
                Some(media::Roots { archive: root, thumbs: cache_root.map(|c| c.join(id).join("miniatury")), key: Some(p.key.clone()) })
            })
            .collect()
    }

    pub fn file_roots(&self) -> FileRoots {
        FileRoots::Many(self.parts.iter().filter_map(|p| Some((p.key.clone(), p.root.clone()?))).collect())
    }

    pub fn people(&self) -> usize {
        self.parts.iter().map(|p| p.people).sum::<usize>() - self.alias.len()
    }

    fn changed_on_disk(&self) -> bool {
        self.parts.iter().any(|p| p.doc.is_some() && p.data_path.as_deref().map(stamp) != Some(p.stamp))
    }

    /// `archive.status` of the combined view: the shape of an archive's (read-only), plus `combined`.
    pub fn status(&self) -> Value {
        let mut editors: Vec<Value> = Vec::new();
        for e in self.parts.iter().flat_map(|p| &p.editors) {
            if !editors.iter().any(|x| x["name"] == e.name.as_str()) {
                editors.push(json!({ "name": e.name, "lastEdited": e.last_edited }));
            }
        }
        let warnings: Vec<Value> = self
            .parts
            .iter()
            .filter_map(|p| {
                let message = match p.state {
                    "missing" => format!("Nie znaleziono archiwum „{}”. Jego osób nie widać.", p.name),
                    "unreadable" => format!("Nie udało się odczytać archiwum „{}”: {}", p.name, p.error.as_deref().unwrap_or("")),
                    "other" => format!("Archiwum „{}” ma inny identyfikator niż zapisany w zestawie — to może być inne archiwum.", p.name),
                    _ => return None,
                };
                Some(json!({ "line": 0, "message": message }))
            })
            .collect();
        let archives: Vec<Value> = self
            .parts
            .iter()
            .enumerate()
            .map(|(n, p)| {
                let entry = self.set.entry(&p.key);
                json!({
                    "key": p.key,
                    "id": p.id,
                    "name": p.name,
                    "colour": n + 1,
                    "state": p.state,
                    "error": p.error,
                    "people": p.people,
                    "root": p.root.as_ref().map(|r| r.display().to_string()),
                    "path": p.opened.as_ref().map(|r| r.display().to_string()).or_else(|| entry.map(|e| e.last_path.clone())),
                    "dataPath": p.data_path.as_ref().map(|r| r.display().to_string()),
                })
            })
            .collect();
        let linked = self.alias.values().collect::<HashSet<_>>().len();
        json!({
            "archiveId": self.set.id,
            "name": self.set.name,
            "root": self.dir().display().to_string(),
            "dataFile": self.path.file_name().map(|n| n.to_string_lossy().into_owned()),
            "dataPath": self.path.display().to_string(),
            "origin": Value::Null,
            "foreign": false,
            "foreignConfirmed": false,
            "readOnly": true,
            "unsavedChanges": 0,
            "canUndo": false,
            "undoDepth": 0,
            "canRedo": false,
            "changedOnDisk": self.changed_on_disk(),
            "warnings": warnings,
            "encoding": "UTF-8",
            "people": self.people(),
            "families": self.built.as_ref().map_or(self.families, |b| b.derived.view.model.families.len()),
            "editors": editors,
            "display": self.display(),
            "lastSaved": Value::Null,
            "backupsToKeep": 0,
            "backupBeforeSave": false,
            "lastBackup": Value::Null,
            "dataBytes": Value::Null,
            "gedcomVersion": Value::Null,
            "sidecar": "",
            "combined": {
                "setPath": self.path.display().to_string(),
                "archives": archives,
                "linked": linked,
                "lost": self.lost.len(),
                "pending": self.pairs.as_ref().map(|p| self.pending(p).count()),
            },
        })
    }

    /// The person's records in the archives, for the profile („Ta osoba jest w 2 archiwach”, „Różnice”, „Edytuj w jego
    /// archiwum”). `id` is the person's id in this view.
    pub fn person_info(&mut self, id: &str) -> Value {
        self.ensure();
        let built = self.built.as_ref().expect("built above");
        let ids = built.members.get(id).cloned().unwrap_or_else(|| vec![id.to_string()]);
        let mut records: Vec<(&str, &Node)> = Vec::new();
        let mut members = Vec::new();
        for member in &ids {
            let Some((key, xref)) = split_id(member) else { continue };
            let Some(part) = self.part(key) else { continue };
            let record = built.member_records.get(member).or_else(|| part.person(&xref).and_then(|_| built.doc.record(member)));
            let person = record.map(|r| model::person(r, member, Version::V7));
            if let Some(r) = record {
                records.push((key, r));
            }
            members.push(json!({
                "archive": key,
                "archiveName": part.name,
                "path": part.opened.as_ref().map(|p| p.display().to_string()),
                "id": xref,
                "combinedId": member,
                "name": person.as_ref().map(model::Person::display_name),
                "years": person.as_ref().map(merge::years),
            }));
        }
        let differences = if records.len() > 1 { merge::differences(&records) } else { json!([]) };
        json!({ "members": members, "differences": differences })
    }

    fn pending<'a>(&'a self, pairs: &'a [pairs::Pair]) -> impl Iterator<Item = &'a pairs::Pair> + 'a {
        let rejected = self.rejected();
        pairs.iter().filter(move |p| {
            let (a, b) = (self.canonical(&p.a), self.canonical(&p.b));
            a != b && !rejected.contains(&(a.clone(), b.clone())) && !rejected.contains(&(b, a))
        })
    }

    /// The pairs answered „nie”, as ids of this view.
    fn rejected(&self) -> HashSet<(String, String)> {
        self.set.rejected.iter().filter_map(|l| Some((self.canonical(&self.find(&l.a)?), self.canonical(&self.find(&l.b)?)))).collect()
    }

    fn lost_json(&self) -> Value {
        let side = |r: &PersonRef| json!({ "archive": r.archive, "archiveName": self.part(&r.archive).map(|p| p.name.clone()), "xref": r.xref, "name": r.name });
        json!(self.lost.iter().enumerate().map(|(index, l)| json!({ "index": index, "a": side(&l.link.a), "b": side(&l.link.b), "reason": l.reason })).collect::<Vec<_>>())
    }

    fn pairs_json(&self) -> Value {
        let Some(pairs) = &self.pairs else { return Value::Null };
        json!(self.pending(pairs).map(pairs::Pair::json).collect::<Vec<_>>())
    }

    /// A person of this view as the set file names them: archive key, own id, UID, a label.
    fn person_ref(&self, id: &str) -> Result<PersonRef, ApiError> {
        let missing = || ApiError::new("not_found", format!("Nie ma osoby {id} w tym zestawie."));
        let (key, xref) = split_id(id).ok_or_else(missing)?;
        let record = self.part(key).and_then(|p| p.person(&xref)).ok_or_else(missing)?;
        let person = model::person(record, &xref, Version::V7);
        let years = merge::years(&person);
        Ok(PersonRef {
            archive: key.to_string(),
            uid: person.uid.clone(),
            name: if years.is_empty() { person.display_name() } else { format!("{} ({years})", person.display_name()) },
            xref,
            extra: Map::new(),
        })
    }

    /// The archives of every member of `id`'s person.
    fn class_archives(&self, id: &str) -> HashSet<String> {
        let first = self.canonical(id);
        std::iter::once(first.clone())
            .chain(self.alias.iter().filter(|(_, c)| **c == first).map(|(m, _)| m.clone()))
            .filter_map(|m| split_id(&m).map(|(k, _)| k.to_string()))
            .collect()
    }

    /// Writes one change to the set file (re-read first) and takes the file as it is then. Refused (and the view read
    /// again) when an archive shown here is no longer the one its key names in the file: another window removed it and
    /// added another under that key, or pointed the key elsewhere; a decision about `b`'s people must not land on them.
    fn modify<T>(&mut self, change: impl FnOnce(&mut SetFile) -> Result<T, ApiError>) -> Result<T, ApiError> {
        let dir = self.dir();
        let shown: Vec<(String, Option<PathBuf>)> = self.parts.iter().filter(|p| p.doc.is_some()).map(|p| (p.key.clone(), p.opened.clone())).collect();
        let result = setfile::modify(&self.path, |set| {
            let found = |key: &str| set.entry(key).and_then(|e| setfile::locate(&dir, e));
            if shown.iter().any(|(key, opened)| !same_place(found(key).as_deref(), opened.as_deref())) {
                return Err(ApiError::new("set_changed", "Zestaw zmienił się w międzyczasie (np. w innym oknie) — widok jest już odświeżony. Zdecyduj jeszcze raz."));
            }
            change(set)
        });
        match result {
            Ok((set, out)) => {
                self.set = set;
                Ok(out)
            }
            Err(e) => {
                if e.code == "set_changed" {
                    let _ = self.refresh();
                }
                Err(e)
            }
        }
    }

    /// The `set.*` commands of an open set (`set.create` and `set.open` are `Api`'s).
    pub fn call(&mut self, method: &str, args: &Value) -> ApiResult {
        match method {
            "set.pairs" => {
                if self.pairs.is_none() || args.get("refresh").and_then(Value::as_bool) == Some(true) {
                    let rejected = self.rejected();
                    let found = pairs::find(self.derived(), &rejected);
                    self.pairs = Some(found);
                }
                let certain = self.pairs.as_ref().map_or(0, |p| self.pending(p).filter(|p| p.percent >= 90).count());
                Ok(json!({ "pairs": self.pairs_json(), "lost": self.lost_json(), "certain": certain }))
            }
            "set.compare" => {
                let (a, b) = (self.canonical(&str_arg(args, "a")?), self.canonical(&str_arg(args, "b")?));
                let d = self.derived();
                let (i, j) = (d.index(&a).ok_or_else(|| crate::people::not_found(&a))?, d.index(&b).ok_or_else(|| crate::people::not_found(&b))?);
                let same_archive = d.origins[i].iter().any(|k| d.origins[j].contains(k));
                let c = if same_archive || i == j { None } else { pairs::compare_two(d, i, j) };
                Ok(json!({
                    "a": pairs::side(d, i),
                    "b": pairs::side(d, j),
                    "percent": c.as_ref().map_or(0, |c| c.percent),
                    "score": c.as_ref().map_or(0.0, |c| c.score),
                    "reasons": c.map(|c| c.reasons).unwrap_or_default(),
                    "sameArchive": same_archive,
                    "linked": i == j,
                }))
            }
            "set.decide" => self.decide(args),
            "set.linkAll" => self.link_all(args),
            "set.unlink" => self.unlink(args),
            "set.forgetLost" => {
                let index = args.get("index").and_then(Value::as_u64).ok_or_else(|| ApiError::bad_args("index"))? as usize;
                let link = self.lost.get(index).map(|l| l.link.clone()).ok_or_else(|| ApiError::bad_args("index"))?;
                self.modify(|set| {
                    set.links.retain(|l| !l.joins(&link.a, &link.b));
                    Ok(())
                })?;
                self.resolve();
                Ok(self.status())
            }
            "set.addArchive" => {
                let path = str_arg(args, "path")?;
                let dir = self.dir();
                let mut part = None;
                let parts = &self.parts;
                self.set = setfile::modify(&self.path, |set| {
                    let key = set.next_key();
                    part = Some(add_part(&dir, set, parts, &key, Path::new(&path))?);
                    Ok(())
                })?
                .0;
                self.parts.extend(part);
                self.pairs = None;
                self.resolve();
                Ok(self.status())
            }
            "set.removeArchive" => {
                let key = str_arg(args, "key")?;
                if self.part(&key).is_none() {
                    return Err(ApiError::bad_args("key"));
                }
                if self.parts.len() < 2 {
                    return Err(ApiError::new("bad_args", "Zestaw musi mieć co najmniej jedno archiwum."));
                }
                self.modify(|set| {
                    set.archives.retain(|a| a.key != key);
                    set.links.retain(|l| l.a.archive != key && l.b.archive != key);
                    set.rejected.retain(|l| l.a.archive != key && l.b.archive != key);
                    Ok(())
                })?;
                self.parts.retain(|p| p.key != key);
                self.pairs = None;
                self.resolve();
                Ok(self.status())
            }
            "set.locate" => {
                let (key, path) = (str_arg(args, "key")?, str_arg(args, "path")?);
                if self.part(&key).is_none() {
                    return Err(ApiError::bad_args("key"));
                }
                if is_set_path(&path) {
                    return Err(ApiError::new("bad_args", "Wskaż folder archiwum albo plik GEDCOM, nie zestaw."));
                }
                let mut part = Part::read(&key, Path::new(&path))?;
                if self.parts.iter().any(|p| p.key != key && p.same_data(&part)) {
                    return Err(ApiError::new("already_in_set", "To archiwum już jest w tym zestawie."));
                }
                let dir = self.dir();
                let opened = part.opened.clone().unwrap_or_default();
                let different = self.set.entry(&key).is_some_and(|e| !e.id.is_empty() && !part.id.is_empty() && e.id != part.id);
                let (id, name) = (part.id.clone(), part.name.clone());
                self.modify(|set| {
                    let entry = set.archives.iter_mut().find(|a| a.key == key).ok_or_else(|| ApiError::bad_args("key"))?;
                    entry.path = setfile::relative_path(&dir, &opened);
                    entry.last_path = opened.display().to_string();
                    entry.name = name;
                    // The user pointed at this archive: from now on it is the one the set means.
                    if !id.is_empty() {
                        entry.id = id;
                    }
                    Ok(())
                })?;
                part.state = "ok";
                if let Some(slot) = self.parts.iter_mut().find(|p| p.key == key) {
                    *slot = part;
                }
                self.pairs = None;
                self.resolve();
                let mut status = self.status();
                status["different"] = json!(different);
                Ok(status)
            }
            "set.refresh" => self.refresh(),
            _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
        }
    }

    fn checked_pair(&self, args: &Value) -> Result<(PersonRef, PersonRef, String, String), ApiError> {
        let (a, b) = (str_arg(args, "a")?, str_arg(args, "b")?);
        let (ra, rb) = (self.person_ref(&a)?, self.person_ref(&b)?);
        if ra.archive == rb.archive {
            return Err(same_archive());
        }
        Ok((ra, rb, a, b))
    }

    /// „To ta sama osoba?” answered: `yes` links the two people, `no` is remembered so they aren't asked about again.
    fn decide(&mut self, args: &Value) -> ApiResult {
        let (ra, rb, a, b) = self.checked_pair(args)?;
        let by = opt_str(args, "by").unwrap_or_default();
        let link = Link {
            a: ra.clone(),
            b: rb.clone(),
            how: Some(opt_str(args, "how").unwrap_or_else(|| "suggested".into())),
            percent: args.get("percent").and_then(Value::as_u64).map(|p| p as u32),
            by,
            at: now(),
            extra: Map::new(),
        };
        match str_arg(args, "answer")?.as_str() {
            "yes" => {
                if self.canonical(&a) != self.canonical(&b) {
                    if !self.class_archives(&a).is_disjoint(&self.class_archives(&b)) {
                        return Err(same_archive());
                    }
                    self.modify(|set| {
                        set.rejected.retain(|l| !l.joins(&ra, &rb));
                        if !set.links.iter().any(|l| l.joins(&ra, &rb)) {
                            set.links.push(link);
                        }
                        Ok(())
                    })?;
                    self.after_link(&a, &b);
                }
            }
            "no" => {
                self.modify(|set| {
                    if !set.rejected.iter().any(|l| l.joins(&ra, &rb)) {
                        set.rejected.push(Link { how: None, percent: None, ..link });
                    }
                    Ok(())
                })?;
            }
            _ => return Err(ApiError::bad_args("answer")),
        }
        self.resolve();
        Ok(json!({ "status": self.status(), "pairs": self.pairs_json() }))
    }

    /// The linked people's families come up next in „Do sprawdzenia” (from the view as it was before the link).
    fn after_link(&mut self, a: &str, b: &str) {
        let rejected = self.rejected();
        let (Some(built), Some(pairs)) = (&self.built, &mut self.pairs) else { return };
        let d = &built.derived;
        if let (Some(i), Some(j)) = (d.index(&self.alias.get(a).cloned().unwrap_or_else(|| a.to_string())), d.index(&self.alias.get(b).cloned().unwrap_or_else(|| b.to_string()))) {
            pairs::after_link(d, pairs, i, j, &rejected);
        }
    }

    /// „Połącz wszystkie ≥ 90 %”: every pending pair at or above `minPercent`, in one write.
    fn link_all(&mut self, args: &Value) -> ApiResult {
        let min = args.get("minPercent").and_then(Value::as_u64).ok_or_else(|| ApiError::bad_args("minPercent"))? as u32;
        let by = opt_str(args, "by").unwrap_or_default();
        if self.pairs.is_none() {
            let rejected = self.rejected();
            let found = pairs::find(self.derived(), &rejected);
            self.pairs = Some(found);
        }
        let chosen: Vec<(String, String, u32)> = self.pairs.as_ref().map(|p| self.pending(p).filter(|p| p.percent >= min).map(|p| (p.a.clone(), p.b.clone(), p.percent)).collect()).unwrap_or_default();
        // Each pair joins two people who are no one else's yet, so no person gets two records of one archive.
        let mut used: HashSet<String> = HashSet::new();
        let mut links = Vec::new();
        let at = now();
        for (a, b, percent) in chosen {
            let (ca, cb) = (self.canonical(&a), self.canonical(&b));
            if used.contains(&ca) || used.contains(&cb) || !self.class_archives(&a).is_disjoint(&self.class_archives(&b)) {
                continue;
            }
            used.extend([ca, cb]);
            let (Ok(ra), Ok(rb)) = (self.person_ref(&a), self.person_ref(&b)) else { continue };
            links.push(Link { a: ra, b: rb, how: Some("bulk".into()), percent: Some(percent), by: by.clone(), at: at.clone(), extra: Map::new() });
        }
        let linked = links.len();
        self.modify(|set| {
            for link in links {
                if !set.links.iter().any(|l| l.joins(&link.a, &link.b)) {
                    set.links.push(link);
                }
            }
            Ok(())
        })?;
        self.resolve();
        Ok(json!({ "linked": linked, "status": self.status(), "pairs": self.pairs_json() }))
    }

    /// „Rozłącz”: the record `id` (a member's id) is no longer one person with anyone: every link touching it goes.
    fn unlink(&mut self, args: &Value) -> ApiResult {
        let id = str_arg(args, "id")?;
        let touching: Vec<Link> = self
            .set
            .links
            .iter()
            .zip(&self.resolved)
            .filter(|(_, r)| r.as_ref().is_some_and(|(a, b)| *a == id || *b == id))
            .map(|(l, _)| l.clone())
            .collect();
        if touching.is_empty() {
            return Err(ApiError::new("not_linked", "Ta osoba nie jest połączona z nikim z innego archiwum."));
        }
        self.modify(|set| {
            set.links.retain(|l| !touching.iter().any(|t| l.joins(&t.a, &t.b)));
            Ok(())
        })?;
        self.pairs = None;
        self.resolve();
        Ok(self.status())
    }

    /// Back in this window after work in another: archives saved meanwhile are read again, and so is the set file
    /// (decisions made in another window).
    fn refresh(&mut self) -> ApiResult {
        let mut changed = Vec::new();
        let mut any = false;
        if let Ok(set) = setfile::read(&self.path) {
            if set != self.set {
                self.set = set;
                any = true;
            }
        }
        let dir = self.dir();
        for entry in self.set.archives.clone() {
            let index = self.parts.iter().position(|p| p.key == entry.key);
            // Unchanged: still the archive the entry names (another window may have pointed the key at another one
            // with „Wskaż folder…”, or removed it and added another under that key) and not saved since.
            let unchanged = index.is_some_and(|n| {
                let p = &self.parts[n];
                p.doc.is_some() && same_place(setfile::locate(&dir, &entry).as_deref(), p.opened.as_deref()) && p.data_path.as_deref().map(stamp) == Some(p.stamp)
            });
            if unchanged {
                continue;
            }
            let part = Part::load(&dir, &entry);
            match index {
                Some(n) => {
                    let old = &self.parts[n];
                    if part.doc.is_some() != old.doc.is_some() || part.hash != old.hash {
                        changed.push(part.name.clone());
                        any = true;
                    }
                    self.parts[n] = part;
                }
                None => {
                    changed.push(part.name.clone());
                    self.parts.push(part);
                    any = true;
                }
            }
        }
        // Archives taken out of the set in another window.
        let before = self.parts.len();
        self.parts.retain(|p| self.set.entry(&p.key).is_some());
        any |= self.parts.len() != before;
        if any {
            self.pairs = None;
            self.resolve();
        }
        Ok(json!({ "changed": changed, "status": self.status() }))
    }
}

/// Both name one existing file or folder (however written: `Rodzina/../Inne` is `Inne`).
fn same_place(a: Option<&Path>, b: Option<&Path>) -> bool {
    let canon = |p: Option<&Path>| p.and_then(|p| p.canonicalize().ok());
    canon(a).is_some_and(|a| Some(a) == canon(b))
}

/// Reads an archive for the set and adds its entry (refusing one that is already in it).
fn add_part(dir: &Path, set: &mut SetFile, parts: &[Part], key: &str, path: &Path) -> Result<Part, ApiError> {
    if is_set_path(&path.to_string_lossy()) {
        return Err(ApiError::new("bad_args", "Zestawu nie można dodać do zestawu — wybierz archiwum."));
    }
    let part = Part::read(key, path).map_err(|e| ApiError::new(e.code, format!("Nie udało się otworzyć archiwum {}: {}", path.display(), e.message)))?;
    let opened = part.opened.clone().unwrap_or_default();
    // Also an entry this window hasn't read yet (added in another window meanwhile).
    let listed = set.archives.iter().any(|e| same_place(setfile::locate(dir, e).as_deref(), Some(&opened)));
    if listed || parts.iter().any(|p| p.same_data(&part)) {
        return Err(ApiError::new("already_in_set", format!("Archiwum „{}” jest już w tym zestawie.", part.name)));
    }
    set.archives.push(Entry {
        key: key.to_string(),
        id: part.id.clone(),
        name: part.name.clone(),
        path: setfile::relative_path(dir, &opened),
        last_path: opened.display().to_string(),
        extra: Map::new(),
    });
    Ok(part)
}

#[cfg(test)]
mod tests;
