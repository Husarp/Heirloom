//! Reading and writing GEDCOM without losing anything (PLAN.md §11.2): a line tree keeps every structure,
//! including tags this program doesn't understand, so a save changes only what was edited.

pub mod ansel;
pub mod date;
pub mod encoding;
pub mod line;
pub mod model;
pub mod view;

pub use encoding::Encoding;
pub use line::{Eol, Node, Warning};

use crate::PRODUCT_ID;

/// The GEDCOM version a document follows (HEAD.GEDC.VERS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// 7.0.x — what Heirloom writes.
    V7,
    /// 5.5, 5.5.1, 5.5.5 — what most other programs write.
    V5,
    Unknown,
}

/// A whole GEDCOM file: its records plus how it was stored, so it can be written back the same way.
#[derive(Debug, Clone)]
pub struct Document {
    pub records: Vec<Node>,
    pub version: Version,
    /// The encoding the file had on disk; Heirloom writes UTF-8.
    pub source_encoding: Encoding,
    /// Write a UTF-8 byte-order mark (GEDCOM 7 says files should start with one).
    pub bom: bool,
    pub eol: Eol,
    pub trailing_newline: bool,
}

impl Document {
    /// A new, empty GEDCOM 7 file written by Heirloom.
    pub fn new_v7() -> Document {
        let head = Node::new("HEAD")
            .with_child(Node::new("GEDC").with_child(Node::with_value("VERS", "7.0")))
            .with_child(
                Node::with_value("SOUR", PRODUCT_ID)
                    .with_child(Node::with_value("NAME", "Heirloom"))
                    .with_child(Node::with_value("VERS", env!("CARGO_PKG_VERSION"))),
            )
            .with_child(Node::with_value("LANG", "pl"));
        Document {
            records: vec![head, Node::new("TRLR")],
            version: Version::V7,
            source_encoding: Encoding::Utf8,
            bom: true,
            eol: Eol::CrLf,
            trailing_newline: true,
        }
    }

    /// Reads a file. Never fails: problems are repaired where possible and returned as warnings.
    pub fn from_bytes(bytes: &[u8]) -> (Document, Vec<Warning>) {
        let decoded = encoding::decode(bytes);
        let parsed = line::parse(&decoded.text);
        let mut warnings: Vec<Warning> =
            decoded.warnings.into_iter().map(|message| Warning { line: 0, message }).collect();
        warnings.extend(parsed.warnings);
        let mut doc = Document {
            records: parsed.records,
            version: Version::Unknown,
            source_encoding: decoded.encoding,
            bom: decoded.bom,
            eol: parsed.eol,
            trailing_newline: parsed.trailing_newline,
        };
        doc.version = doc.detect_version();
        // The next save writes UTF-8, so the header must say so — also in a file labelled ANSEL, ASCII or ANSI
        // that is really UTF-8 (usually plain ASCII): other programs would misread a "Łukasz" added later.
        if let Some(charset) = doc.head_mut().and_then(|h| h.child_mut("CHAR"))
            && !charset.value.as_deref().is_some_and(|v| matches!(v.trim().to_ascii_uppercase().as_str(), "UTF-8" | "UTF8"))
        {
            charset.value = Some("UTF-8".to_string());
        }
        if doc.source_encoding != Encoding::Utf8 {
            doc.bom = true;
            warnings.push(Warning {
                line: 0,
                message: format!(
                    "Plik jest zapisany w kodowaniu {}; przy zapisie zmian Heirloom zapisze go w UTF-8.",
                    doc.source_encoding.label()
                ),
            });
        }
        (doc, warnings)
    }

    /// The file as UTF-8 bytes, with the same line endings it was read with.
    pub fn to_bytes(&self) -> Vec<u8> {
        encoding::encode_utf8(&line::write(&self.records, self.eol, self.trailing_newline), self.bom)
    }

    pub fn head(&self) -> Option<&Node> {
        self.records.iter().find(|r| r.tag == "HEAD")
    }

    pub fn head_mut(&mut self) -> Option<&mut Node> {
        self.records.iter_mut().find(|r| r.tag == "HEAD")
    }

    /// HEAD.SOUR: which program wrote the file.
    pub fn source_program(&self) -> Option<&str> {
        self.head()?.child_value("SOUR")
    }

    pub fn position(&self, xref: &str) -> Option<usize> {
        self.records.iter().position(|r| r.xref.as_deref() == Some(xref))
    }

    pub fn record(&self, xref: &str) -> Option<&Node> {
        self.records.iter().find(|r| r.xref.as_deref() == Some(xref))
    }

    /// Declares an extension tag in HEAD.SCHMA, the official GEDCOM 7 way (`2 TAG _HLM_KIND <uri>`).
    pub fn declare_extension(&mut self, tag: &str, uri: &str) {
        let Some(head) = self.head_mut() else { return };
        if head.child("SCHMA").is_none() {
            head.children.push(Node::new("SCHMA"));
        }
        let Some(schema) = head.child_mut("SCHMA") else { return };
        let declared = schema
            .children_tagged("TAG")
            .any(|t| t.value.as_deref().and_then(|v| v.split_whitespace().next()) == Some(tag));
        if !declared {
            schema.children.push(Node::with_value("TAG", &format!("{tag} {uri}")));
        }
    }

    /// Turns a GEDCOM 5.5.1 document into GEDCOM 7, so everything Heirloom adds is written one way: CONC lines
    /// are joined, `@@` escapes follow the 7.0 rule, NOTE records become SNOTE, OBJE `FORM.TYPE` becomes
    /// `FORM.MEDI`, and the header says 7.0. Unknown data is kept. Returns false when it already is GEDCOM 7.
    pub fn upgrade_to_v7(&mut self) -> bool {
        if self.version == Version::V7 {
            return false;
        }
        self.records = upgrade_records(&self.records, self.version);
        self.version = Version::V7;
        true
    }

    fn detect_version(&self) -> Version {
        let version = self.head().and_then(|h| h.child("GEDC")).and_then(|g| g.child_value("VERS")).map(str::trim);
        match version {
            Some(v) if v.starts_with('7') => Version::V7,
            Some(v) if v.starts_with('5') => Version::V5,
            _ => Version::Unknown,
        }
    }
}

/// The records of a 5.5.1 (or unlabelled) file as GEDCOM 7 (see [`Document::upgrade_to_v7`]).
pub fn upgrade_records(records: &[Node], from: Version) -> Vec<Node> {
    let from = if from == Version::V7 { return records.to_vec() } else { Version::V5 };
    records
        .iter()
        .map(|record| {
            let mut node = record.clone();
            if node.tag == "HEAD" {
                node.children.retain(|c| c.tag != "CHAR");
                if let Some(gedc) = node.child_mut("GEDC") {
                    gedc.children.retain(|c| c.tag != "FORM");
                    match gedc.child_mut("VERS") {
                        Some(vers) => vers.value = Some("7.0".into()),
                        None => gedc.children.insert(0, Node::with_value("VERS", "7.0")),
                    }
                } else {
                    node.children.insert(0, Node::new("GEDC").with_child(Node::with_value("VERS", "7.0")));
                }
                return node;
            }
            if node.tag == "NOTE" && node.xref.is_some() {
                node.tag = "SNOTE".into();
            }
            upgrade_node(&mut node, from, false);
            node
        })
        .collect()
}

fn upgrade_node(node: &mut Node, from: Version, in_form: bool) {
    let has_text = node.value.is_some() || node.children.iter().any(|c| c.tag == "CONC" || c.tag == "CONT");
    if node.tag == "DATE" {
        // A date is not text: its leading `@` must not be doubled, and 5.5.1 calendar escapes become 7.0 names.
        if let Some(value) = node.value.as_deref().filter(|v| v.contains("@#D")) {
            node.value = Some(calendar_names(value));
        }
    } else if node.pointer().is_none() && has_text {
        if let Some(text) = node.text(from) {
            node.set_text(&text, Version::V7);
        }
    } else if node.tag == "NOTE" && node.pointer().is_some() {
        node.tag = "SNOTE".into();
    }
    if in_form && node.tag == "TYPE" {
        node.tag = "MEDI".into();
        let value = node.value.as_deref().unwrap_or("").trim().to_ascii_uppercase();
        const KNOWN: [&str; 14] =
            ["AUDIO", "BOOK", "CARD", "ELECTRONIC", "FICHE", "FILM", "MAGAZINE", "MANUSCRIPT", "MAP", "NEWSPAPER", "PHOTO", "TOMBSTONE", "VIDEO", "OTHER"];
        if KNOWN.contains(&value.as_str()) {
            node.value = Some(value);
        } else if !value.is_empty() {
            let phrase = node.value.take().unwrap_or_default();
            node.value = Some("OTHER".into());
            node.children.push(Node::with_value("PHRASE", phrase.trim()));
        }
    }
    let is_form = node.tag == "FORM";
    for child in node.children.iter_mut().filter(|c| c.tag != "CONT" && c.tag != "CONC") {
        upgrade_node(child, from, is_form);
    }
}

/// `ABT @#DJULIAN@ 1850` (5.5.1) → `ABT JULIAN 1850` (7.0). Calendars 7.0 doesn't know stay as written.
pub(crate) fn calendar_names(value: &str) -> String {
    let mut out = value.to_string();
    for (escape, name) in [("@#DGREGORIAN@", "GREGORIAN"), ("@#DJULIAN@", "JULIAN"), ("@#DHEBREW@", "HEBREW"), ("@#DFRENCH R@", "FRENCH_R")] {
        out = out.replace(escape, &format!(" {name} "));
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_551_file_becomes_gedcom_7() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n2 FORM LINEAGE-LINKED\n1 CHAR UTF-8\n1 SOUR PAF\n\
0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 NOTE @N1@\n1 NOTE Adres: jan@@example.com, dług\n2 CONC a notatka\n2 CONT druga linia\n\
1 OBJE\n2 FILE C:\\zdj\\jan.jpg\n3 FORM jpg\n4 TYPE photo\n\
0 @N1@ NOTE Wspólna\n1 CONC  notatka\n0 TRLR\n";
        let (mut doc, _) = Document::from_bytes(text.as_bytes());
        assert_eq!(doc.version, Version::V5);
        assert!(doc.upgrade_to_v7());
        assert_eq!(doc.version, Version::V7);
        let head = doc.head().unwrap();
        assert_eq!(head.child("GEDC").unwrap().child_value("VERS"), Some("7.0"));
        assert!(head.child("CHAR").is_none() && head.child("GEDC").unwrap().child("FORM").is_none());
        assert_eq!(head.child_value("SOUR"), Some("PAF"), "the file still says which program made it");
        let person = doc.record("@I1@").unwrap();
        assert_eq!(person.children[1].tag, "SNOTE");
        assert_eq!(person.children[2].text(Version::V7).as_deref(), Some("Adres: jan@example.com, długa notatka\ndruga linia"));
        assert!(person.children[2].children.iter().all(|c| c.tag != "CONC"));
        let medi = person.child("OBJE").unwrap().child("FILE").unwrap().child("FORM").unwrap().child("MEDI").unwrap();
        assert_eq!(medi.value.as_deref(), Some("PHOTO"));
        let note = doc.record("@N1@").unwrap();
        assert_eq!((note.tag.as_str(), note.text(Version::V7).as_deref()), ("SNOTE", Some("Wspólna notatka")));
        assert!(!doc.upgrade_to_v7(), "only once");
    }

    #[test]
    fn calendar_escapes_in_dates_become_gedcom_7_calendars() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I1@ INDI\n1 BIRT\n2 DATE @#DJULIAN@ 28 FEB 1878\n\
1 DEAT\n2 DATE ABT @#DHEBREW@ 5 NSN 5650\n1 BURI\n2 DATE @#DFRENCH R@ 1 VEND 8\n0 TRLR\n";
        let (mut doc, _) = Document::from_bytes(text.as_bytes());
        doc.upgrade_to_v7();
        let date = |tag: &str| doc.record("@I1@").unwrap().child(tag).unwrap().child_value("DATE").unwrap().to_string();
        assert_eq!(date("BIRT"), "JULIAN 28 FEB 1878", "not the text escape @@#DJULIAN@, which no longer reads as a date");
        assert_eq!(date("DEAT"), "ABT HEBREW 5 NSN 5650");
        assert_eq!(date("BURI"), "FRENCH_R 1 VEND 8");
        let julian = date::parse(&date("BIRT")).unwrap();
        assert_eq!((julian.calendar, julian.start.day), (date::Calendar::Julian, Some(28)));
    }

    #[test]
    fn new_file_is_gedcom_7_with_bom_and_round_trips() {
        let doc = Document::new_v7();
        let bytes = doc.to_bytes();
        assert!(bytes.starts_with(b"\xEF\xBB\xBF0 HEAD\r\n1 GEDC\r\n2 VERS 7.0\r\n1 SOUR HEIRLOOM\r\n"));
        let (again, warnings) = Document::from_bytes(&bytes);
        assert!(warnings.is_empty());
        assert_eq!((again.version, again.bom), (Version::V7, true));
        assert_eq!(again.to_bytes(), bytes);
        assert_eq!(again.source_program(), Some(PRODUCT_ID));
    }

    #[test]
    fn old_encodings_are_converted_to_utf8_on_write() {
        let text = "0 HEAD\r\n1 GEDC\r\n2 VERS 5.5.1\r\n1 CHAR ANSI\r\n0 @I1@ INDI\r\n1 NAME Józef /Łączyński/\r\n0 TRLR\r\n";
        let (bytes, _, _) = encoding_rs::WINDOWS_1250.encode(text);
        let (doc, warnings) = Document::from_bytes(&bytes);
        assert_eq!((doc.version, doc.source_encoding), (Version::V5, Encoding::Windows1250));
        assert_eq!(warnings.len(), 1);
        let written = String::from_utf8(doc.to_bytes()[3..].to_vec()).unwrap();
        assert_eq!(written, text.replace("1 CHAR ANSI", "1 CHAR UTF-8"));
    }

    #[test]
    fn a_plain_ascii_file_labelled_ansel_is_relabelled_utf8() {
        let text = "0 HEAD\r\n1 GEDC\r\n2 VERS 5.5.1\r\n1 CHAR ANSEL\r\n0 @I1@ INDI\r\n1 NAME John /Smith/\r\n0 TRLR\r\n";
        let (mut doc, warnings) = Document::from_bytes(text.as_bytes());
        assert_eq!((doc.source_encoding, warnings.len()), (Encoding::Utf8, 0));
        doc.records.insert(2, Node::new("INDI").with_xref("@I2@").with_child(Node::with_value("NAME", "Łukasz /Wójcik/")));
        let written = String::from_utf8(doc.to_bytes()).unwrap();
        assert!(written.contains("1 CHAR UTF-8\r\n"), "the UTF-8 bytes of Ł must not be labelled ANSEL");
        let (utf8_file, _) = Document::from_bytes("0 HEAD\n1 CHAR utf-8\n0 TRLR\n".as_bytes());
        assert_eq!(utf8_file.head().unwrap().child_value("CHAR"), Some("utf-8"), "a correct label is left alone");
    }

    #[test]
    fn an_ansel_accent_at_a_line_end_does_not_swallow_the_next_record() {
        let bytes = b"0 HEAD\n1 CHAR ANSEL\n0 @N1@ NOTE Wi\xE2\n0 @I2@ INDI\n1 NAME Jan /Nowak/\n0 TRLR\n";
        let (doc, warnings) = Document::from_bytes(bytes);
        assert!(doc.record("@I2@").is_some_and(|r| r.child_value("NAME") == Some("Jan /Nowak/")));
        assert_eq!(doc.record("@N1@").unwrap().children.len(), 0);
        assert!(warnings.iter().any(|w| w.message.contains("ANSEL nie dało się odczytać")));
    }

    #[test]
    fn declares_extensions_once() {
        let mut doc = Document::new_v7();
        doc.declare_extension("_HLM_KIND", "https://example.org/_HLM_KIND");
        doc.declare_extension("_HLM_KIND", "https://example.org/_HLM_KIND");
        let schema = doc.head().unwrap().child("SCHMA").unwrap();
        assert_eq!(schema.children.len(), 1);
    }
}
