//! The change history as people read it: which fields changed ("Przydomek: — → „Dziadek Józek”"), grouped into
//! editing sessions and imports, for the profile's "Historia zmian" and the archive-wide feed (spec §4.7, §4.21).

use crate::derive::{DateInfo, Derived};
use heirloom_core::gedcom::view::{self, TextKind};
use heirloom_core::gedcom::{Document, Node};
use heirloom_core::history::{Action, Entry};
use serde_json::{Value, json};

const SESSION_GAP_MINUTES: i64 = 30;

/// Minutes since the epoch from an RFC 3339 timestamp (only for comparing gaps).
fn minutes(ts: &str) -> i64 {
    let parse = |a: usize, b: usize| ts.get(a..b).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let (y, mo, d, h, mi) = (parse(0, 4), parse(5, 7), parse(8, 10), parse(11, 13), parse(14, 16));
    // Days from a civil date (Howard Hinnant's algorithm).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    (days * 24 + h) * 60 + mi
}

fn parse_record(text: &str) -> Option<Node> {
    let (doc, _) = Document::from_bytes(text.as_bytes());
    doc.records.into_iter().find(|r| r.tag != "HEAD" && r.tag != "TRLR")
}

/// Readable fields of a record version, as (field, value) pairs.
fn fields(d: &Derived, text: Option<&str>) -> Vec<(String, String)> {
    let Some(text) = text else { return Vec::new() };
    let Some(record) = parse_record(text) else { return Vec::new() };
    let mut doc = Document::new_v7();
    let trailer = doc.records.pop();
    doc.records.push(record.clone());
    doc.records.extend(trailer);
    let v = view::build(&doc);
    let mut out = Vec::new();
    match record.tag.as_str() {
        "INDI" => {
            let (Some(p), Some(details)) = (v.model.persons.first(), v.people.first()) else { return out };
            for name in &p.names {
                let label = match name.kind {
                    heirloom_core::gedcom::model::NameKind::Birth => "Imię i nazwisko",
                    heirloom_core::gedcom::model::NameKind::Married => "Nazwisko po ślubie",
                    heirloom_core::gedcom::model::NameKind::Other => "Inne nazwisko",
                };
                out.push((label.to_string(), format!("{} {}", name.given, name.surname).trim().to_string()));
                if let Some(nick) = &name.nickname {
                    out.push(("Przydomek".into(), format!("„{nick}”")));
                }
            }
            out.push(("Płeć".into(), sex_word(p.sex.code()).into()));
            let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for f in &details.facts {
                let base = crate::people::event_name(&f.tag, f.kind.as_deref());
                let n = counts.entry(base.clone()).or_default();
                *n += 1;
                let label = if *n > 1 { format!("{base} ({n})") } else { base };
                if let Some(value) = &f.value {
                    out.push((label.clone(), value.clone()));
                }
                if let Some(date) = DateInfo::from_fact(f) {
                    out.push((format!("{label} · data"), date.text));
                } else if f.only_happened {
                    out.push((label.clone(), "tak".into()));
                }
                if let Some(place) = &f.place {
                    out.push((format!("{label} · miejsce"), place.clone()));
                }
            }
            for link in &details.media {
                let title = link.title.clone().or_else(|| d.view.media.get(&link.object).and_then(|m| m.title.clone())).unwrap_or_else(|| link.object.clone());
                let kind = d.view.media.get(&link.object).map_or("Plik", |m| if m.kind == view::MediaKind::Photo { "Zdjęcie" } else { "Dokument" });
                out.push((kind.into(), title));
            }
            for t in &details.texts {
                if let view::TextRef::Shared(x) = t {
                    let (kind, title) = d.view.texts.get(x).map_or(("Tekst", x.clone()), |t| (text_kind_word(t.kind), t.title.clone().unwrap_or_else(|| short(&t.body))));
                    out.push((kind.into(), title));
                }
            }
            for tag in &details.tags {
                out.push(("Etykieta".into(), tag.clone()));
            }
            for link in &details.links {
                out.push(("Link".into(), link.title.clone().unwrap_or_else(|| link.url.clone())));
            }
        }
        "FAM" => {
            if let (Some(f), Some(details)) = (v.model.families.first(), v.families.first()) {
                for partner in &f.partners {
                    out.push(("Partner".into(), person_name(d, partner)));
                }
                for child in &f.children {
                    out.push(("Dziecko".into(), person_name(d, child)));
                }
                for fact in &details.facts {
                    let label = crate::people::event_name(&fact.tag, fact.kind.as_deref());
                    if let Some(date) = DateInfo::from_fact(fact) {
                        out.push((format!("{label} · data"), date.text));
                    }
                    if let Some(place) = &fact.place {
                        out.push((format!("{label} · miejsce"), place.clone()));
                    }
                }
            }
        }
        "SNOTE" | "NOTE" => {
            if let Some(t) = v.texts.values().next() {
                let kind = text_kind_word(t.kind);
                if let Some(title) = &t.title {
                    out.push((format!("{kind} · tytuł"), title.clone()));
                }
                out.push((kind.into(), short(&t.body)));
            }
        }
        "OBJE" => {
            if let Some(m) = v.media.values().next() {
                if let Some(title) = &m.title {
                    out.push(("Podpis".into(), title.clone()));
                }
                if let Some(file) = &m.file {
                    out.push(("Plik".into(), file.clone()));
                }
                if m.transcription.is_some() {
                    out.push(("Transkrypcja".into(), "jest".into()));
                }
            }
        }
        "SOUR" => {
            if let Some(s) = v.sources.values().next() {
                out.push(("Źródło".into(), s.title.clone().unwrap_or_default()));
            }
        }
        _ => {}
    }
    out
}

fn short(text: &str) -> String {
    let plain = crate::text::plain(text);
    let chars: Vec<char> = plain.chars().collect();
    if chars.len() > 60 { format!("{}…", chars[..60].iter().collect::<String>().trim_end()) } else { plain }
}

fn sex_word(code: &str) -> &'static str {
    match code {
        "M" => "mężczyzna",
        "F" => "kobieta",
        "X" => "inna",
        _ => "nieznana",
    }
}

pub fn text_kind_word(kind: TextKind) -> &'static str {
    match kind {
        TextKind::Summary => "W skrócie",
        TextKind::Bio => "Życiorys",
        TextKind::Story => "Historia",
        TextKind::Saying => "Powiedzonko",
        TextKind::Trivia => "Ciekawostka",
        TextKind::Note => "Uwaga",
    }
}

fn person_name(d: &Derived, xref: &str) -> String {
    d.index(xref).map_or_else(|| xref.to_string(), |i| d.info[i].name.clone())
}

/// Field-level changes between two versions of a record.
pub fn changes(d: &Derived, entry: &Entry) -> Vec<Value> {
    let before = fields(d, entry.before.as_deref());
    let after = fields(d, entry.after.as_deref());
    let mut out = Vec::new();
    let mut used = vec![false; after.len()];
    for (label, old) in &before {
        match after.iter().enumerate().find(|(j, (l, _))| !used[*j] && l == label) {
            Some((j, (_, new))) => {
                used[j] = true;
                if old != new {
                    out.push(json!({ "field": label, "old": old, "new": new }));
                }
            }
            None => out.push(json!({ "field": label, "old": old, "new": Value::Null })),
        }
    }
    for (j, (label, new)) in after.iter().enumerate() {
        if !used[j] && !before.iter().any(|(l, v)| l == label && v == new) {
            out.push(json!({ "field": label, "old": Value::Null, "new": new }));
        }
    }
    out
}

/// What an entry is about: the person it belongs to (for texts and media, the person who links them).
fn subject(d: &Derived, entry: &Entry) -> (Option<usize>, String) {
    if let Some(i) = d.index(&entry.record) {
        return (Some(i), d.info[i].name.clone());
    }
    let from_text = |text: &Option<String>| -> Option<String> {
        let node = parse_record(text.as_deref()?)?;
        match node.tag.as_str() {
            "INDI" => {
                let name = node.child("NAME")?.value.clone()?;
                Some(name.replace('/', "").split_whitespace().collect::<Vec<_>>().join(" "))
            }
            "FAM" => {
                let names: Vec<String> = node.children.iter().filter(|c| c.tag == "HUSB" || c.tag == "WIFE").filter_map(|c| c.pointer()).map(|p| person_name(d, p)).collect();
                Some(names.join(" i "))
            }
            "SNOTE" | "NOTE" => node.child("_HLM_TITLE").and_then(|t| t.value.clone()),
            "OBJE" => node.child("FILE").and_then(|f| f.child("TITL")).and_then(|t| t.value.clone()),
            "SOUR" => node.child("TITL").and_then(|t| t.value.clone()),
            _ => None,
        }
    };
    let owner = d.view.people.iter().position(|p| {
        p.texts.iter().any(|t| matches!(t, view::TextRef::Shared(x) if *x == entry.record))
            || p.media.iter().any(|m| m.object == entry.record)
    });
    if let Some(o) = owner {
        return (Some(o), d.info[o].name.clone());
    }
    (None, from_text(&entry.after).or_else(|| from_text(&entry.before)).unwrap_or_else(|| entry.record.clone()))
}

fn kind_of(entry: &Entry) -> &'static str {
    let tag = entry.tag.as_str();
    match (entry.action, tag) {
        (_, _) if entry.batch.is_some() => "import",
        (Action::Delete, _) => "deleted",
        (_, "OBJE") => "media",
        (_, "FAM") => "relation",
        _ => "person",
    }
}

fn action_words(entry: &Entry) -> (&'static str, &'static str) {
    // (icon, action text)
    match (entry.action, entry.tag.as_str()) {
        (Action::Create, "INDI") => ("user-plus", "Dodano osobę"),
        (Action::Update, "INDI") => ("pencil", "Edytowano"),
        (Action::Delete, "INDI") => ("trash-2", "Usunięto osobę"),
        (Action::Create, "FAM") => ("link", "Dodano relację"),
        (Action::Update, "FAM") => ("link", "Zmieniono relację"),
        (Action::Delete, "FAM") => ("trash-2", "Usunięto relację"),
        (Action::Create, "SNOTE" | "NOTE") => ("book-open", "Dodano tekst"),
        (Action::Update, "SNOTE" | "NOTE") => ("book-open", "Edytowano tekst"),
        (Action::Delete, "SNOTE" | "NOTE") => ("trash-2", "Usunięto tekst"),
        (Action::Create, "OBJE") => ("image-plus", "Dodano plik"),
        (Action::Update, "OBJE") => ("image", "Edytowano plik"),
        (Action::Delete, "OBJE") => ("trash-2", "Usunięto plik"),
        (Action::Create, "SOUR") => ("library", "Dodano źródło"),
        (Action::Update, "SOUR") => ("library", "Edytowano źródło"),
        (Action::Delete, "SOUR") => ("trash-2", "Usunięto źródło"),
        (_, "HEAD") => ("file-code", "Zmieniono nagłówek pliku"),
        (Action::Create, _) => ("plus", "Dodano rekord"),
        (Action::Update, _) => ("pencil", "Edytowano rekord"),
        (Action::Delete, _) => ("trash-2", "Usunięto rekord"),
    }
}

/// Whether the record is still as the entry left it (so undoing it won't overwrite later changes).
fn still_current(archive_records: &[Node], entry: &Entry) -> bool {
    let current = archive_records.iter().find(|r| r.xref.as_deref() == Some(entry.record.as_str()));
    match (&entry.after, current) {
        (None, None) => true,
        (Some(after), Some(node)) => heirloom_core::history::record_text(node) == *after,
        _ => false,
    }
}

/// The archive-wide feed (spec §4.21): newest first, grouped by day, imports collapsed to one row.
pub fn feed(d: &Derived, entries: &[Entry], records: &[Node], filter: &str, limit: usize) -> Value {
    let mut rows: Vec<Value> = Vec::new();
    let mut seen_batches: Vec<String> = Vec::new();
    for (index, entry) in entries.iter().enumerate().rev() {
        if entry.tag == "HEAD" {
            continue;
        }
        let kind = kind_of(entry);
        if filter != "all" && filter != kind {
            continue;
        }
        if let Some(batch) = &entry.batch {
            if seen_batches.contains(batch) {
                continue;
            }
            seen_batches.push(batch.clone());
            let all: Vec<&Entry> = entries.iter().filter(|e| e.batch.as_deref() == Some(batch)).collect();
            let people = all.iter().filter(|e| e.tag == "INDI" && e.action == Action::Create).count();
            let updated = all.iter().filter(|e| e.tag == "INDI" && e.action == Action::Update).count();
            let files = all.iter().filter(|e| e.tag == "OBJE" && e.action == Action::Create).count();
            let mut detail = Vec::new();
            if people > 0 {
                detail.push(crate::count_pl(people, "nowa osoba", "nowe osoby", "nowych osób"));
            }
            if updated > 0 {
                detail.push(crate::count_pl(updated, "zaktualizowana", "zaktualizowane", "zaktualizowanych"));
            }
            if files > 0 {
                detail.push(crate::count_pl(files, "plik", "pliki", "plików"));
            }
            rows.push(json!({
                "index": index,
                "ts": entry.ts,
                "icon": "import",
                "action": "Zaimportowano paczkę",
                "subject": format!("„{batch}”"),
                "subjectId": Value::Null,
                "detail": detail.join(", "),
                "who": entry.author,
                "batch": batch,
                "note": entry.note,
                "undo": "Cofnij import",
            }));
        } else {
            let (person, name) = subject(d, entry);
            let (icon, action) = action_words(entry);
            let changes = changes(d, entry);
            let detail = changes
                .iter()
                .take(2)
                .map(|c| {
                    let old = c["old"].as_str().map_or("puste".to_string(), str::to_string);
                    let new = c["new"].as_str().map_or("usunięto".to_string(), str::to_string);
                    format!("{}: {old} → {new}", c["field"].as_str().unwrap_or(""))
                })
                .collect::<Vec<_>>()
                .join(" · ");
            rows.push(json!({
                "index": index,
                "ts": entry.ts,
                "icon": icon,
                "action": action,
                "subject": name,
                "subjectId": person.map(|i| d.xref(i).to_string()),
                "detail": detail,
                "who": entry.author,
                "batch": Value::Null,
                "note": entry.note,
                "undo": if entry.action == Action::Delete { "Przywróć" } else { "Cofnij" },
                "current": still_current(records, entry),
            }));
        }
        if rows.len() >= limit {
            break;
        }
    }
    json!({ "rows": rows, "total": entries.len() })
}

/// The history of one person (their record, families, texts and files), in sessions (spec §4.7 #7).
pub fn person_history(d: &Derived, entries: &[Entry], xref: &str) -> Value {
    let Some(i) = d.index(xref) else { return json!({ "origin": null, "groups": [] }) };
    let p = &d.view.model.persons[i];
    let details = &d.view.people[i];
    let mut records: Vec<String> = vec![xref.to_string()];
    records.extend(p.fams.iter().cloned());
    for t in &details.texts {
        if let view::TextRef::Shared(x) = t {
            records.push(x.clone());
        }
    }
    records.extend(details.media.iter().map(|m| m.object.clone()));

    let origin = entries.iter().find(|e| e.record == xref && e.action == Action::Create).map(|e| {
        json!({ "ts": e.ts, "batch": e.batch, "who": e.author })
    });
    let mine: Vec<(usize, &Entry)> = entries.iter().enumerate().filter(|(_, e)| records.contains(&e.record)).collect();
    let mut groups: Vec<Value> = Vec::new();
    let mut current: Vec<(usize, &Entry)> = Vec::new();
    let flush = |current: &mut Vec<(usize, &Entry)>, groups: &mut Vec<Value>| {
        if current.is_empty() {
            return;
        }
        let first = current[0].1;
        let last = current[current.len() - 1].1;
        let lines: Vec<Value> = current
            .iter()
            .flat_map(|(index, e)| {
                let list = changes(d, e);
                let list = if list.is_empty() {
                    vec![json!({ "field": action_words(e).1, "old": Value::Null, "new": Value::Null })]
                } else {
                    list
                };
                list.into_iter().map(move |mut c| {
                    c["index"] = json!(index);
                    c
                })
            })
            .collect();
        groups.push(json!({
            "from": first.ts,
            "to": last.ts,
            "who": first.author,
            "batch": first.batch,
            "kind": if first.batch.is_some() { "import" } else { "manual" },
            "count": lines.len(),
            "lines": lines,
        }));
        current.clear();
    };
    for (index, entry) in mine {
        let same = current.last().is_some_and(|(_, last)| {
            last.author == entry.author && last.batch == entry.batch && minutes(&entry.ts) - minutes(&last.ts) <= SESSION_GAP_MINUTES
        });
        if !same {
            flush(&mut current, &mut groups);
        }
        current.push((index, entry));
    }
    flush(&mut current, &mut groups);
    groups.reverse();
    json!({ "origin": origin, "groups": groups })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_between_timestamps() {
        assert_eq!(minutes("2026-09-28T10:42:00Z") - minutes("2026-09-28T10:38:00Z"), 4);
        assert_eq!(minutes("2026-03-01T00:00:00Z") - minutes("2026-02-28T23:00:00Z"), 60);
    }

    #[test]
    fn field_changes_in_words() {
        let doc = Document::from_bytes(b"0 HEAD\n0 TRLR\n").0;
        let d = crate::derive::build(&doc);
        let entry = Entry {
            ts: "2026-09-28T10:42:00Z".into(),
            author: "Ewa".into(),
            batch: None,
            action: Action::Update,
            record: "@I1@".into(),
            tag: "INDI".into(),
            before: Some("0 @I1@ INDI\n1 NAME Józef /Kowalski/\n1 BIRT\n2 DATE 1878\n".into()),
            after: Some("0 @I1@ INDI\n1 NAME Józef /Kowalski/\n2 NICK Dziadek Józek\n1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka\n".into()),
            note: None,
        };
        let list = changes(&d, &entry);
        assert_eq!(list[0], json!({ "field": "Urodzenie · data", "old": "1878", "new": "12 marca 1878" }));
        assert!(list.contains(&json!({ "field": "Przydomek", "old": null, "new": "„Dziadek Józek”" })));
        assert!(list.contains(&json!({ "field": "Urodzenie · miejsce", "old": null, "new": "Wólka" })));
    }
}
