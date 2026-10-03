//! GEDCOM lines as a tree that keeps everything: every structure, including tags this program doesn't
//! understand, survives reading and writing unchanged (PLAN.md §11.2, RESEARCH §2).

use super::Version;

/// One GEDCOM structure — `level [xref] tag [value]` — with its substructures.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// Record identifier such as `@I1@` (only on top-level records).
    pub xref: Option<String>,
    pub tag: String,
    /// The line value exactly as written in the file: escapes and pointers are left untouched.
    pub value: Option<String>,
    pub children: Vec<Node>,
}

/// The line terminator a file uses; files are written back with the same one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eol {
    CrLf,
    Lf,
    Cr,
}

impl Eol {
    pub fn as_str(self) -> &'static str {
        match self {
            Eol::CrLf => "\r\n",
            Eol::Lf => "\n",
            Eol::Cr => "\r",
        }
    }
}

/// Something odd found while reading, with its 1-based line number (0 = the file as a whole).
/// The message is shown to the user, so it is in Polish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub line: usize,
    pub message: String,
}

impl Node {
    pub fn new(tag: &str) -> Node {
        Node { tag: tag.to_string(), ..Node::default() }
    }

    pub fn with_value(tag: &str, value: &str) -> Node {
        Node { tag: tag.to_string(), value: Some(value.to_string()), ..Node::default() }
    }

    pub fn with_xref(mut self, xref: &str) -> Node {
        self.xref = Some(xref.to_string());
        self
    }

    pub fn with_child(mut self, child: Node) -> Node {
        self.children.push(child);
        self
    }

    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.tag == tag)
    }

    pub fn child_mut(&mut self, tag: &str) -> Option<&mut Node> {
        self.children.iter_mut().find(|c| c.tag == tag)
    }

    pub fn children_tagged<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    pub fn child_value(&self, tag: &str) -> Option<&str> {
        self.child(tag).and_then(|c| c.value.as_deref())
    }

    /// The value as a pointer (`@I1@`, or `@VOID@` meaning "none"), if it is one. Spaces around it are
    /// ignored: some files end lines with a space, and a missed pointer drops a family link from the view.
    pub fn pointer(&self) -> Option<&str> {
        let v = self.value.as_deref()?.trim_matches(' ');
        let is_pointer = v.len() > 2 && v.starts_with('@') && !v.starts_with("@@") && v.ends_with('@') && !v.contains(' ');
        is_pointer.then_some(v)
    }

    /// The text value, with continuation lines joined (CONT = line break, CONC = no break, before GEDCOM 7)
    /// and `@@` escapes removed.
    pub fn text(&self, version: Version) -> Option<String> {
        let mut out = self.value.as_deref().map(|v| unescape(v, version));
        for child in &self.children {
            let piece = child.value.as_deref().map(|v| unescape(v, version)).unwrap_or_default();
            match child.tag.as_str() {
                "CONT" => {
                    let text = out.get_or_insert_with(String::new);
                    text.push('\n');
                    text.push_str(&piece);
                }
                "CONC" => out.get_or_insert_with(String::new).push_str(&piece),
                _ => {}
            }
        }
        out
    }

    /// Replaces the text value. Line breaks become CONT lines; before GEDCOM 7, long lines are also split with
    /// CONC (never at a space, as the 5.5.1 spec requires). Other substructures are kept.
    pub fn set_text(&mut self, text: &str, version: Version) {
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let max = if version == Version::V7 { usize::MAX } else { 200 };
        let mut value = None;
        let mut continuation = Vec::new();
        for (i, line) in normalized.split('\n').enumerate() {
            for (j, chunk) in split_long(line, max).into_iter().enumerate() {
                let escaped = escape_leading_at(chunk);
                let v = (!escaped.is_empty()).then_some(escaped);
                match (i, j) {
                    (0, 0) => value = v,
                    (_, 0) => continuation.push(Node { tag: "CONT".into(), value: v, ..Node::default() }),
                    _ => continuation.push(Node { tag: "CONC".into(), value: v, ..Node::default() }),
                }
            }
        }
        self.value = value;
        self.children.retain(|c| c.tag != "CONT" && c.tag != "CONC");
        self.children.splice(0..0, continuation);
    }
}

fn unescape(v: &str, version: Version) -> String {
    if version == Version::V7 {
        match v.strip_prefix("@@") {
            Some(rest) => format!("@{rest}"),
            None => v.to_string(),
        }
    } else {
        // 5.5.1 asked for every @ to be doubled; accept both styles.
        v.replace("@@", "@")
    }
}

/// GEDCOM 7 requires doubling only a leading @; older readers accept that too.
fn escape_leading_at(s: &str) -> String {
    if s.starts_with('@') { format!("@{s}") } else { s.to_string() }
}

/// Splits `line` into pieces of at most `max` characters, never cutting next to a space.
fn split_long(line: &str, max: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = line;
    while rest.chars().count() > max {
        let bounds: Vec<usize> = rest.char_indices().map(|(i, _)| i).collect();
        let mut k = max;
        while k > 1 && (rest[..bounds[k]].ends_with(' ') || rest[bounds[k]..].starts_with(' ')) {
            k -= 1;
        }
        let cut = if k > 1 { bounds[k] } else { bounds[max] };
        out.push(&rest[..cut]);
        rest = &rest[cut..];
    }
    out.push(rest);
    out
}

/// The result of reading lines: the records, anything odd that was found, and how the file ended its lines.
pub struct Parsed {
    pub records: Vec<Node>,
    pub warnings: Vec<Warning>,
    pub eol: Eol,
    pub trailing_newline: bool,
}

/// Reads GEDCOM text into records. Tolerant of old files: indented lines, extra spaces, missing level numbers
/// and level jumps are repaired and reported instead of failing.
pub fn parse(text: &str) -> Parsed {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let eol = detect_eol(text);
    let trailing_newline = text.ends_with(['\n', '\r']);
    let mut records: Vec<Node> = Vec::new();
    let mut path: Vec<usize> = Vec::new();
    let mut warnings = Vec::new();

    for (i, raw) in split_lines(text).enumerate() {
        let number = i + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let line = raw.trim_start_matches([' ', '\t']);
        let Some((level, xref, tag, value)) = split_line(line) else {
            if path.is_empty() {
                warnings.push(Warning { line: number, message: format!("Pominięto wiersz bez numeru poziomu: „{line}”.") });
            } else {
                warnings.push(Warning {
                    line: number,
                    message: "Wiersz bez numeru poziomu — dołączono go jako dalszy ciąg poprzedniego.".into(),
                });
                let node = Node { tag: "CONT".into(), value: Some(line.to_string()), ..Node::default() };
                node_at_mut(&mut records, &path).children.push(node);
            }
            continue;
        };
        let node = Node {
            xref: xref.map(str::to_string),
            tag: tag.to_string(),
            value: value.map(str::to_string),
            children: Vec::new(),
        };
        if level == 0 || path.is_empty() {
            if level != 0 {
                warnings.push(Warning { line: number, message: "Plik nie zaczyna się od wiersza z poziomem 0.".into() });
            }
            records.push(node);
            path.clear();
            path.push(records.len() - 1);
            continue;
        }
        let depth = path.len() - 1;
        let level = if level > depth + 1 {
            warnings.push(Warning {
                line: number,
                message: format!("Poziom {level} po poziomie {depth} — poprawiono na {}.", depth + 1),
            });
            depth + 1
        } else {
            level
        };
        path.truncate(level);
        let parent = node_at_mut(&mut records, &path);
        parent.children.push(node);
        let index = parent.children.len() - 1;
        path.push(index);
    }
    Parsed { records, warnings, eol, trailing_newline }
}

/// Writes records as GEDCOM text with the given line terminator.
pub fn write(records: &[Node], eol: Eol, trailing_newline: bool) -> String {
    let mut out = String::with_capacity(records.len() * 96);
    for record in records {
        write_node(&mut out, record, 0, eol.as_str());
    }
    if !trailing_newline && out.ends_with(eol.as_str()) {
        out.truncate(out.len() - eol.as_str().len());
    }
    out
}

fn write_node(out: &mut String, node: &Node, level: usize, eol: &str) {
    out.push_str(&level.to_string());
    out.push(' ');
    if let Some(xref) = &node.xref {
        out.push_str(xref);
        out.push(' ');
    }
    out.push_str(&node.tag);
    // A line break inside a value (from an AI answer, or pasted) would start a line that other programs, and this
    // one, read as a structure of its own: what follows it goes into CONT lines, as GEDCOM writes any multi-line value.
    let mut more = None;
    if let Some(value) = &node.value {
        let (first, rest) = value.split_at(value.find(['\r', '\n']).unwrap_or(value.len()));
        out.push(' ');
        out.push_str(first);
        more = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix(['\r', '\n']));
    }
    out.push_str(eol);
    for line in more.into_iter().flat_map(|rest| rest.split("\r\n")).flat_map(|l| l.split(['\r', '\n'])) {
        out.push_str(&(level + 1).to_string());
        out.push_str(" CONT");
        if !line.is_empty() {
            out.push(' ');
            if line.starts_with('@') {
                out.push('@');
            }
            out.push_str(line);
        }
        out.push_str(eol);
    }
    for child in &node.children {
        write_node(out, child, level + 1, eol);
    }
}

fn node_at_mut<'a>(records: &'a mut [Node], path: &[usize]) -> &'a mut Node {
    let mut node = &mut records[path[0]];
    for &i in &path[1..] {
        node = &mut node.children[i];
    }
    node
}

fn detect_eol(text: &str) -> Eol {
    match text.find(['\r', '\n']) {
        // CR CR LF is CRLF damaged by a second conversion; written back as CR alone, the file would be one long
        // line for programs that split only on LF.
        Some(i) if text[i..].starts_with("\r\n") || text[i..].starts_with("\r\r\n") => Eol::CrLf,
        Some(i) if text.as_bytes()[i] == b'\r' => Eol::Cr,
        Some(_) => Eol::Lf,
        None => Eol::CrLf,
    }
}

/// Splits on CRLF, LF or CR.
fn split_lines(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        match rest.find(['\r', '\n']) {
            Some(pos) => {
                let line = &rest[..pos];
                let skip = if rest[pos..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[pos + skip..];
                Some(line)
            }
            None => {
                let line = rest;
                rest = "";
                Some(line)
            }
        }
    })
}

/// Splits "TOKEN rest" at the first space.
fn split_token(s: &str) -> (&str, Option<&str>) {
    match s.find(' ') {
        Some(p) => (&s[..p], Some(&s[p + 1..])),
        None => (s, None),
    }
}

/// `level [xref] tag [value]` → its parts. The value keeps everything after the single space that follows
/// the tag, including leading or trailing spaces (they matter for CONC in 5.5.1 files).
fn split_line(line: &str) -> Option<(usize, Option<&str>, &str, Option<&str>)> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > 3 {
        return None;
    }
    let level = line[..digits].parse().ok()?;
    let rest = line[digits..].strip_prefix(' ')?.trim_start_matches(' ');
    let (first, after) = split_token(rest);
    if first.len() > 2 && first.starts_with('@') && first.ends_with('@') {
        let (tag, value) = split_token(after?.trim_start_matches(' '));
        if tag.is_empty() {
            return None;
        }
        Some((level, Some(first), tag, value))
    } else if first.is_empty() {
        None
    } else {
        Some((level, None, first, after))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_7: &str = "0 HEAD\r\n1 GEDC\r\n2 VERS 7.0\r\n1 SCHMA\r\n2 TAG _HLM_KIND https://example.org/_HLM_KIND\r\n\
0 @I1@ INDI\r\n1 NAME Józef /Kowalski/\r\n2 GIVN Józef\r\n2 SURN Kowalski\r\n1 SEX M\r\n1 BIRT\r\n2 DATE 12 MAR 1878\r\n\
3 PHRASE 28 lutego / 12 marca 1878\r\n2 PLAC Wólka, Łęczna\r\n1 NOTE Pierwsza linia\r\n2 CONT druga linia\r\n2 CONT\r\n\
2 CONT @@e-mail z małpą\r\n1 FAMC @VOID@\r\n1 _HLM_KIND saying\r\n0 @X1@ _HLM_RECORD\r\n1 _HLM_VALUE  dwie spacje \r\n0 TRLR\r\n";

    #[test]
    fn round_trips_gedcom_7_exactly() {
        let parsed = parse(SAMPLE_7);
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.eol, Eol::CrLf);
        assert_eq!(write(&parsed.records, parsed.eol, parsed.trailing_newline), SAMPLE_7);
    }

    #[test]
    fn reads_structure_values_and_pointers() {
        let parsed = parse(SAMPLE_7);
        let indi = &parsed.records[1];
        assert_eq!(indi.xref.as_deref(), Some("@I1@"));
        assert_eq!(indi.child_value("SEX"), Some("M"));
        assert_eq!(indi.child("FAMC").and_then(Node::pointer), Some("@VOID@"));
        let note = indi.child("NOTE").unwrap();
        assert_eq!(note.text(Version::V7).unwrap(), "Pierwsza linia\ndruga linia\n\n@e-mail z małpą");
        let odd = &parsed.records[2].children[0];
        assert_eq!(odd.value.as_deref(), Some(" dwie spacje "), "spaces inside a value are kept");
    }

    #[test]
    fn joins_conc_and_cont_in_old_files() {
        let text = "0 @N1@ NOTE Ala ma \n1 CONC kota\n1 CONT a kot ma @@ale\n";
        let parsed = parse(text);
        assert_eq!(parsed.eol, Eol::Lf);
        assert_eq!(parsed.records[0].text(Version::V5).unwrap(), "Ala ma kota\na kot ma @ale");
        assert_eq!(write(&parsed.records, parsed.eol, parsed.trailing_newline), text);
    }

    #[test]
    fn set_text_writes_cont_and_splits_long_lines_for_old_versions() {
        let mut note = Node::new("NOTE").with_child(Node::with_value("SOUR", "@S1@"));
        note.set_text("@home\nsecond line", Version::V7);
        assert_eq!(note.value.as_deref(), Some("@@home"));
        assert_eq!(note.children[0].tag, "CONT");
        assert_eq!(note.children[1].tag, "SOUR", "other substructures are kept after the continuation lines");
        assert_eq!(note.text(Version::V7).unwrap(), "@home\nsecond line");

        let long = "słowo ".repeat(80);
        let mut old = Node::new("NOTE");
        old.set_text(long.trim_end(), Version::V5);
        assert!(old.children.iter().any(|c| c.tag == "CONC"));
        for piece in std::iter::once(&old).chain(old.children.iter()) {
            let v = piece.value.as_deref().unwrap_or("");
            assert!(v.chars().count() <= 200);
            assert!(!v.starts_with(' ') && !v.ends_with(' '), "CONC must not split at a space: {v:?}");
        }
        assert_eq!(old.text(Version::V5).unwrap(), long.trim_end());
    }

    #[test]
    fn round_trips_line_endings_and_tag_only_lines() {
        for text in [
            "0 HEAD\r1 BIRT\r1 DEAT \r2 DATE  1 JAN 1900 \r0 TRLR\r",
            "0 HEAD\n1 BIRT Y\n0 @I1@ INDI \n0 TRLR",
            "\u{feff}0 HEAD\r\n1 NOTE\r\n2 CONT\r\n2 CONC  x\r\n0 TRLR\r\n",
        ] {
            let parsed = parse(text);
            assert!(parsed.warnings.is_empty(), "{text:?}: {:?}", parsed.warnings);
            assert_eq!(write(&parsed.records, parsed.eol, parsed.trailing_newline), text.trim_start_matches('\u{feff}'));
        }
    }

    #[test]
    fn a_line_break_inside_a_value_never_breaks_the_file() {
        // A value from an AI answer or a pasted text: its second line must not become a record of its own.
        let record = Node::new("INDI")
            .with_xref("@I1@")
            .with_child(Node::with_value("CAUS", "gruźlica\r\n0 dzieci\n\n@I2@"))
            .with_child(Node::with_value("SEX", "M"));
        let text = write(std::slice::from_ref(&record), Eol::Lf, true);
        let parsed = parse(&text);
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.records.len(), 1, "{text}");
        let caus = parsed.records[0].child("CAUS").unwrap();
        assert_eq!(caus.text(Version::V7).as_deref(), Some("gruźlica\n0 dzieci\n\n@I2@"));
        assert!(caus.children.iter().all(|c| c.tag == "CONT" && c.pointer().is_none()), "{text}");
        assert_eq!(parsed.records[0].child_value("SEX"), Some("M"));
    }

    #[test]
    fn cr_cr_lf_is_written_back_as_crlf() {
        let parsed = parse("0 HEAD\r\r\n1 GEDC\r\r\n0 TRLR\r\r\n");
        assert_eq!(parsed.eol, Eol::CrLf, "not CR alone, which many programs read as one long line");
        assert_eq!(write(&parsed.records, parsed.eol, parsed.trailing_newline), "0 HEAD\r\n1 GEDC\r\n0 TRLR\r\n");
    }

    #[test]
    fn conc_splits_multibyte_text_on_character_boundaries() {
        for text in ["ż".repeat(450), format!("{} {}", "ł".repeat(199), "ę".repeat(300)), "😀".repeat(401), " ".repeat(450)] {
            let mut note = Node::new("NOTE");
            note.set_text(&text, Version::V5);
            assert_eq!(note.text(Version::V5).unwrap(), text);
            for piece in std::iter::once(&note).chain(note.children.iter()) {
                assert!(piece.value.as_deref().unwrap_or("").chars().count() <= 200);
            }
        }
    }

    #[test]
    fn repairs_and_reports_broken_lines() {
        let text = "0 HEAD\n  1 GEDC\n3 VERS 5.5.1\nto nie jest wiersz GEDCOM\n0 TRLR";
        let parsed = parse(text);
        assert_eq!(parsed.records.len(), 2);
        assert!(!parsed.trailing_newline);
        let gedc = &parsed.records[0].children[0];
        assert_eq!(gedc.children[0].tag, "VERS", "level jump 1→3 is attached one level down");
        assert_eq!(gedc.children[0].children[0].tag, "CONT", "a line without a level becomes a continuation");
        assert_eq!(parsed.warnings.len(), 2);
    }
}
