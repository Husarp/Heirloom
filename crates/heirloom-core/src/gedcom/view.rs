//! Everything the app shows about people beyond names and dates (those are in `model.rs`): all life events,
//! texts (biography, stories, sayings, trivia, notes), photos and documents, sources, links, tags — and the data
//! Heirloom doesn't understand, shown as "Inne dane z pliku". The storage rules are in docs/GEDCOM_EXTENSIONS.md.

use super::date::{self, DateValue};
use super::model::{self, Model};
use super::{Document, Node, Version};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certainty {
    High,
    Medium,
    Low,
}

impl Certainty {
    pub fn code(self) -> &'static str {
        match self {
            Certainty::High => "high",
            Certainty::Medium => "medium",
            Certainty::Low => "low",
        }
    }

    pub fn from_code(code: &str) -> Option<Certainty> {
        match code.trim() {
            "high" => Some(Certainty::High),
            "medium" => Some(Certainty::Medium),
            "low" => Some(Certainty::Low),
            _ => None,
        }
    }
}

/// A source cited for a fact, a name or a person: `SOUR @S1@` + `PAGE`, or 5.5.1 free text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    /// The source record (`@S1@`); none for a free-text citation.
    pub source: Option<String>,
    pub page: Option<String>,
    /// Free text of a citation without a source record.
    pub text: Option<String>,
    /// GEDCOM QUAY 0–3.
    pub quality: Option<u8>,
}

/// Another person in an event: a godparent, a witness, the father at a baptism (`ASSO` + `ROLE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Associate {
    pub person: String,
    /// ROLE code (`GODP`, `WITN`, …), or the text of `OTHER` + `PHRASE`.
    pub role: String,
    pub phrase: Option<String>,
    pub age: Option<String>,
}

/// One life event or fact of a person or a family (`BIRT`, `OCCU`, `EVEN` + `TYPE`, `MARR`, …).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fact {
    pub tag: String,
    /// `TYPE` of `EVEN`/`FACT` (the event's name), or the classification of another event.
    pub kind: Option<String>,
    /// The line value: the occupation, the residence, the religion…
    pub value: Option<String>,
    pub date_text: Option<String>,
    pub date: Option<DateValue>,
    /// The date as written in the source (`DATE.PHRASE`).
    pub date_phrase: Option<String>,
    /// The Julian date of a double-dated record (`_HLM_JULIAN`).
    pub julian: Option<String>,
    pub place: Option<String>,
    pub place_note: Option<String>,
    pub place_orig: Option<String>,
    pub cause: Option<String>,
    pub note: Option<String>,
    /// The principal's age as written.
    pub age: Option<String>,
    pub certainty: Option<Certainty>,
    pub inferred: bool,
    pub citations: Vec<Citation>,
    pub associates: Vec<Associate>,
    /// `Y` only ("it happened, details unknown"), e.g. `DEAT Y`.
    pub only_happened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Summary,
    Bio,
    Story,
    Saying,
    Trivia,
    /// A research note, or any note from another program.
    Note,
}

impl TextKind {
    pub fn code(self) -> &'static str {
        match self {
            TextKind::Summary => "summary",
            TextKind::Bio => "bio",
            TextKind::Story => "story",
            TextKind::Saying => "saying",
            TextKind::Trivia => "trivia",
            TextKind::Note => "note",
        }
    }

    pub fn from_code(code: &str) -> Option<TextKind> {
        Some(match code.trim() {
            "summary" => TextKind::Summary,
            "bio" => TextKind::Bio,
            "story" => TextKind::Story,
            "saying" => TextKind::Saying,
            "trivia" => TextKind::Trivia,
            "note" => TextKind::Note,
            _ => return None,
        })
    }
}

/// A text: a shared note record (`SNOTE`, or a 5.5.1 `NOTE` record) or a note written inside a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text {
    /// The record (`@N1@`); none for a note inside a person.
    pub xref: Option<String>,
    pub kind: TextKind,
    pub title: Option<String>,
    /// Markdown; mentions are `[text](person:<UID>)`.
    pub body: String,
    pub date_text: Option<String>,
    pub date: Option<DateValue>,
    pub place: Option<String>,
    pub certainty: Option<Certainty>,
    pub inferred: bool,
    pub citations: Vec<Citation>,
    pub batch: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crop {
    pub top: u32,
    pub left: u32,
    pub height: Option<u32>,
    pub width: Option<u32>,
}

/// A person's link to a photo or document (`1 OBJE @O1@`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLink {
    /// The media record, or `<person xref>#<n>` for a 5.5.1 object written inside the person.
    pub object: String,
    pub profile: bool,
    pub crop: Option<Crop>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Photo,
    Document,
    Other,
}

impl MediaKind {
    pub fn code(self) -> &'static str {
        match self {
            MediaKind::Photo => "photo",
            MediaKind::Document => "document",
            MediaKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaObject {
    pub id: String,
    /// The file as written: relative to the archive folder (`media/…`) or an absolute path from another program.
    pub file: Option<String>,
    pub format: Option<String>,
    pub kind: MediaKind,
    /// The document type ("akt urodzenia"), from `MEDI … PHRASE`.
    pub document_type: Option<String>,
    pub title: Option<String>,
    pub date_text: Option<String>,
    pub place: Option<String>,
    pub transcription: Option<String>,
    pub translation: Option<String>,
    pub note: Option<String>,
    pub citations: Vec<Citation>,
    pub batch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Source {
    pub xref: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub publication: Option<String>,
    pub kind: Option<String>,
    pub parish: Option<String>,
    pub year: Option<String>,
    pub akt: Option<String>,
    pub url: Option<String>,
    pub repository: Option<String>,
    pub call_number: Option<String>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub media: Vec<String>,
    pub batch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub url: String,
    pub title: Option<String>,
    pub kind: Option<String>,
}

/// A text linked from a person, in the person's order (the biography's table of contents follows it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextRef {
    Shared(String),
    Inline(Text),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersonDetails {
    pub facts: Vec<Fact>,
    pub texts: Vec<TextRef>,
    pub media: Vec<MediaLink>,
    pub tags: Vec<String>,
    pub links: Vec<Link>,
    /// Person-level citations (`1 SOUR`).
    pub citations: Vec<Citation>,
    /// Families this person is a child in with a link other than birth: (family, PEDI).
    pub pedigree: Vec<(String, String)>,
    /// The form written in a source for each name, in the order of `Person::names` (`TRAN` + `LANG`).
    pub name_originals: Vec<Option<(String, Option<String>)>>,
    /// `CREA` and `CHAN`, as `YYYY-MM-DDTHH:MM:SS`, with `Z` when the time is UTC (sortable).
    pub created: Option<String>,
    pub changed: Option<String>,
    pub living: bool,
    /// People outside the family tree: a friend, a godparent, a neighbour (`1 ASSO @I2@` + `ROLE`).
    pub associates: Vec<Associate>,
    /// Fields marked „Brak danych": the information doesn't exist and won't (`_HLM_NODATA`).
    pub no_data: Vec<String>,
    /// Substructures Heirloom doesn't show anywhere else, as GEDCOM lines ("Inne dane z pliku").
    pub other: Vec<String>,
    pub batch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FamilyDetails {
    pub facts: Vec<Fact>,
    pub married: bool,
    pub certainty: Option<Certainty>,
    pub created: Option<String>,
    pub changed: Option<String>,
}

/// The whole archive as the app shows it.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub model: Model,
    /// Same order as `model.persons`.
    pub people: Vec<PersonDetails>,
    /// Same order as `model.families`.
    pub families: Vec<FamilyDetails>,
    pub person_index: HashMap<String, usize>,
    pub family_index: HashMap<String, usize>,
    pub uid_index: HashMap<String, usize>,
    pub texts: HashMap<String, Text>,
    pub media: HashMap<String, MediaObject>,
    /// Media ids in file order.
    pub media_order: Vec<String>,
    pub sources: HashMap<String, Source>,
    pub source_order: Vec<String>,
    pub repositories: HashMap<String, String>,
    pub version: Option<Version>,
}

/// A person's events and attributes; the profile numbers a person's facts in this order (the `fact.*` commands too).
pub const INDI_EVENTS: &[&str] = &[
    "BIRT", "CHR", "BAPM", "DEAT", "BURI", "CREM", "ADOP", "BARM", "BASM", "BLES", "CHRA", "CONF", "FCOM", "GRAD", "EMIG",
    "IMMI", "NATU", "CENS", "PROB", "WILL", "RETI", "ORDN", "EVEN", "CAST", "DSCR", "EDUC", "IDNO", "NATI", "NCHI", "NMR",
    "OCCU", "PROP", "RELI", "RESI", "SSN", "TITL", "FACT", "_MILT", "_MILI",
];
/// A family's events, numbered the same way.
pub const FAM_EVENTS: &[&str] = &["ANUL", "CENS", "DIV", "DIVF", "ENGA", "MARB", "MARC", "MARR", "MARL", "MARS", "EVEN", "RESI", "NCHI", "FACT"];
/// INDI substructures shown somewhere in the app; everything else goes to "Inne dane z pliku".
const INDI_KNOWN: &[&str] = &[
    "NAME", "SEX", "FAMC", "FAMS", "SNOTE", "NOTE", "OBJE", "SOUR", "UID", "_UID", "CREA", "CHAN", "_MARNM", "_HLM_TAG",
    "_HLM_LINK", "_HLM_BATCH", "_HLM_NODATA", "ASSO",
];

impl View {
    pub fn person(&self, xref: &str) -> Option<(&model::Person, &PersonDetails)> {
        let i = *self.person_index.get(xref)?;
        Some((&self.model.persons[i], &self.people[i]))
    }

    pub fn family(&self, xref: &str) -> Option<(&model::Family, &FamilyDetails)> {
        let i = *self.family_index.get(xref)?;
        Some((&self.model.families[i], &self.families[i]))
    }

    /// The texts linked from a person, in order.
    pub fn person_texts<'a>(&'a self, details: &'a PersonDetails) -> impl Iterator<Item = &'a Text> + 'a {
        details.texts.iter().filter_map(|t| match t {
            TextRef::Shared(xref) => self.texts.get(xref),
            TextRef::Inline(text) => Some(text),
        })
    }
}

/// Builds the view of a whole document.
pub fn build(doc: &Document) -> View {
    let version = doc.version;
    let model = model::extract(doc);
    let mut view = View { version: Some(version), ..View::default() };
    for (i, p) in model.persons.iter().enumerate() {
        view.person_index.insert(p.xref.clone(), i);
        if let Some(uid) = &p.uid {
            view.uid_index.insert(uid.clone(), i);
        }
    }
    for (i, f) in model.families.iter().enumerate() {
        view.family_index.insert(f.xref.clone(), i);
    }
    let this_year = crate::history::now().get(..4).and_then(|y| y.parse::<i32>().ok()).unwrap_or(2026);
    for record in &doc.records {
        let Some(xref) = record.xref.as_deref() else { continue };
        match record.tag.as_str() {
            "INDI" => view.people.push(person_details(record, version, this_year)),
            "FAM" => view.families.push(family_details(record, version)),
            "SNOTE" | "NOTE" => {
                view.texts.insert(xref.to_string(), text(record, Some(xref), version));
            }
            "OBJE" => {
                view.media_order.push(xref.to_string());
                view.media.insert(xref.to_string(), media_object(record, xref, version));
            }
            "SOUR" => {
                view.source_order.push(xref.to_string());
                view.sources.insert(xref.to_string(), source(record, xref, version));
            }
            "REPO" => {
                let name = record.child("NAME").and_then(|n| n.text(version)).unwrap_or_default();
                view.repositories.insert(xref.to_string(), name.trim().to_string());
            }
            _ => {}
        }
    }
    // 5.5.1 objects written inside a person get their own ids (the same numbering as `PersonDetails::media`).
    for record in doc.records.iter().filter(|r| r.tag == "INDI") {
        let Some(xref) = record.xref.as_deref() else { continue };
        for (n, obje) in record.children_tagged("OBJE").filter(|o| o.pointer().is_none()).enumerate() {
            let id = format!("{xref}#{n}");
            view.media_order.push(id.clone());
            view.media.insert(id.clone(), media_object(obje, &id, version));
        }
    }
    for source in view.sources.values_mut() {
        if let Some(repo) = source.repository.take() {
            source.repository = Some(view.repositories.get(&repo).cloned().unwrap_or(repo));
        }
    }
    view.model = model;
    view
}

fn text_of(node: &Node, tag: &str, version: Version) -> Option<String> {
    node.child(tag).and_then(|n| n.text(version)).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn citations(node: &Node, version: Version) -> Vec<Citation> {
    node.children_tagged("SOUR").map(|s| citation(s, version)).collect()
}

fn citation(sour: &Node, version: Version) -> Citation {
    match sour.pointer() {
        Some(p) => Citation {
            source: (p != "@VOID@").then(|| p.to_string()),
            page: text_of(sour, "PAGE", version),
            text: None,
            quality: sour.child_value("QUAY").and_then(|q| q.trim().parse().ok()),
        },
        None => Citation { source: None, page: None, text: sour.text(version), quality: None },
    }
}

fn fact(node: &Node, version: Version) -> Fact {
    let date_node = node.child("DATE");
    // A date GEDCOM can't express ("zima 1915") is an empty DATE with only a PHRASE: shown as written.
    let date_text = date_node
        .and_then(|d| d.value.as_deref())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| date_node.and_then(|d| text_of(d, "PHRASE", version)));
    let place_node = node.child("PLAC");
    let associates = node
        .children_tagged("ASSO")
        .filter_map(|a| {
            let person = a.pointer().filter(|p| *p != "@VOID@")?.to_string();
            let role_node = a.child("ROLE").or_else(|| a.child("RELA"));
            Some(Associate {
                person,
                role: role_node.and_then(|r| r.value.as_deref()).unwrap_or("OTHER").trim().to_string(),
                phrase: role_node.and_then(|r| text_of(r, "PHRASE", version)),
                age: text_of(a, "_HLM_AGE", version),
            })
        })
        .collect();
    let value = node.text(version).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let only_happened = value.as_deref() == Some("Y") && node.children.is_empty();
    Fact {
        tag: node.tag.clone(),
        kind: text_of(node, "TYPE", version),
        value: value.filter(|v| v != "Y"),
        date: date_text.as_deref().and_then(date::parse),
        date_text,
        date_phrase: date_node.and_then(|d| text_of(d, "PHRASE", version)),
        julian: date_node.and_then(|d| text_of(d, "_HLM_JULIAN", version)),
        place: place_node.and_then(|p| p.text(version)).map(|p| p.trim().to_string()).filter(|p| !p.is_empty()),
        place_note: place_node.and_then(|p| text_of(p, "NOTE", version)),
        place_orig: place_node.and_then(|p| text_of(p, "_HLM_ORIG", version)),
        cause: text_of(node, "CAUS", version),
        note: text_of(node, "NOTE", version),
        age: text_of(node, "_HLM_AGE", version).or_else(|| text_of(node, "AGE", version)),
        certainty: node.child_value("_HLM_CERT").and_then(Certainty::from_code),
        inferred: node.child_value("_HLM_BASIS").map(str::trim) == Some("inferred"),
        citations: citations(node, version),
        associates,
        only_happened,
    }
}

/// `CREA`/`CHAN` → `2026-09-28T19:10:00Z` (or just the date).
fn stamp(node: &Node, tag: &str) -> Option<String> {
    let date_node = node.child(tag)?.child("DATE")?;
    let d = date::parse(date_node.value.as_deref()?)?;
    let mut out = format!("{:04}-{:02}-{:02}", d.start.year, d.start.month.unwrap_or(1), d.start.day.unwrap_or(1));
    if let Some(time) = date_node.child_value("TIME") {
        // A trailing Z means UTC (GEDCOM 7); it stays, so the screens show the time in the local zone.
        let utc = time.trim().ends_with('Z');
        let time = time.trim().trim_end_matches('Z');
        let time = time.split('.').next().unwrap_or(time);
        out.push('T');
        out.push_str(if time.len() == 5 { &time[..5] } else { time });
        if time.len() == 5 {
            out.push_str(":00");
        }
        if utc {
            out.push('Z');
        }
    }
    Some(out)
}

fn person_details(record: &Node, version: Version, this_year: i32) -> PersonDetails {
    let mut details = PersonDetails::default();
    let mut other = Vec::new();
    // model.rs lists the married names of person-level _MARNM tags after all the NAMEs.
    let mut vendor_married = 0;
    for child in &record.children {
        let tag = child.tag.as_str();
        if INDI_EVENTS.contains(&tag) {
            details.facts.push(fact(child, version));
            continue;
        }
        match tag {
            "SNOTE" => {
                if let Some(p) = child.pointer() {
                    details.texts.push(TextRef::Shared(p.to_string()));
                }
            }
            "NOTE" => match child.pointer() {
                Some(p) => details.texts.push(TextRef::Shared(p.to_string())),
                None => details.texts.push(TextRef::Inline(text(child, None, version))),
            },
            "OBJE" => {
                let n = details.media.iter().filter(|m| m.object.contains('#')).count();
                let object = match child.pointer() {
                    Some(p) => p.to_string(),
                    None => format!("{}#{n}", record.xref.as_deref().unwrap_or_default()),
                };
                details.media.push(MediaLink {
                    object,
                    profile: child.child_value("_HLM_PROFILE").map(str::trim) == Some("Y")
                        || child.child_value("_PRIM").map(str::trim) == Some("Y"),
                    crop: child.child("CROP").map(|c| Crop {
                        top: c.child_value("TOP").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
                        left: c.child_value("LEFT").and_then(|v| v.trim().parse().ok()).unwrap_or(0),
                        height: c.child_value("HEIGHT").and_then(|v| v.trim().parse().ok()),
                        width: c.child_value("WIDTH").and_then(|v| v.trim().parse().ok()),
                    }),
                    title: text_of(child, "TITL", version),
                });
            }
            "SOUR" => details.citations.push(citation(child, version)),
            "_HLM_TAG" => {
                if let Some(t) = child.text(version).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
                    details.tags.push(t);
                }
            }
            "_HLM_LINK" => {
                if let Some(url) = child.value.as_deref().map(str::trim).filter(|u| !u.is_empty()) {
                    details.links.push(Link { url: url.to_string(), title: text_of(child, "TITL", version), kind: text_of(child, "TYPE", version) });
                }
            }
            "FAMC" => {
                if let (Some(fam), Some(pedi)) = (child.pointer(), child.child_value("PEDI")) {
                    let pedi = pedi.trim().to_ascii_uppercase();
                    if pedi != "BIRTH" {
                        details.pedigree.push((fam.to_string(), pedi));
                    }
                }
            }
            "NAME" => {
                let tran = child.child("TRAN").and_then(|t| {
                    let text = t.text(version)?.replace('/', "").trim().to_string();
                    Some((text, text_of(t, "LANG", version)))
                });
                details.name_originals.push(tran);
                for _ in child.children_tagged("_MARNM") {
                    details.name_originals.push(None);
                }
            }
            "_MARNM" => vendor_married += 1,
            "_HLM_BATCH" => details.batch = child.value.clone(),
            "_HLM_NODATA" => details.no_data.extend(child.value.clone()),
            "ASSO" => {
                if let Some(person) = child.pointer().filter(|p| *p != "@VOID@") {
                    // 7.0 writes ROLE (+ PHRASE); 5.5.1 wrote RELA with free text.
                    let (role, phrase) = match (child.child("ROLE"), child.child_value("RELA")) {
                        (Some(r), _) => (r.value.as_deref().unwrap_or("OTHER").trim().to_string(), text_of(r, "PHRASE", version)),
                        (None, Some(rela)) => ("OTHER".to_string(), Some(rela.trim().to_string())),
                        (None, None) => ("OTHER".to_string(), None),
                    };
                    details.associates.push(Associate { person: person.to_string(), role, phrase, age: None });
                }
            }
            _ if INDI_KNOWN.contains(&tag) => {}
            _ => other.push(subtree_text(child, 1)),
        }
    }
    details.name_originals.extend(std::iter::repeat_n(None, vendor_married));
    details.created = stamp(record, "CREA");
    details.changed = stamp(record, "CHAN");
    let dead = details.facts.iter().any(|f| matches!(f.tag.as_str(), "DEAT" | "BURI" | "CREM"));
    let born = details
        .facts
        .iter()
        .filter(|f| matches!(f.tag.as_str(), "BIRT" | "CHR" | "BAPM"))
        .find_map(|f| f.date.map(|d| d.start.year));
    details.living = !dead && born.is_some_and(|y| y > this_year - 100);
    details.other = other;
    details
}

fn family_details(record: &Node, version: Version) -> FamilyDetails {
    let facts: Vec<Fact> = record.children.iter().filter(|c| FAM_EVENTS.contains(&c.tag.as_str())).map(|c| fact(c, version)).collect();
    FamilyDetails {
        married: facts.iter().any(|f| f.tag == "MARR"),
        facts,
        certainty: record.child_value("_HLM_CERT").and_then(Certainty::from_code),
        created: stamp(record, "CREA"),
        changed: stamp(record, "CHAN"),
    }
}

fn text(node: &Node, xref: Option<&str>, version: Version) -> Text {
    let date_text = text_of(node, "_HLM_DATE", version);
    Text {
        xref: xref.map(str::to_string),
        kind: node.child_value("_HLM_KIND").and_then(TextKind::from_code).unwrap_or(TextKind::Note),
        title: text_of(node, "_HLM_TITLE", version),
        body: node.text(version).unwrap_or_default(),
        date: date_text.as_deref().and_then(date::parse),
        date_text,
        place: text_of(node, "_HLM_PLAC", version),
        certainty: node.child_value("_HLM_CERT").and_then(Certainty::from_code),
        inferred: node.child_value("_HLM_BASIS").map(str::trim) == Some("inferred"),
        citations: citations(node, version),
        batch: node.child_value("_HLM_BATCH").map(str::to_string),
    }
}

/// GEDCOM 7 FILE values are URIs (spaces written as %20); 5.5.1 ones are plain paths.
fn file_path(value: &str, version: Version) -> String {
    let value = value.trim();
    let value = value.strip_prefix("file:///").or_else(|| value.strip_prefix("file://")).unwrap_or(value);
    if version == Version::V7 || value.contains("%20") { percent_decode(value) } else { value.to_string() }
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
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
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

pub fn media_kind_for(format: Option<&str>, medi: Option<&str>, file: Option<&str>) -> MediaKind {
    match medi.map(|m| m.trim().to_ascii_uppercase()).as_deref() {
        Some("PHOTO" | "TOMBSTONE" | "CARD") => return MediaKind::Photo,
        Some("MANUSCRIPT" | "BOOK" | "NEWSPAPER" | "MAGAZINE" | "FICHE" | "FILM" | "ELECTRONIC" | "DOCUMENT") => {
            return MediaKind::Document;
        }
        _ => {}
    }
    let ext = file
        .and_then(|f| f.rsplit_once('.'))
        .map(|(_, e)| e.to_ascii_lowercase())
        .or_else(|| format.map(|f| f.rsplit('/').next().unwrap_or(f).to_ascii_lowercase()))
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "avif" => MediaKind::Photo,
        "pdf" | "doc" | "docx" | "odt" | "txt" | "rtf" | "md" => MediaKind::Document,
        _ => MediaKind::Other,
    }
}

fn media_object(node: &Node, id: &str, version: Version) -> MediaObject {
    let file_node = node.child("FILE");
    let file = file_node.and_then(|f| f.value.as_deref()).map(|v| file_path(v, version)).filter(|f| !f.is_empty());
    // 7.0: FILE.FORM.MEDI; 5.5.1: FILE.FORM.TYPE; 5.5: FORM at the object level.
    let form = file_node.and_then(|f| f.child("FORM")).or_else(|| node.child("FORM"));
    let medi = form.and_then(|f| f.child("MEDI").or_else(|| f.child("TYPE")));
    let format = form.and_then(|f| f.value.as_deref()).map(|v| v.trim().to_string());
    let kind = media_kind_for(format.as_deref(), medi.and_then(|m| m.value.as_deref()), file.as_deref());
    let title = file_node.and_then(|f| text_of(f, "TITL", version)).or_else(|| text_of(node, "TITL", version));
    MediaObject {
        id: id.to_string(),
        document_type: medi.and_then(|m| text_of(m, "PHRASE", version)),
        file,
        format,
        kind,
        title,
        date_text: text_of(node, "_HLM_DATE", version),
        place: text_of(node, "_HLM_PLAC", version),
        transcription: text_of(node, "_HLM_TRANSCRIPTION", version),
        translation: text_of(node, "_HLM_TRANSLATION", version),
        note: text_of(node, "NOTE", version),
        citations: citations(node, version),
        batch: node.child_value("_HLM_BATCH").map(str::to_string),
    }
}

fn source(record: &Node, xref: &str, version: Version) -> Source {
    let repo = record.child("REPO");
    Source {
        xref: xref.to_string(),
        title: text_of(record, "TITL", version),
        author: text_of(record, "AUTH", version),
        publication: text_of(record, "PUBL", version),
        kind: text_of(record, "_HLM_KIND", version),
        parish: text_of(record, "_HLM_PARISH", version),
        year: text_of(record, "_HLM_YEAR", version),
        akt: text_of(record, "_HLM_AKT", version),
        url: text_of(record, "_HLM_URL", version).or_else(|| text_of(record, "WWW", version)),
        repository: repo.and_then(|r| r.pointer().map(str::to_string).or_else(|| r.text(version))),
        call_number: repo.and_then(|r| text_of(r, "CALN", version)),
        text: text_of(record, "TEXT", version),
        note: text_of(record, "NOTE", version),
        media: record.children_tagged("OBJE").filter_map(Node::pointer).map(str::to_string).collect(),
        batch: record.child_value("_HLM_BATCH").map(str::to_string),
    }
}

/// A substructure as GEDCOM lines starting at `level`, for showing data Heirloom doesn't understand.
fn subtree_text(node: &Node, level: usize) -> String {
    let mut out = String::new();
    let mut stack = vec![(node, level)];
    while let Some((n, l)) = stack.pop() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&l.to_string());
        if let Some(x) = &n.xref {
            out.push(' ');
            out.push_str(x);
        }
        out.push(' ');
        out.push_str(&n.tag);
        if let Some(v) = &n.value {
            out.push(' ');
            out.push_str(v);
        }
        for child in n.children.iter().rev() {
            stack.push((child, l + 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 UID u-1\n1 NAME Józef /Kowalski/\n2 TYPE BIRTH\n2 TRAN Іосифъ /Ковальскій/\n3 LANG ru\n1 SEX M\n\
1 BIRT\n2 DATE 12 MAR 1878\n3 PHRASE 28 февраля / 12 марта 1878\n3 _HLM_JULIAN 28 FEB 1878\n2 PLAC Wólka\n3 NOTE parafia Łęczna\n\
2 ASSO @I2@\n3 ROLE FATH\n3 _HLM_AGE 30 лѣтъ\n2 SOUR @S1@\n3 PAGE akt 45\n2 _HLM_CERT high\n\
1 OCCU kolejarz na stacji Lublin\n2 DATE FROM 1905\n2 _HLM_CERT medium\n1 DEAT Y\n\
1 SNOTE @N2@\n1 SNOTE @N1@\n1 NOTE Luźna uwaga\n1 OBJE @O1@\n2 _HLM_PROFILE Y\n2 CROP\n3 TOP 10\n3 LEFT 20\n\
1 _HLM_TAG kolejarz\n1 _HLM_LINK https://pl.wikipedia.org/wiki/Test\n2 TITL Wikipedia\n2 TYPE wikipedia\n\
1 _FSFTID ABC-123\n1 FAMC @F1@\n2 PEDI ADOPTED\n1 CREA\n2 DATE 28 SEP 2026\n3 TIME 19:10:00Z\n\
0 @I2@ INDI\n1 NAME Antoni /Kowalski/\n1 BIRT\n2 DATE 1990\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n1 MARR\n2 DATE 1870\n\
0 @N1@ SNOTE Józef urodził się w [Wólce](person:u-1).\n1 _HLM_KIND bio\n1 _HLM_TITLE Dzieciństwo\n1 SOUR @S1@\n\
0 @N2@ SNOTE Krótko o Józefie.\n1 _HLM_KIND summary\n\
0 @O1@ OBJE\n1 FILE media/J%C3%B3zef%201904.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n2 TITL Józef w 1904\n1 _HLM_DATE 1904\n\
0 @S1@ SOUR\n1 TITL Akt urodzenia nr 45/1878\n1 _HLM_KIND parish_record\n1 _HLM_PARISH Łęczna\n1 REPO @R1@\n2 CALN 35/1878\n\
0 @R1@ REPO\n1 NAME Archiwum Państwowe w Lublinie\n0 TRLR\n";

    fn view() -> View {
        build(&Document::from_bytes(FILE.as_bytes()).0)
    }

    #[test]
    fn facts_keep_the_evidence() {
        let v = view();
        let (_, d) = v.person("@I1@").unwrap();
        let birth = &d.facts[0];
        assert_eq!(birth.tag, "BIRT");
        assert_eq!(birth.date_phrase.as_deref(), Some("28 февраля / 12 марта 1878"));
        assert_eq!(birth.julian.as_deref(), Some("28 FEB 1878"));
        assert_eq!((birth.place.as_deref(), birth.place_note.as_deref()), (Some("Wólka"), Some("parafia Łęczna")));
        assert_eq!(birth.associates[0], Associate { person: "@I2@".into(), role: "FATH".into(), phrase: None, age: Some("30 лѣтъ".into()) });
        assert_eq!(birth.citations[0].page.as_deref(), Some("akt 45"));
        assert_eq!(birth.certainty, Some(Certainty::High));
        assert_eq!(d.facts[1].value.as_deref(), Some("kolejarz na stacji Lublin"));
        assert!(d.facts[2].only_happened && d.facts[2].tag == "DEAT");
        assert!(!d.living, "DEAT Y means the person died");
        assert!(v.person("@I2@").unwrap().1.living, "born 1990, no death");
    }

    #[test]
    fn texts_media_links_and_other_data() {
        let v = view();
        let (_, d) = v.person("@I1@").unwrap();
        let texts: Vec<_> = v.person_texts(d).collect();
        assert_eq!(texts.iter().map(|t| t.kind).collect::<Vec<_>>(), [TextKind::Summary, TextKind::Bio, TextKind::Note]);
        assert_eq!(texts[1].title.as_deref(), Some("Dzieciństwo"));
        assert_eq!(texts[2].body, "Luźna uwaga");
        assert_eq!(d.media[0].crop, Some(Crop { top: 10, left: 20, height: None, width: None }));
        assert!(d.media[0].profile);
        let photo = &v.media["@O1@"];
        assert_eq!((photo.file.as_deref(), photo.kind), (Some("media/Józef 1904.jpg"), MediaKind::Photo));
        assert_eq!(d.tags, ["kolejarz"]);
        assert_eq!(d.links[0].kind.as_deref(), Some("wikipedia"));
        assert_eq!(d.other, ["1 _FSFTID ABC-123"]);
        assert_eq!(d.pedigree, [("@F1@".to_string(), "ADOPTED".to_string())]);
        assert_eq!(d.name_originals[0], Some(("Іосифъ Ковальскій".to_string(), Some("ru".to_string()))));
        assert_eq!(d.created.as_deref(), Some("2026-09-28T19:10:00Z"), "the Z (UTC) is kept, so the screens show local time");
        let source = &v.sources["@S1@"];
        assert_eq!(source.repository.as_deref(), Some("Archiwum Państwowe w Lublinie"));
        assert_eq!(source.call_number.as_deref(), Some("35/1878"));
        assert!(v.family("@F1@").unwrap().1.married);
    }

    #[test]
    fn inline_objects_from_older_files() {
        let file = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 OBJE\n2 FILE C:\\Zdjęcia\\jan.jpg\n2 FORM jpg\n2 TITL Jan\n0 TRLR\n";
        let v = build(&Document::from_bytes(file.as_bytes()).0);
        let (_, d) = v.person("@I1@").unwrap();
        assert_eq!(d.media[0].object, "@I1@#0");
        let object = &v.media["@I1@#0"];
        assert_eq!((object.file.as_deref(), object.title.as_deref()), (Some("C:\\Zdjęcia\\jan.jpg"), Some("Jan")));
    }
}
