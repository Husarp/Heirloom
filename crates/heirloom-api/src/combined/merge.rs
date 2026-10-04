//! One GEDCOM document, in memory only, from the archives of a set: every screen then works on it as on one archive.
//!
//! - Every record id gets the archive's key: `@I23@` of archive `b` is `@b~I23@` (UIDs too: `b~<uid>`), and relative
//!   file paths start with the key (`~b/media/x.jpg`), so the file server knows which folder they are in.
//! - People linked as the same person are one record: the first archive's record with the other records' facts added
//!   (identical lines once, differing ones both). Every pointer to another member points at it.
//! - Families with the same partners (after that) are one family too, their children once.
//! - Mentions in texts (`[x](person:<UID>)`) point straight at the person in this document.

use heirloom_core::gedcom::model::{self, Event};
use heirloom_core::gedcom::{Document, Node, Version, date};
use heirloom_core::polish;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// `@I23@` of archive `b` → `@b~I23@`.
pub fn combined_id(key: &str, xref: &str) -> String {
    format!("@{key}~{}@", xref.trim_matches('@'))
}

/// `@b~I23@` → (`b`, `@I23@`).
pub fn split_id(id: &str) -> Option<(&str, String)> {
    let inner = id.strip_prefix('@')?.strip_suffix('@')?;
    let (key, xref) = inner.split_once('~')?;
    Some((key, format!("@{xref}@")))
}

/// One archive of the set: its key and its records (already GEDCOM 7, see `Document::upgrade_to_v7`).
pub struct Input<'a> {
    pub key: &'a str,
    pub doc: &'a Document,
}

pub struct Merged {
    pub doc: Document,
    /// Per person (by id in `doc`): which inputs (indices) the person comes from.
    pub origins: HashMap<String, Vec<u8>>,
    /// Linked people: the person's id → every member's id, the person's own first, in archive order.
    pub members: HashMap<String, Vec<String>>,
    /// Each linked member's own record as it was before merging (ids already with keys), for „Różnice”.
    pub member_records: HashMap<String, Node>,
}

/// Bookkeeping lines of a record that only make sense once (the first archive's are kept).
const ONCE: [&str; 5] = ["UID", "_UID", "CHAN", "CREA", "RIN"];

fn relative_file(value: &str) -> bool {
    // Absolute paths (`C:\…`, `\\serwer\…`, `/…`) and URLs (`file:///…`, `https://…`) stay as written.
    !value.is_empty() && !value.starts_with(['/', '\\']) && !value.contains(':')
}

struct Names<'a> {
    key: &'a str,
    own: HashSet<&'a str>,
    uids: HashMap<&'a str, &'a str>,
    alias: &'a HashMap<String, String>,
}

impl Names<'_> {
    /// A pointer of this archive as an id of the merged document.
    fn map(&self, xref: &str) -> String {
        let id = combined_id(self.key, xref);
        self.alias.get(&id).cloned().unwrap_or(id)
    }

    fn walk(&self, node: &mut Node, depth: usize) {
        if let Some(xref) = node.xref.take() {
            node.xref = Some(combined_id(self.key, &xref));
        }
        if let Some(p) = node.pointer().filter(|p| self.own.contains(p)).map(str::to_string) {
            node.value = Some(self.map(&p));
        } else if let Some(value) = node.value.as_deref() {
            if depth == 1 && matches!(node.tag.as_str(), "UID" | "_UID") {
                node.value = Some(format!("{}~{}", self.key, value.trim()));
            } else if node.tag == "FILE" && relative_file(value.trim()) {
                node.value = Some(format!("~{}/{}", self.key, value.trim()));
            } else if value.contains("](person:") {
                node.value = Some(crate::text::rewrite(value, |target| {
                    let target = target.trim_start_matches('!');
                    let xref = self.uids.get(target).copied().or_else(|| self.own.contains(target).then_some(target));
                    Some(xref.map_or_else(|| target.to_string(), |x| self.map(x)))
                }));
            }
        }
        for child in &mut node.children {
            self.walk(child, depth + 1);
        }
    }
}

/// Adds `other`'s substructures to `into`: identical ones once, bookkeeping lines not, the sex only when `into` has none.
fn absorb(into: &mut Node, other: Node) {
    for child in other.children {
        if ONCE.contains(&child.tag.as_str()) {
            continue;
        }
        if child.tag == "SEX" {
            let unknown = |n: &Node| n.value.as_deref().is_none_or(|v| matches!(v.trim(), "" | "U"));
            match into.children.iter_mut().find(|c| c.tag == "SEX") {
                Some(sex) if unknown(sex) && !unknown(&child) => *sex = child,
                None if !unknown(&child) => into.children.push(child),
                _ => {}
            }
            continue;
        }
        if !into.children.contains(&child) {
            into.children.push(child);
        }
    }
}

/// `people`: every linked person's id that isn't the person's own (the first archive's record) → that id.
pub fn merge(inputs: &[Input], people: &HashMap<String, String>) -> Merged {
    // Families with the same partners (after linking) are one family; only families of different archives join.
    let mut alias = people.clone();
    let mut by_partners: HashMap<Vec<String>, (String, Vec<usize>)> = HashMap::new();
    for (k, input) in inputs.iter().enumerate() {
        let own: HashSet<&str> = input.doc.records.iter().filter_map(|r| r.xref.as_deref()).collect();
        for fam in input.doc.records.iter().filter(|r| r.tag == "FAM") {
            let Some(xref) = fam.xref.as_deref() else { continue };
            let mut partners: Vec<String> = fam
                .children
                .iter()
                .filter(|c| c.tag == "HUSB" || c.tag == "WIFE")
                .filter_map(|c| c.pointer().filter(|p| own.contains(p)))
                .map(|p| {
                    let id = combined_id(input.key, p);
                    alias.get(&id).cloned().unwrap_or(id)
                })
                .collect();
            if partners.is_empty() {
                continue;
            }
            partners.sort();
            partners.dedup();
            let id = combined_id(input.key, xref);
            match by_partners.get_mut(&partners) {
                Some((first, archives)) if !archives.contains(&k) => {
                    archives.push(k);
                    alias.insert(id, first.clone());
                }
                Some(_) => {}
                None => {
                    by_partners.insert(partners, (id, vec![k]));
                }
            }
        }
    }

    let linked: HashSet<&String> = people.iter().flat_map(|(a, b)| [a, b]).collect();
    let mut doc = Document::new_v7();
    let trailer = doc.records.pop();
    let mut position: HashMap<String, usize> = HashMap::new();
    let mut origins: HashMap<String, Vec<u8>> = HashMap::new();
    let mut members: HashMap<String, Vec<String>> = HashMap::new();
    let mut member_records = HashMap::new();
    for (k, input) in inputs.iter().enumerate() {
        let names = Names {
            key: input.key,
            own: input.doc.records.iter().filter_map(|r| r.xref.as_deref()).collect(),
            uids: input
                .doc
                .records
                .iter()
                .filter(|r| r.tag == "INDI")
                .filter_map(|r| Some((r.child_value("UID").or_else(|| r.child_value("_UID"))?.trim(), r.xref.as_deref()?)))
                .collect(),
            alias: &alias,
        };
        for record in input.doc.records.iter().filter(|r| r.xref.is_some() && r.tag != "HEAD" && r.tag != "TRLR") {
            let mut node = record.clone();
            names.walk(&mut node, 0);
            let id = node.xref.clone().unwrap_or_default();
            let person = node.tag == "INDI";
            if person && linked.contains(&id) {
                member_records.insert(id.clone(), node.clone());
            }
            match alias.get(&id).and_then(|target| Some((target.clone(), *position.get(target)?))) {
                Some((target, at)) => {
                    absorb(&mut doc.records[at], node);
                    if person {
                        origins.entry(target.clone()).or_default().push(k as u8);
                        members.entry(target).or_default().push(id);
                    }
                }
                None => {
                    if person {
                        origins.insert(id.clone(), vec![k as u8]);
                        if linked.contains(&id) {
                            members.insert(id.clone(), vec![id.clone()]);
                        }
                    }
                    position.insert(id, doc.records.len());
                    doc.records.push(node);
                }
            }
        }
    }
    doc.records.extend(trailer);
    members.retain(|_, list| list.len() > 1);
    Merged { doc, origins, members, member_records }
}

fn event_text(event: &Option<Event>) -> String {
    let Some(e) = event else { return String::new() };
    let date = e.date_text.as_deref().map(|t| date::parse(t).map_or_else(|| t.trim_matches(['(', ')']).to_string(), |d| polish::format_date(&d)));
    [date, e.place.clone()].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ")
}

/// „1878–1951”, „ur. 1878”, „zm. 1951” for a record outside the merged document.
pub fn years(person: &model::Person) -> String {
    let year = |e: &Option<Event>| e.as_ref().and_then(|e| e.date.as_ref()).map(polish::year_text);
    match (year(&person.birth), year(&person.death)) {
        (Some(b), Some(d)) => format!("{b}–{d}"),
        (Some(b), None) => format!("ur. {b}"),
        (None, Some(d)) => format!("zm. {d}"),
        (None, None) => String::new(),
    }
}

/// Where the members' own records disagree: name, sex, birth, death. Each value with its archive's key; „—” where an
/// archive says nothing.
pub fn differences(members: &[(&str, &Node)]) -> Value {
    let people: Vec<(&str, model::Person)> =
        members.iter().map(|(key, node)| (*key, model::person(node, node.xref.as_deref().unwrap_or(""), Version::V7))).collect();
    fn sex(p: &model::Person) -> String {
        match p.sex.code() {
            "M" => "mężczyzna",
            "F" => "kobieta",
            "X" => "inna",
            _ => "",
        }
        .to_string()
    }
    let rows: [(&str, fn(&model::Person) -> String); 4] = [
        ("Imię i nazwisko", |p| p.display_name()),
        ("Płeć", sex),
        ("Urodzenie", |p| event_text(&p.birth)),
        ("Śmierć", |p| event_text(&p.death)),
    ];
    let mut out = Vec::new();
    for (label, value) in rows {
        let values: Vec<(&str, String)> = people.iter().map(|(key, p)| (*key, value(p))).collect();
        let given: HashSet<&String> = values.iter().map(|(_, v)| v).filter(|v| !v.is_empty()).collect();
        let some_empty = values.iter().any(|(_, v)| v.is_empty());
        if given.len() > 1 || (!given.is_empty() && some_empty) {
            let values: Vec<Value> = values.iter().map(|(key, v)| json!({ "archive": key, "text": if v.is_empty() { "—" } else { v } })).collect();
            out.push(json!({ "label": label, "values": values }));
        }
    }
    json!(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Jan /Kowalski/\n1 SEX M\n1 UID u-jan\n1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka\n1 FAMS @F1@\n1 FAMC @F9@\n1 OBJE @O1@\n\
0 @I2@ INDI\n1 NAME Anna /Kowalska/\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Piotr /Kowalski/\n1 SEX M\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n\
0 @O1@ OBJE\n1 FILE media/jan.jpg\n2 FORM image/jpeg\n\
0 @N1@ SNOTE O [dziadku](person:u-jan) i [Piotrze](person:@I3@).\n0 TRLR\n";
    const B: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Jan /Kowalski/\n1 SEX U\n1 UID u-jan-b\n1 BIRT\n2 DATE 1878\n2 PLAC Wólka Dolna\n1 OCCU kowal\n1 FAMS @F2@\n\
0 @I7@ INDI\n1 NAME Anna /Kowalska/\n1 SEX F\n1 FAMS @F2@\n\
0 @I8@ INDI\n1 NAME Zofia /Kowalska/\n1 SEX F\n1 FAMC @F2@\n\
0 @F2@ FAM\n1 HUSB @I1@\n1 WIFE @I7@\n1 CHIL @I8@\n1 MARR\n2 DATE 1900\n\
0 @O1@ OBJE\n1 FILE C:\\zdjecia\\x.jpg\n\
0 @N1@ SNOTE Wspomnienie o [Janie](person:u-jan-b).\n0 TRLR\n";

    fn docs() -> (Document, Document) {
        (Document::from_bytes(A.as_bytes()).0, Document::from_bytes(B.as_bytes()).0)
    }

    #[test]
    fn unrelated_archives_keep_their_people_apart() {
        let (a, b) = docs();
        let m = merge(&[Input { key: "a", doc: &a }, Input { key: "b", doc: &b }], &HashMap::new());
        let people = m.doc.records.iter().filter(|r| r.tag == "INDI").count();
        assert_eq!(people, 6);
        let jan_b = m.doc.record("@b~I1@").unwrap();
        assert_eq!(jan_b.child_value("FAMS"), Some("@b~F2@"));
        assert_eq!(jan_b.child_value("UID"), Some("b~u-jan-b"));
        assert_eq!(m.doc.record("@a~I1@").unwrap().child_value("FAMC"), Some("@F9@"), "a pointer to nothing stays as it was");
        assert_eq!(m.doc.record("@a~O1@").unwrap().child_value("FILE"), Some("~a/media/jan.jpg"));
        assert_eq!(m.doc.record("@b~O1@").unwrap().child_value("FILE"), Some("C:\\zdjecia\\x.jpg"));
        assert_eq!(m.doc.record("@a~N1@").unwrap().value.as_deref(), Some("O [dziadku](person:@a~I1@) i [Piotrze](person:@a~I3@)."));
        assert_eq!(m.origins["@b~I8@"], vec![1]);
        assert!(m.members.is_empty());
    }

    #[test]
    fn a_linked_person_is_one_record_with_both_archives_facts() {
        let (a, b) = docs();
        let alias: HashMap<String, String> = [("@b~I1@", "@a~I1@"), ("@b~I7@", "@a~I2@")].iter().map(|(x, y)| (x.to_string(), y.to_string())).collect();
        let m = merge(&[Input { key: "a", doc: &a }, Input { key: "b", doc: &b }], &alias);
        assert_eq!(m.doc.records.iter().filter(|r| r.tag == "INDI").count(), 4);
        assert!(m.doc.record("@b~I1@").is_none());
        let jan = m.doc.record("@a~I1@").unwrap();
        assert_eq!(jan.children_tagged("NAME").count(), 1, "the same name once");
        assert_eq!(jan.children_tagged("BIRT").count(), 2, "two different births stay");
        assert_eq!(jan.child_value("OCCU"), Some("kowal"));
        assert_eq!((jan.child_value("SEX"), jan.children_tagged("UID").count()), (Some("M"), 1));
        assert_eq!(jan.children_tagged("FAMS").count(), 1, "the same partners: one family");
        let family = m.doc.record("@a~F1@").unwrap();
        let children: Vec<&str> = family.children_tagged("CHIL").filter_map(|c| c.value.as_deref()).collect();
        assert_eq!(children, ["@a~I3@", "@b~I8@"]);
        assert_eq!((family.children_tagged("HUSB").count(), family.child("MARR").is_some()), (1, true));
        assert!(m.doc.record("@b~F2@").is_none());
        assert_eq!(m.doc.record("@b~I8@").unwrap().child_value("FAMC"), Some("@a~F1@"));
        assert_eq!(m.doc.record("@b~N1@").unwrap().value.as_deref(), Some("Wspomnienie o [Janie](person:@a~I1@)."), "a mention follows the link");
        assert_eq!((m.origins["@a~I1@"].clone(), m.members["@a~I1@"].clone()), (vec![0, 1], vec!["@a~I1@".to_string(), "@b~I1@".to_string()]));
        let records: Vec<(&str, &Node)> = m.members["@a~I1@"].iter().map(|id| (split_id(id).unwrap().0, &m.member_records[id])).collect();
        let diff = differences(&records);
        let labels: Vec<&str> = diff.as_array().unwrap().iter().filter_map(|d| d["label"].as_str()).collect();
        assert_eq!(labels, ["Płeć", "Urodzenie"]);
        assert_eq!(diff[1]["values"][0]["text"], "12 marca 1878, Wólka");
        assert_eq!(diff[1]["values"][1]["text"], "1878, Wólka Dolna");
        assert_eq!(diff[0]["values"][1]["text"], "—");
    }

    #[test]
    fn ids_go_back_to_their_archive() {
        assert_eq!(combined_id("b", "@I23@"), "@b~I23@");
        assert_eq!(split_id("@b~I23@"), Some(("b", "@I23@".to_string())));
        assert_eq!(split_id("@I23@"), None);
    }
}
