//! Changing photos, documents and sources. New files are copied into the archive's `media/` folder with their
//! own readable names (PLAN §4.7); the same file added twice is recognised by its contents and reused.

use crate::edit::Changes;
use crate::gedwrite::{file_uri, has_pointer, media_type, new_uid, remove_pointer, set_child_text, stamp};
use crate::{ApiError, ApiResult, Session};
use heirloom_core::archive::MEDIA_DIR;
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
        "media.link" => link(s, args),
        "media.unlink" => unlink(s, args),
        "media.setProfile" => set_profile(s, args),
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

/// Where the files of the open data are: one archive's folder, or in archives opened together each archive's folder
/// by its key (their files are written `~b/media/x.jpg` there).
pub enum FileRoots {
    One(PathBuf),
    Many(Vec<(String, PathBuf)>),
}

impl FileRoots {
    pub fn resolve(&self, file: &str) -> PathBuf {
        match self {
            FileRoots::One(root) => resolve(root, file),
            FileRoots::Many(roots) => {
                let keyed = file.strip_prefix('~').and_then(|rest| rest.split_once('/'));
                match keyed.and_then(|(key, rest)| Some((roots.iter().find(|(k, _)| k == key)?, rest))) {
                    Some(((_, root), rest)) => resolve(root, rest),
                    // An absolute path, kept as it was written.
                    None => PathBuf::from(file),
                }
            }
        }
    }
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
pub fn copy_into_archive(root: &Path, source: &Path) -> Result<String, ApiError> {
    copy_new_into_archive(root, source).map(|(relative, _)| relative)
}

/// [`copy_into_archive`], also giving the file it created (None when the file was already inside the archive folder),
/// so a failed import can remove exactly what it added.
pub fn copy_new_into_archive(root: &Path, source: &Path) -> Result<(String, Option<PathBuf>), ApiError> {
    if let (Ok(source_abs), Ok(root_abs)) = (source.canonicalize(), root.canonicalize()) {
        if let Ok(relative) = source_abs.strip_prefix(&root_abs) {
            return Ok((relative.to_string_lossy().replace('\\', "/"), None));
        }
    }
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| ApiError::bad_args("path"))?;
    let dir = root.join(MEDIA_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| ApiError::new("io", format!("Nie można utworzyć folderu media: {e}")))?;
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.clone(), String::new()),
    };
    let mut target = dir.join(&name);
    let mut n = 2;
    while target.exists() {
        target = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    std::fs::copy(source, &target).map_err(|e| {
        // A half-written copy is ours (the name was free): it goes.
        let _ = std::fs::remove_file(&target);
        ApiError::new("io", format!("Nie można skopiować pliku {name}: {e}"))
    })?;
    Ok((format!("{MEDIA_DIR}/{}", target.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or(name)), Some(target)))
}

/// A media object already in the archive with exactly this file's contents.
fn duplicate_of(s: &mut Session, source: &Path) -> Option<String> {
    let size = std::fs::metadata(source).ok()?.len();
    let root = s.archive.root().to_path_buf();
    let d = s.derived();
    let mut hash = None;
    // Only records can be pointed to: an object written inside a person (`@I1@#0`) is not one.
    for id in d.view.media_order.iter().filter(|id| !id.contains('#')) {
        let Some(file) = d.view.media.get(id).and_then(|m| m.file.clone()) else { continue };
        let path = resolve(&root, &file);
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

fn add_link(changes: &mut Changes, person: &str, object: &str, profile: bool) -> Result<(), ApiError> {
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
    let root = s.archive.root().to_path_buf();
    // Work out duplicates and copy files before building the record changes.
    let mut prepared: Vec<(Option<String>, Option<String>)> = Vec::new();
    for path in &paths {
        let source = Path::new(path);
        if !source.is_file() {
            return Err(ApiError::new("not_found", format!("Nie ma pliku {path}.")));
        }
        match duplicate_of(s, source) {
            Some(existing) => prepared.push((Some(existing), None)),
            None => prepared.push((None, Some(copy_into_archive(&root, source)?))),
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
    if let Some(date) = args.get("date") {
        let gedcom = date.as_str().and_then(|t| heirloom_core::polish::parse_date_input(t).or_else(|| (!t.trim().is_empty()).then(|| t.trim().to_string())));
        set_child_text(object, "_HLM_DATE", gedcom.as_deref());
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
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

fn set_profile(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let person = req(args, "person")?;
    let mut changes = Changes::new(&s.archive.doc);
    add_link(&mut changes, &person, &id, true)?;
    if let Some(crop) = args.get("crop").and_then(Value::as_object) {
        let indi = changes.get(&person)?;
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
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// "Brakujące pliki": points an object at a file found elsewhere (copied into `media/` when outside the archive).
fn relink(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let path = req(args, "path")?;
    let root = s.archive.root().to_path_buf();
    let relative = copy_into_archive(&root, Path::new(&path))?;
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
}
