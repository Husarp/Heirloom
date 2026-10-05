//! Changing the archive: people, names, dates and places, relations, texts. Every command becomes whole-record
//! GEDCOM changes applied as one undo step; nothing is written to disk until Save (PLAN §11.2 rule 3).

use crate::gedwrite::{
    declared_head, has_pointer, insert_after, name_value, new_uid, next_xref, remove_children, remove_pointer, set_child_text, stamp,
    text_node, touch,
};
use crate::{ApiError, ApiResult, Session};
use heirloom_core::gedcom::view::{FAM_EVENTS, INDI_EVENTS, TextKind};
use heirloom_core::gedcom::{Document, Node, Version, model};
use heirloom_core::{Edit, polish};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Collects the changed copies of records during one command, then turns them into [`Edit`]s.
pub struct Changes<'a> {
    doc: &'a Document,
    modified: Vec<Node>,
    added: Vec<Node>,
    removed: Vec<String>,
}

impl<'a> Changes<'a> {
    pub fn new(doc: &'a Document) -> Changes<'a> {
        Changes { doc, modified: Vec::new(), added: Vec::new(), removed: Vec::new() }
    }

    pub fn exists(&self, xref: &str) -> bool {
        !self.removed.iter().any(|r| r == xref)
            && (self.added.iter().any(|n| n.xref.as_deref() == Some(xref)) || self.doc.record(xref).is_some())
    }

    /// The record to change (a copy, made on first use).
    pub fn get(&mut self, xref: &str) -> Result<&mut Node, ApiError> {
        if let Some(i) = self.added.iter().position(|n| n.xref.as_deref() == Some(xref)) {
            return Ok(&mut self.added[i]);
        }
        if let Some(i) = self.modified.iter().position(|n| n.xref.as_deref() == Some(xref)) {
            return Ok(&mut self.modified[i]);
        }
        let record = self.doc.record(xref).ok_or_else(|| ApiError::new("no_record", format!("Nie ma rekordu {xref}.")))?;
        self.modified.push(record.clone());
        Ok(self.modified.last_mut().expect("just pushed"))
    }

    /// The record as it is now in this command (changed copy or the original).
    pub fn peek(&self, xref: &str) -> Option<&Node> {
        self.added
            .iter()
            .chain(&self.modified)
            .find(|n| n.xref.as_deref() == Some(xref))
            .or_else(|| self.doc.record(xref))
    }

    pub fn new_xref(&self, prefix: &str) -> String {
        let taken: Vec<String> = self.added.iter().filter_map(|n| n.xref.clone()).collect();
        next_xref(self.doc, prefix, &taken)
    }

    pub fn add(&mut self, node: Node) {
        self.added.push(node);
    }

    pub fn remove(&mut self, xref: &str) {
        self.added.retain(|n| n.xref.as_deref() != Some(xref));
        self.modified.retain(|n| n.xref.as_deref() != Some(xref));
        if self.doc.record(xref).is_some() && !self.removed.iter().any(|r| r == xref) {
            self.removed.push(xref.to_string());
        }
    }

    /// The edits, with CHAN stamps on everything changed and the extension tags declared in HEAD — only when
    /// something really changed, so saving an untouched form leaves the file as it was.
    pub fn into_edits(self) -> Vec<Edit> {
        let mut edits = Vec::new();
        for mut node in self.modified {
            let unchanged = self.doc.record(node.xref.as_deref().unwrap_or("")).is_some_and(|original| *original == node);
            if unchanged {
                continue;
            }
            touch(&mut node);
            edits.push(Edit::Replace(node));
        }
        for node in self.added {
            edits.push(Edit::Add(node));
        }
        for xref in self.removed {
            edits.push(Edit::Remove(xref));
        }
        if !edits.is_empty() {
            if let Some(head) = declared_head(self.doc) {
                edits.insert(0, Edit::Replace(head));
            }
        }
        edits
    }
}

fn arg_str<'v>(args: &'v Value, key: &str) -> Option<&'v str> {
    args.get(key).and_then(Value::as_str)
}

fn req(args: &Value, key: &str) -> Result<String, ApiError> {
    arg_str(args, key).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

/// Applies the collected changes as one undo step.
fn commit(s: &mut Session, edits: Vec<Edit>) -> Result<(), ApiError> {
    if edits.is_empty() {
        return Ok(());
    }
    s.archive.apply_all(edits)?;
    s.changed();
    Ok(())
}

/// Before the first change: read-only archives refuse, and 5.5.1 files become GEDCOM 7.
fn prepare(s: &mut Session) -> Result<(), ApiError> {
    if s.archive.settings().read_only {
        return Err(ApiError::new("read_only", "To archiwum jest tylko do odczytu (Ustawienia › Archiwum)."));
    }
    if s.archive.upgrade_to_v7() {
        s.changed();
    }
    Ok(())
}

pub fn call(s: &mut Session, method: &str, args: &Value) -> ApiResult {
    // Only reads: it says what „Odłącz” would change, before anything is changed.
    if method == "relation.unlinkPreview" {
        return unlink_preview(s, args);
    }
    if method == "source.detachPreview" {
        return crate::media_edit::detach_preview(s, args);
    }
    prepare(s)?;
    match method {
        "person.create" => create_person(s, args),
        "person.update" => update_person(s, args),
        "person.delete" => delete_person(s, args),
        "person.removeBrokenLink" => remove_broken_link(s, args),
        "relation.add" => add_relation(s, args),
        "relation.remove" => remove_relation(s, args),
        "relation.change" => change_relation(s, args),
        "relation.setPedigree" => set_pedigree(s, args),
        "relation.associate" => associate(s, args),
        "relation.dissociate" => dissociate(s, args),
        "family.update" => update_family(s, args),
        "fact.update" => update_fact(s, args),
        "fact.add" => add_fact(s, args),
        "fact.delete" => delete_fact(s, args),
        "text.save" => save_text(s, args),
        "text.delete" => delete_text(s, args),
        "text.move" => move_text(s, args),
        "place.rename" => rename_place(s, args),
        _ => crate::media_edit::call(s, method, args),
    }
}

/// Renames a place everywhere it is written, or merges it into another ("Wulka" → "Wólka"). `from` is the path
/// from the largest part down (as the place list shows it); sub-places move along.
fn rename_place(s: &mut Session, args: &Value) -> ApiResult {
    let from: Vec<String> = args.get("from").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let to = req(args, "to")?.trim().to_string();
    if from.is_empty() || to.is_empty() {
        return Err(ApiError::bad_args("from/to"));
    }
    fn rewrite(node: &mut Node, from: &[String], to: &str) -> bool {
        let mut changed = false;
        if node.tag == "PLAC" {
            if let Some(value) = &node.value {
                let mut path: Vec<String> = value.split(',').map(|p| p.trim().to_string()).collect();
                path.reverse();
                if path.len() >= from.len() && path[..from.len()] == *from {
                    path[from.len() - 1] = to.to_string();
                    path.reverse();
                    node.value = Some(path.join(", "));
                    changed = true;
                }
            }
        }
        for child in &mut node.children {
            changed |= rewrite(child, from, to);
        }
        changed
    }
    let mut changes = Changes::new(&s.archive.doc);
    let targets: Vec<String> = s.archive.doc.records.iter().filter(|r| r.xref.is_some()).filter(|r| {
        let mut copy = (*r).clone();
        rewrite(&mut copy, &from, &to)
    }).filter_map(|r| r.xref.clone()).collect();
    for xref in &targets {
        rewrite(changes.get(xref)?, &from, &to);
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "changed": targets.len() }))
}

fn sex_of(node: &Node) -> &str {
    node.child_value("SEX").map(str::trim).unwrap_or("U")
}

/// The NAME nodes of a person: (index of the birth name, index of the married name).
fn name_slots(indi: &Node) -> (Option<usize>, Option<usize>) {
    let mut birth = None;
    let mut married = None;
    let mut first = None;
    for (i, c) in indi.children.iter().enumerate().filter(|(_, c)| c.tag == "NAME") {
        first.get_or_insert(i);
        match c.child_value("TYPE").map(|t| t.trim().to_ascii_uppercase()) {
            Some(t) if t == "BIRTH" || t == "MAIDEN" => birth = birth.or(Some(i)),
            Some(t) if t == "MARRIED" => married = married.or(Some(i)),
            None if Some(i) == first => birth = birth.or(Some(i)),
            _ => {}
        }
    }
    (birth.or(first.filter(|f| Some(*f) != married)), married)
}

fn set_name(name: &mut Node, given: &str, surname: &str) {
    let clean = |s: &str| s.replace('/', "").trim().to_string();
    // Unchanged, it stays as written ("/Kovács/ János", GIVN and SURN).
    if model::name_parts(name, Version::V7) == (clean(given), clean(surname)) {
        return;
    }
    // A suffix after the surname ("Jan /Kowalski/ Jr") isn't in the form: it stays.
    let suffix = name
        .value
        .as_deref()
        .map(|v| v.splitn(3, '/').map(str::trim).collect::<Vec<_>>())
        .filter(|parts| parts.len() == 3 && !parts[0].is_empty() && !parts[2].is_empty() && !clean(surname).is_empty())
        .map(|parts| parts[2].to_string());
    let value = name_value(given, surname);
    name.value = Some(match suffix {
        Some(suffix) => format!("{value} {suffix}"),
        None => value,
    });
    if name.child("GIVN").is_some() {
        set_child_text(name, "GIVN", Some(given));
    }
    if name.child("SURN").is_some() {
        set_child_text(name, "SURN", Some(surname));
    }
}

/// A name `person.editData` lists among the other names: any TYPE but birth, maiden and married, or none after the
/// first name (as `model.rs` reads them).
fn is_other_name(name: &Node, first: bool) -> bool {
    match name.child_value("TYPE").map(|t| t.trim().to_ascii_uppercase()) {
        Some(t) => !matches!(t.as_str(), "BIRTH" | "MAIDEN" | "MARRIED"),
        None => !first,
    }
}

/// The words saying what kind of name it is ("zapis w akcie"): the TYPE's PHRASE; a name without a TYPE gets `OTHER`.
fn set_name_note(name: &mut Node, note: &str) {
    let current = name.child("TYPE").and_then(|t| t.child("PHRASE")).and_then(|p| p.text(Version::V7)).unwrap_or_default();
    if current.trim() == note {
        return;
    }
    match name.child_mut("TYPE") {
        Some(kind) => set_child_text(kind, "PHRASE", Some(note)),
        None => name.children.push(Node::with_value("TYPE", "OTHER").with_child(Node::with_value("PHRASE", note))),
    }
}

/// Where another program keeps a married name: `_MARNM` under a NAME, or on the person (the first with a surname).
fn vendor_married_name(indi: &Node) -> Option<(usize, Option<usize>)> {
    let named = |m: &Node| m.tag == "_MARNM" && m.value.as_deref().is_some_and(|v| !v.replace('/', "").trim().is_empty());
    indi.children
        .iter()
        .enumerate()
        .filter(|(_, c)| c.tag == "NAME")
        .find_map(|(i, c)| c.children.iter().position(named).map(|j| (i, Some(j))))
        .or_else(|| indi.children.iter().position(named).map(|i| (i, None)))
}

/// Names: `given`, `surname` (the one used now), `birthSurname` (when different: a woman's maiden name), `nickname`,
/// `otherNames`. They come back from the edit form as `person.editData` gave them: what wasn't changed stays exactly as
/// written.
fn apply_names(indi: &mut Node, args: &Value) {
    let Some(given) = arg_str(args, "given") else { return };
    let surname = arg_str(args, "surname").unwrap_or("").trim().to_string();
    let birth_surname = arg_str(args, "birthSurname").map(str::trim).filter(|b| !b.is_empty()).map(str::to_string);
    let current = model::person(indi, "", Version::V7);
    let (shown_birth, shown_married) = (current.birth_name(), current.married_name());
    let names_changed = shown_birth.map_or("", |n| n.given.as_str()) != given.trim()
        || shown_married.or(shown_birth).map_or("", |n| n.surname.as_str()) != surname
        || shown_married.and(shown_birth).map_or("", |n| n.surname.as_str()) != birth_surname.as_deref().unwrap_or("");
    let shown_nickname = current.names.iter().find_map(|n| n.nickname.clone()).filter(|n| !n.is_empty()).unwrap_or_default();
    let nickname = args.get("nickname").map(|n| n.as_str().unwrap_or("").trim().to_string()).filter(|n| *n != shown_nickname);
    if names_changed || nickname.is_some() {
        let separate = birth_surname.as_ref().is_some_and(|b| *b != surname);
        let (birth, married) = name_slots(indi);
        let birth_index = match birth {
            Some(i) => i,
            None => {
                let pos = indi.children.iter().position(|c| c.tag != "UID" && c.tag != "_UID").unwrap_or(indi.children.len());
                indi.children.insert(pos, Node::new("NAME"));
                pos
            }
        };
        // Recompute after a possible insert.
        let married = married.map(|m| if birth.is_none() && m >= birth_index { m + 1 } else { m });
        {
            let name = &mut indi.children[birth_index];
            if names_changed {
                set_name(name, given, birth_surname.as_deref().filter(|_| separate).unwrap_or(&surname));
                if separate && name.child("TYPE").is_none() {
                    name.children.push(Node::with_value("TYPE", "BIRTH"));
                }
            }
            if let Some(nick) = &nickname {
                set_child_text(name, "NICK", Some(nick));
            }
        }
        match (separate && names_changed, married, vendor_married_name(indi)) {
            (true, Some(m), _) => set_name(&mut indi.children[m], given, &surname),
            // Another program's `_MARNM` is changed where it is, not doubled by a NAME next to it.
            (true, None, Some((i, nested))) => {
                if shown_married.map(|n| n.surname.as_str()) != Some(surname.as_str()) {
                    let marnm = match nested {
                        Some(j) => &mut indi.children[i].children[j],
                        None => &mut indi.children[i],
                    };
                    marnm.value = Some(surname.replace('/', ""));
                }
            }
            (true, None, None) => {
                let married_name = Node::with_value("NAME", &name_value(given, &surname)).with_child(Node::with_value("TYPE", "MARRIED"));
                indi.children.insert(birth_index + 1, married_name);
            }
            (false, Some(m), _) if names_changed => {
                indi.children.remove(m);
            }
            _ => {}
        }
    }
    // Other names ("Inne nazwiska": a form written in a record, a religious name, an alias), in the order
    // `person.editData` gives them: an unchanged one stays as written (its TYPE, sources, original script), a changed
    // one is rewritten where it is, a removed one goes and a new one comes after the last name.
    if let Some(others) = args.get("otherNames").and_then(Value::as_array) {
        let first = indi.children.iter().position(|c| c.tag == "NAME");
        let slots: Vec<usize> = indi.children.iter().enumerate().filter(|&(i, c)| c.tag == "NAME" && is_other_name(c, Some(i) == first)).map(|(i, _)| i).collect();
        let mut removed = Vec::new();
        let mut added = Vec::new();
        for k in 0..slots.len().max(others.len()) {
            let wanted = others.get(k).map(|o| {
                let text = |key: &str| o.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string();
                (text("given"), text("surname"), text("note"))
            });
            match (slots.get(k), wanted) {
                (Some(&i), Some((given, surname, note))) if !name_value(&given, &surname).is_empty() => {
                    set_name(&mut indi.children[i], &given, &surname);
                    set_name_note(&mut indi.children[i], &note);
                }
                (Some(&i), _) => removed.push(i),
                (None, Some(wanted)) => added.push(wanted),
                (None, None) => {}
            }
        }
        for &i in removed.iter().rev() {
            indi.children.remove(i);
        }
        let pos = indi.children.iter().rposition(|c| c.tag == "NAME").map_or(0, |p| p + 1);
        for (n, (given, surname, note)) in added.into_iter().filter(|(g, s, _)| !name_value(g, s).is_empty()).enumerate() {
            // TYPE OTHER + PHRASE keeps a description ("zapis w akcie", "imię zakonne"); plain AKA otherwise.
            let kind = if note.is_empty() {
                Node::with_value("TYPE", "AKA")
            } else {
                Node::with_value("TYPE", "OTHER").with_child(Node::with_value("PHRASE", &note))
            };
            indi.children.insert(pos + n, Node::with_value("NAME", &name_value(&given, &surname)).with_child(kind));
        }
    }
}

/// A typed date ("12.03.1878", "ok. 1850") as a DATE node: GEDCOM syntax when it can be read, otherwise an empty
/// DATE with the words kept as a PHRASE.
pub fn date_node(input: &str) -> Option<Node> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    Some(match polish::parse_date_input(input) {
        Some(value) => Node::with_value("DATE", &value),
        None => Node::new("DATE").with_child(Node::with_value("PHRASE", input)),
    })
}

/// Sets the date and place of an event, creating or removing the event as needed.
fn apply_event(indi: &mut Node, tags: &[&str], data: &Value) {
    let date = data.get("date").and_then(Value::as_str);
    let place = data.get("place").and_then(Value::as_str);
    // „pewne”, „prawdopodobne”, „niepewne” (_HLM_CERT); an empty value takes the mark off.
    let certainty = data.get("certainty").and_then(Value::as_str);
    if date.is_none() && place.is_none() && certainty.is_none() {
        return;
    }
    let pos = indi.children.iter().position(|c| tags.contains(&c.tag.as_str()));
    let pos = match pos {
        Some(p) => p,
        // Certainty alone has no event to go with.
        None if date.is_none() && place.is_none() => return,
        None => {
            if date.is_none_or(|d| d.trim().is_empty()) && place.is_none_or(|p| p.trim().is_empty()) {
                return;
            }
            // In life order: names, sex, birth, baptism, death, burial.
            const ORDER: [&str; 8] = ["NAME", "SEX", "BIRT", "CHR", "BAPM", "DEAT", "BURI", "MARR"];
            let rank = |tag: &str| ORDER.iter().position(|t| *t == tag);
            let mine = rank(tags[0]).unwrap_or(ORDER.len());
            let at = indi.children.iter().rposition(|c| rank(&c.tag).is_some_and(|r| r <= mine)).map_or(indi.children.len(), |p| p + 1);
            indi.children.insert(at, Node::new(tags[0]));
            at
        }
    };
    let event = &mut indi.children[pos];
    let only_happened = event.value.as_deref() == Some("Y");
    if only_happened {
        event.value = None;
    }
    if let Some(date) = date {
        match date_node(date) {
            Some(mut new) => match event.child_mut("DATE") {
                Some(old) => {
                    // Keep what was attached to the old date (the wording in the source, the Julian date).
                    let keep: Vec<Node> = old.children.iter().filter(|c| c.tag != "PHRASE" || new.value.is_some()).cloned().collect();
                    if new.value.is_some() {
                        new.children.extend(keep);
                    }
                    *old = new;
                }
                None => {
                    let at = event.children.iter().position(|c| c.tag != "TYPE").unwrap_or(event.children.len());
                    event.children.insert(at, new);
                }
            },
            None => remove_children(event, "DATE"),
        }
    }
    if let Some(place) = place {
        let place = place.trim();
        match (event.child_mut("PLAC"), place.is_empty()) {
            (Some(p), false) => p.value = Some(place.to_string()),
            (Some(_), true) => remove_children(event, "PLAC"),
            (None, false) => {
                let at = event.children.iter().rposition(|c| c.tag == "DATE" || c.tag == "TYPE").map_or(0, |p| p + 1);
                event.children.insert(at, Node::with_value("PLAC", place));
            }
            (None, true) => {}
        }
    }
    if let Some(certainty) = certainty {
        remove_children(event, "_HLM_CERT");
        if matches!(certainty, "high" | "medium" | "low") {
            event.children.push(Node::with_value("_HLM_CERT", certainty));
        }
    }
    // An event known only to have happened ("BIRT Y") stays so when no date or place is typed for it.
    if only_happened && event.child("DATE").is_none() && event.child("PLAC").is_none() {
        event.value = Some("Y".into());
    }
    // Nothing left but the import's marks (certainty, basis): the event goes, and its marks with it.
    if event.value.is_none() && event.children.iter().all(|c| c.tag == "_HLM_CERT" || c.tag == "_HLM_BASIS") {
        indi.children.remove(pos);
    }
}

fn apply_simple_fact(indi: &mut Node, tag: &str, value: Option<&str>) {
    let Some(value) = value else { return };
    let value = value.trim();
    match indi.children.iter().position(|c| c.tag == tag) {
        Some(p) if value.is_empty() && indi.children[p].children.is_empty() => {
            indi.children.remove(p);
        }
        Some(p) => indi.children[p].value = (!value.is_empty()).then(|| value.to_string()),
        None if !value.is_empty() => {
            let at = indi.children.iter().rposition(|c| matches!(c.tag.as_str(), "NAME" | "SEX" | "BIRT" | "CHR" | "BAPM" | "DEAT" | "BURI" | "OCCU" | "RELI")).map_or(indi.children.len(), |p| p + 1);
            indi.children.insert(at, Node::with_value(tag, value));
        }
        None => {}
    }
}

fn apply_person_fields(indi: &mut Node, args: &Value) {
    apply_names(indi, args);
    if let Some(sex) = arg_str(args, "sex") {
        let code = match sex {
            "M" | "F" | "X" => sex,
            _ => "U",
        };
        match indi.child_mut("SEX") {
            Some(node) => node.value = Some(code.to_string()),
            // No SEX line already means unknown.
            None if code == "U" => {}
            None => {
                let at = indi.children.iter().rposition(|c| c.tag == "NAME").map_or(0, |p| p + 1);
                indi.children.insert(at, Node::with_value("SEX", code));
            }
        }
    }
    if let Some(events) = args.get("events").and_then(Value::as_object) {
        for (key, tags) in [("birth", &["BIRT"][..]), ("baptism", &["BAPM", "CHR"][..]), ("death", &["DEAT"][..]), ("burial", &["BURI"][..])] {
            if let Some(data) = events.get(key) {
                apply_event(indi, tags, data);
            }
        }
    }
    apply_simple_fact(indi, "OCCU", arg_str(args, "occupation"));
    apply_simple_fact(indi, "RELI", arg_str(args, "religion"));
    match arg_str(args, "status") {
        // "Zmarły" without any death details: DEAT Y ("died, date unknown").
        Some("deceased") if !indi.children.iter().any(|c| matches!(c.tag.as_str(), "DEAT" | "BURI" | "CREM")) => {
            let at = indi.children.iter().rposition(|c| matches!(c.tag.as_str(), "NAME" | "SEX" | "BIRT" | "CHR" | "BAPM")).map_or(indi.children.len(), |p| p + 1);
            indi.children.insert(at, Node::with_value("DEAT", "Y"));
        }
        Some("living" | "unknown") => {
            indi.children.retain(|c| !(c.tag == "DEAT" && c.value.as_deref() == Some("Y") && c.children.is_empty()));
        }
        _ => {}
    }
    if let Some(tags) = args.get("tags").and_then(Value::as_array) {
        remove_children(indi, "_HLM_TAG");
        for tag in tags.iter().filter_map(Value::as_str).map(str::trim).filter(|t| !t.is_empty()) {
            indi.children.push(Node::with_value("_HLM_TAG", tag));
        }
    }
    if let Some(links) = args.get("links").and_then(Value::as_array) {
        remove_children(indi, "_HLM_LINK");
        for link in links {
            let Some(url) = link.get("url").and_then(Value::as_str).map(str::trim).filter(|u| !u.is_empty()) else { continue };
            let mut node = Node::with_value("_HLM_LINK", url);
            if let Some(title) = link.get("title").and_then(Value::as_str).filter(|t| !t.trim().is_empty()) {
                node.children.push(Node::with_value("TITL", title.trim()));
            }
            if let Some(kind) = link.get("kind").and_then(Value::as_str).filter(|k| !k.is_empty()) {
                node.children.push(Node::with_value("TYPE", kind));
            }
            indi.children.push(node);
        }
    }
    if let Some(no_data) = args.get("noData").and_then(Value::as_array) {
        remove_children(indi, "_HLM_NODATA");
        for key in no_data.iter().filter_map(Value::as_str) {
            indi.children.push(Node::with_value("_HLM_NODATA", key));
        }
    }
}

fn create_person(s: &mut Session, args: &Value) -> ApiResult {
    let doc = &s.archive.doc;
    let mut changes = Changes::new(doc);
    let xref = changes.new_xref("I");
    let mut indi = Node::new("INDI").with_xref(&xref).with_child(Node::with_value("UID", &new_uid()));
    apply_person_fields(&mut indi, args);
    if indi.child("NAME").is_none() {
        return Err(ApiError::new("bad_args", "Podaj imię albo nazwisko."));
    }
    indi.children.push(stamp("CREA"));
    changes.add(indi);
    if let Some(relation) = args.get("relation") {
        let kind = relation.get("kind").and_then(Value::as_str).unwrap_or("");
        let of = relation.get("of").and_then(Value::as_str).ok_or_else(|| ApiError::bad_args("relation.of"))?;
        let family = relation.get("family").and_then(Value::as_str);
        let pedi = relation.get("pedi").and_then(Value::as_str);
        link(&mut changes, kind, of, &xref, family, pedi, relation)?;
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "id": xref }))
}

fn update_person(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&id)?;
    let before = indi.clone();
    apply_person_fields(indi, args);
    // A changed person gets a UID (mentions in texts use it); an unchanged one stays as it is.
    if *indi != before && !indi.children.iter().any(|c| c.tag == "UID" || c.tag == "_UID") {
        let at = indi.children.iter().position(|c| c.tag == "NAME").unwrap_or(0);
        indi.children.insert(at, Node::with_value("UID", &new_uid()));
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "id": id }))
}

fn delete_person(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let doc = &s.archive.doc;
    let indi = doc.record(&id).ok_or_else(|| crate::people::not_found(&id))?.clone();
    let mut changes = Changes::new(doc);
    let families: Vec<String> =
        indi.children.iter().filter(|c| c.tag == "FAMC" || c.tag == "FAMS").filter_map(|c| c.pointer().map(str::to_string)).collect();
    for fam in families {
        if !changes.exists(&fam) {
            continue;
        }
        let family = changes.get(&fam)?;
        for tag in ["HUSB", "WIFE", "CHIL"] {
            remove_pointer(family, tag, &id);
        }
        drop_empty_family(&mut changes, &fam)?;
    }
    // Texts only this person links go with them (the history can bring everything back).
    for text in indi.children.iter().filter(|c| c.tag == "SNOTE").filter_map(|c| c.pointer()) {
        let others = doc.records.iter().any(|r| r.xref.as_deref() != Some(id.as_str()) && has_pointer(r, "SNOTE", text));
        if !others {
            changes.remove(text);
        }
    }
    changes.remove(&id);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Takes out a link that leads nowhere (its record is gone), wherever it sits in the record: `1 CHIL @I9@`,
/// `2 SOUR @S3@` under an event… Refused while the target exists: this is not a way to unlink real people.
fn remove_broken_link(s: &mut Session, args: &Value) -> ApiResult {
    let record = req(args, "record")?;
    let xref = req(args, "xref")?;
    if s.archive.doc.record(&xref).is_some() {
        return Err(ApiError::new("bad_args", "Ten link prowadzi do istniejącego wpisu."));
    }
    fn strip(node: &mut Node, xref: &str) {
        node.children.retain(|c| c.pointer() != Some(xref));
        for c in &mut node.children {
            strip(c, xref);
        }
    }
    let mut changes = Changes::new(&s.archive.doc);
    strip(changes.get(&record)?, &xref);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// A family left with fewer than two members is removed, and its last member's link to it too.
pub(crate) fn drop_empty_family(changes: &mut Changes, fam: &str) -> Result<(), ApiError> {
    let Some(family) = changes.peek(fam) else { return Ok(()) };
    let members: Vec<(String, String)> = family
        .children
        .iter()
        .filter(|c| matches!(c.tag.as_str(), "HUSB" | "WIFE" | "CHIL"))
        .filter_map(|c| Some((c.tag.clone(), c.pointer()?.to_string())))
        .collect();
    if members.len() >= 2 {
        return Ok(());
    }
    for (tag, person) in members {
        if changes.exists(&person) {
            let indi = changes.get(&person)?;
            remove_pointer(indi, if tag == "CHIL" { "FAMC" } else { "FAMS" }, fam);
        }
    }
    changes.remove(fam);
    Ok(())
}

fn partner_slot(family: &Node, sex: &str) -> Option<&'static str> {
    let has = |tag: &str| family.child(tag).and_then(Node::pointer).is_some_and(|p| p != "@VOID@");
    match sex {
        "M" if !has("HUSB") => Some("HUSB"),
        "F" if !has("WIFE") => Some("WIFE"),
        "M" | "F" => None,
        _ if !has("HUSB") => Some("HUSB"),
        _ if !has("WIFE") => Some("WIFE"),
        _ => None,
    }
}

pub(crate) fn add_partner_to_family(changes: &mut Changes, fam: &str, person: &str) -> Result<bool, ApiError> {
    let sex = changes.peek(person).map(|n| sex_of(n).to_string()).unwrap_or_default();
    let family = changes.get(fam)?;
    if has_pointer(family, "HUSB", person) || has_pointer(family, "WIFE", person) {
        return Ok(true);
    }
    let Some(slot) = partner_slot(family, &sex) else { return Ok(false) };
    family.children.retain(|c| !(c.tag == slot && c.pointer() == Some("@VOID@")));
    let at = if slot == "HUSB" { 0 } else { family.children.iter().rposition(|c| c.tag == "HUSB").map_or(0, |p| p + 1) };
    family.children.insert(at, Node::with_value(slot, person));
    let indi = changes.get(person)?;
    if !has_pointer(indi, "FAMS", fam) {
        insert_after(indi, &["FAMC", "FAMS"], Node::with_value("FAMS", fam));
    }
    Ok(true)
}

pub(crate) fn add_child_to_family(changes: &mut Changes, fam: &str, child: &str, pedi: Option<&str>) -> Result<(), ApiError> {
    let family = changes.get(fam)?;
    if !has_pointer(family, "CHIL", child) {
        insert_after(family, &["HUSB", "WIFE", "CHIL"], Node::with_value("CHIL", child));
    }
    let indi = changes.get(child)?;
    if !has_pointer(indi, "FAMC", fam) {
        insert_after(indi, &["FAMC"], Node::with_value("FAMC", fam));
        let at = indi.children.iter().rposition(|c| c.tag == "FAMC").unwrap_or(0);
        if let Some(pedi) = pedi_code(pedi) {
            indi.children[at].children.push(Node::with_value("PEDI", pedi));
        }
    }
    Ok(())
}

fn pedi_code(pedi: Option<&str>) -> Option<&'static str> {
    match pedi? {
        "adopted" => Some("ADOPTED"),
        "foster" => Some("FOSTER"),
        "step" | "unknown" | "other" => Some("OTHER"),
        _ => None,
    }
}

pub(crate) fn new_family(changes: &mut Changes) -> String {
    let fam = changes.new_xref("F");
    changes.add(Node::new("FAM").with_xref(&fam).with_child(Node::with_value("UID", &new_uid())).with_child(stamp("CREA")));
    fam
}

/// Links `other` to `person` as their `kind` (parent, partner, child, sibling).
pub(crate) fn link(changes: &mut Changes, kind: &str, person: &str, other: &str, family: Option<&str>, pedi: Option<&str>, extra: &Value) -> Result<(), ApiError> {
    if person == other {
        return Err(ApiError::new("bad_args", "Osoba nie może być swoim własnym krewnym."));
    }
    let pointers = |changes: &Changes, xref: &str, tag: &str| -> Vec<String> {
        changes.peek(xref).map(|n| n.children.iter().filter(|c| c.tag == tag).filter_map(|c| c.pointer().map(str::to_string)).collect()).unwrap_or_default()
    };
    match kind {
        "parent" => {
            // Into a family the person is already a child of, if the parent's place there is free.
            let mut joined = false;
            for fam in pointers(changes, person, "FAMC") {
                if changes.exists(&fam) && add_partner_to_family(changes, &fam, other)? {
                    joined = true;
                    break;
                }
            }
            if !joined {
                let fam = new_family(changes);
                add_partner_to_family(changes, &fam, other)?;
                add_child_to_family(changes, &fam, person, pedi)?;
            }
        }
        "partner" => {
            let shared = pointers(changes, person, "FAMS").into_iter().find(|f| pointers(changes, other, "FAMS").contains(f));
            let fam = match shared {
                Some(f) => f,
                None => {
                    let fam = new_family(changes);
                    add_partner_to_family(changes, &fam, person)?;
                    if !add_partner_to_family(changes, &fam, other)? {
                        // Both partners have the same sex: GEDCOM 7 allows any partners in HUSB/WIFE.
                        let family = changes.get(&fam)?;
                        family.children.insert(1, Node::with_value("WIFE", other));
                        let indi = changes.get(other)?;
                        insert_after(indi, &["FAMC", "FAMS"], Node::with_value("FAMS", &fam));
                    }
                    fam
                }
            };
            if extra.get("married").and_then(Value::as_bool).unwrap_or(false) {
                let date = extra.get("date").and_then(Value::as_str).and_then(date_node);
                let place = extra.get("place").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty());
                let family = changes.get(&fam)?;
                if family.child("MARR").is_none() {
                    let mut marr = Node::new("MARR");
                    if let Some(date) = date {
                        marr.children.push(date);
                    }
                    if let Some(place) = place {
                        marr.children.push(Node::with_value("PLAC", place));
                    }
                    if marr.children.is_empty() {
                        marr.value = Some("Y".into());
                    }
                    family.children.push(marr);
                }
            }
        }
        "child" => {
            let own = pointers(changes, person, "FAMS");
            let fam = match family.map(str::to_string) {
                Some(f) if own.contains(&f) => f,
                _ => {
                    let other_parent = extra.get("otherParent").and_then(Value::as_str);
                    let chosen = match other_parent {
                        Some(op) => own.iter().find(|f| pointers(changes, op, "FAMS").contains(f)).cloned(),
                        None => own.first().cloned().filter(|_| own.len() == 1),
                    };
                    match chosen {
                        Some(f) => f,
                        None => {
                            let fam = new_family(changes);
                            add_partner_to_family(changes, &fam, person)?;
                            if let Some(op) = other_parent {
                                add_partner_to_family(changes, &fam, op)?;
                            }
                            fam
                        }
                    }
                }
            };
            add_child_to_family(changes, &fam, other, pedi)?;
        }
        "sibling" => {
            let fam = match pointers(changes, person, "FAMC").into_iter().next() {
                Some(f) => f,
                None => {
                    let fam = new_family(changes);
                    add_child_to_family(changes, &fam, person, None)?;
                    fam
                }
            };
            add_child_to_family(changes, &fam, other, pedi)?;
        }
        _ => return Err(ApiError::bad_args("kind")),
    }
    // A new parent–child link may close a circle (a grandson as his grandfather's father): refused. The archive had
    // none before, so a new one would run through one of the two people just linked.
    if is_own_ancestor(changes, person) || is_own_ancestor(changes, other) {
        return Err(ApiError::new("bad_args", "Tak się nie da: ktoś byłby wtedy swoim własnym przodkiem (np. dziadek dzieckiem własnego wnuka)."));
    }
    Ok(())
}

/// Whether `xref` is among their own ancestors (through the families in this command's state of the records).
fn is_own_ancestor(changes: &Changes, xref: &str) -> bool {
    let mut seen: Vec<String> = Vec::new();
    let mut queue = vec![xref.to_string()];
    while let Some(at) = queue.pop() {
        let Some(indi) = changes.peek(&at) else { continue };
        for fam in indi.children.iter().filter(|c| c.tag == "FAMC").filter_map(Node::pointer) {
            let Some(family) = changes.peek(fam) else { continue };
            for parent in family.children.iter().filter(|c| c.tag == "HUSB" || c.tag == "WIFE").filter_map(Node::pointer) {
                if parent == xref {
                    return true;
                }
                if !seen.iter().any(|x| x == parent) {
                    seen.push(parent.to_string());
                    queue.push(parent.to_string());
                }
            }
        }
    }
    false
}

fn add_relation(s: &mut Session, args: &Value) -> ApiResult {
    let kind = req(args, "kind")?;
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let mut changes = Changes::new(&s.archive.doc);
    if !changes.exists(&person) || !changes.exists(&other) {
        return Err(crate::people::not_found(if changes.exists(&person) { &other } else { &person }));
    }
    link(&mut changes, &kind, &person, &other, arg_str(args, "family"), arg_str(args, "pedi"), args)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// One family `other` leaves when it is unlinked from `person` (for the sentence that says so).
pub(crate) struct Leaving {
    /// They were a child in it (else a partner).
    as_child: bool,
    /// The family's partners and children besides `other`.
    partners: Vec<String>,
    children: Vec<String>,
}

/// The families that tie `other` to `person` directly: the ones both are in, or, for half-siblings, the family of
/// `other` with the parent they share.
fn shared_families(changes: &Changes, person: &str, other: &str) -> Vec<String> {
    let members = |fam: &str| -> Vec<(String, String)> {
        changes.peek(fam).map(|f| f.children.iter().filter(|c| matches!(c.tag.as_str(), "HUSB" | "WIFE" | "CHIL")).filter_map(|c| Some((c.tag.clone(), c.pointer()?.to_string()))).collect()).unwrap_or_default()
    };
    let families = |xref: &str, tags: &[&str]| -> Vec<String> {
        changes.peek(xref).map(|n| n.children.iter().filter(|c| tags.contains(&c.tag.as_str())).filter_map(|c| c.pointer().map(str::to_string)).collect()).unwrap_or_default()
    };
    let both: Vec<String> = families(other, &["FAMC", "FAMS"]).into_iter().filter(|f| changes.exists(f) && members(f).iter().any(|(_, x)| x == person) && members(f).iter().any(|(_, x)| x == other)).collect();
    if !both.is_empty() {
        return both;
    }
    let parents: Vec<String> = families(person, &["FAMC"]).iter().flat_map(|f| members(f)).filter(|(tag, _)| tag != "CHIL").map(|(_, x)| x).collect();
    families(other, &["FAMC"]).into_iter().filter(|f| members(f).iter().any(|(tag, x)| tag != "CHIL" && parents.contains(x))).collect()
}

/// Unlinks `other` from `person`: `other` leaves the family that ties them (as its child or as its partner) and
/// everyone else's links stay. A family left with fewer than two people goes (GEDCOM keeps no empty FAM).
fn unlink(changes: &mut Changes, person: &str, other: &str) -> Result<Vec<Leaving>, ApiError> {
    if person == other {
        return Err(ApiError::new("bad_args", "Osoba nie może być swoim własnym krewnym."));
    }
    let families = shared_families(changes, person, other);
    if families.is_empty() {
        return Err(ApiError::new("not_found", "Te osoby nie są bezpośrednio powiązane."));
    }
    let mut out = Vec::new();
    for fam in families {
        let family = changes.peek(&fam).cloned().ok_or_else(|| ApiError::bad_args("family"))?;
        let Some(role) = family.children.iter().find(|c| c.pointer() == Some(other)).map(|c| c.tag.clone()) else { continue };
        let list = |child: bool| -> Vec<String> {
            family.children.iter().filter(|c| (c.tag == "CHIL") == child && matches!(c.tag.as_str(), "HUSB" | "WIFE" | "CHIL")).filter_map(Node::pointer).filter(|x| *x != other && *x != "@VOID@").map(str::to_string).collect()
        };
        out.push(Leaving { as_child: role == "CHIL", partners: list(false), children: list(true) });
        remove_pointer(changes.get(&fam)?, &role, other);
        remove_pointer(changes.get(other)?, if role == "CHIL" { "FAMC" } else { "FAMS" }, &fam);
        drop_empty_family(changes, &fam)?;
    }
    Ok(out)
}

/// Removes the direct link between two people (parent, child, partner or sibling): `other` leaves the family that
/// ties them; both stay in the archive.
fn remove_relation(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let mut changes = Changes::new(&s.archive.doc);
    unlink(&mut changes, &person, &other)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// „Zmień pokrewieństwo”: `other` becomes `person`'s `kind` (parent, partner, child, sibling) instead of what they
/// were: they leave the family that tied them and are linked anew, in one undo step.
fn change_relation(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let kind = req(args, "kind")?;
    if !matches!(kind.as_str(), "parent" | "partner" | "child" | "sibling") {
        return Err(ApiError::bad_args("kind"));
    }
    let mut changes = Changes::new(&s.archive.doc);
    if !changes.exists(&person) || !changes.exists(&other) {
        return Err(crate::people::not_found(if changes.exists(&person) { &other } else { &person }));
    }
    unlink(&mut changes, &person, &other)?;
    link(&mut changes, &kind, &person, &other, None, arg_str(args, "pedi"), &Value::Null)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// What „Odłącz” would change, in words: „Franciszek Kowalski przestanie być dzieckiem: Jan Kowalski i Maria
/// Kowalska.” Nothing is changed.
fn unlink_preview(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let leaving = {
        let mut changes = Changes::new(&s.archive.doc);
        unlink(&mut changes, &person, &other)?
    };
    let d = s.derived();
    let name = |x: &str| d.index(x).map(|i| d.info[i].name.clone()).unwrap_or_else(|| x.to_string());
    let list = |xs: &[String]| -> String {
        let names: Vec<String> = xs.iter().map(|x| name(x)).collect();
        match names.len() {
            0 => String::new(),
            1 => names[0].clone(),
            n if n <= 4 => format!("{} i {}", names[..n - 1].join(", "), names[n - 1]),
            n => format!("{} i {}", names[..3].join(", "), crate::count_pl(n - 3, "inna osoba", "inne osoby", "innych osób")),
        }
    };
    let who = name(&other);
    let mut parts = Vec::new();
    for l in &leaving {
        parts.push(if l.as_child && !l.partners.is_empty() {
            format!("{who} przestanie być dzieckiem: {}.", list(&l.partners))
        } else if l.as_child {
            format!("{who} przestanie być rodzeństwem: {}.", list(&l.children))
        } else if l.partners.contains(&person) {
            let kids = if l.children.is_empty() { String::new() } else { format!(" Ich dzieci ({}) zostaną dziećmi tylko: {}.", list(&l.children), name(&person)) };
            format!("{} i {who} nie będą już parą.{kids}", name(&person))
        } else {
            format!("{who} przestanie być rodzicem: {}.", list(&l.children))
        });
    }
    parts.push(format!("{who} zostaje w archiwum. Do zapisu możesz to cofnąć (Ctrl Z)."));
    Ok(json!({ "title": format!("Odłączyć: {who}?"), "text": parts.join(" ") }))
}

/// A relation outside the family (friend, neighbour, godparent, colleague…): `1 ASSO @I2@` + `ROLE` (+ `PHRASE`).
fn associate(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let role = match arg_str(args, "role").unwrap_or("OTHER") {
        r @ ("FRIEND" | "NGHBR" | "GODP" | "WITN" | "CLERGY" | "OTHER") => r,
        _ => "OTHER",
    };
    let phrase = arg_str(args, "phrase").map(str::trim).filter(|p| !p.is_empty());
    let mut changes = Changes::new(&s.archive.doc);
    if !changes.exists(&other) {
        return Err(crate::people::not_found(&other));
    }
    let indi = changes.get(&person)?;
    indi.children.retain(|c| !(c.tag == "ASSO" && c.pointer() == Some(other.as_str())));
    let mut role_node = Node::with_value("ROLE", role);
    if let Some(p) = phrase {
        role_node.children.push(Node::with_value("PHRASE", p));
    }
    insert_after(indi, &["FAMC", "FAMS", "ASSO"], Node::with_value("ASSO", &other).with_child(role_node));
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

fn dissociate(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let other = req(args, "other")?;
    let mut changes = Changes::new(&s.archive.doc);
    for (a, b) in [(&person, &other), (&other, &person)] {
        if changes.peek(a).is_some_and(|n| n.children.iter().any(|c| c.tag == "ASSO" && c.pointer() == Some(b.as_str()))) {
            let indi = changes.get(a)?;
            indi.children.retain(|c| !(c.tag == "ASSO" && c.pointer() == Some(b.as_str())));
        }
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}
fn set_pedigree(s: &mut Session, args: &Value) -> ApiResult {
    let child = req(args, "child")?;
    let family = req(args, "family")?;
    let pedi = arg_str(args, "pedi");
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&child)?;
    let famc = indi
        .children
        .iter_mut()
        .find(|c| c.tag == "FAMC" && c.pointer() == Some(family.as_str()))
        .ok_or_else(|| ApiError::bad_args("family"))?;
    famc.children.retain(|c| c.tag != "PEDI");
    if let Some(code) = pedi_code(pedi) {
        famc.children.insert(0, Node::with_value("PEDI", code));
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

fn update_family(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let mut changes = Changes::new(&s.archive.doc);
    let family = changes.get(&id)?;
    if let Some(married) = args.get("married").and_then(Value::as_bool) {
        if !married {
            family.children.retain(|c| c.tag != "MARR");
        } else if family.child("MARR").is_none() {
            family.children.push(Node::with_value("MARR", "Y"));
        }
    }
    if args.get("date").is_some() || args.get("place").is_some() {
        if family.child("MARR").is_none() {
            family.children.push(Node::new("MARR"));
        }
        let mut wrapper = Node::new("X");
        wrapper.children = std::mem::take(&mut family.children);
        apply_event(&mut wrapper, &["MARR"], args);
        family.children = wrapper.children;
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// The position among a record's children of its `index`-th event (as the profile numbers them: a person's events
/// and attributes, or a family's), checked against the tag the screen saw, so a stale screen changes nothing.
fn fact_position(record: &Node, index: usize, tag: &str) -> Result<usize, ApiError> {
    let events = if record.tag == "FAM" { FAM_EVENTS } else { INDI_EVENTS };
    record
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| events.contains(&c.tag.as_str()))
        .nth(index)
        .filter(|(_, c)| c.tag == tag)
        .map(|(p, _)| p)
        .ok_or_else(stale)
}

/// Events whose being there says something even without a date or a place (born, died, married): emptied in place,
/// they stay as "DEAT Y" („tak, bez daty”). Only „Usuń wpis…” takes them away. Other facts emptied in place are
/// refused (the screen offers „Usuń wpis…” instead), so nothing goes as a side effect of a form.
pub(crate) const HAPPENED: &[&str] = &["BIRT", "CHR", "BAPM", "DEAT", "BURI", "CREM", "MARR"];

/// Sets a fact's date, place and description (`text`: the occupation for OCCU, a note for a birth or an event).
/// Keys left out stay as they are.
fn apply_fact(record: &mut Node, pos: usize, args: &Value) -> Result<(), ApiError> {
    let mut wrapper = Node::new("X");
    wrapper.children.push(record.children.remove(pos));
    let tag = wrapper.children[0].tag.clone();
    // The description first: an event with only a description typed in is not empty.
    if let Some(text) = arg_str(args, "text") {
        let event = &mut wrapper.children[0];
        if crate::people::holds_value(&event.tag, event.value.as_deref()) {
            event.set_text(text.trim(), Version::V7);
        } else {
            set_child_text(event, "NOTE", Some(text));
        }
    }
    apply_event(&mut wrapper, &[tag.as_str()], args);
    // apply_event drops an event left with nothing in it; a birth, a death or a wedding stays as one that happened.
    let mut event = wrapper.children.pop().unwrap_or_else(|| Node::new(&tag));
    let bare = event.value.is_none() && event.child("DATE").is_none() && event.child("PLAC").is_none();
    if bare && HAPPENED.contains(&tag.as_str()) {
        event.value = Some("Y".into());
    } else if bare && event.child("NOTE").is_none() {
        return Err(ApiError::new("bad_args", "Wpis byłby pusty. Żeby go usunąć, użyj „Usuń wpis…”."));
    }
    record.children.insert(pos, event);
    Ok(())
}

/// The fact as its form showed it when it opened (`was`: date, place, text, as `person.get` gave them). A form left
/// open over an undo would otherwise write onto whatever fact now has its place.
fn check_was(s: &mut Session, record: &str, index: usize, args: &Value) -> Result<(), ApiError> {
    let Some(was) = args.get("was") else { return Ok(()) };
    let now = crate::people::fact_ref_at(s.derived(), record, index);
    let same = now.is_some_and(|now| ["date", "place", "text"].iter().all(|k| was.get(k).is_none_or(|w| *w == now[k])));
    if same { Ok(()) } else { Err(stale()) }
}

fn stale() -> ApiError {
    ApiError::new("stale", "Ten wpis zmienił się w międzyczasie. Odśwież profil i spróbuj jeszcze raz.")
}

/// `fact.update`: one fact of a person or a family, edited in place on the profile („W skrócie”, „Oś życia”).
fn update_fact(s: &mut Session, args: &Value) -> ApiResult {
    let record = req(args, "record")?;
    let index = args.get("index").and_then(Value::as_u64).ok_or_else(|| ApiError::bad_args("index"))? as usize;
    let tag = req(args, "tag")?;
    check_was(s, &record, index, args)?;
    let mut changes = Changes::new(&s.archive.doc);
    let node = changes.get(&record)?;
    let pos = fact_position(node, index, &tag)?;
    apply_fact(node, pos, args)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// The kinds of facts the profile can add, with the event name an "EVEN" needs.
pub(crate) const ADDABLE: &[&str] = &["OCCU", "RESI", "EDUC", "_MILT", "EMIG", "IMMI", "RELI", "EVEN"];

/// `fact.add`: a new fact for a person (a second occupation, a place they lived, an event), after the facts of its
/// kind. Returns its index.
fn add_fact(s: &mut Session, args: &Value) -> ApiResult {
    let record = req(args, "record")?;
    let tag = req(args, "tag")?;
    if !ADDABLE.contains(&tag.as_str()) {
        return Err(ApiError::bad_args("tag"));
    }
    let kind = arg_str(args, "kind").map(str::trim).filter(|k| !k.is_empty());
    if tag == "EVEN" && kind.is_none() {
        return Err(ApiError::new("bad_args", "Podaj, co to za wydarzenie."));
    }
    let filled = |key: &str| arg_str(args, key).is_some_and(|v| !v.trim().is_empty());
    if !filled("date") && !filled("place") && !filled("text") {
        return Err(ApiError::new("bad_args", "Wpisz datę, miejsce albo opis."));
    }
    let mut changes = Changes::new(&s.archive.doc);
    let node = changes.get(&record)?;
    if node.tag != "INDI" {
        return Err(ApiError::bad_args("record"));
    }
    let mut fact = Node::new(&tag);
    if let Some(kind) = kind {
        fact.children.push(Node::with_value("TYPE", kind));
    }
    // After the facts of its kind, else after the last event (else after the names).
    let at = node
        .children
        .iter()
        .rposition(|c| c.tag == tag)
        .or_else(|| node.children.iter().rposition(|c| INDI_EVENTS.contains(&c.tag.as_str())))
        .or_else(|| node.children.iter().rposition(|c| matches!(c.tag.as_str(), "NAME" | "SEX")))
        .map_or(node.children.len(), |p| p + 1);
    node.children.insert(at, fact);
    let index = node.children[..at].iter().filter(|c| INDI_EVENTS.contains(&c.tag.as_str())).count();
    apply_fact(node, at, args)?;
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "index": index }))
}

/// `fact.delete`: one fact of a person or a family, with its date, place, notes and citations.
fn delete_fact(s: &mut Session, args: &Value) -> ApiResult {
    let record = req(args, "record")?;
    let index = args.get("index").and_then(Value::as_u64).ok_or_else(|| ApiError::bad_args("index"))? as usize;
    let tag = req(args, "tag")?;
    check_was(s, &record, index, args)?;
    let mut changes = Changes::new(&s.archive.doc);
    let node = changes.get(&record)?;
    let pos = fact_position(node, index, &tag)?;
    node.children.remove(pos);
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Creates or updates a text (biography section, story, saying, trivia, note, summary) and links it to the person.
fn save_text(s: &mut Session, args: &Value) -> ApiResult {
    let person = req(args, "person")?;
    let kind = TextKind::from_code(&req(args, "kind")?).ok_or_else(|| ApiError::bad_args("kind"))?;
    let body = req(args, "body")?;
    s.ensure();
    let d = s.derived.as_ref().expect("ensured");
    let mut changes = Changes::new(&s.archive.doc);
    // Mentions of people without a UID: give them one first, so the link survives renumbering.
    let mut new_uids: HashMap<String, String> = HashMap::new();
    for xref in crate::text::mentioned_without_uid(d, &body) {
        let uid = new_uid();
        let indi = changes.get(&xref)?;
        let at = indi.children.iter().position(|c| c.tag == "NAME").unwrap_or(0);
        indi.children.insert(at, Node::with_value("UID", &uid));
        new_uids.insert(xref, uid);
    }
    let stored = crate::text::storage_markdown(d, &body, &new_uids);
    let existing = arg_str(args, "id").map(str::to_string).or_else(|| {
        // Only one summary per person.
        (kind == TextKind::Summary)
            .then(|| d.person_texts_of(&person).into_iter().find(|t| t.kind == TextKind::Summary).and_then(|t| t.xref.clone()))
            .flatten()
    });
    let xref = match existing {
        Some(xref) => {
            let note = changes.get(&xref)?;
            note.set_text(&stored, heirloom_core::gedcom::Version::V7);
            xref
        }
        None => {
            let xref = changes.new_xref("N");
            let mut note = text_node("SNOTE", &stored);
            note.xref = Some(xref.clone());
            note.children.push(Node::with_value("_HLM_KIND", kind.code()));
            note.children.push(stamp("CREA"));
            changes.add(note);
            let indi = changes.get(&person)?;
            let after_same_kind = indi.children.iter().rposition(|c| c.tag == "SNOTE");
            let at = after_same_kind.map_or_else(|| indi.children.iter().rposition(|c| c.tag == "NOTE" || c.tag == "FAMS" || c.tag == "FAMC").map_or(indi.children.len(), |p| p + 1), |p| p + 1);
            indi.children.insert(at, Node::with_value("SNOTE", &xref));
            xref
        }
    };
    let note = changes.get(&xref)?;
    note.children.retain(|c| c.tag != "_HLM_KIND");
    note.children.insert(0, Node::with_value("_HLM_KIND", kind.code()));
    for (key, tag) in [("title", "_HLM_TITLE"), ("place", "_HLM_PLAC"), ("certainty", "_HLM_CERT")] {
        if let Some(value) = args.get(key) {
            set_child_text(note, tag, value.as_str());
        }
    }
    if let Some(date) = args.get("date") {
        let gedcom = date.as_str().and_then(|t| polish::parse_date_input(t).or_else(|| (!t.trim().is_empty()).then(|| t.trim().to_string())));
        set_child_text(note, "_HLM_DATE", gedcom.as_deref());
    }
    // Other people this text also belongs to (a story shown on several profiles).
    if let Some(people) = args.get("people").and_then(Value::as_array) {
        for other in people.iter().filter_map(Value::as_str).filter(|o| *o != person) {
            if changes.exists(other) {
                let indi = changes.get(other)?;
                if !has_pointer(indi, "SNOTE", &xref) {
                    indi.children.push(Node::with_value("SNOTE", &xref));
                }
            }
        }
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(json!({ "id": xref }))
}

fn delete_text(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let person = arg_str(args, "person");
    let doc = &s.archive.doc;
    let mut changes = Changes::new(doc);
    let linked: Vec<String> = doc.records.iter().filter(|r| r.tag == "INDI" && has_pointer(r, "SNOTE", &id)).filter_map(|r| r.xref.clone()).collect();
    let unlink: Vec<String> = match person {
        Some(p) => vec![p.to_string()],
        None => linked.clone(),
    };
    for p in &unlink {
        let indi = changes.get(p)?;
        remove_pointer(indi, "SNOTE", &id);
    }
    if linked.iter().all(|p| unlink.contains(p)) {
        changes.remove(&id);
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

/// Moves a text up or down among the person's texts (the biography's table of contents follows this order).
fn move_text(s: &mut Session, args: &Value) -> ApiResult {
    let id = req(args, "id")?;
    let person = req(args, "person")?;
    let delta = args.get("delta").and_then(Value::as_i64).unwrap_or(0);
    let mut changes = Changes::new(&s.archive.doc);
    let indi = changes.get(&person)?;
    let positions: Vec<usize> = indi.children.iter().enumerate().filter(|(_, c)| c.tag == "SNOTE").map(|(i, _)| i).collect();
    let Some(k) = positions.iter().position(|&i| indi.children[i].pointer() == Some(id.as_str())) else {
        return Err(ApiError::bad_args("id"));
    };
    let target = k as i64 + delta;
    if target >= 0 && (target as usize) < positions.len() {
        indi.children.swap(positions[k], positions[target as usize]);
    }
    let edits = changes.into_edits();
    commit(s, edits)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::Archive;

    fn session() -> (tempfile::TempDir, Session) {
        let dir = tempfile::tempdir().unwrap();
        let archive = Archive::create(&dir.path().join("a"), "A").unwrap();
        (dir, Session::new(archive))
    }

    fn run(s: &mut Session, method: &str, args: Value) -> Value {
        call(s, method, &args).unwrap_or_else(|e| panic!("{method}: {}", e.message))
    }

    #[test]
    fn create_people_and_link_a_family() {
        let (_dir, mut s) = session();
        let jozef = run(&mut s, "person.create", json!({ "given": "Józef", "surname": "Kowalski", "sex": "M", "events": { "birth": { "date": "12.03.1878", "place": "Wólka" } } }))["id"].as_str().unwrap().to_string();
        let marianna = run(&mut s, "person.create", json!({ "given": "Marianna", "surname": "Kowalska", "birthSurname": "Nowak", "sex": "F", "relation": { "kind": "partner", "of": jozef, "married": true, "date": "14.02.1904" } }))["id"].as_str().unwrap().to_string();
        let jan = run(&mut s, "person.create", json!({ "given": "Jan", "surname": "Kowalski", "sex": "M", "relation": { "kind": "child", "of": jozef } }))["id"].as_str().unwrap().to_string();
        let antoni = run(&mut s, "person.create", json!({ "given": "Antoni", "surname": "Kowalski", "sex": "M", "status": "deceased", "relation": { "kind": "parent", "of": jozef } }))["id"].as_str().unwrap().to_string();

        let d = s.derived();
        let j = d.index(&jozef).unwrap();
        assert_eq!(d.info[j].birth.as_ref().unwrap().text, "12 marca 1878");
        assert_eq!(d.info[j].birth_place.as_deref(), Some("Wólka"));
        let m = d.index(&marianna).unwrap();
        assert_eq!((d.info[m].name.as_str(), d.info[m].maiden.as_deref()), ("Marianna Kowalska", Some("Nowak")));
        assert!(d.info[j].partners.contains(&m));
        assert_eq!(crate::kin::marriage(d, j, m), (true, Some("1904".into())));
        let child = d.index(&jan).unwrap();
        assert!(d.info[child].parents.contains(&j) && d.info[child].parents.contains(&m), "the child joins the couple's family");
        let a = d.index(&antoni).unwrap();
        assert!(d.info[j].parents.contains(&a));
        assert!(!d.info[a].living && d.view.people[a].facts.iter().any(|f| f.tag == "DEAT" && f.only_happened));

        // One command is one undo step.
        s.archive.undo();
        s.changed();
        assert!(s.derived().index(&antoni).is_none());
        assert!(s.archive.doc.head().unwrap().child("SCHMA").is_some(), "extension tags were declared");
    }

    #[test]
    fn update_names_dates_and_remove_relations() {
        let (_dir, mut s) = session();
        let a = run(&mut s, "person.create", json!({ "given": "Helena", "surname": "Kowalska", "sex": "F" }))["id"].as_str().unwrap().to_string();
        let b = run(&mut s, "person.create", json!({ "given": "Tadeusz", "surname": "Wiśniewski", "sex": "M", "relation": { "kind": "partner", "of": a } }))["id"].as_str().unwrap().to_string();
        run(&mut s, "person.update", json!({ "id": a, "given": "Helena", "surname": "Wiśniewska", "birthSurname": "Kowalska", "nickname": "Hela",
            "events": { "birth": { "date": "ok. 1908", "place": "Lublin" }, "death": { "date": "zima 1995" } }, "tags": ["nauczycielka"] }));
        let d = s.derived();
        let h = d.index(&a).unwrap();
        assert_eq!((d.info[h].name.as_str(), d.info[h].maiden.as_deref(), d.info[h].nickname.as_deref()), ("Helena Wiśniewska", Some("Kowalska"), Some("Hela")));
        assert_eq!(d.info[h].birth.as_ref().unwrap().year, "ok. 1908");
        assert_eq!(d.info[h].death.as_ref().unwrap().text, "zima 1995", "words that aren't a date are kept");
        assert_eq!(d.view.people[h].tags, ["nauczycielka"]);
        run(&mut s, "relation.remove", json!({ "person": a, "other": b }));
        let d = s.derived();
        assert!(d.info[d.index(&a).unwrap()].partners.is_empty());
        assert!(s.archive.doc.records.iter().all(|r| r.tag != "FAM"), "an empty family is removed");
    }

    #[test]
    fn facts_are_changed_added_and_deleted_in_place() {
        let (_dir, mut s) = session();
        let a = run(&mut s, "person.create", json!({ "given": "Józef", "surname": "Kowalski", "sex": "M", "occupation": "rolnik", "events": { "birth": { "date": "1878", "place": "Wólka" } } }))["id"].as_str().unwrap().to_string();
        let b = run(&mut s, "person.create", json!({ "given": "Marianna", "surname": "Nowak", "sex": "F", "relation": { "kind": "partner", "of": a, "married": true, "date": "1904" } }))["id"].as_str().unwrap().to_string();
        let facts = |s: &mut Session| -> Vec<(String, Option<String>, Option<String>, Option<String>, Option<String>)> {
            let d = s.derived();
            d.view.people[d.index(&a).unwrap()].facts.iter().map(|f| (f.tag.clone(), f.value.clone(), f.date_text.clone(), f.place.clone(), f.note.clone())).collect()
        };
        // The occupation (index 1, after the birth) is changed, and a second one added with a date.
        run(&mut s, "fact.update", json!({ "record": a, "index": 1, "tag": "OCCU", "text": "kolejarz", "place": "Lublin" }));
        let added = run(&mut s, "fact.add", json!({ "record": a, "tag": "OCCU", "text": "dróżnik", "date": "od 1920" }));
        assert_eq!(added["index"], 2);
        // A note on the birth (in brackets, as a family member would add it) and a fixed town.
        run(&mut s, "fact.update", json!({ "record": a, "index": 0, "tag": "BIRT", "place": "Wólka (dziś Wólka Łęczyńska)", "text": "w domu dziadków" }));
        let f = facts(&mut s);
        assert_eq!(f[0], ("BIRT".into(), None, Some("1878".into()), Some("Wólka (dziś Wólka Łęczyńska)".into()), Some("w domu dziadków".into())));
        assert_eq!(f[1], ("OCCU".into(), Some("kolejarz".into()), None, Some("Lublin".into()), None));
        assert_eq!(f[2], ("OCCU".into(), Some("dróżnik".into()), Some("FROM 1920".into()), None, None));
        // A stale screen changes nothing.
        assert!(call(&mut s, "fact.update", &json!({ "record": a, "index": 1, "tag": "RESI", "text": "x" })).is_err());
        // The wedding is a family fact.
        let fam = s.archive.doc.record(&a).unwrap().children.iter().find(|c| c.tag == "FAMS").unwrap().pointer().unwrap().to_string();
        run(&mut s, "fact.update", json!({ "record": fam, "index": 0, "tag": "MARR", "date": "14.02.1904", "place": "Łęczna" }));
        let d = s.derived();
        assert_eq!(crate::kin::marriage(d, d.index(&a).unwrap(), d.index(&b).unwrap()), (true, Some("1904".into())));
        // Deleting, and each command is one undo step.
        let depth = s.archive.undo_depth();
        run(&mut s, "fact.delete", json!({ "record": a, "index": 2, "tag": "OCCU" }));
        assert_eq!(facts(&mut s).len(), 2);
        assert_eq!(s.archive.undo_depth(), depth + 1);
        s.archive.undo();
        s.changed();
        assert_eq!(facts(&mut s).len(), 3);
        // A fact is never removed as a side effect of its form: an emptied occupation is refused ("Usuń wpis…" is the
        // way), an emptied wedding or death stays as one that happened ("MARR Y", "DEAT Y").
        assert!(call(&mut s, "fact.update", &json!({ "record": a, "index": 2, "tag": "OCCU", "text": "", "date": "", "place": "" })).is_err());
        assert_eq!(facts(&mut s).len(), 3);
        assert!(call(&mut s, "fact.add", &json!({ "record": a, "tag": "OCCU", "text": " " })).is_err(), "nothing typed, nothing added");
        run(&mut s, "fact.update", json!({ "record": fam, "index": 0, "tag": "MARR", "date": "", "place": "" }));
        let d = s.derived();
        assert_eq!(crate::kin::marriage(d, d.index(&a).unwrap(), d.index(&b).unwrap()), (true, None), "still married, date unknown");
        run(&mut s, "person.update", json!({ "id": a, "events": { "death": { "date": "1950", "place": "Lublin" } } }));
        let at = facts(&mut s).iter().position(|f| f.0 == "DEAT").unwrap();
        run(&mut s, "fact.update", json!({ "record": a, "index": at, "tag": "DEAT", "date": "", "place": "" }));
        let deat = s.archive.doc.record(&a).unwrap().children.iter().find(|c| c.tag == "DEAT").unwrap().clone();
        assert_eq!(deat.value.as_deref(), Some("Y"));
        // A form opened before an undo (`was`: what it showed) changes nothing once its fact is another.
        let at = facts(&mut s).iter().position(|f| f.1.as_deref() == Some("kolejarz")).unwrap();
        let was = json!({ "date": "", "place": "Lublin", "text": "kolejarz" });
        assert!(call(&mut s, "fact.update", &json!({ "record": a, "index": at, "tag": "OCCU", "place": "Kraków", "was": was })).is_ok());
        let err = call(&mut s, "fact.update", &json!({ "record": a, "index": at, "tag": "OCCU", "place": "Kraków", "was": was })).unwrap_err();
        assert_eq!(err.code, "stale");
        assert!(call(&mut s, "fact.delete", &json!({ "record": a, "index": at, "tag": "OCCU", "was": { "text": "dróżnik" } })).is_err());
    }

    /// Every FAM has at least two people and every link is written on both sides (CHIL ↔ FAMC, HUSB/WIFE ↔ FAMS).
    fn assert_tidy(s: &Session) {
        let doc = &s.archive.doc;
        for fam in doc.records.iter().filter(|r| r.tag == "FAM") {
            let x = fam.xref.as_deref().unwrap();
            let members: Vec<&Node> = fam.children.iter().filter(|c| matches!(c.tag.as_str(), "HUSB" | "WIFE" | "CHIL")).collect();
            assert!(members.len() >= 2, "{x} has {} people", members.len());
            for m in members {
                let back = if m.tag == "CHIL" { "FAMC" } else { "FAMS" };
                assert!(has_pointer(doc.record(m.pointer().unwrap()).unwrap(), back, x), "{x}: {} {} has no {back}", m.tag, m.pointer().unwrap());
            }
        }
        for indi in doc.records.iter().filter(|r| r.tag == "INDI") {
            let x = indi.xref.as_deref().unwrap();
            for c in indi.children.iter().filter(|c| c.tag == "FAMC" || c.tag == "FAMS") {
                let fam = doc.record(c.pointer().unwrap()).unwrap_or_else(|| panic!("{x}: {} leads nowhere", c.tag));
                let tags: &[&str] = if c.tag == "FAMC" { &["CHIL"] } else { &["HUSB", "WIFE"] };
                assert!(tags.iter().any(|t| has_pointer(fam, t, x)), "{x}: {} without its pair", c.tag);
            }
        }
    }

    #[test]
    fn relatives_are_moved_unlinked_and_refused_in_a_circle() {
        let (_dir, mut s) = session();
        let new = |s: &mut Session, given: &str, sex: &str, relation: Value| -> String {
            let mut args = json!({ "given": given, "surname": "Kowalski", "sex": sex });
            if !relation.is_null() {
                args["relation"] = relation;
            }
            run(s, "person.create", args)["id"].as_str().unwrap().to_string()
        };
        let ewa = new(&mut s, "Ewa", "F", Value::Null);
        let jan = new(&mut s, "Jan", "M", json!({ "kind": "parent", "of": ewa }));
        let maria = new(&mut s, "Maria", "F", json!({ "kind": "parent", "of": ewa }));
        let franciszek = new(&mut s, "Franciszek", "M", json!({ "kind": "sibling", "of": ewa }));
        let tomasz = new(&mut s, "Tomasz", "M", json!({ "kind": "partner", "of": ewa }));
        let zosia = new(&mut s, "Zosia", "F", json!({ "kind": "child", "of": ewa }));
        let anna = new(&mut s, "Anna", "F", json!({ "kind": "partner", "of": jan }));
        let piotr = new(&mut s, "Piotr", "M", json!({ "kind": "child", "of": jan, "otherParent": anna }));
        assert_tidy(&s);
        let parents = |s: &mut Session, x: &str| -> Vec<String> {
            let d = s.derived();
            let mut v: Vec<String> = d.info[d.index(x).unwrap()].parents.iter().map(|&p| d.xref(p).to_string()).collect();
            v.sort();
            v
        };
        let pair = |a: &str, b: &str| -> Vec<String> {
            let mut v = vec![a.to_string(), b.to_string()];
            v.sort();
            v
        };
        assert_eq!(parents(&mut s, &zosia), pair(&ewa, &tomasz));
        let fams = s.archive.doc.records.iter().filter(|r| r.tag == "FAM").count();

        // A child made a sibling: out of Ewa and Tomasz's family, into Jan and Maria's.
        run(&mut s, "relation.change", json!({ "person": ewa, "other": zosia, "kind": "sibling" }));
        assert_eq!(parents(&mut s, &zosia), pair(&jan, &maria));
        assert_tidy(&s);
        assert_eq!(s.archive.doc.records.iter().filter(|r| r.tag == "FAM").count(), fams, "Ewa and Tomasz keep their family");
        // …and back, in one undo step.
        s.archive.undo();
        s.changed();
        assert_eq!(parents(&mut s, &zosia), pair(&ewa, &tomasz));
        assert_tidy(&s);

        // A sibling made a child: into the family Ewa already has.
        run(&mut s, "relation.change", json!({ "person": ewa, "other": franciszek, "kind": "child" }));
        assert_eq!(parents(&mut s, &franciszek), pair(&ewa, &tomasz));
        assert_tidy(&s);
        s.archive.undo();
        s.changed();

        // Unlinking a partner: Tomasz leaves, the children stay with Ewa; both stay in the archive.
        let preview = run(&mut s, "relation.unlinkPreview", json!({ "person": ewa, "other": tomasz }));
        assert_eq!(preview["text"], "Ewa Kowalski i Tomasz Kowalski nie będą już parą. Ich dzieci (Zosia Kowalski) zostaną dziećmi tylko: Ewa Kowalski. Tomasz Kowalski zostaje w archiwum. Do zapisu możesz to cofnąć (Ctrl Z).");
        let before = s.archive.doc.clone();
        assert_eq!(s.archive.doc.records.len(), before.records.len(), "the preview changes nothing");
        run(&mut s, "relation.remove", json!({ "person": ewa, "other": tomasz }));
        assert_eq!(parents(&mut s, &zosia), [ewa.clone()]);
        assert!(s.derived().index(&tomasz).is_some());
        assert_tidy(&s);
        s.archive.undo();
        s.changed();

        // Unlinking a parent: Jan leaves Ewa's family, Maria stays the mother of both children.
        let preview = run(&mut s, "relation.unlinkPreview", json!({ "person": ewa, "other": jan }));
        assert!(preview["text"].as_str().unwrap().starts_with("Jan Kowalski przestanie być rodzicem: Ewa Kowalski i Franciszek Kowalski."), "{preview}");
        run(&mut s, "relation.remove", json!({ "person": ewa, "other": jan }));
        assert_eq!(parents(&mut s, &ewa), [maria.clone()]);
        assert_eq!(parents(&mut s, &franciszek), [maria.clone()]);
        assert_tidy(&s);
        s.archive.undo();
        s.changed();

        // A half-brother: Piotr leaves Jan and Anna's family (it stays: they are still a couple).
        let preview = run(&mut s, "relation.unlinkPreview", json!({ "person": ewa, "other": piotr }));
        assert!(preview["text"].as_str().unwrap().starts_with("Piotr Kowalski przestanie być dzieckiem: Jan Kowalski i Anna Kowalski."), "{preview}");
        run(&mut s, "relation.remove", json!({ "person": ewa, "other": piotr }));
        assert!(parents(&mut s, &piotr).is_empty());
        assert_tidy(&s);

        // A circle is refused and changes nothing: a granddaughter as her grandfather's mother, a person as their
        // own relative.
        let undo_depth = s.archive.undo_depth();
        let error = call(&mut s, "relation.add", &json!({ "kind": "parent", "person": jan, "other": zosia })).unwrap_err();
        assert!(error.message.contains("swoim własnym przodkiem"), "{}", error.message);
        assert!(call(&mut s, "relation.change", &json!({ "person": ewa, "other": jan, "kind": "child" })).is_ok(), "no longer her father, so he can be her son");
        s.archive.undo();
        s.changed();
        assert!(call(&mut s, "relation.change", &json!({ "person": zosia, "other": jan, "kind": "child" })).is_err(), "not directly related");
        assert!(call(&mut s, "relation.change", &json!({ "person": ewa, "other": ewa, "kind": "child" })).is_err());
        assert_eq!(s.archive.undo_depth(), undo_depth);
        assert_tidy(&s);
    }

    /// Every change and unlink between any two people of a small family keeps the families tidy and is undone
    /// exactly; a parent joining a family that already has children can't close a circle either.
    #[test]
    fn every_change_between_relatives_is_tidy_and_undone_exactly() {
        let (_dir, mut s) = session();
        let new = |s: &mut Session, given: &str, sex: &str, relation: Value| -> String {
            let mut args = json!({ "given": given, "surname": "Nowak", "sex": sex });
            if !relation.is_null() {
                args["relation"] = relation;
            }
            run(s, "person.create", args)["id"].as_str().unwrap().to_string()
        };
        let ewa = new(&mut s, "Ewa", "F", Value::Null);
        let jan = new(&mut s, "Jan", "M", json!({ "kind": "parent", "of": ewa }));
        let franciszek = new(&mut s, "Franciszek", "M", json!({ "kind": "sibling", "of": ewa }));
        let dorota = new(&mut s, "Dorota", "F", json!({ "kind": "child", "of": franciszek }));
        let tomasz = new(&mut s, "Tomasz", "M", json!({ "kind": "partner", "of": ewa, "married": true, "date": "1.1.1950" }));
        let zosia = new(&mut s, "Zosia", "F", json!({ "kind": "child", "of": ewa, "otherParent": tomasz }));
        let piotr = new(&mut s, "Piotr", "M", json!({ "kind": "child", "of": zosia }));
        assert_tidy(&s);
        // Franciszek's daughter as Ewa's mother would join Jan's family, so she'd be her own father's mother.
        let error = call(&mut s, "relation.add", &json!({ "kind": "parent", "person": ewa, "other": dorota })).unwrap_err();
        assert!(error.message.contains("swoim własnym przodkiem"), "{}", error.message);
        let people = [&ewa, &jan, &franciszek, &dorota, &tomasz, &zosia, &piotr];
        for a in people {
            for b in people {
                let mut tries = vec![("relation.remove", json!({ "person": a, "other": b }))];
                tries.extend(["parent", "partner", "child", "sibling"].map(|k| ("relation.change", json!({ "person": a, "other": b, "kind": k }))));
                for (method, args) in tries {
                    let before = s.archive.doc.records.clone();
                    let depth = s.archive.undo_depth();
                    if call(&mut s, method, &args).is_ok() && s.archive.undo_depth() > depth {
                        assert_tidy(&s);
                        assert_eq!(s.archive.undo_depth(), depth + 1, "{method} {args}: one undo step");
                        s.archive.undo();
                        s.changed();
                    }
                    assert!(s.archive.doc.records == before, "{method} {args}: as before");
                }
            }
        }
    }

    /// Adds records written by another program, as they would be read from its file.
    fn add_records(s: &mut Session, text: &str) {
        let (doc, _) = Document::from_bytes(format!("0 HEAD\n1 GEDC\n2 VERS 7.0\n{text}0 TRLR\n").as_bytes());
        for record in doc.records.into_iter().filter(|r| r.xref.is_some()) {
            s.archive.apply(Edit::Add(record)).unwrap();
        }
        s.changed();
    }

    /// What the edit form sends back when nothing was changed: `person.editData` as it came.
    fn form(s: &mut Session, id: &str) -> Value {
        let mut data = crate::people::edit_data(s.derived(), id).unwrap();
        data["id"] = json!(id);
        data
    }

    #[test]
    fn saving_the_edit_form_unchanged_changes_nothing() {
        let (_dir, mut s) = session();
        add_records(&mut s, "0 @I1@ INDI\n1 NAME Jan /Kowalski/ Jr\n2 NICK Janek\n1 NAME Johann /Kowalsky/\n2 TYPE IMMIGRANT\n\
1 NAME Jan /Kowalsky/\n1 NAME Iohannes /Kowalski/\n2 TYPE OTHER\n3 PHRASE zapis w akcie\n1 SEX X\n1 BIRT Y\n1 CHR Y\n\
1 DEAT\n2 DATE EST 1850\n1 BURI\n2 DATE INT 1851 (w lutym)\n2 PLAC Wólka\n\
0 @I2@ INDI\n1 NAME Anna /Nowak/\n2 _MARNM Kowalska\n1 SEX F\n1 BIRT\n2 DATE 1750/51\n1 DEAT\n2 DATE HEBREW 5 NSN 5650\n");
        run(&mut s, "person.create", json!({ "given": "Ewa", "surname": "Nowak" }));
        for id in ["@I1@", "@I2@"] {
            let before = s.archive.doc.record(id).unwrap().clone();
            let data = form(&mut s, id);
            run(&mut s, "person.update", data);
            assert_eq!(s.archive.doc.record(id).unwrap(), &before, "{id}");
        }

        // One field changed: only that one is written.
        let mut data = form(&mut s, "@I1@");
        data["given"] = json!("Janusz");
        run(&mut s, "person.update", data);
        let jan = s.archive.doc.record("@I1@").unwrap();
        let names: Vec<&str> = jan.children_tagged("NAME").filter_map(|n| n.value.as_deref()).collect();
        assert_eq!(names, ["Janusz /Kowalski/ Jr", "Johann /Kowalsky/", "Jan /Kowalsky/", "Iohannes /Kowalski/"], "the suffix stays, other names aren't doubled");
        assert_eq!(jan.child("NAME").unwrap().child_value("NICK"), Some("Janek"));
        assert_eq!((jan.child_value("BIRT"), jan.child_value("CHR"), jan.child_value("SEX")), (Some("Y"), Some("Y"), Some("X")));
        let mut data = form(&mut s, "@I2@");
        data["surname"] = json!("Kowalewska");
        run(&mut s, "person.update", data);
        let d = s.derived();
        let anna = d.index("@I2@").unwrap();
        assert_eq!((d.info[anna].name.as_str(), d.info[anna].maiden.as_deref()), ("Anna Kowalewska", Some("Nowak")));
        let names = s.archive.doc.record("@I2@").unwrap().children_tagged("NAME").count();
        assert_eq!(names, 1, "the married name stays in the other program's _MARNM, not a second one next to it");
    }

    #[test]
    fn links_that_lead_nowhere_are_listed_and_can_be_taken_out() {
        let (_dir, mut s) = session();
        add_records(&mut s, "0 @I1@ INDI
1 NAME Jan /Kowalski/
1 FAMS @F1@
1 OBJE @O9@
1 BIRT
2 SOUR @S9@
0 @I2@ INDI
1 NAME Anna /Nowak/
1 FAMS @F1@
0 @F1@ FAM
1 HUSB @I1@
1 WIFE @I2@
1 CHIL @I7@
1 CHIL @VOID@
");
        let broken = crate::people::broken_links(&s.archive.doc, "@I1@");
        let found: Vec<(String, String)> = broken.as_array().unwrap().iter().map(|b| (b["record"].as_str().unwrap().to_string(), b["xref"].as_str().unwrap().to_string())).collect();
        assert_eq!(found, [("@I1@".into(), "@O9@".into()), ("@I1@".into(), "@S9@".into()), ("@F1@".into(), "@I7@".into())], "@VOID@ is a legal empty link");
        assert!(call(&mut s, "person.removeBrokenLink", &json!({ "record": "@F1@", "xref": "@I2@" })).is_err(), "real people are not unlinked this way");
        run(&mut s, "person.removeBrokenLink", json!({ "record": "@F1@", "xref": "@I7@" }));
        run(&mut s, "person.removeBrokenLink", json!({ "record": "@I1@", "xref": "@S9@" }));
        let left = crate::people::broken_links(&s.archive.doc, "@I1@");
        assert_eq!(left.as_array().unwrap().len(), 1, "only the photo is left: {left}");
        assert!(s.archive.doc.record("@I1@").unwrap().child("BIRT").is_some(), "the event stays, only its citation goes");
    }

    #[test]
    fn texts_with_mentions_are_stored_by_uid() {
        let (_dir, mut s) = session();
        let a = run(&mut s, "person.create", json!({ "given": "Józef", "surname": "Kowalski" }))["id"].as_str().unwrap().to_string();
        let b = run(&mut s, "person.create", json!({ "given": "Antoni", "surname": "Kowalski" }))["id"].as_str().unwrap().to_string();
        let text = run(&mut s, "text.save", json!({ "person": a, "kind": "bio", "title": "Dzieciństwo", "body": format!("Syn [Antoniego](person:{b}).") }))["id"].as_str().unwrap().to_string();
        let d = s.derived();
        let uid = d.view.model.persons[d.index(&b).unwrap()].uid.clone().unwrap();
        let stored = s.archive.doc.record(&text).unwrap().text(heirloom_core::gedcom::Version::V7).unwrap();
        assert_eq!(stored, format!("Syn [Antoniego](person:{uid})."));
        let profile = crate::people::profile(s.derived(), &a, &[]).unwrap();
        assert_eq!(profile["bio"][0]["body"], format!("Syn [Antoniego](person:{b})."));
        run(&mut s, "text.save", json!({ "person": a, "kind": "summary", "body": "Krótko." }));
        run(&mut s, "text.save", json!({ "person": a, "kind": "summary", "body": "Krócej." }));
        let profile = crate::people::profile(s.derived(), &a, &[]).unwrap();
        assert_eq!(profile["summary"]["body"], "Krócej.", "one summary per person");
        run(&mut s, "text.delete", json!({ "id": text, "person": a }));
        assert!(s.archive.doc.record(&text).is_none());
    }
}
