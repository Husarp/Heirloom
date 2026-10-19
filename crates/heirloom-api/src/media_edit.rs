//! Changing photos, documents and sources. New files are copied into the archive's `media/` folder with their
//! own readable names (PLAN §4.7); the same file added twice is recognised by its contents and reused.

use crate::edit::Changes;
use crate::gedwrite::{file_uri, has_pointer, media_type, new_uid, remove_pointer, set_child_text, stamp};
use crate::{ApiError, ApiResult, Session};
use heirloom_core::gedcom::Node;

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn req(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

fn commit(s: &mut Session, edits: Vec<heirloom_core::Edit>) -> Result<(), ApiError> {
    if !edits.is_empty() {
        s.archive.apply_all(edits)?;
        s.changed();
    }
    Ok(())
}

pub fn call(s: &mut Session, method: &str, args: &Value) -> ApiResult {
    match method {
        "media.add" => add(s, args),
        "media.update" => update(s, args),
        "media.renameCategory" => rename_category(s, args),
        "media.removeCategory" => remove_category(s, args),
        "media.link" => link(s, args),
        "media.unlink" => unlink(s, args),
        "media.setProfile" => set_profile(s, args),
        "media.unsetProfile" => unset_profile(s, args),
        "media.reorder" => reorder(s, args),
        "media.relink" => relink(s, args),
        "media.delete" => delete(s, args),
        "source.save" => save_source(s, args),
        "source.delete" => delete_source(s, args),
        "source.detach" => detach_source(s, args),
        "citation.add" => add_citation(s, args),
        _ => Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
    }
}

/// The absolute path of a media file written in the archive (relative to its folder, or absolute).
pub fn resolve(root: &Path, file: &str) -> PathBuf {
    let path = Path::new(file);
    if path.is_absolute() { path.to_path_buf() } else { root.join(file.replace('/', std::path::MAIN_SEPARATOR_STR)) }
}

/// Where the files of the open data are: one archive's folders — in „Nie ruszaj oryginału” the folder of the user's
/// changes first, then the original archive — or in archives opened together each archive's folders by its key (their
/// files are written `~b/media/x.jpg` there).
pub enum FileRoots {
    /// The folders, top first, and the files the scan found moved (served when every folder misses).
    Layers(Vec<PathBuf>, crate::media::Moved),
    Many(Vec<(String, Vec<PathBuf>)>),
}

impl FileRoots {
    /// One archive folder.
    pub fn one(root: PathBuf) -> FileRoots {
        FileRoots::Layers(vec![root], crate::media::Moved::default())
    }

    pub fn resolve(&self, file: &str) -> PathBuf {
        match self {
            FileRoots::Layers(layers, moved) => {
                let path = resolve_in(layers, file);
                match moved_file(moved, &path, file) {
                    Some(there) => there,
                    None => path,
                }
            }
            FileRoots::Many(roots) => {
                let keyed = file.strip_prefix('~').and_then(|rest| rest.split_once('/'));
                match keyed.and_then(|(key, rest)| Some((roots.iter().find(|(k, _)| k == key)?, rest))) {
                    Some(((_, layers), rest)) => resolve_in(layers, rest),
                    // An absolute path, kept as it was written.
                    None => PathBuf::from(file),
                }
            }
        }
    }

    /// The file is not where its path says, but the scan found it elsewhere in the archive („foundElsewhere”).
    pub fn found_elsewhere(&self, file: &str) -> bool {
        match self {
            FileRoots::Layers(layers, moved) => moved_file(moved, &resolve_in(layers, file), file).is_some(),
            FileRoots::Many(_) => false,
        }
    }

    /// `mine` when the file is in the folder of the user's changes, `archive` when only the original has it; nothing
    /// for an archive worked on in place.
    pub fn layer_of(&self, path: &Path) -> Option<&'static str> {
        match self {
            FileRoots::Layers(layers, _) if layers.len() > 1 => Some(if path.starts_with(&layers[0]) { "mine" } else { "archive" }),
            _ => None,
        }
    }
}

/// Where the scan found a file that is missing at `path` (its written place), if it did.
fn moved_file(moved: &crate::media::Moved, path: &Path, file: &str) -> Option<PathBuf> {
    if path.is_file() || Path::new(file).is_absolute() {
        return None;
    }
    moved.get(file).filter(|p| p.is_file())
}

/// A file written relative to the archive: in the first folder that has it, else where the top folder would have it
/// (so a missing file is reported as before). An absolute path stays as written.
pub fn resolve_in(layers: &[PathBuf], file: &str) -> PathBuf {
    let Some(top) = layers.first() else { return PathBuf::from(file) };
    if layers.len() == 1 || Path::new(file).is_absolute() {
        return resolve(top, file);
    }
    layers.iter().map(|layer| resolve(layer, file)).find(|p| p.is_file()).unwrap_or_else(|| resolve(top, file))
}

/// Names an own category cannot take: they are the built-in kinds' (compared in lower case).
const RESERVED_CATEGORIES: [&str; 6] = ["zdjęcie", "zdjęcia", "dokument", "dokumenty", "inne", "inny"];

/// A category name as it is stored: on one line with single spaces, not empty, not too long, not a built-in kind.
pub(crate) fn clean_category(name: &str) -> Result<String, ApiError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(ApiError::new("bad_category", "Podaj nazwę kategorii."));
    }
    if name.chars().count() > 60 {
        return Err(ApiError::new("bad_category", "Nazwa kategorii jest za długa (najwyżej 60 znaków)."));
    }
    if RESERVED_CATEGORIES.contains(&name.to_lowercase().as_str()) {
        return Err(ApiError::new("bad_category", "Zdjęcie, Dokument i Inne to rodzaje wbudowane — wybierz inną nazwę."));
    }
    Ok(name)
}

fn is_image(name: &str) -> bool {
    media_type(name).starts_with("image/")
}

/// The object record for a file in `media/`: photo or document, with a caption and a document type.
pub fn object_node(xref: &str, relative: &str, kind: Option<&str>, title: Option<&str>, document_type: Option<&str>) -> Node {
    let photo = match kind {
        Some("photo") => true,
        Some("document") | Some("other") => false,
        _ => is_image(relative) && document_type.is_none(),
    };
    let mut medi = Node::with_value("MEDI", if photo { "PHOTO" } else if kind == Some("other") { "OTHER" } else { "MANUSCRIPT" });
    if let Some(doc_type) = document_type.map(str::trim).filter(|t| !t.is_empty()) {
        medi.children.push(Node::with_value("PHRASE", doc_type));
    }
    let mut file = Node::with_value("FILE", &file_uri(relative)).with_child(Node::with_value("FORM", media_type(relative)).with_child(medi));
    if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
        file.children.push(Node::with_value("TITL", title));
    }
    Node::new("OBJE").with_xref(xref).with_child(file).with_child(Node::with_value("UID", &new_uid())).with_child(stamp("CREA"))
}

/// Copies a file into `media/`, keeping its name ("Józef 1904.jpg", or "Józef 1904 (2).jpg" when taken). Returns
/// the path relative to the archive folder. A file already inside the archive folder is not copied.
pub fn copy_into_archive(layers: &[PathBuf], target_dir: &str, source: &Path) -> Result<String, ApiError> {
    copy_new_into_archive(layers, target_dir, source).map(|(relative, _)| relative)
}

/// [`copy_into_archive`], also giving the file it created (None when the file was already inside the archive folder),
/// so a failed import can remove exactly what it added. `layers`: the archive's folders, the one written into first
/// (in „Nie ruszaj oryginału” the user's changes, then the original): a file inside any of them is referenced where it
/// is, and a new name must be free in all of them, so a new file never hides an original one. `target_dir`: `media`, or
/// `media/<folder>` for the folder of a person (Part B); the folder is made when it isn't there.
pub fn copy_new_into_archive(layers: &[PathBuf], target_dir: &str, source: &Path) -> Result<(String, Option<PathBuf>), ApiError> {
    let root = layers.first().ok_or_else(|| ApiError::bad_args("root"))?;
    if let Ok(source_abs) = source.canonicalize() {
        for layer in layers {
            if let Some(relative) = layer.canonicalize().ok().and_then(|l| source_abs.strip_prefix(&l).ok().map(Path::to_path_buf)) {
                return Ok((relative.to_string_lossy().replace('\\', "/"), None));
            }
        }
    }
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| ApiError::bad_args("path"))?;
    let dir = target_dir.split('/').filter(|p| !p.is_empty()).fold(root.clone(), |dir, part| dir.join(part));
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::new("io", format!("Nie można utworzyć folderu media: {e}")))?;
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), e.to_string()),
        _ => (name.clone(), String::new()),
    };
    let mut taken: std::collections::HashSet<String> = std::collections::HashSet::new();
    for layer in layers {
        let there = target_dir.split('/').filter(|p| !p.is_empty()).fold(layer.clone(), |dir, part| dir.join(part));
        for entry in std::fs::read_dir(there).into_iter().flatten().flatten() {
            taken.insert(crate::fsx::name_key(&entry.file_name().to_string_lossy()));
        }
    }
    // A name taken meanwhile (another window) is skipped: the copy never replaces a file.
    for _ in 0..100 {
        let free = crate::fsx::free_name(&|n| taken.contains(n), &stem, &ext);
        let target = dir.join(&free);
        match crate::fsx::copy_no_replace(source, &target) {
            Ok(_) => return Ok((format!("{}/{free}", target_dir.trim_end_matches('/')), Some(target))),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                taken.insert(crate::fsx::name_key(&free));
            }
            Err(e) => return Err(ApiError::new("io", format!("Nie można skopiować pliku {name}: {e}"))),
        }
    }
    Err(ApiError::new("io", format!("Nie można skopiować pliku {name}: brak wolnej nazwy.")))
}

/// A media object already in the archive with exactly this file's contents.
fn duplicate_of(s: &mut Session, source: &Path) -> Option<String> {
    let size = std::fs::metadata(source).ok()?.len();
    let roots = s.file_roots();
    let d = s.derived();
    let mut hash = None;
    // Only records can be pointed to: an object written inside a person (`@I1@#0`) is not one.
    for id in d.view.media_order.iter().filter(|id| !id.contains('#')) {
        let Some(file) = d.view.media.get(id).and_then(|m| m.file.clone()) else { continue };
        let path = roots.resolve(&file);
        if std::fs::metadata(&path).ok().map(|m| m.len()) != Some(size) {
            continue;
        }
        let ours = hash.get_or_insert_with(|| std::fs::read(source).map(|b| blake3::hash(&b)).ok());
        let theirs = std::fs::read(&path).map(|b| blake3::hash(&b)).ok();
        if ours.is_some() && *ours == theirs {
            return Some(id.clone());
        }
    }
    None
}

fn link_node(object: &str, profile: bool) -> Node {
    let mut node = Node::with_value("OBJE", object);
    if profile {
        node.children.push(Node::with_value("_HLM_PROFILE", "Y"));
    }
    node
}

pub(crate) fn add_link(changes: &mut Changes, person: &str, object: &str, profile: bool) -> Result<(), ApiError> {
    let indi = changes.get(person)?;
    if profile {
        for link in indi.children.iter_mut().filter(|c| c.tag == "OBJE") {
            link.children.retain(|c| c.tag != "_HLM_PROFILE" && c.tag != "_PRIM");
        }
    }
    match indi.children.iter_mut().find(|c| c.tag == "OBJE" && c.pointer() == Some(object)) {
        Some(existing) if profile => existing.children.push(Node::with_value("_HLM_PROFILE", "Y")),
        Some(_) => {}
        None => {
            let at = indi.children.iter().rposition(|c| c.tag == "OBJE").map_or(indi.children.len(), |p| p + 1);
            indi.children.insert(at, link_node(object, profile));
        }
    }
    Ok(())
}

fn add(s: &mut Session, args: &Value) -> ApiResult {
    let paths: Vec<String> = args.get("paths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    if paths.is_empty() {
        return Err(ApiError::bad_args("paths"));
    }
    let person = args.get("person").and_then(Value::as_str).map(str::to_string);
    let kind = args.get("kind").and_then(Value::as_str);
    let caption = args.get("caption").and_then(Value::as_str);
    let document_type = args.get("documentType").and_then(Value::as_str);
    let profile = args.get("profile").and_then(Value::as_bool).unwrap_or(false);
    let layers = s.layers();
    // Part B: in „W folderach osób” a file for a person goes to that person's folder.
    let target_dir = crate::layout::dir_for_person(s, person.as_deref());
    // Work out duplicates and copy files before building the record changes.
    let mut prepared: Vec<(Option<String>, Option<String>)> = Vec::new();
    for path in &paths {
        let source = Path::new(path);
        if !source.is_file() {
            return Err(ApiError::new("not_found", format!("Nie ma pliku {path}.")));
        }
        match duplicate_of(s, source) {
            Some(existing) => prepared.push((Some(existing), None)),
            None => prepared.push((None, Some(copy_into_archive(&layers, &target_dir, source)?))),
        }
    }
    let mut changes = Changes::new(&s.archive.doc);
    let mut result = Vec::new();
    for (n, (existing, relative)) in prepared.into_iter().enumerate() {
        let (id, duplicate) = match (existing, relative) {
            (Some(id), _) => (id, true),
            (None, Some(relative)) => {
                let xref = changes.new_xref("O");
                changes.add(object_node(&xref, &relative, kind, caption, document_type));
                (xref, false)
            }
            (None, None) => continue,
        };
        if let Some(person) = &person {
            add_link(&mut changes, person, &id, profile && n == 0)?;
        }
        result.push(json!({ "id": id, "duplicate": duplicate }));
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Array(result))
}

fn update(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let mut changes = Changes::new(&s.archive.doc);
    let object = changes.get(&id)?;
    if object.child("FILE").is_none() {
        object.children.insert(0, Node::new("FILE"));
    }
    if let Some(title) = args.get("title") {
        let file = object.child_mut("FILE").expect("added above");
        set_child_text(file, "TITL", title.as_str());
    }
    if args.get("kind").is_some() || args.get("documentType").is_some() {
        let file = object.child_mut("FILE").expect("added above");
        let path = file.value.clone().unwrap_or_default();
        if file.child("FORM").is_none() {
            file.children.insert(0, Node::with_value("FORM", media_type(&path)));
        }
        let form = file.child_mut("FORM").expect("added above");
        let kind = args.get("kind").and_then(Value::as_str);
        let doc_type = args.get("documentType").and_then(Value::as_str);
        let current = form.child("MEDI").and_then(|m| m.value.clone()).unwrap_or_default();
        let value = match kind {
            Some("photo") => "PHOTO".to_string(),
            Some("document") => "MANUSCRIPT".to_string(),
            Some("other") => "OTHER".to_string(),
            _ if current.is_empty() => "MANUSCRIPT".to_string(),
            _ => current,
        };
        form.children.retain(|c| c.tag != "MEDI" && c.tag != "TYPE");
        let mut medi = Node::with_value("MEDI", &value);
        if let Some(t) = doc_type.map(str::trim).filter(|t| !t.is_empty()) {
            medi.children.push(Node::with_value("PHRASE", t));
        }
        form.children.push(medi);
    }
    for (key, tag) in [("place", "_HLM_PLAC"), ("transcription", "_HLM_TRANSCRIPTION"), ("translation", "_HLM_TRANSLATION"), ("note", "NOTE")] {
        if let Some(value) = args.get(key) {
            set_child_text(object, tag, value.as_str());
        }
    }
    // The own category: a name sets it (the kind it behaves like is sent as `kind`), null takes it off.
    match args.get("category") {
        Some(Value::String(name)) => set_child_text(object, "_HLM_CATEGORY", Some(&clean_category(name)?)),
        Some(Value::Null) => set_child_text(object, "_HLM_CATEGORY", None),
        _ => {}
    }
    if let Some(date) = args.get("date") {
        let gedcom = date.as_str().and_then(|t| heirloom_core::polish::parse_date_input(t).or_else(|| (!t.trim().is_empty()).then(|| t.trim().to_string())));
        set_child_text(object, "_HLM_DATE", gedcom.as_deref());
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// The media records (not objects written inside a person) whose category is `name`.
fn with_category(s: &Session, name: &str) -> Vec<String> {
    s.archive
        .doc
        .records
        .iter()
        .filter(|r| r.tag == "OBJE" && r.child_value("_HLM_CATEGORY").map(str::trim) == Some(name))
        .filter_map(|r| r.xref.clone())
        .collect()
}

/// „Zmień nazwę…” of an own category: the new name on every file that has it, as one edit. The list in the archive's
/// settings is the interface's to change.
fn rename_category(s: &mut Session, args: &Value) -> ApiResult {
    let from = req(args, "from")?.trim().to_string();
    let to = clean_category(&req(args, "to")?)?;
    let taken = s.archive.doc.records.iter().filter(|r| r.tag == "OBJE").filter_map(|r| r.child_value("_HLM_CATEGORY")).map(str::trim);
    if from != to && taken.into_iter().any(|c| c != from && c.to_lowercase() == to.to_lowercase()) {
        return Err(ApiError::new("bad_category", format!("Kategoria „{to}” już jest.")));
    }
    let ids = with_category(s, &from);
    let mut changes = Changes::new(&s.archive.doc);
    for id in &ids {
        set_child_text(changes.get(id)?, "_HLM_CATEGORY", Some(&to));
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "changed": ids.len() }))
}

/// „Usuń…” of an own category: the files lose the name and stay what they were (photos, documents).
fn remove_category(s: &mut Session, args: &Value) -> ApiResult {
    let name = req(args, "name")?.trim().to_string();
    let ids = with_category(s, &name);
    let mut changes = Changes::new(&s.archive.doc);
    for id in &ids {
        set_child_text(changes.get(id)?, "_HLM_CATEGORY", None);
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "changed": ids.len() }))
}

fn link(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let person = req(args, "person")?;
    let profile = args.get("profile").and_then(Value::as_bool).unwrap_or(false);
    let mut changes = Changes::new(&s.archive.doc);
    add_link(&mut changes, &person, &id, profile)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

fn unlink(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let person = req(args, "person")?;
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&person)?;
    remove_pointer(indi, "OBJE", &id);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// The people a command is about: `persons` (several), or the single `person`.
fn persons_arg(args: &Value) -> Result<Vec<String>, ApiError> {
    let mut persons: Vec<String> = match args.get("persons").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        None => vec![req(args, "person")?],
    };
    if persons.is_empty() {
        return Err(ApiError::bad_args("persons"));
    }
    let mut seen = std::collections::HashSet::new();
    persons.retain(|p| seen.insert(p.clone()));
    Ok(persons)
}

/// The photo becomes the profile photo of each person (one per person: the old one is taken off).
fn set_profile(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let persons = persons_arg(args)?;
    let mut changes = Changes::new(&s.archive.doc);
    for person in &persons {
        add_link(&mut changes, person, &id, true)?;
        if let Some(crop) = args.get("crop").and_then(Value::as_object) {
            let indi = changes.get(person)?;
            if let Some(link) = indi.children.iter_mut().find(|c| c.tag == "OBJE" && c.pointer() == Some(id.as_str())) {
                link.children.retain(|c| c.tag != "CROP");
                let mut node = Node::new("CROP");
                for key in ["TOP", "LEFT", "HEIGHT", "WIDTH"] {
                    if let Some(v) = crop.get(&key.to_ascii_lowercase()).and_then(Value::as_u64) {
                        node.children.push(Node::with_value(key, &v.to_string()));
                    }
                }
                link.children.push(node);
            }
        }
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// The photo stops being the profile photo of each person: `_HLM_PROFILE N` on their link, so the first photo does
/// not step in; they have none until another is chosen.
fn unset_profile(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let persons = persons_arg(args)?;
    let mut changes = Changes::new(&s.archive.doc);
    for person in &persons {
        let indi = changes.get(person)?;
        let link = indi
            .children
            .iter_mut()
            .find(|c| c.tag == "OBJE" && c.pointer() == Some(id.as_str()))
            .ok_or_else(|| ApiError::new("not_found", "To zdjęcie nie jest już na profilu tej osoby. Odśwież profil."))?;
        mark_profile(link, "N");
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Replaces the profile marker of a photo link (`Y` or `N`; any `_PRIM` from another program goes).
fn mark_profile(link: &mut Node, value: &str) {
    link.children.retain(|c| c.tag != "_HLM_PROFILE" && c.tag != "_PRIM");
    link.children.push(Node::with_value("_HLM_PROFILE", value));
}

/// „Własna kolejność”: puts the person's photos in the given order. The photo links change places only among the
/// places photo links have now — documents and every other line of the person stay where they are. Photos missing
/// from `order` follow, in their old order. The profile photo does not change: a person who has none marked gets
/// the first photo (the one standing in for it) marked first.
fn reorder(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let order: Vec<String> = args.get("order").and_then(Value::as_array).ok_or_else(|| ApiError::bad_args("order"))?.iter().filter_map(Value::as_str).map(str::to_string).collect();
    // The person's links as the profile reads them (same order as the `OBJE` lines): which are photos, which are marked.
    let d = s.derived();
    let i = d.index(&person).ok_or_else(|| crate::people::not_found(&person))?;
    let links: Vec<(String, bool, bool)> = d.view.people[i]
        .media
        .iter()
        .map(|l| (l.object.clone(), d.view.media.get(&l.object).is_some_and(|m| m.kind == heirloom_core::gedcom::view::MediaKind::Photo), l.profile || l.profile_off))
        .collect();
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&person)?;
    let lines: Vec<usize> = indi.children.iter().enumerate().filter(|(_, c)| c.tag == "OBJE").map(|(n, _)| n).collect();
    if lines.len() != links.len() {
        return Err(ApiError::new("stale", "Zdjęcia tej osoby zmieniły się. Odśwież profil."));
    }
    // The lines photos stand on. Each id in `order` takes the first photo line with that id not taken yet (a photo
    // linked twice keeps both lines); the lines left follow in their old order.
    let photos: Vec<usize> = (0..links.len()).filter(|&n| links[n].1).collect();
    if let Some(unknown) = order.iter().find(|id| !photos.iter().any(|&n| links[n].0 == **id)) {
        return Err(ApiError::new("not_found", format!("Zdjęcie {unknown} nie jest na profilu tej osoby. Odśwież profil.")));
    }
    let mut taken = vec![false; photos.len()];
    let mut sequence: Vec<usize> = Vec::new();
    for id in &order {
        if let Some(k) = (0..photos.len()).find(|&k| !taken[k] && links[photos[k]].0 == *id) {
            taken[k] = true;
            sequence.push(k);
        }
    }
    sequence.extend((0..photos.len()).filter(|&k| !taken[k]));
    if sequence.iter().enumerate().all(|(at, &k)| at == k) {
        return Ok(Value::Null);
    }
    if !photos.iter().any(|&n| links[n].2) {
        mark_profile(&mut indi.children[lines[photos[0]]], "Y");
    }
    let before: Vec<Node> = photos.iter().map(|&n| indi.children[lines[n]].clone()).collect();
    for (slot, &k) in photos.iter().zip(&sequence) {
        indi.children[lines[*slot]] = before[k].clone();
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// "Brakujące pliki": points an object at a file found elsewhere (copied into `media/` when outside the archive).
fn relink(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let path = req(args, "path")?;
    let layers = s.layers();
    // The owner's folder in „W folderach osób”.
    let target_dir = crate::layout::dir_for_object(s, &id);
    let relative = copy_into_archive(&layers, &target_dir, Path::new(&path))?;
    let mut changes = Changes::new(&s.archive.doc);
    let object = changes.get(&id)?;
    match object.child_mut("FILE") {
        Some(file) => file.value = Some(file_uri(&relative)),
        None => object.children.insert(0, Node::with_value("FILE", &file_uri(&relative))),
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "path": relative }))
}

/// Removes a photo or document from the archive's data. The file itself stays in `media/`.
fn delete(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let doc = &s.archive.doc;
    let mut changes = Changes::new(doc);
    let users: Vec<String> = doc.records.iter().filter(|r| r.children.iter().any(|c| c.tag == "OBJE" && c.pointer() == Some(id.as_str()))).filter_map(|r| r.xref.clone()).collect();
    for user in users {
        let record = changes.get(&user)?;
        remove_pointer(record, "OBJE", &id);
    }
    changes.remove(&id);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

fn save_source(s: &mut Session, args: &Value) -> ApiResult {
    let mut changes = Changes::new(&s.archive.doc);
    let id = match args.get("id").and_then(Value::as_str) {
        Some(id) => id.to_string(),
        None => {
            let xref = changes.new_xref("S");
            changes.add(Node::new("SOUR").with_xref(&xref).with_child(Node::with_value("UID", &new_uid())).with_child(stamp("CREA")));
            xref
        }
    };
    // The archive (repository) is its own record, shared by every source kept there.
    let repository = args.get("archive").and_then(Value::as_str).map(str::trim);
    let repo_xref = match repository {
        Some(name) if !name.is_empty() => {
            let found = s.archive.doc.records.iter().find(|r| r.tag == "REPO" && r.child_value("NAME").map(str::trim) == Some(name)).and_then(|r| r.xref.clone());
            Some(match found {
                Some(x) => x,
                None => {
                    let x = changes.new_xref("R");
                    changes.add(Node::new("REPO").with_xref(&x).with_child(Node::with_value("NAME", name)));
                    x
                }
            })
        }
        _ => None,
    };
    let source = changes.get(&id)?;
    for (key, tag) in [
        ("title", "TITL"),
        ("author", "AUTH"),
        ("publication", "PUBL"),
        ("text", "TEXT"),
        ("note", "NOTE"),
        ("kind", "_HLM_KIND"),
        ("parish", "_HLM_PARISH"),
        ("year", "_HLM_YEAR"),
        ("akt", "_HLM_AKT"),
        ("url", "_HLM_URL"),
    ] {
        if let Some(value) = args.get(key) {
            let text = match value {
                Value::Number(n) => Some(n.to_string()),
                other => other.as_str().map(str::to_string),
            };
            set_child_text(source, tag, text.as_deref());
        }
    }
    if repository.is_some() {
        source.children.retain(|c| c.tag != "REPO");
        if let Some(repo) = &repo_xref {
            let mut node = Node::with_value("REPO", repo);
            if let Some(call) = args.get("callNumber").and_then(Value::as_str).map(str::trim).filter(|c| !c.is_empty()) {
                node.children.push(Node::with_value("CALN", call));
            }
            source.children.push(node);
        }
    }
    if let Some(media) = args.get("media").and_then(Value::as_array) {
        source.children.retain(|c| c.tag != "OBJE");
        for m in media.iter().filter_map(Value::as_str) {
            source.children.push(Node::with_value("OBJE", m));
        }
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "id": id }))
}

/// Removes a source and every citation of it.
fn delete_source(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let doc = &s.archive.doc;
    let mut changes = Changes::new(doc);
    let users: Vec<String> = doc.records.iter().filter(|r| r.xref.as_deref() != Some(id.as_str()) && cites(r, &id)).filter_map(|r| r.xref.clone()).collect();
    for user in users {
        strip_citations(changes.get(&user)?, &id);
    }
    changes.remove(&id);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Where a person's profile cites a source, record by record: the person (anywhere in it), the events of their
/// families, and their texts. `what` says which entries, `others` who else sees that record on their profile.
struct Citing {
    record: String,
    /// Strip only the family's events (a family's own citations are not on the profile).
    events_only: bool,
    what: Vec<String>,
    others: Vec<String>,
}

fn cites(node: &Node, id: &str) -> bool {
    node.children.iter().any(|c| (c.tag == "SOUR" && c.pointer() == Some(id)) || cites(c, id))
}

fn strip_citations(node: &mut Node, id: &str) {
    node.children.retain(|c| !(c.tag == "SOUR" && c.pointer() == Some(id)));
    for c in &mut node.children {
        strip_citations(c, id);
    }
}

fn citing(s: &mut Session, person: &str, source: &str) -> Result<Vec<Citing>, ApiError> {
    use heirloom_core::gedcom::view::{FAM_EVENTS, INDI_EVENTS};
    let doc = &s.archive.doc;
    let indi = doc.record(person).ok_or_else(|| crate::people::not_found(person))?.clone();
    let linked = |tags: &[&str]| -> Vec<Node> {
        indi.children.iter().filter(|c| tags.contains(&c.tag.as_str())).filter_map(|c| c.pointer()).filter_map(|x| doc.record(x).cloned()).collect()
    };
    let families = linked(&["FAMS"]);
    let texts = linked(&["SNOTE", "NOTE"]);
    let d = s.derived();
    let name = |x: &str| d.index(x).map(|i| d.info[i].name.clone()).unwrap_or_else(|| x.to_string());
    let event = |c: &Node| crate::people::event_name(&c.tag, c.child_value("TYPE"));
    let mut out = Vec::new();
    let mut what: Vec<String> = Vec::new();
    for c in indi.children.iter().filter(|c| (c.tag == "SOUR" && c.pointer() == Some(source)) || cites(c, source)) {
        let word = match c.tag.as_str() {
            "SOUR" => "Dane osoby (ogólnie)".to_string(),
            tag if INDI_EVENTS.contains(&tag) => event(c),
            "NAME" => "Imię i nazwisko".into(),
            "NOTE" => "Notatka".into(),
            "OBJE" => "Zdjęcie".into(),
            _ => "Inne dane z pliku".into(),
        };
        if !what.contains(&word) {
            what.push(word);
        }
    }
    if !what.is_empty() {
        out.push(Citing { record: person.to_string(), events_only: false, what, others: Vec::new() });
    }
    for fam in &families {
        let what: Vec<String> = fam.children.iter().filter(|c| FAM_EVENTS.contains(&c.tag.as_str()) && cites(c, source)).map(event).collect();
        if what.is_empty() {
            continue;
        }
        let others = fam.children.iter().filter(|c| c.tag == "HUSB" || c.tag == "WIFE").filter_map(|c| c.pointer()).filter(|x| *x != person).map(name).collect();
        out.push(Citing { record: fam.xref.clone().unwrap_or_default(), events_only: true, what, others });
    }
    for text in texts.iter().filter(|t| cites(t, source)) {
        let xref = text.xref.clone().unwrap_or_default();
        let label = match d.view.texts.get(&xref) {
            Some(t) => match &t.title {
                Some(title) => format!("{} „{title}”", crate::people::kind_word(t.kind)),
                None => {
                    let plain = crate::text::plain(&t.body).trim().trim_matches(['„', '”', '"']).to_string();
                    let short: String = plain.chars().take(40).collect();
                    let more = if plain.chars().count() > 40 { "…" } else { "" };
                    format!("{} „{short}{more}”", crate::people::kind_word(t.kind))
                }
            },
            None => "Tekst".into(),
        };
        let others = d
            .view
            .people
            .iter()
            .enumerate()
            .filter(|(i, p)| d.xref(*i) != person && p.texts.iter().any(|r| matches!(r, heirloom_core::gedcom::view::TextRef::Shared(x) if *x == xref)))
            .map(|(i, _)| d.info[i].name.clone())
            .collect();
        out.push(Citing { record: xref, events_only: false, what: vec![label], others });
    }
    Ok(out)
}

/// What „Odłącz źródło” would change, in words. Nothing is changed.
pub fn detach_preview(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let source = req(args, "source")?;
    let plan = citing(s, &person, &source)?;
    let d = s.derived();
    let who = d.index(&person).map(|i| d.info[i].name.clone()).unwrap_or_else(|| person.clone());
    let title = d.view.sources.get(&source).and_then(|x| x.title.clone()).unwrap_or_else(|| source.clone());
    if plan.is_empty() {
        return Err(ApiError::new("stale", "Ta osoba nie ma już przypisu do tego źródła. Odśwież profil."));
    }
    let what: Vec<String> = plan.iter().flat_map(|c| c.what.iter().cloned()).collect();
    let gone = if what.len() == 1 { "Zniknie przypis przy" } else { "Znikną przypisy przy" };
    let mut parts = vec![format!("„{title}” przestanie być źródłem dla: {who}. {gone}: {}.", what.join(", "))];
    for c in plan.iter().filter(|c| !c.others.is_empty()) {
        let also = if c.events_only { format!("{} to wpis wspólny z", c.what.join(", ")) } else { format!("Ten tekst ({}) jest też na profilu", c.what.join(", ")) };
        parts.push(format!("{also}: {} — tam przypis też zniknie.", c.others.join(", ")));
    }
    parts.push("Samo źródło zostaje w archiwum (Źródła), razem z przypisami u innych osób. Do zapisu możesz to cofnąć (Ctrl Z).".into());
    Ok(json!({ "title": "Odłączyć źródło od tej osoby?", "text": parts.join(" "), "source": title }))
}

/// „Odłącz źródło”: takes a source's citations off one person's profile (see [`citing`]); the source stays.
fn detach_source(s: &mut Session, args: &Value) -> ApiResult {
    use heirloom_core::gedcom::view::FAM_EVENTS;
    let person = req(args, "person")?;
    let source = req(args, "source")?;
    let plan = citing(s, &person, &source)?;
    let mut changes = Changes::new(&s.archive.doc);
    for c in &plan {
        let record = changes.get(&c.record)?;
        if c.events_only {
            for e in record.children.iter_mut().filter(|e| FAM_EVENTS.contains(&e.tag.as_str())) {
                strip_citations(e, &source);
            }
        } else {
            strip_citations(record, &source);
        }
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Cites a source for a person's fact (`fact` = BIRT, DEAT…) or for the person as a whole.
fn add_citation(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let source = req(args, "source")?;
    let fact = args.get("fact").and_then(Value::as_str);
    let page = args.get("page").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty());
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&person)?;
    let target = match fact {
        Some(tag) => indi.children.iter_mut().find(|c| c.tag == tag).ok_or_else(|| ApiError::bad_args("fact"))?,
        None => indi,
    };
    if !has_pointer(target, "SOUR", &source) {
        let mut citation = Node::with_value("SOUR", &source);
        if let Some(page) = page {
            citation.children.push(Node::with_value("PAGE", page));
        }
        target.children.push(citation);
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::Archive;
    use heirloom_core::gedcom::view::MediaKind;

    #[test]
    fn photos_are_copied_linked_and_recognised_twice() {
        let dir = tempfile::tempdir().unwrap();
        let archive = Archive::create(&dir.path().join("a"), "A").unwrap();
        let mut s = Session::new(archive);
        let person = crate::edit::call(&mut s, "person.create", &json!({ "given": "Józef", "surname": "Kowalski" })).unwrap()["id"].as_str().unwrap().to_string();
        let photo = dir.path().join("Józef 1904.jpg");
        image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3])).save(&photo).unwrap();
        let added = crate::edit::call(&mut s, "media.add", &json!({ "paths": [photo.to_string_lossy()], "person": person, "caption": "Ślub", "profile": true })).unwrap();
        assert_eq!(added[0]["duplicate"], false);
        assert!(dir.path().join("a").join("media").join("Józef 1904.jpg").is_file());
        let again = crate::edit::call(&mut s, "media.add", &json!({ "paths": [photo.to_string_lossy()] })).unwrap();
        assert_eq!((again[0]["duplicate"].as_bool(), again[0]["id"].as_str()), (Some(true), added[0]["id"].as_str()));
        let d = s.derived();
        let i = d.index(&person).unwrap();
        assert_eq!(d.info[i].photo.as_ref().map(|p| p.1.as_str()), Some("media/Józef 1904.jpg"));
        let object = d.view.media.values().next().unwrap();
        assert_eq!((object.kind, object.title.as_deref()), (MediaKind::Photo, Some("Ślub")));
    }

    #[test]
    fn a_file_linked_inside_a_person_is_not_reused_as_a_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(root.join("media")).unwrap();
        let photo = root.join("media").join("jan.jpg");
        image::RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3])).save(&photo).unwrap();
        // An older file's object written inside the person has no record id to point to.
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 OBJE\n2 FILE media/jan.jpg\n3 FORM image/jpeg\n\
0 @I2@ INDI\n1 NAME Anna /Nowak/\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        let mut s = Session::new(Archive::open(&root).unwrap());
        let copy = dir.path().join("jan kopia.jpg");
        std::fs::copy(&photo, &copy).unwrap();
        let added = crate::edit::call(&mut s, "media.add", &json!({ "paths": [copy.to_string_lossy()], "person": "@I2@" })).unwrap();
        let link = s.archive.doc.record("@I2@").unwrap().child("OBJE").unwrap();
        assert!(link.pointer().is_some_and(|p| s.archive.doc.record(p).is_some()), "{link:?}, {added}");
    }

    /// Three files: a photo and a document with a category, one with none; one category is also in the settings list.
    fn category_session(dir: &Path) -> Session {
        let root = dir.join("a");
        std::fs::create_dir_all(&root).unwrap();
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Anna /Nowak/\n1 OBJE @O1@\n1 OBJE @D1@\n\
0 @O1@ OBJE\n1 FILE media/p1.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n1 _HLM_CATEGORY Pocztówki\n\
0 @O2@ OBJE\n1 FILE media/p2.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n1 _HLM_CATEGORY Pocztówki\n\
0 @D1@ OBJE\n1 FILE media/akt.pdf\n2 FORM application/pdf\n3 MEDI MANUSCRIPT\n1 _HLM_CATEGORY Listy\n\
0 @O3@ OBJE\n1 FILE media/p3.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        Session::new(Archive::open(&root).unwrap())
    }

    fn category_of(s: &Session, id: &str) -> Option<String> {
        s.archive.doc.record(id).unwrap().child_value("_HLM_CATEGORY").map(str::to_string)
    }

    #[test]
    fn a_category_is_set_renamed_and_removed_as_undoable_edits() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = category_session(dir.path());
        // Set: the name and the kind it behaves like go together; null takes the name off again.
        crate::edit::call(&mut s, "media.update", &json!({ "id": "@O3@", "kind": "document", "documentType": "", "category": "  Listy   rodzinne " })).unwrap();
        assert_eq!(category_of(&s, "@O3@").as_deref(), Some("Listy rodzinne"));
        let object = s.derived().view.media.get("@O3@").unwrap().clone();
        assert_eq!((object.kind, object.category.as_deref()), (MediaKind::Document, Some("Listy rodzinne")));
        let medi = s.archive.doc.record("@O3@").unwrap().child("FILE").unwrap().child("FORM").unwrap().child_value("MEDI").map(str::to_string);
        assert_eq!(medi.as_deref(), Some("MANUSCRIPT"));
        crate::edit::call(&mut s, "media.update", &json!({ "id": "@O3@", "kind": "photo", "category": null })).unwrap();
        assert_eq!(category_of(&s, "@O3@"), None);
        assert_eq!(s.derived().view.media.get("@O3@").unwrap().kind, MediaKind::Photo);
        // A built-in kind's name, an empty name or one that is too long is refused.
        for bad in ["Zdjęcie", " dokument ", "INNE", "  ", &"x".repeat(61)] {
            assert!(crate::edit::call(&mut s, "media.update", &json!({ "id": "@O3@", "category": bad })).is_err(), "{bad:?}");
        }
        assert_eq!(category_of(&s, "@O3@"), None);
        // Rename: every file that has it, one undo step; the rest stay.
        let depth = s.archive.undo_depth();
        let done = crate::edit::call(&mut s, "media.renameCategory", &json!({ "from": "Pocztówki", "to": "Kartki" })).unwrap();
        assert_eq!(done["changed"], 2);
        assert_eq!((category_of(&s, "@O1@").as_deref(), category_of(&s, "@O2@").as_deref(), category_of(&s, "@D1@").as_deref()), (Some("Kartki"), Some("Kartki"), Some("Listy")));
        assert_eq!(s.archive.undo_depth(), depth + 1, "one edit");
        // Not onto another category (even in other letters), not onto a built-in kind's name.
        assert!(crate::edit::call(&mut s, "media.renameCategory", &json!({ "from": "Kartki", "to": "listy" })).is_err());
        assert!(crate::edit::call(&mut s, "media.renameCategory", &json!({ "from": "Kartki", "to": "Dokument" })).is_err());
        // Only the letters of its own name may change.
        crate::edit::call(&mut s, "media.renameCategory", &json!({ "from": "Kartki", "to": "KARTKI" })).unwrap();
        assert_eq!(category_of(&s, "@O1@").as_deref(), Some("KARTKI"));
        // Remove: the files lose the name and stay what they were.
        let done = crate::edit::call(&mut s, "media.removeCategory", &json!({ "name": "KARTKI" })).unwrap();
        assert_eq!(done["changed"], 2);
        assert_eq!((category_of(&s, "@O1@"), category_of(&s, "@O2@")), (None, None));
        let d = s.derived();
        assert!(d.view.media.get("@O1@").is_some_and(|m| m.kind == MediaKind::Photo) && d.view.media.get("@D1@").is_some_and(|m| m.kind == MediaKind::Document));
        assert_eq!(category_of(&s, "@D1@").as_deref(), Some("Listy"));
        // A name nobody has changes nothing and records nothing.
        let depth = s.archive.undo_depth();
        crate::edit::call(&mut s, "media.removeCategory", &json!({ "name": "Nikt" })).unwrap();
        assert_eq!(s.archive.undo_depth(), depth);
        // Undo brings the names back.
        assert!(s.archive.undo());
        assert_eq!(category_of(&s, "@O1@").as_deref(), Some("KARTKI"));
    }

    #[test]
    fn the_categories_are_the_saved_list_plus_the_names_found_on_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = category_session(dir.path());
        let display = |text: &str| -> serde_json::Map<String, Value> { serde_json::from_str::<Value>(text).unwrap().as_object().unwrap().clone() };
        // The list has „Pocztówki” (kind from the list), an unused „Mapy”, a repeat and two wrong entries.
        let saved = crate::lists::saved_categories(&display(
            r#"{ "mediaCategories": [ { "name": "Mapy", "base": "document" }, { "name": "Pocztówki", "base": "photo" }, { "name": "Pocztówki", "base": "other" }, { "name": "", "base": "photo" }, { "name": "Zły", "base": "video" } ] }"#,
        ));
        assert_eq!(saved.iter().map(|(n, b)| (n.as_str(), b.code())).collect::<Vec<_>>(), [("Mapy", "document"), ("Pocztówki", "photo")]);
        let rows = crate::lists::media_categories(s.derived(), &saved);
        let rows: Vec<_> = rows.as_array().unwrap().iter().map(|r| (r["name"].as_str().unwrap(), r["base"].as_str().unwrap(), r["count"].as_u64().unwrap(), r["saved"].as_bool().unwrap())).collect();
        // „Listy” is on a file but not in the list: it is shown all the same, its kind taken from the file.
        assert_eq!(rows, [("Mapy", "document", 0, true), ("Pocztówki", "photo", 2, true), ("Listy", "document", 1, false)]);
        // Without any settings the files still carry their categories.
        let rows = crate::lists::media_categories(s.derived(), &[]);
        assert_eq!(rows.as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Listy", "Pocztówki"]);
        // The media list and the profile carry the category (a document's „kind” is its name).
        let roots = s.file_roots();
        assert_eq!(crate::lists::media_get(s.derived(), "@D1@", &roots).unwrap()["category"], "Listy");
        let profile = crate::people::profile(s.derived(), "@I1@", &[]).unwrap();
        assert_eq!(profile["gallery"][0]["category"], "Pocztówki");
        assert_eq!((profile["documents"][0]["category"].as_str(), profile["documents"][0]["kind"].as_str()), (Some("Listy"), Some("Listy")));
    }

    #[test]
    fn a_source_is_detached_from_one_person_and_stays_in_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(&root).unwrap();
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Józef /Kowalski/\n1 BIRT\n2 DATE 1878\n2 SOUR @S1@\n3 PAGE 45\n1 OCCU kolejarz\n2 SOUR @S2@\n1 SOUR @S1@\n1 FAMS @F1@\n1 SNOTE @N1@\n\
0 @I2@ INDI\n1 NAME Marianna /Nowak/\n1 FAMS @F1@\n1 BIRT\n2 SOUR @S1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 MARR\n2 DATE 1904\n2 SOUR @S1@\n\
0 @N1@ SNOTE Dzieciństwo w Wólce.\n1 _HLM_KIND bio\n1 _HLM_TITLE Dzieciństwo\n1 SOUR @S1@\n\
0 @S1@ SOUR\n1 TITL Księga chrztów Łęczna\n0 @S2@ SOUR\n1 TITL Akta kolei\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        let mut s = Session::new(Archive::open(&root).unwrap());
        let preview = crate::edit::call(&mut s, "source.detachPreview", &json!({ "person": "@I1@", "source": "@S1@" })).unwrap();
        let words = preview["text"].as_str().unwrap();
        assert!(words.contains("Urodzenie, Dane osoby (ogólnie), Ślub, Życiorys „Dzieciństwo”"), "{words}");
        assert!(words.contains("Marianna Nowak"), "the wedding is shared: {words}");
        assert_eq!(s.archive.undo_depth(), 0, "the preview changes nothing");
        crate::edit::call(&mut s, "source.detach", &json!({ "person": "@I1@", "source": "@S1@" })).unwrap();
        let doc = &s.archive.doc;
        assert!(!cites(doc.record("@I1@").unwrap(), "@S1@") && !cites(doc.record("@F1@").unwrap(), "@S1@") && !cites(doc.record("@N1@").unwrap(), "@S1@"));
        assert!(cites(doc.record("@I1@").unwrap(), "@S2@"), "other sources stay");
        assert!(cites(doc.record("@I2@").unwrap(), "@S1@"), "the wife's own citation stays");
        assert!(doc.record("@S1@").is_some(), "the source stays in the archive");
        assert_eq!(s.archive.undo_depth(), 1, "one undo step");
    }

    /// Two people, three photos (P1 is on both), one document in the middle of Anna's links. Nobody has a marker.
    fn gallery_session(dir: &Path) -> Session {
        let root = dir.join("a");
        std::fs::create_dir_all(&root).unwrap();
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Anna /Nowak/\n1 OBJE @O1@\n1 OBJE @D1@\n1 OBJE @O2@\n1 NOTE Zostaje\n1 OBJE @O3@\n\
0 @I2@ INDI\n1 NAME Jan /Nowak/\n1 OBJE @O2@\n1 OBJE @O3@\n\
0 @O1@ OBJE\n1 FILE media/p1.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @O2@ OBJE\n1 FILE media/p2.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @O3@ OBJE\n1 FILE media/p3.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n\
0 @D1@ OBJE\n1 FILE media/akt.pdf\n2 FORM application/pdf\n3 MEDI MANUSCRIPT\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        Session::new(Archive::open(&root).unwrap())
    }

    fn links_of(s: &Session, person: &str) -> Vec<String> {
        s.archive.doc.record(person).unwrap().children.iter().map(|c| format!("{} {}", c.tag, c.value.clone().unwrap_or_default())).collect()
    }

    fn profile_of(s: &mut Session, person: &str) -> Option<String> {
        let d = s.derived();
        d.info[d.index(person).unwrap()].photo.as_ref().map(|p| p.0.clone())
    }

    #[test]
    fn reorder_moves_photos_only_and_keeps_the_profile_photo() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = gallery_session(dir.path());
        assert_eq!(profile_of(&mut s, "@I1@").as_deref(), Some("@O1@"), "the first photo stands in");
        crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I1@", "order": ["@O3@", "@O1@", "@O2@"] })).unwrap();
        // Documents and other lines stay where they were; only the photo places are permuted.
        assert_eq!(links_of(&s, "@I1@"), ["NAME Anna /Nowak/", "OBJE @O3@", "OBJE @D1@", "OBJE @O1@", "NOTE Zostaje", "OBJE @O2@", "CHAN "]);
        assert_eq!(profile_of(&mut s, "@I1@").as_deref(), Some("@O1@"), "reordering does not change the profile photo");
        assert_eq!(s.archive.doc.record("@I1@").unwrap().children.iter().filter(|c| c.tag == "OBJE").filter_map(|c| c.child_value("_HLM_PROFILE")).collect::<Vec<_>>(), ["Y"]);
        assert_eq!(s.archive.undo_depth(), 1, "one edit");
        // The gallery shows the new order.
        let gallery: Vec<String> = crate::people::profile(s.derived(), "@I1@", &[]).unwrap()["gallery"].as_array().unwrap().iter().map(|g| g["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(gallery, ["@O3@", "@O1@", "@O2@"]);
        // Photos missing from the list follow in their old order; the other person is untouched.
        crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I1@", "order": ["@O2@"] })).unwrap();
        let gallery: Vec<String> = crate::people::profile(s.derived(), "@I1@", &[]).unwrap()["gallery"].as_array().unwrap().iter().map(|g| g["id"].as_str().unwrap().to_string()).collect();
        assert_eq!(gallery, ["@O2@", "@O3@", "@O1@"]);
        assert_eq!(links_of(&s, "@I2@"), ["NAME Jan /Nowak/", "OBJE @O2@", "OBJE @O3@"]);
        // Nothing to change, nothing recorded; a photo that is not hers (or a document) is refused.
        let depth = s.archive.undo_depth();
        crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I1@", "order": ["@O2@", "@O3@", "@O1@"] })).unwrap();
        assert_eq!(s.archive.undo_depth(), depth);
        assert!(crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I2@", "order": ["@O1@"] })).is_err());
        assert!(crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I1@", "order": ["@D1@"] })).is_err());
    }

    #[test]
    fn reorder_never_loses_a_photo_linked_twice() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(&root).unwrap();
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Anna /Nowak/\n1 OBJE @O1@\n1 OBJE @O1@\n1 OBJE @O2@\n\
0 @O1@ OBJE\n1 FILE media/p1.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n0 @O2@ OBJE\n1 FILE media/p2.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        let mut s = Session::new(Archive::open(&root).unwrap());
        crate::edit::call(&mut s, "media.reorder", &json!({ "person": "@I1@", "order": ["@O2@"] })).unwrap();
        let objects: Vec<_> = s.archive.doc.record("@I1@").unwrap().children.iter().filter(|c| c.tag == "OBJE").filter_map(|c| c.pointer()).collect();
        assert_eq!(objects, ["@O2@", "@O1@", "@O1@"]);
    }

    #[test]
    fn unsetting_the_profile_photo_leaves_the_person_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = gallery_session(dir.path());
        crate::edit::call(&mut s, "media.setProfile", &json!({ "id": "@O2@", "person": "@I1@" })).unwrap();
        assert_eq!(profile_of(&mut s, "@I1@").as_deref(), Some("@O2@"));
        crate::edit::call(&mut s, "media.unsetProfile", &json!({ "id": "@O2@", "persons": ["@I1@"] })).unwrap();
        assert_eq!(profile_of(&mut s, "@I1@"), None, "the first photo does not step in");
        let profile = crate::people::profile(s.derived(), "@I1@", &[]).unwrap();
        assert!(profile["gallery"].as_array().unwrap().iter().all(|g| g["profile"] == false && !g["profileOf"].as_array().unwrap().contains(&json!("@I1@"))));
        // The other person was not touched, and the `N` survives saving and opening the archive again.
        assert_eq!(profile_of(&mut s, "@I2@").as_deref(), Some("@O2@"));
        s.archive.save(&heirloom_core::SaveOptions { author: "test", batch: None, note: None, allow_foreign: false, kind: None }).unwrap();
        let mut again = Session::new(Archive::open(&dir.path().join("a")).unwrap());
        let marks: Vec<_> = again.archive.doc.record("@I1@").unwrap().children.iter().filter(|c| c.tag == "OBJE").filter_map(|c| c.child_value("_HLM_PROFILE").map(str::to_string)).collect();
        assert_eq!(marks, ["N"]);
        assert_eq!(profile_of(&mut again, "@I1@"), None);
        // Choosing a photo again clears the `N`.
        crate::edit::call(&mut again, "media.setProfile", &json!({ "id": "@O3@", "person": "@I1@" })).unwrap();
        let marks: Vec<_> = again.archive.doc.record("@I1@").unwrap().children.iter().filter(|c| c.tag == "OBJE").filter_map(|c| c.child_value("_HLM_PROFILE").map(str::to_string)).collect();
        assert_eq!(marks, ["Y"]);
        assert_eq!(profile_of(&mut again, "@I1@").as_deref(), Some("@O3@"));
        // A photo that is not on the person's profile is an error.
        assert!(crate::edit::call(&mut again, "media.unsetProfile", &json!({ "id": "@O1@", "persons": ["@I2@"] })).is_err());
    }

    #[test]
    fn a_photo_can_be_the_profile_photo_of_several_people_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = gallery_session(dir.path());
        crate::edit::call(&mut s, "media.setProfile", &json!({ "id": "@O1@", "person": "@I1@" })).unwrap();
        crate::edit::call(&mut s, "media.setProfile", &json!({ "id": "@O3@", "persons": ["@I1@", "@I2@"] })).unwrap();
        assert_eq!((profile_of(&mut s, "@I1@").as_deref(), profile_of(&mut s, "@I2@").as_deref()), (Some("@O3@"), Some("@O3@")));
        let marks = |s: &Session, p: &str| -> Vec<String> { s.archive.doc.record(p).unwrap().children.iter().filter(|c| c.tag == "OBJE").map(|c| c.child_value("_HLM_PROFILE").unwrap_or("-").to_string()).collect() };
        assert_eq!(marks(&s, "@I1@"), ["-", "-", "-", "Y"], "one profile photo per person: the old marker is gone");
        // The gallery lists everyone on the photo (this person first) and whose profile photo it is.
        let gallery = crate::people::profile(s.derived(), "@I2@", &[]).unwrap()["gallery"].clone();
        let shared = gallery.as_array().unwrap().iter().find(|g| g["id"] == "@O3@").unwrap();
        assert_eq!(shared["people"], json!([{ "id": "@I2@", "name": "Jan Nowak" }, { "id": "@I1@", "name": "Anna Nowak" }]));
        assert_eq!(shared["profileOf"], json!(["@I2@", "@I1@"]));
        // Taking it off one of them leaves the other.
        crate::edit::call(&mut s, "media.unsetProfile", &json!({ "id": "@O3@", "persons": ["@I2@"] })).unwrap();
        assert_eq!((profile_of(&mut s, "@I1@").as_deref(), profile_of(&mut s, "@I2@")), (Some("@O3@"), None));
    }
}
