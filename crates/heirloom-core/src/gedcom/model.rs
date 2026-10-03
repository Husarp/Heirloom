//! A read-only view of people and families on top of the line tree, for lists, search and the tree.
//! Editing goes through the line tree itself, so nothing the view doesn't know about is ever lost.

use super::date::{self, DateValue};
use super::{Document, Node, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
    Other,
    Unknown,
}

impl Sex {
    pub fn code(self) -> &'static str {
        match self {
            Sex::Male => "M",
            Sex::Female => "F",
            Sex::Other => "X",
            Sex::Unknown => "U",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    Birth,
    Married,
    Other,
}

impl NameKind {
    pub fn code(self) -> &'static str {
        match self {
            NameKind::Birth => "birth",
            NameKind::Married => "married",
            NameKind::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub kind: NameKind,
    pub given: String,
    pub surname: String,
    pub nickname: Option<String>,
    /// What kind of name it is, in words (`TYPE OTHER` + `PHRASE`: "zapis w akcie", "imię zakonne").
    pub phrase: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Event {
    /// The date exactly as written, for display.
    pub date_text: Option<String>,
    pub date: Option<DateValue>,
    pub place: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub xref: String,
    pub uid: Option<String>,
    pub sex: Sex,
    pub names: Vec<Name>,
    /// Birth, or baptism when there is no birth.
    pub birth: Option<Event>,
    /// Death, or burial when there is no death.
    pub death: Option<Event>,
    /// Families this person is a child in (FAMC) and a partner in (FAMS).
    pub famc: Vec<String>,
    pub fams: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub xref: String,
    /// Partners in file order (HUSB, WIFE); either may be missing.
    pub partners: Vec<String>,
    pub children: Vec<String>,
    pub marriage: Option<Event>,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    pub persons: Vec<Person>,
    pub families: Vec<Family>,
}

impl Person {
    /// The birth name (for a woman: her maiden name), or the first name if none is marked.
    pub fn birth_name(&self) -> Option<&Name> {
        self.names.iter().find(|n| n.kind == NameKind::Birth).or(self.names.first())
    }

    /// A married name without a surname (an empty `_MARNM`, `NAME` + `TYPE MARRIED`) doesn't count: it would
    /// hide the surname everywhere.
    pub fn married_name(&self) -> Option<&Name> {
        self.names.iter().find(|n| n.kind == NameKind::Married && !n.surname.is_empty())
    }

    pub fn given(&self) -> &str {
        self.birth_name().map_or("", |n| n.given.as_str())
    }

    /// The surname a person is known by: after marriage, the married one.
    pub fn surname(&self) -> &str {
        self.married_name().or(self.birth_name()).map_or("", |n| n.surname.as_str())
    }

    /// "Marianna Kowalska" — the maiden name is shown separately ("z d. Nowak").
    pub fn display_name(&self) -> String {
        [self.given(), self.surname()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" ")
    }

    /// The birth surname when it differs from the surname used after marriage.
    pub fn maiden_name(&self) -> Option<&str> {
        let married = self.married_name()?;
        let birth = self.names.iter().find(|n| n.kind == NameKind::Birth)?;
        (!birth.surname.is_empty() && birth.surname != married.surname).then_some(birth.surname.as_str())
    }
}

/// Builds the people-and-families view of a document.
pub fn extract(doc: &Document) -> Model {
    let mut model = Model::default();
    for record in &doc.records {
        let Some(xref) = record.xref.as_deref() else { continue };
        match record.tag.as_str() {
            "INDI" => model.persons.push(person(record, xref, doc.version)),
            "FAM" => model.families.push(family(record, xref, doc.version)),
            _ => {}
        }
    }
    model
}

/// One person record as the app reads it.
pub fn person(record: &Node, xref: &str, version: Version) -> Person {
    let mut names = Vec::new();
    for (i, node) in record.children_tagged("NAME").enumerate() {
        names.push(name(node, i == 0, version));
        // Married surnames written the old vendor way (_MARNM under NAME).
        let first = names[0].clone();
        for marnm in node.children_tagged("_MARNM") {
            names.push(married_from_vendor_tag(marnm, &first, version));
        }
    }
    for marnm in record.children_tagged("_MARNM") {
        if let Some(first) = names.first().cloned() {
            names.push(married_from_vendor_tag(marnm, &first, version));
        }
    }
    let sex = match record.child_value("SEX").map(str::trim) {
        Some("M") => Sex::Male,
        Some("F") => Sex::Female,
        Some("X") => Sex::Other,
        _ => Sex::Unknown,
    };
    let uid = record.child_value("UID").or_else(|| record.child_value("_UID")).map(|s| s.trim().to_string());
    Person {
        xref: xref.to_string(),
        uid,
        sex,
        names,
        birth: first_event(record, &["BIRT", "CHR", "BAPM"], version),
        death: first_event(record, &["DEAT", "BURI"], version),
        famc: pointers(record, "FAMC"),
        fams: pointers(record, "FAMS"),
    }
}

/// The given names and the surname of a NAME structure, as the app shows them (GIVN and SURN win over the value).
pub fn name_parts(node: &Node, version: Version) -> (String, String) {
    let raw = node.text(version).unwrap_or_default();
    let (mut given, mut surname) = split_name(&raw);
    if let Some(g) = node.child("GIVN").and_then(|n| n.text(version)) {
        given = g.trim().to_string();
    }
    if let Some(s) = node.child("SURN").and_then(|n| n.text(version)) {
        surname = s.trim().to_string();
    }
    (given, surname)
}

fn name(node: &Node, first: bool, version: Version) -> Name {
    let (given, surname) = name_parts(node, version);
    let kind = match node.child_value("TYPE").map(|t| t.trim().to_ascii_uppercase()) {
        Some(t) if t == "BIRTH" || t == "MAIDEN" => NameKind::Birth,
        Some(t) if t == "MARRIED" => NameKind::Married,
        Some(_) => NameKind::Other,
        None if first => NameKind::Birth,
        None => NameKind::Other,
    };
    let nickname = node.child("NICK").and_then(|n| n.text(version)).map(|s| s.trim().to_string());
    let phrase = node.child("TYPE").and_then(|t| t.child("PHRASE")).and_then(|p| p.text(version)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    Name { kind, given, surname, nickname, phrase }
}

fn married_from_vendor_tag(node: &Node, birth: &Name, version: Version) -> Name {
    let (given, surname) = split_name(&node.text(version).unwrap_or_default());
    let surname = if surname.is_empty() { given.clone() } else { surname };
    let given = if given.is_empty() || given == surname { birth.given.clone() } else { given };
    Name { kind: NameKind::Married, given, surname, nickname: None, phrase: None }
}

/// "Józef Antoni /Kowalski/ Jr" → ("Józef Antoni", "Kowalski"); written surname first,
/// "/Kovács/ János" → ("János", "Kovács").
fn split_name(raw: &str) -> (String, String) {
    match raw.split_once('/') {
        Some((given, rest)) => {
            let (surname, after) = rest.split_once('/').unwrap_or((rest, ""));
            let given = if given.trim().is_empty() { after } else { given };
            (given.trim().to_string(), surname.trim().to_string())
        }
        None => (raw.trim().to_string(), String::new()),
    }
}

fn first_event(record: &Node, tags: &[&str], version: Version) -> Option<Event> {
    let node = tags.iter().find_map(|t| record.child(t))?;
    let date_text = node.child("DATE").and_then(|d| d.text(version)).map(|s| s.trim().to_string());
    Some(Event {
        date: date_text.as_deref().and_then(date::parse),
        date_text,
        place: node.child("PLAC").and_then(|p| p.text(version)).map(|s| s.trim().to_string()),
    })
}

fn pointers(record: &Node, tag: &str) -> Vec<String> {
    record.children_tagged(tag).filter_map(Node::pointer).filter(|p| *p != "@VOID@").map(str::to_string).collect()
}

fn family(record: &Node, xref: &str, version: Version) -> Family {
    let mut partners = pointers(record, "HUSB");
    partners.extend(pointers(record, "WIFE"));
    Family {
        xref: xref.to_string(),
        partners,
        children: pointers(record, "CHIL"),
        marriage: first_event(record, &["MARR"], version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 UID 5f1d4a2c-0000-4000-8000-000000000001\n1 NAME Józef /Kowalski/\n2 NICK Dziadek Józek\n1 SEX M\n\
1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka, Łęczna\n1 DEAT\n2 DATE 3 FEB 1951\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Marianna /Nowak/\n2 TYPE BIRTH\n1 NAME Marianna /Kowalska/\n2 TYPE MARRIED\n1 SEX F\n\
1 CHR\n2 DATE ABT 1882\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 _UID ABC-123\n1 NAME Helena /Kowalska/\n1 _MARNM Wiśniewska\n1 SEX F\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n1 MARR\n2 DATE 14 FEB 1904\n2 PLAC Łęczna\n0 TRLR\n";

    fn model() -> Model {
        extract(&Document::from_bytes(FILE.as_bytes()).0)
    }

    #[test]
    fn people_names_and_events() {
        let m = model();
        let jozef = &m.persons[0];
        assert_eq!((jozef.display_name(), jozef.sex), ("Józef Kowalski".to_string(), Sex::Male));
        assert_eq!(jozef.uid.as_deref(), Some("5f1d4a2c-0000-4000-8000-000000000001"));
        assert_eq!(jozef.names[0].nickname.as_deref(), Some("Dziadek Józek"));
        let birth = jozef.birth.as_ref().unwrap();
        assert_eq!((birth.date_text.as_deref(), birth.place.as_deref()), (Some("12 MAR 1878"), Some("Wólka, Łęczna")));
        assert_eq!(birth.date.unwrap().start.year, 1878);
    }

    #[test]
    fn married_women_keep_their_maiden_name() {
        let m = model();
        let marianna = &m.persons[1];
        assert_eq!(marianna.display_name(), "Marianna Kowalska");
        assert_eq!(marianna.maiden_name(), Some("Nowak"));
        assert_eq!(marianna.birth.as_ref().unwrap().date_text.as_deref(), Some("ABT 1882"), "baptism stands in for birth");
        let helena = &m.persons[2];
        assert_eq!((helena.display_name(), helena.maiden_name()), ("Helena Wiśniewska".to_string(), Some("Kowalska")));
        assert_eq!(helena.uid.as_deref(), Some("ABC-123"), "the old _UID tag is read too");
    }

    #[test]
    fn odd_names_and_pointers_from_other_programs() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n\
0 @I1@ INDI\n1 NAME /Kovács/ János\n1 FAMS @F1@ \n\
0 @I2@ INDI\n1 NAME Helena /Kowalska/\n1 _MARNM\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Anna /Nowak/\n1 NAME\n2 TYPE MARRIED\n\
0 @F1@ FAM\n1 HUSB @I1@ \n1 WIFE @I2@\n0 TRLR\n";
        let m = extract(&Document::from_bytes(text.as_bytes()).0);
        assert_eq!(m.persons[0].display_name(), "János Kovács", "surname written first");
        assert_eq!(m.persons[0].fams, ["@F1@"], "a space after a pointer doesn't drop the link");
        assert_eq!(m.families[0].partners, ["@I1@", "@I2@"]);
        let helena = &m.persons[1];
        assert_eq!((helena.display_name(), helena.maiden_name()), ("Helena Kowalska".to_string(), None), "empty _MARNM");
        assert_eq!(m.persons[2].display_name(), "Anna Nowak", "married name without a surname");
    }

    #[test]
    fn families() {
        let m = model();
        let f = &m.families[0];
        assert_eq!(f.partners, ["@I1@", "@I2@"]);
        assert_eq!(f.children, ["@I3@"]);
        assert_eq!(f.marriage.as_ref().unwrap().place.as_deref(), Some("Łęczna"));
        assert_eq!(m.persons[2].famc, ["@F1@"]);
    }
}
