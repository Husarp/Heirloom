//! Where the archive's photos and documents lie in `media/` (design 0.7.0, part B): all together („Wszystkie razem”)
//! or in a folder for each person („W folderach osób”). What is SHOWN never depends on it: a photo is found through its
//! FILE path, the archive's folders and then the „moved” map. The layout decides only where new files go, what
//! „Uporządkuj” does and what the check on open reports.
//!
//! This module reads: the owner of a file, the folder of a person, the scan and its report. The engine that moves
//! files (plan, journal, rollback, undo) is in [`engine`].

pub mod engine;

use crate::edit::Changes;
use crate::fsx::{self, name_key};
use crate::media::moved_key;
use crate::{ApiError, ApiResult, Session};
use heirloom_core::fold::fold;
use heirloom_core::gedcom::{Document, Node};
use heirloom_core::history::Action;
use heirloom_core::polish::surname_key;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::derive::Derived;

/// `display.mediaLayout`.
pub const SETTING: &str = "mediaLayout";
/// `display.mediaLeftAlone`: `[{ path, kind: "file" | "folder", count, newest }]`.
pub const LEFT_ALONE: &str = "mediaLeftAlone";
/// The scan stops after this many entries.
pub const MAX_ENTRIES: usize = 50_000;
const MEDIA: &str = "media";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Flat,
    PerPerson,
}

impl Layout {
    pub fn code(self) -> &'static str {
        match self {
            Layout::Flat => "flat",
            Layout::PerPerson => "perPerson",
        }
    }

    pub fn parse(text: &str) -> Option<Layout> {
        match text {
            "flat" => Some(Layout::Flat),
            "perPerson" => Some(Layout::PerPerson),
            _ => None,
        }
    }
}

/// The layout the archive says it has (`display.mediaLayout`).
pub fn setting(s: &Session) -> Option<Layout> {
    s.archive.settings().display.get(SETTING).and_then(Value::as_str).and_then(Layout::parse)
}

fn slash(text: &str) -> String {
    text.replace('\\', "/")
}

/// A path written with a drive letter, a UNC share or a leading slash (on any system).
fn is_abs(file: &str) -> bool {
    let b = file.as_bytes();
    Path::new(file).is_absolute() || file.starts_with('/') || file.starts_with("\\\\") || (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic())
}

/// A FILE path as a path inside the archive (`/` separators): a relative path as it is, an absolute one that points
/// into one of the folders cut down to its inside part, and None for a path outside the archive.
fn rel_in_layers(file: &str, layers: &[PathBuf]) -> Option<String> {
    let file = slash(file);
    let file = file.trim_start_matches("./");
    if !is_abs(file) {
        return Some(file.to_string());
    }
    for layer in layers {
        let root = slash(&layer.to_string_lossy());
        let root = root.trim_end_matches('/');
        if file.len() > root.len() && file.is_char_boundary(root.len()) && name_key(&file[..root.len()]) == name_key(root) && file.as_bytes()[root.len()] == b'/' {
            return Some(file[root.len() + 1..].to_string());
        }
    }
    None
}

/// For a path inside `media/`: the first subfolder (None: directly in `media/`) and the file's name.
fn split_media(rel: &str) -> Option<(Option<&str>, &str)> {
    let rest = rel.get(..MEDIA.len() + 1).filter(|p| p[..MEDIA.len()].eq_ignore_ascii_case(MEDIA) && p.ends_with('/')).map(|_| &rel[MEDIA.len() + 1..])?;
    match rest.split_once('/') {
        Some((top, _)) if !top.is_empty() => Some((Some(top), rest.rsplit('/').next().unwrap_or(rest))),
        _ => Some((None, rest)),
    }
}

/// Letters and digits of a text, folded (ł → l, no accents), as words.
fn tokens(text: &str) -> Vec<String> {
    let nfc = name_key(text);
    fold(&nfc).split(|c: char| !c.is_alphanumeric()).filter(|t| !t.is_empty()).map(str::to_string).collect()
}

/// A file name or a path decoded from a FILE value, for the files the user removed from Heirloom.
fn decode_value(raw: &str) -> String {
    let bytes = raw.trim().as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%').then(|| bytes.get(i + 1..i + 3)).flatten().and_then(|h| std::str::from_utf8(h).ok()).and_then(|h| u8::from_str_radix(h, 16).ok());
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
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

// ---------- people and files ----------

struct Person {
    xref: String,
    name: String,
    years: Option<String>,
    given: String,
    surname: String,
    /// The first given name, folded.
    first_given: String,
    surname_keys: Vec<String>,
    birth_year: Option<i32>,
    death_year: Option<i32>,
}

/// One object record (`0 @O1@ OBJE`) with a FILE.
struct Obj {
    id: String,
    /// The FILE value as written.
    raw: String,
    /// Decoded, `/` separators.
    file: String,
    /// The path inside the archive, None for a file outside it.
    rel: Option<String>,
    key: String,
    profile_owner: Option<usize>,
    first_linker: Option<usize>,
    linkers: Vec<usize>,
}

/// The people and the files of the open data, as the layout needs them.
struct Tree {
    persons: Vec<Person>,
    by_given: HashMap<String, Vec<usize>>,
    objs: Vec<Obj>,
    /// Compared key of a path inside the archive → objects.
    by_key: HashMap<String, Vec<usize>>,
    by_id: HashMap<String, usize>,
    /// Files written inside a person (`@I1@#0`): referenced, never moved.
    embedded: HashSet<String>,
    person_index: HashMap<String, usize>,
}

impl Tree {
    fn build(doc: &Document, d: &Derived, layers: &[PathBuf]) -> Tree {
        let mut persons = Vec::with_capacity(d.info.len());
        let mut by_given: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, info) in d.info.iter().enumerate() {
            let year = |date: &Option<crate::derive::DateInfo>| date.as_ref().and_then(|x| x.value).map(|v| v.start.year);
            let mut surname_keys: Vec<String> = Vec::new();
            for text in [Some(info.surname.as_str()), info.maiden.as_deref()].into_iter().flatten() {
                for t in tokens(text) {
                    let key = surname_key(&t);
                    if !surname_keys.contains(&key) {
                        surname_keys.push(key);
                    }
                }
            }
            let first_given = tokens(&info.given).into_iter().next().unwrap_or_default();
            if !first_given.is_empty() {
                by_given.entry(first_given.clone()).or_default().push(i);
            }
            let years = match (&info.birth, &info.death) {
                (None, None) => None,
                (b, dth) => Some(format!("{}–{}", b.as_ref().map_or("", |x| x.year.as_str()), dth.as_ref().map_or("", |x| x.year.as_str()))),
            };
            persons.push(Person {
                xref: d.xref(i).to_string(),
                name: info.name.clone(),
                years,
                given: info.given.clone(),
                surname: if info.surname.is_empty() { info.maiden.clone().unwrap_or_default() } else { info.surname.clone() },
                first_given,
                surname_keys,
                birth_year: year(&info.birth),
                death_year: year(&info.death),
            });
        }
        let person_index = persons.iter().enumerate().map(|(i, p)| (p.xref.clone(), i)).collect();
        let mut objs: Vec<Obj> = Vec::new();
        let mut by_id = HashMap::new();
        for record in doc.records.iter().filter(|r| r.tag == "OBJE") {
            let (Some(id), Some(raw)) = (record.xref.as_deref(), record.child("FILE").and_then(|f| f.value.as_deref())) else { continue };
            let file = d.view.media.get(id).and_then(|m| m.file.clone()).unwrap_or_else(|| decode_value(raw));
            let file = slash(&file);
            let rel = rel_in_layers(&file, layers);
            let key = rel.as_deref().map_or_else(|| name_key(&file), name_key);
            by_id.insert(id.to_string(), objs.len());
            objs.push(Obj { id: id.to_string(), raw: raw.to_string(), file, rel, key, profile_owner: None, first_linker: None, linkers: Vec::new() });
        }
        for (pi, details) in d.view.people.iter().enumerate() {
            for link in &details.media {
                let Some(&oi) = by_id.get(&link.object) else { continue };
                let obj = &mut objs[oi];
                if !obj.linkers.contains(&pi) {
                    obj.linkers.push(pi);
                }
                obj.first_linker.get_or_insert(pi);
                if link.profile {
                    obj.profile_owner.get_or_insert(pi);
                }
            }
        }
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, o) in objs.iter().enumerate() {
            by_key.entry(o.key.clone()).or_default().push(i);
        }
        let embedded = d.view.media_order.iter().filter(|id| id.contains('#')).filter_map(|id| d.view.media.get(id)?.file.as_ref()).filter_map(|f| rel_in_layers(f, layers)).map(|f| name_key(&f)).collect();
        Tree { persons, by_given, objs, by_key, by_id, embedded, person_index }
    }

    /// The owner of a file: the person whose profile photo it is, else the first person (file order) that links it.
    fn file_owner(&self, objs: &[usize]) -> Option<usize> {
        objs.iter().find_map(|&o| self.objs[o].profile_owner).or_else(|| objs.iter().find_map(|&o| self.objs[o].first_linker))
    }

    fn file_linkers(&self, objs: &[usize]) -> Vec<usize> {
        let mut all: Vec<usize> = Vec::new();
        for &o in objs {
            for &p in &self.objs[o].linkers {
                if !all.contains(&p) {
                    all.push(p);
                }
            }
        }
        all
    }

    /// People that own a file or link one: the ones who may need a folder.
    fn linked_persons(&self) -> Vec<usize> {
        let mut seen = HashSet::new();
        self.objs.iter().flat_map(|o| o.linkers.iter().copied()).filter(|p| seen.insert(*p)).collect()
    }

    fn person_json(&self, i: usize) -> Value {
        let p = &self.persons[i];
        json!({ "id": p.xref, "name": p.name, "years": p.years })
    }

    /// Whether a person's name is in a text: the first given name and a token of the surname.
    fn name_in(&self, p: usize, words: &[String]) -> bool {
        let person = &self.persons[p];
        !person.first_given.is_empty() && words.contains(&person.first_given) && words.iter().any(|w| person.surname_keys.contains(&surname_key(w)))
    }

    /// Everyone whose name is in the words (given name looked up, surname checked).
    fn named_in(&self, words: &[String]) -> Vec<usize> {
        let mut found: Vec<usize> = Vec::new();
        for word in words {
            for &p in self.by_given.get(word).into_iter().flatten() {
                if !found.contains(&p) && self.name_in(p, words) {
                    found.push(p);
                }
            }
        }
        found.sort_unstable();
        found
    }
}

// ---------- folders of people (B.2) ----------

/// The folder of each person in `media/`.
pub struct Folders {
    /// Person → folder name (an existing folder by rule (1) or (2), else a new name by rule (3)).
    by_person: HashMap<usize, String>,
    /// Existing folder (compared name) → the person it is the folder of (rules (1) and (2) only).
    owner_of: HashMap<String, usize>,
    /// Compared name → name as written, for every subfolder of `media/` in any folder of the archive.
    existing: BTreeMap<String, String>,
}

impl Folders {
    fn build(t: &Tree, layers: &[PathBuf], include: &[usize]) -> Folders {
        let mut existing: BTreeMap<String, String> = BTreeMap::new();
        for layer in layers {
            for entry in std::fs::read_dir(layer.join(MEDIA)).into_iter().flatten().flatten() {
                if entry.file_type().is_ok_and(|k| k.is_dir()) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    existing.entry(name_key(&name)).or_insert(name);
                }
            }
        }
        for o in &t.objs {
            if let Some((Some(top), _)) = o.rel.as_deref().and_then(split_media) {
                existing.entry(name_key(top)).or_insert_with(|| top.to_string());
            }
        }
        // (1) The folder that holds most of the files a person owns.
        let mut total: HashMap<usize, usize> = HashMap::new();
        let mut count: HashMap<(usize, String), usize> = HashMap::new();
        for objs in t.by_key.values() {
            let Some(owner) = t.file_owner(objs) else { continue };
            let Some((top, _)) = t.objs[objs[0]].rel.as_deref().and_then(split_media) else { continue };
            *total.entry(owner).or_default() += 1;
            if let Some(top) = top {
                *count.entry((owner, name_key(top))).or_default() += 1;
            }
        }
        let mut candidates: Vec<(usize, usize, String)> = count.into_iter().filter(|((p, _), n)| n * 2 > total[p]).map(|((p, f), n)| (n, p, f)).collect();
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut by_person: HashMap<usize, String> = HashMap::new();
        let mut owner_of: HashMap<String, usize> = HashMap::new();
        for (_, p, folder) in candidates {
            if by_person.contains_key(&p) || owner_of.contains_key(&folder) {
                continue;
            }
            by_person.insert(p, existing[&folder].clone());
            owner_of.insert(folder, p);
        }
        // (2) A folder named after a person, when nobody else's name fits it too.
        for (key, name) in &existing {
            if owner_of.contains_key(key) {
                continue;
            }
            let words = tokens(name);
            let named = t.named_in(&words);
            if let [p] = named.as_slice() {
                if include.contains(p) && !by_person.contains_key(p) {
                    by_person.insert(*p, name.clone());
                    owner_of.insert(key.clone(), *p);
                }
            }
        }
        // (3) A new name, „Nazwisko Imię”: the people who own files first, so a namesake without files doesn't take it.
        let mut taken: HashSet<String> = existing.keys().cloned().collect();
        taken.extend(by_person.values().map(|n| name_key(n)));
        let owners: HashSet<usize> = total.keys().copied().collect();
        let mut order: Vec<usize> = include.to_vec();
        order.sort_by_key(|p| (!owners.contains(p), *p));
        order.dedup();
        for p in order {
            if by_person.contains_key(&p) {
                continue;
            }
            let person = &t.persons[p];
            let base = fsx::folder_name(&format!("{} {}", person.surname, person.given));
            let mut name = base.clone();
            if taken.contains(&name_key(&name)) {
                if let Some(year) = person.birth_year {
                    name = format!("{base} ({year})");
                }
            }
            if taken.contains(&name_key(&name)) {
                name = fsx::free_name(&|n| taken.contains(n), &base, "");
            }
            taken.insert(name_key(&name));
            by_person.insert(p, name);
        }
        Folders { by_person, owner_of, existing }
    }
}

/// How the files lie now: the guess for the setting.
struct Counts {
    root_owned: usize,
    folder_owned: usize,
    person_folders: usize,
}

fn count_owned(t: &Tree, f: &Folders) -> Counts {
    let (mut root_owned, mut folder_owned) = (0, 0);
    for objs in t.by_key.values() {
        if t.file_owner(objs).is_none() {
            continue;
        }
        match t.objs[objs[0]].rel.as_deref().and_then(split_media) {
            Some((None, _)) => root_owned += 1,
            Some((Some(top), _)) if f.owner_of.contains_key(&name_key(top)) => folder_owned += 1,
            _ => {}
        }
    }
    Counts { root_owned, folder_owned, person_folders: f.owner_of.len() }
}

fn guess_of(c: &Counts) -> Layout {
    if c.person_folders >= 1 && c.folder_owned >= c.root_owned { Layout::PerPerson } else { Layout::Flat }
}

/// The layout files are placed by: the archive's setting, or what the files look like.
pub fn effective_layout(s: &mut Session) -> Layout {
    if let Some(layout) = setting(s) {
        return layout;
    }
    s.ensure();
    let layers = s.layers();
    let t = Tree::build(&s.archive.doc, s.derived.as_ref().expect("ensured"), &layers);
    let f = Folders::build(&t, &layers, &[]);
    guess_of(&count_owned(&t, &f))
}

/// `media`, or `media/<folder>` where a new file for `person` goes (B.3).
pub fn dir_for_person(s: &mut Session, person: Option<&str>) -> String {
    let Some(person) = person else { return MEDIA.into() };
    if effective_layout(s) != Layout::PerPerson {
        return MEDIA.into();
    }
    s.ensure();
    let layers = s.layers();
    let t = Tree::build(&s.archive.doc, s.derived.as_ref().expect("ensured"), &layers);
    let Some(&p) = t.person_index.get(person) else { return MEDIA.into() };
    let mut include = t.linked_persons();
    include.push(p);
    let f = Folders::build(&t, &layers, &include);
    f.by_person.get(&p).map_or_else(|| MEDIA.into(), |folder| format!("{MEDIA}/{folder}"))
}

/// Where a file replacing the one of object `id` goes: the folder of the object's owner.
pub fn dir_for_object(s: &mut Session, id: &str) -> String {
    if effective_layout(s) != Layout::PerPerson {
        return MEDIA.into();
    }
    s.ensure();
    let layers = s.layers();
    let t = Tree::build(&s.archive.doc, s.derived.as_ref().expect("ensured"), &layers);
    let Some(&o) = t.by_id.get(id) else { return MEDIA.into() };
    let Some(owner) = t.file_owner(&t.by_key[&t.objs[o].key]) else { return MEDIA.into() };
    let mut include = t.linked_persons();
    include.push(owner);
    let f = Folders::build(&t, &layers, &include);
    f.by_person.get(&owner).map_or_else(|| MEDIA.into(), |folder| format!("{MEDIA}/{folder}"))
}

/// Who a file of an import is for.
pub enum Who {
    Existing(String),
    New { given: String, surname: String },
}

/// `media/<folder>` for each file of an import (by its key), when the archive keeps files in folders of people; the
/// first person each file is assigned to decides. New people of the batch have no record yet: their folder name comes
/// from the batch name.
pub fn dirs_for_import(s: &mut Session, wanted: &[(String, Who)]) -> HashMap<String, String> {
    let mut dirs = HashMap::new();
    if wanted.is_empty() || effective_layout(s) != Layout::PerPerson {
        return dirs;
    }
    s.ensure();
    let layers = s.layers();
    let t = Tree::build(&s.archive.doc, s.derived.as_ref().expect("ensured"), &layers);
    let mut include = t.linked_persons();
    include.extend(wanted.iter().filter_map(|(_, w)| match w {
        Who::Existing(x) => t.person_index.get(x).copied(),
        Who::New { .. } => None,
    }));
    let f = Folders::build(&t, &layers, &include);
    let mut new_names: HashMap<String, String> = HashMap::new();
    for (key, who) in wanted {
        let folder = match who {
            Who::Existing(x) => t.person_index.get(x).and_then(|p| f.by_person.get(p)).cloned(),
            Who::New { given, surname } => {
                let base = fsx::folder_name(&format!("{} {}", surname.trim(), given.trim()));
                let k = name_key(&base);
                Some(new_names.entry(k.clone()).or_insert_with(|| match f.existing.get(&k) {
                    // A folder of that name: adopted unless it is already somebody's.
                    Some(name) if !f.owner_of.contains_key(&k) => name.clone(),
                    Some(_) => fsx::free_name(&|n| f.existing.contains_key(n), &base, ""),
                    None => base.clone(),
                }).clone())
            }
        };
        if let Some(folder) = folder {
            dirs.insert(key.clone(), format!("{MEDIA}/{folder}"));
        }
    }
    dirs
}

// ---------- the scan (B.4) ----------

#[derive(Debug, Clone)]
pub(crate) struct Found {
    /// Path inside the archive, `/` separators, as the file system spells it.
    pub rel: String,
    pub abs: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

impl Found {
    pub fn key(&self) -> String {
        name_key(&self.rel)
    }
}

/// Every file under `media/` of each folder of the archive, and under the other top folders FILE values point into.
/// Noise is skipped; stops at [`MAX_ENTRIES`]. A file in the top folder wins over the same path in the original.
pub(crate) fn walk(layers: &[PathBuf], extra_tops: &[String]) -> (Vec<Found>, bool) {
    let mut found: Vec<Found> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut tops: Vec<String> = vec![MEDIA.to_string()];
    tops.extend(extra_tops.iter().cloned());
    let mut count = 0usize;
    for layer in layers {
        for top in &tops {
            let mut stack: Vec<(PathBuf, String)> = vec![(layer.join(top), top.clone())];
            while let Some((dir, rel)) = stack.pop() {
                let Ok(entries) = std::fs::read_dir(&dir) else { continue };
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let Ok(kind) = entry.file_type() else { continue };
                    let child = format!("{rel}/{name}");
                    if kind.is_dir() {
                        stack.push((entry.path(), child));
                    } else if kind.is_file() && !fsx::is_noise(&name) {
                        count += 1;
                        if count > MAX_ENTRIES {
                            found.sort_by(|a, b| a.rel.cmp(&b.rel));
                            return (found, true);
                        }
                        if !seen.insert(name_key(&child)) {
                            continue;
                        }
                        let meta = entry.metadata().ok();
                        let mtime = meta.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs() as i64);
                        found.push(Found { rel: child, abs: entry.path(), size: meta.map_or(0, |m| m.len()), mtime });
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| a.rel.cmp(&b.rel));
    (found, false)
}

/// The other top-level folders FILE values point into (e.g. a 5.5.1 archive's `zdjecia/`).
fn extra_tops(t: &Tree, layers: &[PathBuf]) -> Vec<String> {
    let mut tops: Vec<String> = Vec::new();
    for o in &t.objs {
        let Some(rel) = o.rel.as_deref() else { continue };
        let Some((first, _)) = rel.split_once('/') else { continue };
        if first.is_empty() || first.eq_ignore_ascii_case(MEDIA) || first.starts_with('.') || first.contains("..") || first.contains(':') {
            continue;
        }
        if !tops.iter().any(|x| name_key(x) == name_key(first)) && layers.iter().any(|l| l.join(first).is_dir()) {
            tops.push(first.to_string());
        }
    }
    tops
}

/// Files and folders the user chose to leave alone: `[{ path, kind, count, newest }]`.
#[derive(Debug, Clone)]
struct LeftAlone {
    path: String,
    kind: String,
    count: u64,
    newest: i64,
}

fn left_alone_entries(s: &Session) -> Vec<LeftAlone> {
    s.archive
        .settings()
        .display
        .get(LEFT_ALONE)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| {
                    Some(LeftAlone {
                        path: e.get("path")?.as_str()?.to_string(),
                        kind: e.get("kind")?.as_str()?.to_string(),
                        count: e.get("count").and_then(Value::as_u64).unwrap_or(0),
                        newest: e.get("newest").and_then(Value::as_i64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The files the user removed from Heirloom earlier (media.delete and „Cofnij import” keep the files on disk).
fn deleted_files(s: &Session) -> HashSet<String> {
    let mut deleted = HashSet::new();
    for entry in s.history().iter().filter(|e| e.action == Action::Delete && e.tag == "OBJE") {
        let Some(before) = entry.before.as_deref() else { continue };
        let (doc, _) = Document::from_bytes(before.as_bytes());
        for value in doc.records.iter().filter_map(|r| r.child("FILE")?.value.as_deref()) {
            deleted.insert(moved_key(value));
            deleted.insert(moved_key(&decode_value(value)));
        }
    }
    deleted
}

/// What the scan found.
pub struct Scan {
    pub report: Value,
    /// FILE path (compared) → where the file really is.
    pub moved: HashMap<String, PathBuf>,
    pub guess: Layout,
}

/// Walks the archive's `media/` (and the other folders FILE values point into) and reports what is out of order.
pub fn scan(s: &mut Session) -> Scan {
    s.ensure();
    let layers = s.layers();
    let left = left_alone_entries(s);
    let deleted = deleted_files(s);
    let setting_now = setting(s);
    let interrupted = engine::interrupted(s.archive.sidecar_path());
    let journal = engine::last_journal_summary(s.archive.sidecar_path());
    let d = s.derived.as_ref().expect("ensured");
    let t = Tree::build(&s.archive.doc, d, &layers);
    let (files, truncated) = walk(&layers, &extra_tops(&t, &layers));
    let include = t.linked_persons();
    let folders = Folders::build(&t, &layers, &include);
    let counts = count_owned(&t, &folders);
    let guess = guess_of(&counts);
    let effective = setting_now.unwrap_or(guess);

    // What FILE values say, and where those files really are.
    let disk: HashMap<String, &Found> = files.iter().map(|f| (f.key(), f)).collect();
    let mut referenced: HashSet<String> = t.objs.iter().map(|o| o.key.clone()).collect();
    referenced.extend(t.embedded.iter().cloned());
    let mut present: HashSet<String> = HashSet::new();
    let mut outside = Vec::new();
    let mut missing: Vec<usize> = Vec::new();
    for (i, o) in t.objs.iter().enumerate() {
        match &o.rel {
            None => {
                if Path::new(&o.file).is_file() {
                    outside.push(json!({ "id": o.id, "file": o.file }));
                }
            }
            Some(rel) => {
                if crate::media_edit::resolve_in(&layers, rel).is_file() {
                    present.insert(o.key.clone());
                } else {
                    missing.push(i);
                }
            }
        }
    }
    // „Moved”: a file that is missing where it is written, and EXACTLY ONE file of that name (not another record's) is
    // in the walked folders.
    let mut by_name: HashMap<String, Vec<&Found>> = HashMap::new();
    for f in &files {
        by_name.entry(name_key(f.rel.rsplit('/').next().unwrap_or(&f.rel))).or_default().push(f);
    }
    let mut moved: HashMap<String, PathBuf> = HashMap::new();
    let mut moved_rows = Vec::new();
    let mut moved_targets: HashSet<String> = HashSet::new();
    for &i in &missing {
        let o = &t.objs[i];
        let name = name_key(o.file.rsplit('/').next().unwrap_or(&o.file));
        let candidates: Vec<&&Found> = by_name.get(&name).into_iter().flatten().filter(|f| !present.contains(&f.key())).collect();
        if let [only] = candidates.as_slice() {
            moved.insert(moved_key(&o.file), only.abs.clone());
            moved_targets.insert(only.key());
            moved_rows.push(json!({ "id": o.id, "file": o.file, "found": only.rel }));
        }
    }

    // Files nothing references.
    let in_left_alone = |f: &Found, top: Option<&str>, group: &HashMap<String, (u64, i64)>| -> bool {
        left.iter().any(|e| {
            let key = name_key(&e.path);
            match e.kind.as_str() {
                "file" => key == f.key() && e.newest == f.mtime,
                _ => top.is_some_and(|t| name_key(&format!("{MEDIA}/{t}")) == key) && group.get(&key).is_some_and(|&(c, n)| c == e.count && n == e.newest),
            }
        })
    };
    // Per top folder of media/: how many files, the newest time.
    let mut group: HashMap<String, (u64, i64)> = HashMap::new();
    for f in &files {
        if let Some((Some(top), _)) = split_media(&f.rel) {
            let g = group.entry(name_key(&format!("{MEDIA}/{top}"))).or_default();
            g.0 += 1;
            g.1 = g.1.max(f.mtime);
        }
    }
    let mut unknown = Vec::new();
    let mut unknown_per_top: HashMap<String, usize> = HashMap::new();
    for f in &files {
        let Some((top, name)) = split_media(&f.rel) else { continue };
        let key = f.key();
        if referenced.contains(&key) || moved_targets.contains(&key) || deleted.contains(&key) {
            continue;
        }
        if let Some(top) = top {
            *unknown_per_top.entry(name_key(&format!("{MEDIA}/{top}"))).or_default() += 1;
        }
        if in_left_alone(f, top, &group) {
            continue;
        }
        let (suggestion, candidates) = suggest(&t, &folders, top, name);
        let folder = match top {
            Some(top) => format!("{MEDIA}/{top}"),
            None => MEDIA.to_string(),
        };
        unknown.push(json!({ "path": f.rel, "name": name, "size": f.size, "folder": folder, "suggestion": suggestion, "candidates": candidates }));
    }

    // Person-owned files lying loose in the media folder (when files are kept in folders of people).
    let mut loose = Vec::new();
    if effective == Layout::PerPerson {
        let mut seen: HashSet<&str> = HashSet::new();
        for o in &t.objs {
            let Some(rel) = o.rel.as_deref() else { continue };
            if !matches!(split_media(rel), Some((None, _))) || !disk.contains_key(&o.key) || !seen.insert(&o.key) {
                continue;
            }
            let objs = &t.by_key[&o.key];
            if let Some(owner) = t.file_owner(objs) {
                loose.push(json!({ "id": o.id, "ids": objs.iter().map(|&x| t.objs[x].id.clone()).collect::<Vec<_>>(), "file": rel, "owner": t.person_json(owner) }));
            }
        }
    }

    // Subfolders of media/.
    let mut tops: BTreeMap<String, (String, u64, u64)> = BTreeMap::new();
    for f in &files {
        if let Some((Some(top), _)) = split_media(&f.rel) {
            let e = tops.entry(name_key(top)).or_insert_with(|| (top.to_string(), 0, 0));
            e.1 += 1;
            if referenced.contains(&f.key()) || moved_targets.contains(&f.key()) {
                e.2 += 1;
            }
        }
    }
    let mut folder_rows = Vec::new();
    for (key, (name, n, linked)) in &tops {
        let path = format!("{MEDIA}/{name}");
        let gkey = name_key(&path);
        let entry = left.iter().find(|e| e.kind == "folder" && name_key(&e.path) == gkey);
        let quiet = entry.is_some_and(|e| group.get(&gkey).is_some_and(|&(c, nw)| c == e.count && nw == e.newest));
        let unknown_n = unknown_per_top.get(&gkey).copied().unwrap_or(0);
        let person = folders.owner_of.get(key).map(|&p| t.person_json(p));
        let question = if quiet {
            Value::Null
        } else if effective == Layout::Flat {
            json!("unpack")
        } else if person.is_none() && unknown_n > 0 {
            json!("whose")
        } else {
            Value::Null
        };
        folder_rows.push(json!({ "path": path, "name": name, "files": n, "linked": linked, "unknown": unknown_n, "person": person, "leftAlone": quiet, "changed": entry.is_some() && !quiet, "question": question }));
    }

    // The fingerprint: the findings, so „Nie teraz” hides the banner until they change.
    let mut keys: Vec<String> = Vec::new();
    keys.extend(moved_rows.iter().filter_map(|r| Some(format!("moved:{}", r["id"].as_str()?))));
    keys.extend(loose.iter().filter_map(|r| Some(format!("loose:{}", r["file"].as_str()?))));
    keys.extend(unknown.iter().filter_map(|r| Some(format!("unknown:{}", r["path"].as_str()?))));
    keys.extend(folder_rows.iter().filter(|r| !r["question"].is_null()).filter_map(|r| Some(format!("folder:{}", r["path"].as_str()?))));
    keys.extend(outside.iter().filter_map(|r| Some(format!("outside:{}", r["id"].as_str()?))));
    keys.sort();
    let fingerprint = blake3::hash(keys.join("\n").as_bytes()).to_hex().to_string();
    let questions = folder_rows.iter().filter(|r| !r["question"].is_null()).count();
    let report = json!({
        "layout": {
            "setting": setting_now.map(Layout::code),
            "guess": guess.code(),
            "effective": effective.code(),
            "rootOwned": counts.root_owned,
            "folderOwned": counts.folder_owned,
            "personFolders": counts.person_folders,
        },
        "moved": moved_rows,
        "looseLinked": loose,
        "unknown": unknown,
        "folders": folder_rows,
        "outside": outside,
        "counts": { "moved": moved.len(), "looseLinked": loose.len(), "unknown": unknown.len(), "folders": questions, "outside": outside.len(), "missing": missing.len() - moved.len(), "files": files.len() },
        "total": moved.len() + loose.len() + unknown.len() + questions + outside.len(),
        "fingerprint": fingerprint,
        "truncated": truncated,
        "journal": journal,
        "interrupted": interrupted,
    });
    Scan { report, moved, guess }
}

/// The person a file is probably of: the owner of the folder it lies in, else the one name found in its name (a
/// year of birth or death breaks a tie). Two or three names fitting are listed as candidates. Nothing is assigned.
fn suggest(t: &Tree, f: &Folders, top: Option<&str>, file_name: &str) -> (Value, Vec<Value>) {
    if let Some(&p) = top.and_then(|top| f.owner_of.get(&name_key(top))) {
        return (json!({ "id": t.persons[p].xref, "name": t.persons[p].name, "why": "w folderze osoby" }), Vec::new());
    }
    let stem = file_name.rsplit_once('.').map_or(file_name, |(s, _)| s);
    let words = tokens(stem);
    let mut matches = t.named_in(&words);
    if matches.len() > 1 {
        let years: Vec<i32> = words.iter().filter(|w| w.len() == 4).filter_map(|w| w.parse().ok()).collect();
        let by_year: Vec<usize> = matches.iter().copied().filter(|&p| [t.persons[p].birth_year, t.persons[p].death_year].iter().flatten().any(|y| years.contains(y))).collect();
        if !by_year.is_empty() {
            matches = by_year;
        }
    }
    match matches.as_slice() {
        [p] => (json!({ "id": t.persons[*p].xref, "name": t.persons[*p].name, "why": "pasuje nazwa pliku" }), Vec::new()),
        [_, _] | [_, _, _] => (Value::Null, matches.iter().map(|&p| t.person_json(p)).collect()),
        _ => (Value::Null, Vec::new()),
    }
}

/// `media.scan`: the report, the moved map refreshed, and the guess stored when the archive has no setting yet.
pub fn run_scan(s: &mut Session) -> Value {
    let scan = scan(s);
    s.moved.set(scan.moved);
    store_guess(s, scan.guess);
    s.layout_interrupted = engine::interrupted(s.archive.sidecar_path());
    scan.report
}

/// With no `display.mediaLayout` the scan's guess is stored, silently (in „Nie ruszaj oryginału” in the changes folder).
/// Not before the user chose how to work with the archive: until then nothing is written into it.
fn store_guess(s: &mut Session, guess: Layout) {
    if setting(s).is_some() || s.work_mode.is_none() || s.archive.settings().read_only {
        return;
    }
    store_setting(s, guess);
}

pub(crate) fn store_setting(s: &mut Session, layout: Layout) {
    s.archive.settings_mut().display.insert(SETTING.into(), json!(layout.code()));
    // A settings file that can't be written now is written with the next save.
    let _ = s.archive.save_settings();
}

// ---------- the commands that change data ----------

fn arg_str(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

/// A path from the interface, inside `media/` of the archive: `/` separators, no `..`, no drive.
fn clean_media_path(path: &str) -> Result<String, ApiError> {
    let rel = slash(path);
    let rel = rel.trim_start_matches("./").to_string();
    let bad = is_abs(&rel) || rel.split('/').any(|p| p == ".." || p.contains(':') || fsx::is_device_name(p));
    if bad || split_media(&rel).is_none() || rel.ends_with('/') {
        return Err(ApiError::bad_args("path"));
    }
    Ok(rel)
}

/// `media.fixMoved`: every FILE value that points at nothing but whose file the scan found elsewhere in the archive is
/// pointed at it. An ordinary unsaved edit (it can be undone).
pub fn fix_moved(s: &mut Session) -> ApiResult {
    crate::edit::prepare(s)?;
    let scan = scan(s);
    let layers = s.layers();
    s.ensure();
    let d = s.derived.as_ref().expect("ensured");
    let objects: Vec<(String, String)> = s
        .archive
        .doc
        .records
        .iter()
        .filter(|r| r.tag == "OBJE")
        .filter_map(|r| Some((r.xref.clone()?, d.view.media.get(r.xref.as_deref()?).and_then(|m| m.file.clone()).or_else(|| Some(decode_value(r.child("FILE")?.value.as_deref()?)))?)))
        .collect();
    let mut changes = Changes::new(&s.archive.doc);
    let mut fixed = 0usize;
    for (id, decoded) in objects {
        let Some(there) = scan.moved.get(&moved_key(&decoded)) else { continue };
        let Some(rel) = layers.iter().find_map(|l| there.strip_prefix(l).ok()).map(|p| p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")) else { continue };
        if let Some(file) = changes.get(&id)?.child_mut("FILE") {
            file.value = Some(crate::gedwrite::file_uri(&rel));
            fixed += 1;
        }
    }
    let edits = changes.into_edits();
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(json!({ "fixed": fixed }))
}

/// `media.assign { path, person }` or `{ items: [{ path, person }] }`: a file nothing references gets its object record
/// and a link to the person (one unsaved edit). The file is referenced where it is; in „Pracuj na archiwum” with files
/// in folders of people a file from the media root first moves into the person's folder (never replacing a file; harmless
/// if the edit is dropped, nothing points at the file yet). In „Nie ruszaj oryginału” nothing is moved or copied.
pub fn assign(s: &mut Session, args: &Value) -> ApiResult {
    let items: Vec<(String, String)> = match args.get("items").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(|i| Some((i.get("path")?.as_str()?.to_string(), i.get("person")?.as_str()?.to_string()))).collect(),
        None => vec![(arg_str(args, "path")?, arg_str(args, "person")?)],
    };
    if items.is_empty() {
        return Err(ApiError::bad_args("items"));
    }
    crate::edit::prepare(s)?;
    let layers = s.layers();
    let in_place = s.overlay.is_none();
    let mut prepared: Vec<(String, String, bool)> = Vec::new();
    let mut taken_in: HashMap<String, HashSet<String>> = HashMap::new();
    for (path, person) in &items {
        let rel = clean_media_path(path)?;
        if !s.archive.doc.record(person).is_some_and(|r| r.tag == "INDI") {
            return Err(ApiError::new("no_record", format!("Nie ma osoby {person}.")));
        }
        let Some(abs) = layers.iter().map(|l| crate::media_edit::resolve(l, &rel)).find(|p| p.is_file()) else {
            return Err(ApiError::new("not_found", format!("Nie ma pliku {rel}.")));
        };
        // A1 moves nothing. A2: from the media root to the folder of the person (when files are kept in folders).
        let mut final_rel = rel.clone();
        let mut did_move = false;
        if in_place && matches!(split_media(&rel), Some((None, _))) {
            let dir = dir_for_person(s, Some(person));
            if dir != MEDIA {
                let root = &layers[0];
                let target_dir = crate::media_edit::resolve(root, &dir);
                std::fs::create_dir_all(&target_dir).map_err(|e| ApiError::new("io", format!("Nie można utworzyć folderu {dir}: {e}")))?;
                let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                let (stem, ext) = match name.rsplit_once('.') {
                    Some((st, e)) if !st.is_empty() => (st.to_string(), e.to_string()),
                    _ => (name.clone(), String::new()),
                };
                let taken = taken_in.entry(dir.clone()).or_insert_with(|| std::fs::read_dir(&target_dir).into_iter().flatten().flatten().map(|e| name_key(&e.file_name().to_string_lossy())).collect());
                loop {
                    let free = fsx::free_name(&|n| taken.contains(n), &stem, &ext);
                    match fsx::with_retry(|| fsx::move_no_replace(&abs, &target_dir.join(&free))) {
                        Ok(()) => {
                            taken.insert(name_key(&free));
                            final_rel = format!("{dir}/{free}");
                            did_move = true;
                            break;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                            taken.insert(name_key(&free));
                        }
                        Err(e) => return Err(ApiError::new("io", format!("Nie można przenieść pliku {name}: {e}"))),
                    }
                }
            }
        }
        prepared.push((final_rel, person.clone(), did_move));
    }
    let mut changes = Changes::new(&s.archive.doc);
    let mut result = Vec::new();
    let mut made: HashMap<String, String> = HashMap::new();
    for (rel, person, did_move) in prepared {
        let id = match made.get(&name_key(&rel)) {
            Some(id) => id.clone(),
            None => {
                let id = changes.new_xref("O");
                changes.add(crate::media_edit::object_node(&id, &rel, None, None, None));
                made.insert(name_key(&rel), id.clone());
                id
            }
        };
        crate::media_edit::add_link(&mut changes, &person, &id, false)?;
        result.push(json!({ "path": rel, "id": id, "person": person, "moved": did_move }));
    }
    let edits = changes.into_edits();
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(json!({ "assigned": result }))
}

/// `media.leaveAlone { path, kind }` or `{ items: [...] }`: remembered in `display.mediaLeftAlone` with the file's time
/// (a folder: its file count and newest time), so a folder that changes is asked about again.
pub fn leave_alone(s: &mut Session, args: &Value) -> ApiResult {
    let items: Vec<(String, String)> = match args.get("items").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(|i| Some((i.get("path")?.as_str()?.to_string(), i.get("kind")?.as_str()?.to_string()))).collect(),
        None => vec![(arg_str(args, "path")?, arg_str(args, "kind")?)],
    };
    let layers = s.layers();
    let mut entries = left_alone_entries(s);
    let mut added = Vec::new();
    for (path, kind) in items {
        if kind != "file" && kind != "folder" {
            return Err(ApiError::bad_args("kind"));
        }
        let rel = clean_media_path(&if kind == "folder" { format!("{}/x", path.trim_end_matches('/')) } else { path.clone() })?;
        let rel = if kind == "folder" { rel.trim_end_matches("/x").to_string() } else { rel };
        let (files, _) = walk(&layers, &[]);
        let key = name_key(&rel);
        let inside: Vec<&Found> = files.iter().filter(|f| if kind == "file" { f.key() == key } else { f.key().starts_with(&format!("{key}/")) }).collect();
        let entry = LeftAlone { path: rel.clone(), kind: kind.clone(), count: inside.len() as u64, newest: inside.iter().map(|f| f.mtime).max().unwrap_or(0) };
        entries.retain(|e| name_key(&e.path) != key || e.kind != kind);
        added.push(json!({ "path": entry.path, "kind": entry.kind, "count": entry.count, "newest": entry.newest }));
        entries.push(entry);
    }
    let list: Vec<Value> = entries.iter().map(|e| json!({ "path": e.path, "kind": e.kind, "count": e.count, "newest": e.newest })).collect();
    s.archive.settings_mut().display.insert(LEFT_ALONE.into(), Value::Array(list));
    s.archive.save_settings()?;
    Ok(json!({ "left": added }))
}

/// `media.setLayout { layout }`: only stores the setting („Tylko nowe pliki”); what is left over shows as „Do uporządkowania”.
pub fn set_layout(s: &mut Session, args: &Value) -> ApiResult {
    if s.overlay.is_some() {
        return Err(engine::overlay_mode());
    }
    let layout = Layout::parse(&arg_str(args, "layout")?).ok_or_else(|| ApiError::bad_args("layout"))?;
    s.archive.settings_mut().display.insert(SETTING.into(), json!(layout.code()));
    s.archive.save_settings()?;
    Ok(json!({ "layout": layout.code() }))
}

/// A node's FILE value, for the engine.
pub(crate) fn file_value(node: &Node) -> Option<&str> {
    node.child("FILE")?.value.as_deref()
}
