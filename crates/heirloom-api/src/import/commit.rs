//! Import step 5: writes what the user approved into the archive — people, families, events, texts, files and
//! sources — as one change group, then saves with the batch name, so "Cofnij import" can take it all back.

use super::check::{self, m_label, m_number};
use super::format;
use super::{Decision, Draft, Kind};
use crate::edit::{Changes, add_child_to_family, link};
use crate::gedwrite::{file_uri, has_pointer, media_type, name_value, new_uid, stamp, text_node};
use crate::media_edit::copy_into_archive;
use crate::{ApiError, Session};
use heirloom_core::gedcom::Node;
use heirloom_core::SaveOptions;
use serde_json::{Value, json};
use std::collections::HashMap;

fn batch_mark(node: &mut Node, batch: &str) {
    node.children.push(Node::with_value("_HLM_BATCH", batch));
}

fn cert(node: &mut Node, certainty: Option<&str>, basis: Option<&str>) {
    if let Some(c) = certainty.filter(|c| matches!(*c, "high" | "medium" | "low")) {
        node.children.push(Node::with_value("_HLM_CERT", c));
    }
    if let Some(b) = basis.filter(|b| matches!(*b, "stated" | "inferred")) {
        node.children.push(Node::with_value("_HLM_BASIS", b));
    }
}

/// A DATE node from the format's date, keeping the source's wording as PHRASE.
fn date_node(date: Option<&str>, original: Option<&str>, julian: Option<&str>) -> Option<Node> {
    let original = original.map(str::trim).filter(|o| !o.is_empty());
    let mut node = match date.map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => match format::gedcom_date(d) {
            Some(g) => Node::with_value("DATE", &g),
            None => Node::new("DATE").with_child(Node::with_value("PHRASE", original.unwrap_or(d))),
        },
        None => Node::new("DATE").with_child(Node::with_value("PHRASE", original?)),
    };
    if node.value.is_some() {
        if let Some(o) = original {
            node.children.push(Node::with_value("PHRASE", o));
        }
    }
    if let Some(j) = julian.and_then(format::gedcom_date) {
        node.children.push(Node::with_value("_HLM_JULIAN", &j));
    }
    Some(node)
}

fn role_code(role: &str) -> &'static str {
    match role {
        "father" => "FATH",
        "mother" => "MOTH",
        "godparent" => "GODP",
        "witness" => "WITN",
        "officiant" => "OFFICIATOR",
        "spouse" => "SPOU",
        _ => "OTHER",
    }
}

struct Context<'a> {
    draft: &'a Draft,
    name: String,
    /// P-id → record id of the saved person.
    people: HashMap<String, String>,
    /// P-id → UID (for mentions).
    uids: HashMap<String, String>,
    /// S-id / M-label / N-id → source record id.
    sources: HashMap<String, String>,
    /// M-label → media record id.
    media: HashMap<String, String>,
}

impl Context<'_> {
    fn citations(&self, src: &[String]) -> Vec<Node> {
        let mut out: Vec<Node> = Vec::new();
        for s in src {
            let key = m_number(s).map(m_label).unwrap_or_else(|| s.clone());
            if let Some(x) = self.sources.get(&key) {
                if !out.iter().any(|n| n.value.as_deref() == Some(x.as_str())) {
                    out.push(Node::with_value("SOUR", x));
                }
            }
        }
        out
    }

    fn decision(&self, pid: &str) -> Option<&Decision> {
        self.draft.decisions.get(pid)
    }

    /// Whether a merged person's field should be written ("variant", "replace", "add"), and how.
    fn choice(&self, pid: &str, field: &str) -> String {
        let d = self.decision(pid);
        if d.is_some_and(|d| d.excluded_fields.contains(field)) {
            return "skip".into();
        }
        d.and_then(|d| d.fields.get(field).cloned()).unwrap_or_else(|| "variant".into())
    }
}

fn is_included(decision: Option<&Decision>) -> bool {
    decision.is_some_and(|d| !d.excluded && matches!(d.kind, Kind::New | Kind::Merge))
}

pub fn commit(s: &mut Session, draft: &mut Draft, author: &str, note: Option<&str>, allow_foreign: bool) -> Result<Value, ApiError> {
    if draft.issues.iter().any(|i| i.level == "error") {
        return Err(ApiError::new("import_errors", "Najpierw napraw błędy w kroku „Sprawdź”."));
    }
    if draft.decisions.values().any(|d| d.kind == Kind::Undecided && !d.excluded) {
        return Err(ApiError::new("import_undecided", "Niektóre osoby czekają na decyzję w kroku „Dopasuj osoby”."));
    }
    if s.archive.settings().read_only {
        return Err(ApiError::new("read_only", "To archiwum jest tylko do odczytu (Ustawienia › Archiwum)."));
    }
    // The first save into another program's file needs the user's yes (PLAN §11.2 rule 4): asked before anything
    // is copied, so a refused import changes nothing.
    if s.archive.is_foreign() && !s.archive.settings().foreign_save_confirmed && !allow_foreign {
        let program = s.archive.origin().unwrap_or("nieznany program").to_string();
        return Err(heirloom_core::Error::ForeignNeedsConfirmation(program).into());
    }
    if s.archive.upgrade_to_v7() {
        s.changed();
    }
    let root = s.archive.root().to_path_buf();
    // "Cofnij import" takes back everything saved under the batch's name: a name already in the history (the same
    // answer imported twice, a corrected one) gets a number, or taking this import back would take the earlier too.
    let history = s.history();
    let taken = |name: &str| history.iter().any(|e| e.batch.as_deref() == Some(name));
    let name = (1..).map(|k| if k == 1 { draft.batch.name.clone() } else { format!("{} ({k})", draft.batch.name) }).find(|n| !taken(n)).unwrap_or_default();
    // Copy the files first (outside the data file); a failure here changes nothing in the data.
    let batch = &draft.batch.merged;
    let mut copied: HashMap<String, String> = HashMap::new();
    for f in &draft.files {
        let key = f.m.map(m_label).unwrap_or_else(|| f.name.clone());
        let choice = draft.file_choices.get(&key);
        let described = batch.media.iter().any(|m| m_number(&m.id).map(m_label).as_deref() == Some(key.as_str()));
        let assigned = choice.is_some_and(|c| !c.people.is_empty());
        if choice.is_some_and(|c| c.skip) || draft.duplicates.contains_key(&key) || (!described && !assigned) {
            continue;
        }
        copied.insert(key, copy_into_archive(&root, &f.path)?);
    }
    // The researcher's AI answers, kept as delivered (PLAN §11.2: zrodla-ai/).
    let folder = root.join("zrodla-ai").join(sanitize(&name));
    if std::fs::create_dir_all(&folder).is_ok() {
        for (n, text) in draft.raw.iter().enumerate() {
            keep_answer(&folder, n + 1, text);
        }
    }

    s.ensure();
    let derived = s.derived.as_ref().expect("ensured");
    let doc = &s.archive.doc;
    let mut changes = Changes::new(doc);
    let mut ctx = Context { draft, name: name.clone(), people: HashMap::new(), uids: HashMap::new(), sources: HashMap::new(), media: HashMap::new() };
    let mut new_people = 0;
    let mut merged_people = 0;

    // 1. People: new records, or the chosen existing ones (with a UID for mentions).
    for p in &batch.persons {
        let decision = ctx.decision(&p.id);
        if !is_included(decision) {
            continue;
        }
        let decision = decision.expect("included");
        match decision.kind {
            Kind::New => {
                let xref = changes.new_xref("I");
                let uid = new_uid();
                let mut indi = Node::new("INDI").with_xref(&xref).with_child(Node::with_value("UID", &uid));
                for name in &p.names {
                    let value = name_value(name.given.as_deref().unwrap_or(""), name.surname.as_deref().unwrap_or(""));
                    if value.is_empty() {
                        continue;
                    }
                    let mut node = Node::with_value("NAME", &value);
                    let kind = match name.kind.as_deref() {
                        Some("married") => "MARRIED",
                        Some("other") => "AKA",
                        _ => "BIRTH",
                    };
                    node.children.push(Node::with_value("TYPE", kind));
                    if let Some(nick) = name.nickname.as_deref().filter(|n| !n.trim().is_empty()) {
                        node.children.push(Node::with_value("NICK", nick.trim()));
                    }
                    if let Some(orig) = name.orig.as_deref().filter(|o| !o.trim().is_empty()) {
                        node.children.push(Node::with_value("TRAN", orig.trim()).with_child(Node::with_value("LANG", name.lang.as_deref().unwrap_or("und"))));
                    }
                    indi.children.push(node);
                }
                let sex = match p.sex.as_deref() {
                    Some("M") => "M",
                    Some("F") => "F",
                    _ => "U",
                };
                indi.children.push(Node::with_value("SEX", sex));
                for tag in &p.tags {
                    indi.children.push(Node::with_value("_HLM_TAG", tag));
                }
                indi.children.push(stamp("CREA"));
                batch_mark(&mut indi, &ctx.name);
                changes.add(indi);
                ctx.people.insert(p.id.clone(), xref);
                ctx.uids.insert(p.id.clone(), uid);
                new_people += 1;
            }
            Kind::Merge => {
                let target = decision.target.clone().ok_or_else(|| ApiError::bad_args("target"))?;
                let indi = changes.get(&target)?;
                let uid = match indi.child_value("UID").or_else(|| indi.child_value("_UID")) {
                    Some(u) => u.trim().to_string(),
                    None => {
                        let u = new_uid();
                        indi.children.insert(0, Node::with_value("UID", &u));
                        u
                    }
                };
                // A name form the archive doesn't have yet is added as a variant.
                if ctx.choice(&p.id, "name") != "keep" {
                    for name in &p.names {
                        let value = name_value(name.given.as_deref().unwrap_or(""), name.surname.as_deref().unwrap_or(""));
                        let exists = indi.children.iter().any(|c| c.tag == "NAME" && c.value.as_deref().map(|v| heirloom_core::fold::fold(&v.replace('/', ""))) == Some(heirloom_core::fold::fold(&value.replace('/', ""))));
                        if !exists && !value.is_empty() {
                            let at = indi.children.iter().rposition(|c| c.tag == "NAME").map_or(0, |i| i + 1);
                            indi.children.insert(at, Node::with_value("NAME", &value).with_child(Node::with_value("TYPE", "AKA")));
                        }
                    }
                }
                for tag in &p.tags {
                    if !indi.children.iter().any(|c| c.tag == "_HLM_TAG" && c.value.as_deref() == Some(tag.as_str())) {
                        indi.children.push(Node::with_value("_HLM_TAG", tag));
                    }
                }
                ctx.people.insert(p.id.clone(), target);
                ctx.uids.insert(p.id.clone(), uid);
                merged_people += 1;
            }
            _ => {}
        }
    }

    // 2. Media records for the files (or the records already in the archive).
    let mut profile_links: Vec<(String, String)> = Vec::new();
    for (key, relative) in &copied {
        let described = batch.media.iter().find(|m| m_number(&m.id).map(m_label).as_deref() == Some(key.as_str()));
        let choice = ctx.draft.file_choices.get(key).cloned().unwrap_or_default();
        let kind = choice.kind.clone().or_else(|| described.and_then(|m| m.kind.clone()));
        let photo = match kind.as_deref() {
            Some("photo") => true,
            Some(_) => false,
            None => media_type(relative).starts_with("image/"),
        };
        let xref = changes.new_xref("O");
        let mut medi = Node::with_value("MEDI", if photo { "PHOTO" } else { "MANUSCRIPT" });
        if let Some(t) = described.and_then(|m| m.document_type.clone()) {
            medi.children.push(Node::with_value("PHRASE", &t));
        }
        let mut file = Node::with_value("FILE", &file_uri(relative)).with_child(Node::with_value("FORM", media_type(relative)).with_child(medi));
        if let Some(caption) = choice.caption.clone().or_else(|| described.and_then(|m| m.caption.clone())).filter(|c| !c.trim().is_empty()) {
            file.children.push(Node::with_value("TITL", caption.trim()));
        }
        let mut obje = Node::new("OBJE").with_xref(&xref).with_child(file).with_child(Node::with_value("UID", &new_uid()));
        if let Some(m) = described {
            if let Some(date) = m.date.as_deref() {
                let value = format::gedcom_date(date).unwrap_or_else(|| date.to_string());
                obje.children.push(Node::with_value("_HLM_DATE", &value));
            }
            if let Some(place) = &m.place {
                obje.children.push(Node::with_value("_HLM_PLAC", place));
            }
            if let Some(t) = &m.transcription {
                obje.children.push(text_node("_HLM_TRANSCRIPTION", t));
            }
            if let Some(t) = &m.translation {
                obje.children.push(text_node("_HLM_TRANSLATION", t));
            }
            if let Some(n) = &m.note {
                obje.children.push(text_node("NOTE", n));
            }
        }
        obje.children.push(stamp("CREA"));
        batch_mark(&mut obje, &ctx.name);
        changes.add(obje);
        ctx.media.insert(key.clone(), xref);
    }
    for (key, existing) in &ctx.draft.duplicates {
        ctx.media.insert(key.clone(), existing.clone());
    }
    // Links from people to their files; the chosen profile photo gets the star.
    let mut file_people: Vec<(String, Vec<String>)> = Vec::new();
    for m in &batch.media {
        let Some(key) = m_number(&m.id).map(m_label) else { continue };
        let mut people: Vec<String> = m.depicts.iter().chain(&m.about).cloned().collect();
        if let Some(choice) = ctx.draft.file_choices.get(&key) {
            people.extend(choice.people.iter().cloned());
        }
        file_people.push((key, people));
    }
    for f in &ctx.draft.files {
        let key = f.m.map(m_label).unwrap_or_else(|| f.name.clone());
        if !file_people.iter().any(|(k, _)| *k == key) {
            if let Some(choice) = ctx.draft.file_choices.get(&key) {
                file_people.push((key, choice.people.clone()));
            }
        }
    }
    for (key, people) in file_people {
        let Some(object) = ctx.media.get(&key).cloned() else { continue };
        let mut seen = Vec::new();
        for pid in people {
            if seen.contains(&pid) {
                continue;
            }
            seen.push(pid.clone());
            let Some(xref) = ctx.people.get(&pid).cloned() else { continue };
            let profile = ctx.draft.profile.get(&pid).cloned().flatten().as_deref() == Some(key.as_str());
            let indi = changes.get(&xref)?;
            if !has_pointer(indi, "OBJE", &object) {
                let at = indi.children.iter().rposition(|c| c.tag == "OBJE").map_or(indi.children.len(), |i| i + 1);
                indi.children.insert(at, Node::with_value("OBJE", &object));
            }
            if profile {
                profile_links.push((xref, object.clone()));
            }
        }
    }
    for (xref, object) in profile_links {
        let indi = changes.get(&xref)?;
        for link in indi.children.iter_mut().filter(|c| c.tag == "OBJE") {
            link.children.retain(|c| c.tag != "_HLM_PROFILE");
            if link.value.as_deref() == Some(object.as_str()) {
                link.children.push(Node::with_value("_HLM_PROFILE", "Y"));
            }
        }
    }

    // 3. Sources (with the archive as a repository record), plus implicit sources for files and pasted notes
    // cited directly.
    let mut repositories: HashMap<String, String> = doc
        .records
        .iter()
        .filter(|r| r.tag == "REPO")
        .filter_map(|r| Some((r.child_value("NAME")?.trim().to_string(), r.xref.clone()?)))
        .collect();
    for src in &batch.sources {
        let xref = changes.new_xref("S");
        let mut node = Node::new("SOUR").with_xref(&xref).with_child(Node::with_value("UID", &new_uid()));
        if let Some(t) = &src.title {
            node.children.push(Node::with_value("TITL", t));
        }
        for (tag, value) in [("_HLM_KIND", &src.kind), ("_HLM_PARISH", &src.parish), ("_HLM_YEAR", &src.year), ("_HLM_AKT", &src.akt), ("_HLM_URL", &src.url)] {
            if let Some(v) = value.as_deref().filter(|v| !v.trim().is_empty()) {
                node.children.push(Node::with_value(tag, v.trim()));
            }
        }
        if let Some(archive) = src.archive.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            let repo = match repositories.get(archive) {
                Some(r) => r.clone(),
                None => {
                    let r = changes.new_xref("R");
                    changes.add(Node::new("REPO").with_xref(&r).with_child(Node::with_value("NAME", archive)));
                    repositories.insert(archive.to_string(), r.clone());
                    r
                }
            };
            let mut repo_node = Node::with_value("REPO", &repo);
            if let Some(call) = src.call_number.as_deref().filter(|c| !c.trim().is_empty()) {
                repo_node.children.push(Node::with_value("CALN", call.trim()));
            }
            node.children.push(repo_node);
        }
        for m in &src.media {
            if let Some(object) = m_number(m).map(m_label).and_then(|k| ctx.media.get(&k)) {
                node.children.push(Node::with_value("OBJE", object));
                ctx.sources.entry(m_label(m_number(m).unwrap_or(0))).or_insert_with(|| xref.clone());
            }
        }
        if let Some(n) = &src.note {
            node.children.push(text_node("NOTE", n));
        }
        node.children.push(stamp("CREA"));
        batch_mark(&mut node, &ctx.name);
        changes.add(node);
        ctx.sources.insert(src.id.clone(), xref);
    }
    let cited: Vec<String> = batch
        .events
        .iter()
        .flat_map(|e| e.src.iter())
        .chain(batch.texts.iter().flat_map(|t| t.src.iter()))
        .chain(batch.persons.iter().flat_map(|p| p.names.iter().flat_map(|n| n.src.iter())))
        .chain(batch.relationships.iter().flat_map(|r| r.src.iter()))
        .cloned()
        .collect();
    for id in cited {
        let key = m_number(&id).map(m_label).unwrap_or_else(|| id.clone());
        if ctx.sources.contains_key(&key) || id.starts_with('S') {
            continue;
        }
        let xref = changes.new_xref("S");
        let title = if id.starts_with('N') {
            format!("Notatka {id} wklejona do czatu ({})", ctx.name)
        } else {
            let caption = batch.media.iter().find(|m| m_number(&m.id).map(m_label).as_deref() == Some(key.as_str())).and_then(|m| m.caption.clone());
            format!("Dokument {key}{}", caption.map(|c| format!(": {c}")).unwrap_or_default())
        };
        let mut node = Node::new("SOUR").with_xref(&xref).with_child(Node::with_value("TITL", &title)).with_child(Node::with_value("UID", &new_uid()));
        if let Some(object) = ctx.media.get(&key) {
            node.children.push(Node::with_value("OBJE", object));
        }
        node.children.push(stamp("CREA"));
        batch_mark(&mut node, &ctx.name);
        changes.add(node);
        ctx.sources.insert(key, xref);
    }

    // 4. Families first (partners, then parents, then siblings), so the events find them.
    let skip_relative = |ctx: &Context, pid: &str, field: &str| -> bool { ctx.decision(pid).is_some_and(|d| d.kind == Kind::Merge) && ctx.choice(pid, field) == "skip" };
    for r in batch.relationships.iter().filter(|r| r.relation.as_deref() == Some("partners")) {
        let (Some(a), Some(b)) = (r.a.as_deref(), r.b.as_deref()) else { continue };
        if skip_relative(&ctx, a, "spouse") || skip_relative(&ctx, b, "spouse") {
            continue;
        }
        let (Some(xa), Some(xb)) = (ctx.people.get(a).cloned(), ctx.people.get(b).cloned()) else { continue };
        let married = r.kind.as_deref() == Some("marriage") && !batch.events.iter().any(|e| e.kind.as_deref() == Some("marriage") && e.people.iter().any(|p| p.p == a) && e.people.iter().any(|p| p.p == b));
        link(&mut changes, "partner", &xa, &xb, None, None, &json!({ "married": married }))?;
    }
    let mut parents_of: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();
    for r in batch.relationships.iter().filter(|r| r.relation.as_deref() == Some("parent")) {
        let (Some(parent), Some(child)) = (r.parent.as_deref(), r.child.as_deref()) else { continue };
        let female = batch.persons.iter().find(|p| p.id == parent).and_then(|p| p.sex.as_deref()) == Some("F");
        if skip_relative(&ctx, child, if female { "mother" } else { "father" }) {
            continue;
        }
        let (Some(xp), Some(_)) = (ctx.people.get(parent).cloned(), ctx.people.get(child)) else { continue };
        let pedi = match r.kind.as_deref() {
            Some("adopted") => Some("adopted".to_string()),
            Some("foster") => Some("foster".to_string()),
            Some("step") | Some("unknown") => Some(r.kind.clone().unwrap_or_default()),
            _ => None,
        };
        parents_of.entry(child.to_string()).or_default().push((xp, pedi));
    }
    for (child, parents) in parents_of {
        let xc = ctx.people[&child].clone();
        // Both parents known and already a couple: the child joins that family.
        if let [(p1, pedi), (p2, _)] = parents.as_slice() {
            let fams = |x: &str, changes: &Changes| -> Vec<String> {
                changes.peek(x).map(|n| n.children.iter().filter(|c| c.tag == "FAMS").filter_map(|c| c.pointer().map(str::to_string)).collect()).unwrap_or_default()
            };
            let shared = fams(p1, &changes).into_iter().find(|f| fams(p2, &changes).contains(f));
            if let Some(fam) = shared {
                add_child_to_family(&mut changes, &fam, &xc, pedi.as_deref())?;
                continue;
            }
        }
        for (parent, pedi) in &parents {
            link(&mut changes, "parent", &xc, parent, None, pedi.as_deref(), &Value::Null)?;
        }
    }
    for r in batch.relationships.iter().filter(|r| r.relation.as_deref() == Some("sibling")) {
        let (Some(a), Some(b)) = (r.a.as_deref().and_then(|a| ctx.people.get(a)).cloned(), r.b.as_deref().and_then(|b| ctx.people.get(b)).cloned()) else { continue };
        link(&mut changes, "sibling", &a, &b, None, None, &Value::Null)?;
    }

    // 5. Events become facts on the principal (family events on the couple's family).
    let mut facts_written = 0;
    for e in &batch.events {
        let tag = check::import_tag(e.kind.as_deref());
        let principals: Vec<&str> = {
            let explicit: Vec<&str> = e.people.iter().filter(|r| r.role.as_deref() == Some("principal")).map(|r| r.p.as_str()).collect();
            if !explicit.is_empty() {
                explicit
            } else if matches!(tag.as_str(), "MARR" | "MARB" | "DIV") {
                e.people.iter().filter(|r| r.role.as_deref().is_none_or(|x| x == "spouse")).map(|r| r.p.as_str()).collect()
            } else {
                e.people.first().map(|r| vec![r.p.as_str()]).unwrap_or_default()
            }
        };
        let mut fact = Node::new(&tag);
        if matches!(tag.as_str(), "OCCU" | "RESI" | "EDUC" | "RELI") {
            if let Some(v) = e.value.as_deref().filter(|v| !v.trim().is_empty()) {
                fact.value = Some(v.trim().to_string());
            }
        }
        if tag == "EVEN" {
            fact.children.push(Node::with_value("TYPE", e.type_other.as_deref().or(e.kind.as_deref()).unwrap_or("inne")));
        }
        if tag == "_MILT" {
            if let Some(v) = &e.value {
                fact.value = Some(v.clone());
            }
        }
        if let Some(date) = date_node(e.date.as_deref(), e.date_orig.as_deref(), e.julian.as_deref()) {
            fact.children.push(date);
        }
        if let Some(place) = e.place.as_deref().filter(|p| !p.trim().is_empty()) {
            let mut plac = Node::with_value("PLAC", place.trim());
            if let Some(n) = &e.place_note {
                plac.children.push(Node::with_value("NOTE", n));
            }
            if let Some(o) = &e.place_orig {
                plac.children.push(Node::with_value("_HLM_ORIG", o));
            }
            fact.children.push(plac);
        }
        if let Some(c) = &e.cause {
            fact.children.push(Node::with_value("CAUS", c));
        }
        if let Some(n) = &e.note {
            fact.children.push(text_node("NOTE", n));
        }
        for role in e.people.iter().filter(|r| !principals.contains(&r.p.as_str())) {
            if let Some(x) = ctx.people.get(&role.p) {
                let mut asso = Node::with_value("ASSO", x).with_child(Node::with_value("ROLE", role_code(role.role.as_deref().unwrap_or("other"))));
                if let Some(age) = &role.age_orig {
                    asso.children.push(Node::with_value("_HLM_AGE", age));
                }
                fact.children.push(asso);
            }
        }
        if let Some(age) = e.people.iter().find(|r| principals.contains(&r.p.as_str())).and_then(|r| r.age_orig.clone()) {
            fact.children.push(Node::with_value("_HLM_AGE", &age));
        }
        fact.children.extend(ctx.citations(&e.src));
        cert(&mut fact, e.certainty.as_deref(), e.basis.as_deref());

        if matches!(tag.as_str(), "MARR" | "MARB" | "DIV") {
            let xs: Vec<String> = principals.iter().filter_map(|p| ctx.people.get(*p).cloned()).collect();
            let Some(first) = xs.first().cloned() else { continue };
            let fam = match xs.get(1) {
                Some(second) => {
                    link(&mut changes, "partner", &first, second, None, None, &Value::Null)?;
                    let fams = |x: &str, changes: &Changes| -> Vec<String> {
                        changes.peek(x).map(|n| n.children.iter().filter(|c| c.tag == "FAMS").filter_map(|c| c.pointer().map(str::to_string)).collect()).unwrap_or_default()
                    };
                    fams(&first, &changes).into_iter().find(|f| fams(second, &changes).contains(f))
                }
                None => None,
            };
            if let Some(fam) = fam {
                let family = changes.get(&fam)?;
                // A bare "MARR Y" from the relationship is replaced by the real event.
                family.children.retain(|c| !(c.tag == tag && c.value.as_deref() == Some("Y") && c.children.is_empty()));
                // The same event already written (the couple came in an earlier batch): only the new citations.
                match family.children.iter().position(|c| c.tag == tag && same_fact(c, &fact)) {
                    Some(pos) => add_citations(&mut family.children[pos], &fact),
                    None => family.children.push(fact),
                }
                facts_written += 1;
            }
            continue;
        }
        for pid in principals {
            let Some(xref) = ctx.people.get(pid).cloned() else { continue };
            let field = match tag.as_str() {
                "BIRT" => "birth",
                "BAPM" => "baptism",
                "DEAT" => "death",
                "BURI" => "burial",
                "OCCU" => "occupation",
                _ => "other",
            };
            let merged = ctx.decision(pid).is_some_and(|d| d.kind == Kind::Merge);
            let choice = if merged { ctx.choice(pid, field) } else { "add".into() };
            if choice == "skip" || choice == "keep" {
                continue;
            }
            let indi = changes.get(&xref)?;
            let existing = indi.children.iter().position(|c| c.tag == tag || (tag == "BAPM" && c.tag == "CHR"));
            match (choice.as_str(), existing) {
                ("replace", Some(pos)) => {
                    let old = &mut indi.children[pos];
                    old.children.retain(|c| c.tag != "DATE" && c.tag != "PLAC");
                    let at = 0;
                    for (k, child) in fact.children.iter().filter(|c| c.tag == "DATE" || c.tag == "PLAC").enumerate() {
                        old.children.insert(at + k, child.clone());
                    }
                    for c in fact.children.iter().filter(|c| c.tag == "SOUR") {
                        old.children.push(c.clone());
                    }
                }
                _ => {
                    // The same fact, whichever of its kind it is (a second occupation, a later residence): only the
                    // new citations are added.
                    match indi.children.iter().position(|c| (c.tag == tag || (tag == "BAPM" && c.tag == "CHR")) && same_fact(c, &fact)) {
                        Some(pos) => add_citations(&mut indi.children[pos], &fact),
                        None => {
                            let at = indi.children.iter().rposition(|c| matches!(c.tag.as_str(), "NAME" | "SEX" | "BIRT" | "BAPM" | "CHR" | "DEAT" | "BURI" | "OCCU" | "RESI" | "EDUC" | "RELI" | "EVEN" | "EMIG" | "_MILT")).map_or(indi.children.len(), |i| i + 1);
                            indi.children.insert(at, fact.clone());
                        }
                    }
                }
            }
            facts_written += 1;
        }
    }
    // Ages written in records: an estimated birth for new people who have no birth date.
    for e in &batch.events {
        let Some(year) = e.date.as_deref().and_then(format::gedcom_date).and_then(|g| g.split_whitespace().last().and_then(|y| y.parse::<i32>().ok())) else { continue };
        for role in e.people.iter().filter(|r| r.role.as_deref() != Some("principal")) {
            let Some(age) = role.age_orig.as_deref().and_then(|a| super::age_at(a, year)) else { continue };
            if ctx.decision(&role.p).is_none_or(|d| d.kind != Kind::New) {
                continue;
            }
            let Some(xref) = ctx.people.get(&role.p).cloned() else { continue };
            let indi = changes.get(&xref)?;
            if indi.children.iter().any(|c| matches!(c.tag.as_str(), "BIRT" | "BAPM" | "CHR")) {
                continue;
            }
            let birth = Node::new("BIRT")
                .with_child(Node::with_value("DATE", &format!("EST {}", year - age)))
                .with_child(Node::with_value("NOTE", &format!("Wyliczone z wieku „{}” w zdarzeniu {}.", role.age_orig.clone().unwrap_or_default(), e.id)))
                .with_child(Node::with_value("_HLM_BASIS", "inferred"));
            let at = indi.children.iter().rposition(|c| matches!(c.tag.as_str(), "NAME" | "SEX")).map_or(indi.children.len(), |i| i + 1);
            indi.children.insert(at, birth);
        }
    }
    // "Zmarły" without a death event.
    for p in &batch.persons {
        if p.living == Some(false) {
            if let Some(xref) = ctx.people.get(&p.id).cloned() {
                let indi = changes.get(&xref)?;
                if !indi.children.iter().any(|c| matches!(c.tag.as_str(), "DEAT" | "BURI" | "CREM")) {
                    indi.children.push(Node::with_value("DEAT", "Y"));
                }
            }
        }
    }

    // 6. Texts (mentions of batch people point to their UIDs) and the summaries.
    let mention = |md: &str, ctx: &Context| -> String {
        let mut out = String::new();
        let mut last = 0;
        for (start, end, visible, id) in check::mention_links(md) {
            out.push_str(&md[last..start]);
            match ctx.uids.get(&id) {
                Some(uid) => out.push_str(&format!("[{visible}](person:{uid})")),
                None => out.push_str(&visible),
            }
            last = end;
        }
        out.push_str(&md[last..]);
        out
    };
    let mut texts_written = 0;
    let mut add_text = |changes: &mut Changes, ctx: &Context, owner: &str, kind: &str, title: Option<&str>, body: &str, date: Option<&str>, place: Option<&str>, src: &[String], certainty: Option<&str>, basis: Option<&str>| -> Result<(), ApiError> {
        let Some(xref) = ctx.people.get(owner).cloned() else { return Ok(()) };
        let note_xref = changes.new_xref("N");
        let mut note = text_node("SNOTE", &mention(body, ctx));
        note.xref = Some(note_xref.clone());
        note.children.push(Node::with_value("_HLM_KIND", kind));
        if let Some(t) = title.filter(|t| !t.trim().is_empty()) {
            note.children.push(Node::with_value("_HLM_TITLE", t.trim()));
        }
        if let Some(d) = date.filter(|d| !d.trim().is_empty()) {
            note.children.push(Node::with_value("_HLM_DATE", &format::gedcom_date(d).unwrap_or_else(|| d.trim().to_string())));
        }
        if let Some(p) = place.filter(|p| !p.trim().is_empty()) {
            note.children.push(Node::with_value("_HLM_PLAC", p.trim()));
        }
        note.children.extend(ctx.citations(src));
        cert(&mut note, certainty, basis);
        note.children.push(stamp("CREA"));
        batch_mark(&mut note, &ctx.name);
        changes.add(note);
        let indi = changes.get(&xref)?;
        let at = indi.children.iter().rposition(|c| c.tag == "SNOTE").map_or_else(|| indi.children.iter().rposition(|c| c.tag == "FAMS" || c.tag == "FAMC").map_or(indi.children.len(), |i| i + 1), |i| i + 1);
        indi.children.insert(at, Node::with_value("SNOTE", &note_xref));
        texts_written += 1;
        Ok(())
    };
    for p in &batch.persons {
        if let Some(summary) = p.summary.as_deref().filter(|s| !s.trim().is_empty()) {
            let has_summary = ctx.people.get(&p.id).and_then(|x| derived.index(x)).is_some_and(|i| derived.view.person_texts(&derived.view.people[i]).any(|t| t.kind == heirloom_core::gedcom::view::TextKind::Summary));
            if !has_summary {
                add_text(&mut changes, &ctx, &p.id, "summary", None, summary, None, None, &[], None, None)?;
            }
        }
    }
    for t in &batch.texts {
        let (Some(owner), Some(body)) = (t.person.as_deref(), t.md.as_deref()) else { continue };
        let kind = match t.kind.as_deref() {
            Some(k @ ("bio" | "story" | "saying" | "trivia" | "note")) => k,
            _ => "note",
        };
        add_text(&mut changes, &ctx, owner, kind, t.title.as_deref(), body, t.date.as_deref(), t.place.as_deref(), &t.src, t.certainty.as_deref(), t.basis.as_deref())?;
    }
    // Answers to the AI's questions become research notes at the people they are about.
    for q in &batch.questions {
        let Some(text) = q.text.as_deref() else { continue };
        let (answer, answer_note) = ctx.draft.answers.get(&q.id).cloned().unwrap_or_default();
        let answer_word = match answer.as_str() {
            "yes" => "tak",
            "no" => "nie",
            "unknown" => "nie wiem",
            _ => "bez odpowiedzi",
        };
        let mut body = format!("Pytanie z importu „{}”: {text}\nOdpowiedź: {answer_word}.", ctx.name);
        if !answer_note.trim().is_empty() {
            body.push_str(&format!(" {}", answer_note.trim()));
        }
        if let Some(first) = q.about.first() {
            add_text(&mut changes, &ctx, first, "note", None, &body, None, None, &[], None, None)?;
        }
    }
    // Links.
    for l in &batch.links {
        let (Some(pid), Some(url)) = (l.person.as_deref(), l.url.as_deref()) else { continue };
        let Some(xref) = ctx.people.get(pid).cloned() else { continue };
        let indi = changes.get(&xref)?;
        if indi.children.iter().any(|c| c.tag == "_HLM_LINK" && c.value.as_deref() == Some(url)) {
            continue;
        }
        let mut node = Node::with_value("_HLM_LINK", url);
        if let Some(t) = &l.title {
            node.children.push(Node::with_value("TITL", t));
        }
        if let Some(k) = &l.kind {
            node.children.push(Node::with_value("TYPE", k));
        }
        indi.children.push(node);
    }
    // Name citations for new people.
    for p in &batch.persons {
        let Some(xref) = ctx.people.get(&p.id).cloned() else { continue };
        if ctx.decision(&p.id).is_none_or(|d| d.kind != Kind::New) {
            continue;
        }
        let cites: Vec<Vec<Node>> = p.names.iter().map(|n| ctx.citations(&n.src)).collect();
        let indi = changes.get(&xref)?;
        let mut k = 0;
        for child in indi.children.iter_mut().filter(|c| c.tag == "NAME") {
            if let Some(list) = cites.get(k) {
                child.children.extend(list.iter().cloned());
            }
            k += 1;
        }
    }

    let edits = changes.into_edits();
    let applied = !edits.is_empty();
    s.archive.apply_all(edits)?;
    s.changed();
    let report = match s.archive.save(&SaveOptions { author, batch: Some(&name), note, allow_foreign }) {
        Ok(report) => report,
        Err(e) => {
            // Not written: the import must not stay in memory, or committing again would add it a second time.
            if applied {
                s.archive.discard_last();
                s.changed();
            }
            return Err(e.into());
        }
    };
    s.last_saved = Some(heirloom_core::history::now());
    Ok(json!({
        "batch": name,
        "people": new_people,
        "merged": merged_people,
        "facts": facts_written,
        "texts": texts_written,
        "files": copied.len(),
        "changes": report.history_entries,
        "message": format!("Zaimportowano {}", crate::count_pl(new_people + merged_people, "osobę", "osoby", "osób")),
    }))
}

/// A fact already written with the same date, place and value (the occupation, the residence).
fn same_fact(old: &Node, new: &Node) -> bool {
    old.child_value("DATE") == new.child_value("DATE") && old.child_value("PLAC") == new.child_value("PLAC") && old.value == new.value
}

/// Adds the citations of `new` that `old` doesn't have yet.
fn add_citations(old: &mut Node, new: &Node) {
    for c in new.children.iter().filter(|c| c.tag == "SOUR") {
        if !has_pointer(old, "SOUR", c.value.as_deref().unwrap_or("")) {
            old.children.push(c.clone());
        }
    }
}

/// Keeps an AI answer in the batch's folder without replacing an earlier file: a batch name can come back in a later
/// import. The same text is kept once; a different one gets the next free name ("odpowiedz-1 (2).txt").
fn keep_answer(folder: &std::path::Path, n: usize, text: &str) {
    for k in 1..1000 {
        let name = if k == 1 { format!("odpowiedz-{n}.txt") } else { format!("odpowiedz-{n} ({k}).txt") };
        let path = folder.join(name);
        if path.exists() {
            if std::fs::read(&path).is_ok_and(|old| old == text.as_bytes()) {
                return;
            }
            continue;
        }
        // `create_new`: never over a file that appeared meanwhile.
        let _ = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
        return;
    }
}

/// A folder name from a batch name.
fn sanitize(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' }).collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() { "import".into() } else { trimmed }
}


#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::Archive;

    #[test]
    fn the_documented_example_imports_and_can_be_undone() {
        let dir = tempfile::tempdir().unwrap();
        let archive = Archive::create(&dir.path().join("a"), "A").unwrap();
        let mut s = Session::new(archive);
        // The researcher's files: M002 (a photo) is dropped with the answer.
        let photo = dir.path().join("M002 slub.jpg");
        image::RgbImage::from_pixel(4, 3, image::Rgb([9, 9, 9])).save(&photo).unwrap();
        let text = include_str!("../../../../docs/IMPORT_FORMAT.md");
        let json = check::extract_blocks(text, "doc").into_iter().next().unwrap().text;
        let answer = format!("```json\n{json}\n```");
        let mut draft = None;
        let state = super::super::call(&mut s, &mut draft, "import.load", &json!({ "texts": [answer], "paths": [photo.to_string_lossy()] })).unwrap();
        assert_eq!(state["result"], "warnings", "{}", state["issues"]);
        assert_eq!(state["persons"].as_array().unwrap().len(), 4);
        assert!(state["persons"].as_array().unwrap().iter().all(|p| p["decision"] == "new"), "an empty archive has no matches");
        super::super::call(&mut s, &mut draft, "import.answer", &json!({ "question": "Q1", "answer": "unknown", "note": "sprawdzić w Ciechankach" })).unwrap();
        let result = super::super::call(&mut s, &mut draft, "import.commit", &json!({ "author": "Ewa", "note": "akty z Łęcznej" })).unwrap();
        assert_eq!(result["people"], 4);
        assert!(draft.is_none());

        let d = s.derived();
        let jozef = (0..d.info.len()).find(|&i| d.info[i].name == "Józef Kowalski").unwrap();
        assert_eq!(d.info[jozef].birth.as_ref().unwrap().text, "12 marca 1878");
        assert_eq!(d.info[jozef].birth_place.as_deref(), Some("Wólka"));
        assert_eq!(d.info[jozef].nickname.as_deref(), Some("Dziadek Józek"));
        let parents: Vec<&str> = d.info[jozef].parents.iter().map(|&p| d.info[p].name.as_str()).collect();
        assert!(parents.contains(&"Antoni Kowalski") && parents.contains(&"Agnieszka Kowalska"), "{parents:?}");
        let wife = d.info[jozef].partners[0];
        assert_eq!((d.info[wife].name.as_str(), d.info[wife].maiden.as_deref()), ("Marianna Kowalska", Some("Nowak")));
        assert_eq!(crate::kin::marriage(d, jozef, wife), (true, Some("1904".into())));
        assert_eq!(d.info[jozef].photo.as_ref().map(|p| p.1.as_str()), Some("media/M002 slub.jpg"), "the AI's profile photo suggestion");
        let profile = crate::people::profile(d, d.xref(jozef), &[]).unwrap();
        assert_eq!(profile["bio"][0]["title"], "Dzieciństwo");
        assert!(profile["bio"][0]["body"].as_str().unwrap().contains("(person:@I"), "mentions point to the new records");
        assert_eq!(profile["sayings"].as_array().unwrap().len(), 1);
        assert!(profile["facts"].as_array().unwrap().iter().any(|f| f["key"] == "Zawód"));
        assert!(profile["sources"].as_array().unwrap().iter().any(|s| s["title"] == "Akt urodzenia nr 45/1878, parafia rzymskokatolicka Łęczna"));
        let marianna_notes = crate::people::profile(d, d.xref(wife), &[]).unwrap();
        assert!(marianna_notes["notes"][0]["body"].as_str().unwrap().contains("Odpowiedź: nie wiem. sprawdzić w Ciechankach"));
        assert!(dir.path().join("a").join("zrodla-ai").join("Kowalscy-Leczna-2026-10-01").join("odpowiedz-1.txt").is_file());

        // The whole import comes back out with one undo from the history.
        let history = s.history();
        assert!(history.iter().all(|e| e.batch.as_deref() == Some("Kowalscy-Leczna-2026-10-01")));
        assert_eq!(history[0].note.as_deref(), Some("akty z Łęcznej"));
    }

    /// The documented example answer, with its photo M002 dropped next to it.
    fn example(dir: &std::path::Path) -> (String, std::path::PathBuf) {
        let photo = dir.join("M002 slub.jpg");
        image::RgbImage::from_pixel(4, 3, image::Rgb([9, 9, 9])).save(&photo).unwrap();
        let json = check::extract_blocks(include_str!("../../../../docs/IMPORT_FORMAT.md"), "doc").into_iter().next().unwrap().text;
        (format!("```json\n{json}\n```"), photo)
    }

    fn call(s: &mut Session, draft: &mut Option<Draft>, method: &str, args: Value) -> Result<Value, ApiError> {
        super::super::call(s, draft, method, &args)
    }

    fn load(s: &mut Session, draft: &mut Option<Draft>, answer: &str, photo: &std::path::Path) -> Value {
        call(s, draft, "import.load", json!({ "texts": [answer], "paths": [photo.to_string_lossy()] })).unwrap()
    }

    fn xref_of(s: &mut Session, given: &str) -> String {
        let d = s.derived();
        d.xref((0..d.info.len()).find(|&i| d.info[i].given == given).unwrap()).to_string()
    }

    #[test]
    fn an_import_into_another_programs_file_is_confirmed_before_anything_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("drzewo.ged"), "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 7.0\n0 TRLR\n").unwrap();
        let mut s = Session::new(Archive::open(&root).unwrap());
        let (answer, photo) = example(dir.path());
        let mut draft = None;
        load(&mut s, &mut draft, &answer, &photo);
        assert_eq!(call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap_err().code, "foreign");
        assert!(!root.join("media").exists() && !root.join("zrodla-ai").exists(), "no file copied before the answer");
        assert!(!s.archive.has_unsaved_changes() && !s.archive.can_undo() && draft.is_some());
        let result = call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa", "allowForeign": true })).unwrap();
        assert_eq!(result["people"], 4);
        assert_eq!(std::fs::read_dir(s.archive.backup_dir()).unwrap().count(), 1, "the other program's original is kept");
    }

    #[cfg(windows)]
    #[test]
    fn an_import_that_cant_be_saved_does_not_stay_in_memory() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new(Archive::create(&dir.path().join("a"), "A").unwrap());
        let (answer, photo) = example(dir.path());
        let mut draft = None;
        load(&mut s, &mut draft, &answer, &photo);
        let records = s.archive.doc.records.clone();
        // The data file is open in another program that doesn't share it, so it can't be replaced.
        let held = std::fs::File::options().read(true).share_mode(1).open(s.archive.data_path()).unwrap();
        assert!(call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).is_err());
        assert_eq!(s.archive.doc.records, records, "nothing of the import is left unsaved");
        assert!(!s.archive.can_redo() && draft.is_some());
        drop(held);
        call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(s.archive.doc.records.iter().filter(|r| r.tag == "INDI").count(), 4, "imported once, not twice");
    }

    #[test]
    fn the_answers_of_an_earlier_import_with_the_same_name_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let mut s = Session::new(Archive::create(&root, "A").unwrap());
        // An answer kept by an attempt that was never saved, so the history doesn't know the name.
        let folder = root.join("zrodla-ai").join("Kowalscy-Leczna-2026-10-01");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("odpowiedz-1.txt"), "wcześniejsza odpowiedź").unwrap();
        let (answer, photo) = example(dir.path());
        let mut draft = None;
        load(&mut s, &mut draft, &answer, &photo);
        call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(std::fs::read_to_string(folder.join("odpowiedz-1.txt")).unwrap(), "wcześniejsza odpowiedź", "not replaced");
        assert_eq!(std::fs::read_to_string(folder.join("odpowiedz-1 (2).txt")).unwrap(), answer);
        // A corrected answer under the same batch name (its people are skipped this time) is a batch of its own.
        let second = format!("{answer}\nPoprawiona odpowiedź.");
        let state = load(&mut s, &mut draft, &second, &photo);
        for p in state["persons"].as_array().unwrap() {
            call(&mut s, &mut draft, "import.decide", json!({ "person": p["id"], "kind": "skip" })).unwrap();
        }
        let result = call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        assert_eq!(result["batch"], "Kowalscy-Leczna-2026-10-01 (2)");
        let own = root.join("zrodla-ai").join("Kowalscy-Leczna-2026-10-01 (2)").join("odpowiedz-1.txt");
        assert_eq!(std::fs::read_to_string(own).unwrap(), second);
    }

    #[test]
    fn importing_the_same_facts_into_merged_people_adds_no_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new(Archive::create(&dir.path().join("a"), "A").unwrap());
        let (answer, photo) = example(dir.path());
        let mut draft = None;
        load(&mut s, &mut draft, &answer, &photo);
        call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        // Józef has another occupation written before the imported one.
        let jozef = xref_of(&mut s, "Józef");
        let mut record = s.archive.doc.record(&jozef).unwrap().clone();
        let at = record.children.iter().position(|c| c.tag == "OCCU").unwrap();
        record.children.insert(at, Node::with_value("OCCU", "rolnik"));
        s.archive.apply(heirloom_core::Edit::Replace(record)).unwrap();
        s.changed();
        let state = load(&mut s, &mut draft, &answer, &photo);
        for p in state["persons"].as_array().unwrap() {
            let target = xref_of(&mut s, p["name"].as_str().unwrap().split(' ').next().unwrap());
            call(&mut s, &mut draft, "import.decide", json!({ "person": p["id"], "kind": "merge", "target": target })).unwrap();
        }
        call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        let record = s.archive.doc.record(&jozef).unwrap();
        let occupations: Vec<&str> = record.children_tagged("OCCU").filter_map(|c| c.value.as_deref()).collect();
        assert_eq!(occupations, ["rolnik", "kolejarz na stacji Lublin"]);
        assert_eq!(record.children_tagged("BIRT").count(), 1);
        let family = record.children_tagged("FAMS").find_map(|c| c.pointer()).unwrap();
        assert_eq!(s.archive.doc.record(family).unwrap().children_tagged("MARR").count(), 1, "the wedding once");
    }

    #[test]
    fn an_age_that_is_not_an_age_gives_no_birth_date() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new(Archive::create(&dir.path().join("a"), "A").unwrap());
        let (answer, photo) = example(dir.path());
        // The father's age written as a year by mistake: born "EST 0" is not a date.
        let answer = answer.replace("\"age_orig\": \"30 лѣтъ\"", "\"age_orig\": \"1878\"");
        let mut draft = None;
        let state = load(&mut s, &mut draft, &answer, &photo);
        assert!(!state["issues"].to_string().contains("ok. 0"), "{}", state["issues"]);
        call(&mut s, &mut draft, "import.commit", json!({ "author": "Ewa" })).unwrap();
        let antoni = xref_of(&mut s, "Antoni");
        assert!(s.archive.doc.record(&antoni).unwrap().child("BIRT").is_none());
    }

    #[test]
    fn a_file_already_in_the_archive_is_found_past_an_object_without_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let (answer, photo) = example(dir.path());
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::copy(&photo, root.join("media").join("slub.jpg")).unwrap();
        let text = "0 HEAD\n1 SOUR HEIRLOOM\n1 GEDC\n2 VERS 7.0\n0 @O1@ OBJE\n1 _HLM_TITLE bez pliku\n\
0 @O2@ OBJE\n1 FILE media/slub.jpg\n2 FORM image/jpeg\n0 TRLR\n";
        std::fs::write(root.join("rodzina.ged"), text).unwrap();
        let mut s = Session::new(Archive::open(&root).unwrap());
        let mut draft = None;
        let state = load(&mut s, &mut draft, &answer, &photo);
        assert_eq!(state["summary"]["duplicates"], 1, "M002 is the photo @O2@ already shows");
    }

    #[test]
    fn candidates_still_point_at_the_right_people_after_the_archive_changes() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = Session::new(Archive::create(&dir.path().join("a"), "A").unwrap());
        let other = crate::edit::call(&mut s, "person.create", &json!({ "given": "Ewa", "surname": "Zielińska" })).unwrap()["id"].as_str().unwrap().to_string();
        let jozef = crate::edit::call(&mut s, "person.create", &json!({ "given": "Józef", "surname": "Kowalski", "sex": "M", "events": { "birth": { "date": "12.03.1878", "place": "Wólka" } } })).unwrap()["id"].as_str().unwrap().to_string();
        let (answer, photo) = example(dir.path());
        let mut draft = None;
        load(&mut s, &mut draft, &answer, &photo);
        // Someone listed before him is deleted while the import waits: the people after her move up.
        crate::edit::call(&mut s, "person.delete", &json!({ "id": other })).unwrap();
        let state = call(&mut s, &mut draft, "import.state", Value::Null).unwrap();
        let p1 = state["persons"].as_array().unwrap().iter().find(|p| p["id"] == "P1").unwrap();
        assert_eq!(p1["candidates"][0]["id"], jozef.as_str());
    }
}
