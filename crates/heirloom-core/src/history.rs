//! The change history, `.heirloom/historia.jsonl`: one JSON line per record changed by a save — who, when,
//! and the record before and after as GEDCOM text — so every change can be shown and undone later.

use crate::gedcom::{Eol, Node, line};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// UTC, RFC 3339.
    pub ts: String,
    /// The name given in "Kto edytuje?", or the import batch's author.
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<String>,
    pub action: Action,
    /// `@I12@`, or the tag for records without an identifier (`HEAD`).
    pub record: String,
    pub tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// A note given with the save (the Import's "Notatka do historii zmian").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The records that differ between two versions of a file, as history entries.
pub fn diff(old: &[Node], new: &[Node], author: &str, batch: Option<&str>) -> Vec<Entry> {
    let ts = now();
    let (old, new) = (keyed(old), keyed(new));
    let old_by_key: HashMap<&Key, &Node> = old.iter().map(|(k, n)| (k, *n)).collect();
    let new_keys: HashSet<&Key> = new.iter().map(|(k, _)| k).collect();
    let make = |action, record: String, tag: &str, before: Option<&Node>, after: Option<&Node>| Entry {
        ts: ts.clone(),
        author: author.to_string(),
        batch: batch.map(str::to_string),
        action,
        record,
        tag: tag.to_string(),
        before: before.map(record_text),
        after: after.map(record_text),
        note: None,
    };
    let mut entries = Vec::new();
    for &(ref k, n) in &new {
        match old_by_key.get(k).copied() {
            None => entries.push(make(Action::Create, k.0.clone(), &n.tag, None, Some(n))),
            Some(o) if o != n => entries.push(make(Action::Update, k.0.clone(), &n.tag, Some(o), Some(n))),
            Some(_) => {}
        }
    }
    for &(ref k, o) in &old {
        if !new_keys.contains(k) {
            entries.push(make(Action::Delete, k.0.clone(), &o.tag, Some(o), None));
        }
    }
    entries
}

/// A record's xref (or its tag when it has none, like `HEAD`) and which occurrence of that key it is.
type Key = (String, usize);

/// Numbers repeated keys in file order, so they are compared pairwise: some programs write many records with
/// the same tag and no xref (Legacy's `0 _PLAC_DEFN`), and broken files repeat xrefs. Keyed only by name, every
/// save would log all but one of them as changed.
fn keyed(records: &[Node]) -> Vec<(Key, &Node)> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    records
        .iter()
        .map(|n| {
            let name = n.xref.as_deref().unwrap_or(&n.tag);
            let count = seen.entry(name).or_default();
            *count += 1;
            ((name.to_string(), *count), n)
        })
        .collect()
}

/// A record as GEDCOM text (LF line breaks).
pub fn record_text(node: &Node) -> String {
    line::write(std::slice::from_ref(node), Eol::Lf, false)
}

/// The current time, UTC, RFC 3339.
pub fn now() -> String {
    time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

/// Adds entries to the end of the history file (created if missing).
pub fn append(path: &Path, entries: &[Entry]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut lines = String::new();
    for entry in entries {
        lines.push_str(&serde_json::to_string(entry).map_err(std::io::Error::other)?);
        lines.push('\n');
    }
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(lines.as_bytes())?;
    file.sync_all()
}

/// All entries, oldest first. A missing file means no history yet. Lines that can't be read (e.g. half-written
/// during a crash) are skipped, so one bad line doesn't hide the whole history.
pub fn read(path: &Path) -> std::io::Result<Vec<Entry>> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    Ok(String::from_utf8_lossy(&bytes).lines().filter_map(|line| serde_json::from_str(line).ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indi(xref: &str, name: &str) -> Node {
        Node::new("INDI").with_xref(xref).with_child(Node::with_value("NAME", name))
    }

    #[test]
    fn diff_finds_created_updated_and_deleted_records() {
        let old = vec![Node::new("HEAD"), indi("@I1@", "Jan /Nowak/"), indi("@I2@", "Anna /Nowak/"), Node::new("TRLR")];
        let new = vec![Node::new("HEAD"), indi("@I1@", "Jan /Nowakowski/"), indi("@I3@", "Ewa /Nowak/"), Node::new("TRLR")];
        let entries = diff(&old, &new, "Ewa", Some("B1"));
        let summary: Vec<_> = entries.iter().map(|e| (e.action, e.record.as_str())).collect();
        assert_eq!(summary, [(Action::Update, "@I1@"), (Action::Create, "@I3@"), (Action::Delete, "@I2@")]);
        assert_eq!(entries[0].before.as_deref(), Some("0 @I1@ INDI\n1 NAME Jan /Nowak/"));
        assert_eq!(entries[0].after.as_deref(), Some("0 @I1@ INDI\n1 NAME Jan /Nowakowski/"));
        assert_eq!((entries[1].author.as_str(), entries[1].batch.as_deref()), ("Ewa", Some("B1")));
    }

    #[test]
    fn records_sharing_a_key_are_compared_in_order() {
        let plac = |name: &str| Node::new("_PLAC_DEFN").with_child(Node::with_value("PLAC", name));
        let old = vec![Node::new("HEAD"), plac("Warszawa"), plac("Kraków"), plac("Łódź"), indi("@I1@", "Jan /Nowak/")];
        assert!(diff(&old, &old, "Ewa", None).is_empty(), "nothing changed, nothing logged");
        let mut new = old.clone();
        new[2] = plac("Krakow");
        new.push(indi("@I1@", "Jan /Kowalski/"));
        let summary: Vec<_> = diff(&old, &new, "Ewa", None).into_iter().map(|e| (e.action, e.record)).collect();
        assert_eq!(
            summary,
            [(Action::Update, "_PLAC_DEFN".to_string()), (Action::Create, "@I1@".to_string())],
            "only the edited place; the repeated xref counts as a new record"
        );
    }

    #[test]
    fn append_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".heirloom").join("historia.jsonl");
        assert!(read(&path).unwrap().is_empty());
        let entries = diff(&[], &[indi("@I1@", "Jan /Nowak/")], "Ewa", None);
        append(&path, &entries).unwrap();
        append(&path, &entries).unwrap();
        assert_eq!(read(&path).unwrap().len(), 2);
    }
}
