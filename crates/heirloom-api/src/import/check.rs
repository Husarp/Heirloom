//! Import steps 1–2: collect the parts from pasted text and dropped files, check that each part is complete (end
//! marker), repair only cosmetic slips, and validate the whole batch (IMPORT_FORMAT §6, DESIGNER_ANSWERS §7).

use super::format::{self, MARKER, Part};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    /// `error` blocks the import; `warning` and `info` don't.
    pub level: &'static str,
    pub message: String,
    pub reference: String,
    /// `paste` (paste the missing part), `show`, `skip`, `fix`.
    pub action: Option<&'static str>,
    pub target: Option<String>,
}

impl Issue {
    fn new(level: &'static str, message: impl Into<String>, reference: impl Into<String>) -> Issue {
        Issue { level, message: message.into(), reference: reference.into(), action: None, target: None }
    }

    fn act(mut self, action: &'static str, target: Option<String>) -> Issue {
        self.action = Some(action);
        self.target = target;
        self
    }

    pub fn json(&self) -> Value {
        json!({ "level": self.level, "message": self.message, "reference": self.reference, "action": self.action, "target": self.target })
    }
}

/// One JSON block found in the input.
#[derive(Debug, Clone)]
pub struct Block {
    pub text: String,
    /// Where it came from: "wklejony tekst" or a file name.
    pub origin: String,
}

/// A file dropped with the batch.
#[derive(Debug, Clone)]
pub struct InputFile {
    pub path: PathBuf,
    pub name: String,
    /// The M-number as a number (M001 = 1), if the name starts with one.
    pub m: Option<u32>,
    pub size: u64,
}

/// "M001 akt urodzenia.jpg" → 1; "IMG_2031.jpg" → None.
pub fn m_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(['M', 'm'])?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    let after = rest[digits.len()..].chars().next();
    if after.is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    digits.parse().ok()
}

pub fn m_label(n: u32) -> String {
    format!("M{n:03}")
}

/// The fenced ```json blocks of an AI answer; a whole text that is JSON counts as one block. A block whose closing
/// fence is missing is kept too (the check then reports it as cut off).
pub fn extract_blocks(text: &str, origin: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    // ASCII only: its positions must be the same as in `text` ("İ" is longer in full lower case).
    let lower = text.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find("```json") {
        let fence = search + rel;
        // A fence starts a line; "```json" in the middle of a sentence is just text.
        let line_start = lower[..fence].rfind('\n').map_or(0, |n| n + 1);
        if !lower[line_start..fence].trim().is_empty() {
            search = fence + 7;
            continue;
        }
        let start = fence + "```json".len();
        let body_start = text[start..].find('\n').map_or(text.len(), |n| start + n + 1);
        let end = text[body_start..].find("```").map_or(text.len(), |e| body_start + e);
        blocks.push(Block { text: text[body_start..end].trim().to_string(), origin: origin.to_string() });
        search = (end + 3).min(text.len());
        if end == text.len() {
            break;
        }
    }
    if blocks.is_empty() {
        let trimmed = text.trim();
        if let Some(open) = trimmed.find('{') {
            blocks.push(Block { text: trimmed[open..].to_string(), origin: origin.to_string() });
        }
    }
    blocks
}

/// Cosmetic repairs, only for a part whose end marker is present: trailing commas and typographic quotes used as
/// JSON quotes.
fn repair(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    // The current string was opened with a typographic quote, so a typographic quote closes it.
    let mut typographic = false;
    let mut escaped = false;
    for (i, &c) in chars.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
                out.push(c);
                continue;
            }
            match c {
                '\\' => {
                    escaped = true;
                    out.push(c);
                }
                '"' => {
                    in_string = false;
                    out.push(c);
                }
                '\u{201D}' if typographic => {
                    in_string = false;
                    out.push('"');
                }
                _ => out.push(c),
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                typographic = false;
                out.push(c);
            }
            '\u{201C}' | '\u{201D}' => {
                in_string = true;
                typographic = true;
                out.push('"');
            }
            ',' => {
                let next = chars[i + 1..].iter().find(|c| !c.is_whitespace());
                if !matches!(next, Some('}') | Some(']')) {
                    out.push(c);
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// The part number written in a (possibly broken) block, for messages.
fn part_number_hint(text: &str) -> Option<u32> {
    let at = text.find("\"part\"")?;
    let rest = &text[at + 6..];
    let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

pub struct Parsed {
    /// Each part with its text and the origin of its block.
    pub parts: Vec<(Part, String, String)>,
    pub issues: Vec<Issue>,
}

/// Reads every block: checks the marker, repairs cosmetic slips, parses.
pub fn parse_blocks(blocks: &[Block]) -> Parsed {
    let mut parts: Vec<(Part, String, String)> = Vec::new();
    let mut issues = Vec::new();
    // Parts that came cut off: the error goes once the whole part is pasted too.
    let mut cut: Vec<(usize, u32)> = Vec::new();
    let mut seen_texts: HashSet<String> = HashSet::new();
    for block in blocks {
        if !seen_texts.insert(block.text.clone()) {
            continue; // The same part pasted twice.
        }
        let hint = part_number_hint(&block.text);
        let label = hint.map_or_else(|| "Część bez numeru".to_string(), |n| format!("Część {n}"));
        if !block.text.contains(MARKER) {
            // Asked to go on, a chat AI continues in the middle of the JSON, which can't be read on its own: the
            // whole part is what's needed.
            let ask = hint.map_or_else(|| "tę część".to_string(), |n| format!("całą część {n}"));
            if let Some(n) = hint {
                cut.push((issues.len(), n));
            }
            issues.push(
                Issue::new(
                    "error",
                    format!("{label} jest ucięta — brak znacznika końca. Poproś AI: „Wyślij {ask} jeszcze raz” i wklej ją tutaj."),
                    format!("{label} · {}", block.origin),
                )
                .act("paste", hint.map(|n| n.to_string())),
            );
            continue;
        }
        let value: Result<Value, serde_json::Error> = serde_json::from_str(&block.text).or_else(|_| serde_json::from_str(&repair(&block.text)));
        let parsed = value.map_err(|e| format!("(wiersz {}): brakuje przecinka lub nawiasu", e.line())).and_then(|mut v| {
            format::normalize(&mut v);
            serde_json::from_value::<Part>(v).map_err(|_| "jedna z wartości ma niewłaściwą postać".to_string())
        });
        match parsed {
            Ok(part) => parts.push((part, block.text.clone(), block.origin.clone())),
            Err(why) => issues.push(Issue::new("error", format!("Nie mogę odczytać: {} {why}.", label.to_lowercase()), format!("{label} · {}", block.origin))),
        }
    }
    let complete: HashSet<u32> = parts.iter().filter_map(|(p, _, _)| p.part).collect();
    for (at, _) in cut.iter().rev().filter(|(_, n)| complete.contains(n)) {
        issues.remove(*at);
    }
    parts.sort_by_key(|(p, _, _)| p.part.unwrap_or(u32::MAX));
    Parsed { parts, issues }
}

/// All parts of a batch as one.
#[derive(Debug, Clone, Default)]
pub struct Batch {
    pub name: String,
    pub author: Option<String>,
    pub created: Option<String>,
    pub parts_seen: Vec<u32>,
    pub total_parts: Option<u32>,
    pub merged: Part,
}

fn person_label(batch: &Part, id: &str) -> String {
    match batch.persons.iter().find(|p| p.id == id) {
        Some(p) => {
            let name = p.names.first().map(|n| format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default())).unwrap_or_default();
            format!("Osoba {id} · {}", name.trim())
        }
        None => format!("Osoba {id}"),
    }
}

/// Joins the parts and runs every check. Unknown event types become "other" here (with a warning).
pub fn check(parsed: Parsed, files: &[InputFile]) -> (Batch, Vec<Issue>) {
    let mut issues = parsed.issues;
    let mut batch = Batch::default();
    let mut names: Vec<String> = Vec::new();
    // Where each part number came from, for a second part with the same number.
    let mut origin_of: HashMap<u32, &str> = HashMap::new();
    for (part, _, origin) in &parsed.parts {
        // „Część 2 · odpowiedz.txt”, as the parse-level issues name their part.
        let label = part.part.map_or_else(|| "Część bez numeru".to_string(), |n| format!("Część {n}"));
        let reference = format!("{label} · {origin}");
        if part.format.as_deref() != Some("heirloom-import") {
            issues.push(Issue::new(
                "error",
                "To nie jest odpowiedź w formacie Heirloom. Usuń ten plik z listy albo poproś AI o odpowiedź według instrukcji („Kopiuj instrukcję dla AI”).",
                reference,
            ));
            continue;
        }
        let major = part.version.as_deref().and_then(|v| v.split('.').next()).unwrap_or("");
        if major != "1" {
            issues.push(Issue::new(
                "error",
                format!(
                    "Ta odpowiedź jest w wersji formatu {}, a Heirloom zna wersję 1.x. Skopiuj instrukcję dla AI jeszcze raz i poproś o odpowiedź według niej.",
                    part.version.clone().unwrap_or_else(|| "?".into())
                ),
                reference,
            ));
            continue;
        }
        if let Some(name) = &part.batch {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        let number = part.part.unwrap_or(0);
        if let Some(first) = origin_of.get(&number) {
            // The same text came once already (parse_blocks keeps one); this one differs, so the user picks.
            issues.push(Issue::new(
                "warning",
                format!("{label} jest dwa razy, w różnych wersjach — użyto tej z „{first}”, a tę pominięto. Jeśli to poprawiona wersja, usuń z listy tamtą."),
                reference,
            ));
            continue;
        }
        origin_of.insert(number, origin);
        batch.parts_seen.push(number);
        if part.end.as_ref().and_then(|e| e.is_final) == Some(true) {
            batch.total_parts = Some(number);
        }
        batch.author = batch.author.clone().or_else(|| part.author.clone());
        batch.created = batch.created.clone().or_else(|| part.created.clone());
        if batch.merged.manifest.is_empty() {
            batch.merged.manifest = part.manifest.clone();
        }
        // Counts written by the AI vs what the part holds.
        if let Some(end) = &part.end {
            let actual = [("persons", part.persons.len()), ("events", part.events.len()), ("texts", part.texts.len()), ("media", part.media.len()), ("sources", part.sources.len())];
            for (key, n) in actual {
                if let Some(claimed) = end.counts.get(key).and_then(Value::as_u64) {
                    if claimed as usize != n {
                        // "AI podało 1 osobę / 2 osoby / 5 osób".
                        let (one, few, many) = match key {
                            "persons" => ("osobę", "osoby", "osób"),
                            "events" => ("zdarzenie", "zdarzenia", "zdarzeń"),
                            "texts" => ("tekst", "teksty", "tekstów"),
                            "media" => ("plik", "pliki", "plików"),
                            _ => ("źródło", "źródła", "źródeł"),
                        };
                        let claimed = crate::count_pl(claimed as usize, one, few, many);
                        issues.push(Issue::new("warning", format!("AI podało {claimed}, a w części {number} jest {n}."), format!("Część {number}")));
                    }
                }
            }
        }
        let m = &mut batch.merged;
        m.persons.extend(part.persons.clone());
        m.events.extend(part.events.clone());
        m.relationships.extend(part.relationships.clone());
        m.texts.extend(part.texts.clone());
        m.media.extend(part.media.clone());
        m.sources.extend(part.sources.clone());
        m.links.extend(part.links.clone());
        m.coverage.extend(part.coverage.clone());
        m.questions.extend(part.questions.clone());
    }
    if names.len() > 1 {
        issues.push(Issue::new("error", "Wklejone części pochodzą z różnych paczek.", names.join(", ")));
    }
    batch.name = names.first().cloned().unwrap_or_else(|| "Import".into());
    let m = &batch.merged;

    // Duplicate ids.
    let mut ids: HashMap<String, usize> = HashMap::new();
    for id in m.persons.iter().map(|p| &p.id).chain(m.events.iter().map(|e| &e.id)).chain(m.texts.iter().map(|t| &t.id)).chain(m.sources.iter().map(|s| &s.id)).chain(m.questions.iter().map(|q| &q.id)) {
        *ids.entry(id.clone()).or_default() += 1;
    }
    for (id, n) in &ids {
        if *n > 1 {
            issues.push(Issue::new("error", format!("Identyfikator {id} występuje dwa razy."), id.clone()));
        }
    }
    let persons: HashSet<&str> = m.persons.iter().map(|p| p.id.as_str()).collect();
    // The manifest: every planned person must arrive.
    let missing: Vec<&String> = m.manifest.iter().filter(|id| !persons.contains(id.as_str())).collect();
    if !missing.is_empty() {
        let list = match (missing.first(), missing.last()) {
            (Some(a), Some(b)) if missing.len() > 2 => format!("{a}–{b}"),
            _ => missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
        };
        issues.push(Issue::new("error", format!("Brakuje części: osoby {list} z listy nie pojawiły się w żadnej części."), "Lista osób (manifest)").act("paste", None));
    }
    // References to persons that aren't in the batch.
    let check_ref = |issues: &mut Vec<Issue>, what: String, id: &str| {
        if !id.is_empty() && !persons.contains(id) {
            issues.push(Issue::new("error", format!("{what} wskazuje osobę {id}, której nie ma w paczce."), what));
        }
    };
    for e in &m.events {
        for role in &e.people {
            check_ref(&mut issues, format!("Zdarzenie {}", e.id), &role.p);
        }
        match e.kind.as_deref() {
            Some(kind) if format::EVENT_TYPES.contains(&kind) => {}
            other => issues.push(Issue::new(
                "warning",
                format!("Nieznany rodzaj zdarzenia „{}” — zapisano jako „inne”.", other.unwrap_or("brak")),
                format!("Zdarzenie {}", e.id),
            )),
        }
        if let Some(date) = &e.date {
            if format::gedcom_date(date).is_none() {
                let who = e.people.first().map(|r| person_label(m, &r.p)).unwrap_or_default();
                issues.push(Issue::new("warning", format!("Data „{date}” zapisana jako tekst — ustaw ją ręcznie."), format!("Zdarzenie {} · {who}", e.id)).act("fix", Some(e.id.clone())));
            }
        }
        if e.certainty.as_deref() == Some("low") {
            let who = e.people.first().map(|r| r.p.clone()).unwrap_or_default();
            let kind = crate::people::event_name(&import_tag(e.kind.as_deref()), None).to_lowercase();
            issues.push(Issue::new("warning", format!("Niska pewność: {kind} ({who})."), person_label(m, &who)).act("show", Some(who)));
        }
    }
    for r in &m.relationships {
        for id in [&r.parent, &r.child, &r.a, &r.b].into_iter().flatten() {
            check_ref(&mut issues, "Relacja".into(), id);
        }
    }
    for t in &m.texts {
        if let Some(p) = &t.person {
            check_ref(&mut issues, format!("Tekst {}", t.id), p);
        }
        // Mentions of ids that aren't in the batch become plain text.
        for (_, _, visible, id) in mention_links(t.md.as_deref().unwrap_or("")) {
            if !persons.contains(id.as_str()) && !id.starts_with('@') {
                issues.push(Issue::new("warning", format!("Wzmianka o „{visible}” wskazuje nieznaną osobę — zostanie zwykłym tekstem."), format!("Tekst {}", t.id)));
            }
        }
    }
    for q in &m.questions {
        for p in &q.about {
            check_ref(&mut issues, format!("Pytanie {}", q.id), p);
        }
    }
    // Unknown keys.
    for p in &m.persons {
        for key in p.extra.keys() {
            issues.push(Issue::new("warning", format!("Nieznane pole „{key}” (osoba {}) — zachowane, ale nieużywane.", p.id), person_label(m, &p.id)));
        }
    }
    // Possible duplicates inside the batch: same name and birth year.
    let birth_year = |id: &str| {
        m.events.iter().filter(|e| matches!(e.kind.as_deref(), Some("birth" | "baptism"))).filter(|e| e.people.iter().any(|r| r.p == id && r.role.as_deref().is_none_or(|x| x == "principal"))).find_map(|e| e.date.as_deref().and_then(|d| d.get(..4)).map(str::to_string))
    };
    for (a_index, a) in m.persons.iter().enumerate() {
        for b in m.persons.iter().skip(a_index + 1) {
            let key = |p: &format::Person| p.names.first().map(|n| heirloom_core::fold::fold(&format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default())));
            if key(a).is_some() && key(a) == key(b) && birth_year(&a.id) == birth_year(&b.id) {
                let name = a.names.first().map(|n| format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default())).unwrap_or_default();
                issues.push(Issue::new("warning", format!("{} i {} ({}) mogą być tą samą osobą.", a.id, b.id, name.trim()), format!("Osoby {}, {}", a.id, b.id)).act("show", Some(a.id.clone())));
            }
        }
    }
    // Files.
    let described: HashSet<u32> = m.media.iter().filter_map(|x| m_number(&x.id)).collect();
    let provided: HashSet<u32> = files.iter().filter_map(|f| f.m).collect();
    for media in &m.media {
        let Some(n) = m_number(&media.id) else { continue };
        if !provided.contains(&n) {
            // "Plik M004 · Marianna Nowak" (spec §4.11).
            let who = media.depicts.iter().chain(&media.about).next().and_then(|id| m.persons.iter().find(|p| p.id == *id)).and_then(|p| p.names.first()).map(|x| format!(" · {} {}", x.given.clone().unwrap_or_default(), x.surname.clone().unwrap_or_default()).trim_end().to_string()).unwrap_or_default();
            issues.push(Issue::new("warning", format!("Plik {} jest opisany w paczce, ale nie został dołączony.", m_label(n)), format!("Plik {}{who}", m_label(n))).act("show", Some(m_label(n))));
        }
    }
    // Files without an M-number: one warning for all of them (200 photos are not 200 warnings).
    let unnumbered: Vec<&str> = files.iter().filter(|f| f.m.is_none()).map(|f| f.name.as_str()).collect();
    match unnumbered.as_slice() {
        [] => {}
        [one] => issues.push(Issue::new("warning", format!("Plik „{one}” nie ma numeru M — pominąć czy przypisać ręcznie?"), format!("Plik {one}"))),
        many => {
            let shown = many.iter().take(3).copied().collect::<Vec<_>>().join(", ");
            let more = if many.len() > 3 { format!(" i {}", crate::count_pl(many.len() - 3, "inny", "inne", "innych")) } else { String::new() };
            issues.push(Issue::new(
                "warning",
                format!("{} numeru M — pominąć czy przypisać ręcznie?", crate::count_pl(many.len(), "plik nie ma", "pliki nie mają", "plików nie ma")),
                format!("Pliki {shown}{more}"),
            ));
        }
    }
    for f in files {
        match f.m {
            None => {}
            Some(n) if !described.contains(&n) && !m.media.is_empty() => {
                issues.push(Issue::new("info", format!("Plik {} nie jest opisany w paczce — przypiszesz go w kroku „Zdjęcia i pliki”.", m_label(n)), format!("Plik {}", f.name)))
            }
            _ => {}
        }
        if f.size > 100_000_000 {
            issues.push(Issue::new("warning", format!("Plik ma {} MB — dodawanie potrwa dłużej.", f.size / 1_000_000), format!("Plik {}", f.name)));
        }
    }
    for c in &m.coverage {
        if c.status.as_deref() == Some("unreadable") {
            let label = c.m.clone().unwrap_or_default();
            issues.push(Issue::new("warning", format!("Plik {label} oznaczono jako nieczytelny — sprawdź go ręcznie."), format!("Plik {label}")));
        }
    }
    (batch, issues)
}

/// The GEDCOM tag for an import event type.
pub fn import_tag(kind: Option<&str>) -> String {
    match kind.unwrap_or("other") {
        "birth" => "BIRT",
        "baptism" => "BAPM",
        "banns" => "MARB",
        "marriage" => "MARR",
        "divorce" => "DIV",
        "death" => "DEAT",
        "burial" => "BURI",
        "residence" => "RESI",
        "occupation" => "OCCU",
        "education" => "EDUC",
        "religion" => "RELI",
        "emigration" => "EMIG",
        "military" => "_MILT",
        _ => "EVEN",
    }
    .to_string()
}

/// `[visible](person:P12)` links: (start, end, visible, id).
pub fn mention_links(md: &str) -> Vec<(usize, usize, String, String)> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(rel) = md[search..].find("](person:") {
        let close = search + rel;
        let id_start = close + "](person:".len();
        let Some(len) = md[id_start..].find(')') else { break };
        search = id_start + len + 1;
        let Some(open) = md[..close].rfind('[') else { continue };
        out.push((open, id_start + len + 1, md[open + 1..close].to_string(), md[id_start..id_start + len].trim().to_string()));
    }
    out
}

/// Reads the files dropped with the batch: JSON and text answers become blocks, the rest are the batch's files.
pub fn read_inputs(texts: &[String], paths: &[PathBuf]) -> (Vec<Block>, Vec<InputFile>, Vec<String>) {
    let inputs = read_inputs_with(texts, paths, &[]);
    (inputs.blocks, inputs.files, inputs.raw)
}

/// Where an AI answer came from: the pasted text or a file.
#[derive(Debug, Clone)]
pub struct AnswerInput {
    /// The origin its blocks carry ("wklejony tekst" or the file name).
    pub origin: String,
    pub path: Option<PathBuf>,
    pub size: u64,
}

/// A dropped file (or pasted text) with the same contents as one read before it: left out (design 17d).
#[derive(Debug, Clone)]
pub struct SkippedInput {
    pub name: String,
    pub path: Option<PathBuf>,
    pub size: u64,
    pub same_as: String,
    pub answer: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub blocks: Vec<Block>,
    pub files: Vec<InputFile>,
    pub raw: Vec<String>,
    pub answers: Vec<AnswerInput>,
    pub skipped: Vec<SkippedInput>,
    /// Dropped paths that are gone (moved or deleted since): listed, never read as an empty file.
    pub missing: Vec<PathBuf>,
}

/// Files the system leaves in folders (thumbnail caches, folder settings, macOS extras, Office lock files): never part
/// of a package.
fn is_system_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(lower.as_str(), "thumbs.db" | "ehthumbs.db" | "desktop.ini" | ".ds_store" | "__macosx") || lower.starts_with("._") || lower.starts_with("~$")
}

/// Everything dropped at once (design 17d): AI answers (as .json, or text with the marker), photos, PDFs and notes.
/// Folders are opened; `exclude` leaves out files taken off the list; a second copy of the same contents is skipped.
pub fn read_inputs_with(texts: &[String], paths: &[PathBuf], exclude: &[PathBuf]) -> Inputs {
    let mut out = Inputs::default();
    // Contents already read: the answer texts, and the files by size (each hashed at most once, and only when a
    // second file of its size comes).
    let mut answer_texts: Vec<(String, String)> = Vec::new();
    let mut by_size: HashMap<u64, Vec<SeenFile>> = HashMap::new();
    for text in texts.iter().filter(|t| !t.trim().is_empty()) {
        if let Some((_, first)) = answer_texts.iter().find(|(t, _)| t.trim() == text.trim()) {
            out.skipped.push(SkippedInput { name: "Wklejony tekst".into(), path: None, size: text.len() as u64, same_as: first.clone(), answer: true });
            continue;
        }
        let origin = unique_origin(&out.answers, "wklejony tekst");
        answer_texts.push((text.clone(), origin.clone()));
        out.raw.push(text.clone());
        out.blocks.extend(extract_blocks(text, &origin));
        out.answers.push(AnswerInput { origin, path: None, size: text.len() as u64 });
    }
    // In the order they were dropped (a stack: the first path goes on top).
    let mut stack: Vec<PathBuf> = paths.iter().rev().cloned().collect();
    // A file dropped on its own and again inside its dropped folder is one file, not a copy of itself.
    let mut read: HashSet<PathBuf> = HashSet::new();
    while let Some(path) = stack.pop() {
        if exclude.contains(&path) || !read.insert(path.clone()) {
            continue;
        }
        if path.file_name().is_some_and(|n| is_system_file(&n.to_string_lossy())) {
            continue;
        }
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut list: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
                list.sort();
                stack.extend(list.into_iter().rev());
            }
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let lower = name.to_lowercase();
        let m = m_number(&name);
        let Ok(size) = std::fs::metadata(&path).map(|m| m.len()) else {
            out.missing.push(path);
            continue;
        };
        let is_answer_text = |content: &str| content.contains(MARKER) || content.to_lowercase().contains("```json") || content.contains("\"heirloom-import\"");
        if m.is_none() && (lower.ends_with(".json") || lower.ends_with(".txt") || lower.ends_with(".md")) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if lower.ends_with(".json") || is_answer_text(&content) {
                    if let Some((_, first)) = answer_texts.iter().find(|(t, _)| t.trim() == content.trim()) {
                        out.skipped.push(SkippedInput { name, path: Some(path), size, same_as: first.clone(), answer: true });
                        continue;
                    }
                    // Two answers can have the same file name (from two folders): each keeps its own origin.
                    let origin = unique_origin(&out.answers, &name);
                    answer_texts.push((content.clone(), origin.clone()));
                    out.raw.push(content.clone());
                    out.blocks.extend(extract_blocks(&content, &origin));
                    out.answers.push(AnswerInput { origin, path: Some(path), size });
                    continue;
                }
            }
        }
        // A file the batch names by another M-number is kept even if it is the same picture: the batch expects it.
        let mut ours: Option<Option<blake3::Hash>> = None;
        let same = by_size.get_mut(&size).and_then(|list| {
            list.iter_mut().filter(|f| m.is_none() || f.m == m).find_map(|first| {
                let mine = *ours.get_or_insert_with(|| hash_file(&path));
                let theirs = *first.hash.get_or_insert_with(|| hash_file(&first.path));
                (mine.is_some() && mine == theirs).then(|| first.name.clone())
            })
        });
        if let Some(first) = same {
            out.skipped.push(SkippedInput { name, path: Some(path), size, same_as: first, answer: false });
            continue;
        }
        by_size.entry(size).or_default().push(SeenFile { path: path.clone(), name: name.clone(), m, hash: ours });
        out.files.push(InputFile { path, name, m, size });
    }
    out
}

/// A file read in step 1, with its contents' hash once it was needed.
struct SeenFile {
    path: PathBuf,
    name: String,
    m: Option<u32>,
    hash: Option<Option<blake3::Hash>>,
}

/// Hashed as it is read, never held in memory whole.
fn hash_file(path: &Path) -> Option<blake3::Hash> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(hasher.finalize())
}

/// „odpowiedz.txt”, then „odpowiedz.txt (2)” for a second answer with the same name.
fn unique_origin(answers: &[AnswerInput], name: &str) -> String {
    let taken = |candidate: &str| answers.iter().any(|a| a.origin == candidate);
    if !taken(name) {
        return name.to_string();
    }
    (2..).map(|n| format!("{name} ({n})")).find(|c| !taken(c)).expect("some number is free")
}

/// What a dropped file looks like it is, by its extension: `photo`, `document` (PDF, Word…), `note` (.txt, .md
/// that is not an AI answer) or `other`.
pub fn file_kind(path: &Path) -> &'static str {
    let lower = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if is_image(path) {
        "photo"
    } else if matches!(lower.as_str(), "pdf" | "doc" | "docx" | "odt" | "rtf" | "djvu") {
        "document"
    } else if matches!(lower.as_str(), "txt" | "md") {
        "note"
    } else {
        "other"
    }
}

pub fn file_by_m<'a>(files: &'a [InputFile], id: &str) -> Option<&'a InputFile> {
    let n = m_number(id)?;
    files.iter().find(|f| f.m == Some(n))
}

pub fn is_image(path: &Path) -> bool {
    crate::gedwrite::media_type(&path.to_string_lossy()).starts_with("image/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../../../docs/IMPORT_FORMAT.md");

    fn example_json() -> String {
        extract_blocks(EXAMPLE, "doc").into_iter().next().unwrap().text
    }

    #[test]
    fn the_documented_example_passes() {
        let text = format!("Oto paczka.\n```json\n{}\n```\nPytania na koniec.", example_json());
        let (blocks, _, _) = read_inputs(&[text], &[]);
        let parsed = parse_blocks(&blocks);
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        let (batch, issues) = check(parsed, &[]);
        let errors: Vec<_> = issues.iter().filter(|i| i.level == "error").collect();
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(batch.merged.persons.len(), 4);
        assert_eq!(batch.name, "Kowalscy-Leczna-2026-10-01");
        assert!(issues.iter().any(|i| i.message.contains("Plik M001 jest opisany w paczce, ale nie został dołączony.")));
    }

    #[test]
    fn counts_the_ai_gave_are_told_in_good_polish() {
        let json = example_json().replace("\"persons\": 4", "\"persons\": 1").replace("\"events\": 3", "\"events\": 2");
        let (_, issues) = check(parse_blocks(&extract_blocks(&json, "x")), &[]);
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert!(messages.contains(&"AI podało 1 osobę, a w części 1 jest 4."), "{messages:?}");
        assert!(messages.contains(&"AI podało 2 zdarzenia, a w części 1 jest 3."), "{messages:?}");
    }

    #[test]
    fn letters_longer_in_lower_case_before_the_block_do_not_shift_it() {
        // "İ" takes one byte more in lower case; the block used to lose its first line.
        let text = format!("İlona (Łódź) przesyła:\n```json\n{}\n```", example_json());
        assert_eq!(extract_blocks(&text, "x")[0].text, example_json());
    }

    #[test]
    fn a_cut_off_part_is_an_error_and_never_repaired() {
        let json = example_json();
        let cut = &json[..json.len() / 2];
        let parsed = parse_blocks(&extract_blocks(&format!("```json\n{cut}"), "wklejony tekst"));
        assert!(parsed.parts.is_empty());
        assert!(parsed.issues[0].message.starts_with("Część 1 jest ucięta — brak znacznika końca"));
    }

    #[test]
    fn cosmetic_slips_are_repaired() {
        let json = example_json().replace("\"end\": {", "\"extra_trailing\": [1, 2,],\n  \"end\": {").replacen("\"format\"", "\u{201C}format\u{201D}", 1);
        let parsed = parse_blocks(&extract_blocks(&json, "x"));
        assert_eq!(parsed.parts.len(), 1, "{:?}", parsed.issues);
    }

    #[test]
    fn broken_references_and_duplicates() {
        let json = example_json().replace("{ \"p\": \"P3\", \"role\": \"mother\" }", "{ \"p\": \"P99\", \"role\": \"mother\" }").replace("\"id\": \"P4\"", "\"id\": \"P2\"");
        let (batch, issues) = check(parse_blocks(&extract_blocks(&json, "x")), &[]);
        let messages: Vec<&str> = issues.iter().filter(|i| i.level == "error").map(|i| i.message.as_str()).collect();
        assert!(messages.contains(&"Zdarzenie E1 wskazuje osobę P99, której nie ma w paczce."), "{messages:?}");
        assert!(messages.contains(&"Identyfikator P2 występuje dwa razy."));
        assert!(messages.iter().any(|m| m.starts_with("Brakuje części: osoby P4")));
        assert_eq!(batch.merged.persons.len(), 4);
    }

    #[test]
    fn a_wrong_format_or_version_names_its_part_and_file() {
        let not_ours = example_json().replace("\"heirloom-import\"", "\"cos-innego\"");
        let newer = example_json().replace("\"version\": \"1.0\"", "\"version\": \"2.0\"");
        let blocks = [extract_blocks(&not_ours, "inne.json"), extract_blocks(&newer, "nowa.json")].concat();
        let (_, issues) = check(parse_blocks(&blocks), &[]);
        let errors: Vec<(&str, &str)> = issues.iter().filter(|i| i.level == "error").map(|i| (i.reference.as_str(), i.message.as_str())).collect();
        assert!(errors.iter().any(|(r, m)| *r == "Część 1 · inne.json" && m.starts_with("To nie jest odpowiedź w formacie Heirloom.")), "{errors:?}");
        assert!(errors.iter().any(|(r, m)| *r == "Część 1 · nowa.json" && m.starts_with("Ta odpowiedź jest w wersji formatu 2.0")), "{errors:?}");
        assert!(!issues.iter().any(|i| i.reference.contains("Część 0")));
    }

    #[test]
    fn a_second_part_with_the_same_number_is_reported_not_dropped_silently() {
        let corrected = example_json().replace("Kowalski", "Kowalsky");
        let blocks = [extract_blocks(&example_json(), "czesc-1.json"), extract_blocks(&corrected, "czesc-1 poprawiona.json")].concat();
        let (batch, issues) = check(parse_blocks(&blocks), &[]);
        assert_eq!(batch.merged.persons.len(), 4, "the first one is used");
        let warning = issues.iter().find(|i| i.reference == "Część 1 · czesc-1 poprawiona.json").expect("reported");
        assert_eq!(warning.level, "warning");
        assert!(warning.message.contains("użyto tej z „czesc-1.json”"), "{}", warning.message);
    }

    #[test]
    fn files_without_an_m_number_give_one_warning() {
        let file = |name: &str| InputFile { path: PathBuf::from(name), name: name.into(), m: None, size: 10 };
        let files: Vec<InputFile> = (1..=5).map(|n| file(&format!("IMG_{n}.jpg"))).collect();
        let (_, issues) = check(parse_blocks(&extract_blocks(&example_json(), "x")), &files);
        let warnings: Vec<&Issue> = issues.iter().filter(|i| i.message.contains("numeru M")).collect();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].message, "5 plików nie ma numeru M — pominąć czy przypisać ręcznie?");
        assert_eq!(warnings[0].reference, "Pliki IMG_1.jpg, IMG_2.jpg, IMG_3.jpg i 2 inne");
        let (_, issues) = check(parse_blocks(&extract_blocks(&example_json(), "x")), &files[..1]);
        assert!(issues.iter().any(|i| i.message == "Plik „IMG_1.jpg” nie ma numeru M — pominąć czy przypisać ręcznie?"));
    }

    #[test]
    fn system_files_are_skipped_and_a_path_that_is_gone_is_not_an_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["M001 akt.jpg", "Thumbs.db", "desktop.ini", ".DS_Store", "._M001 akt.jpg"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let gone = dir.path().join("M002 slub.jpg");
        let inputs = read_inputs_with(&[], &[dir.path().to_path_buf(), gone.clone()], &[]);
        let names: Vec<&str> = inputs.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["M001 akt.jpg"]);
        assert_eq!(inputs.missing, [gone]);
    }

    #[test]
    fn m_numbers() {
        assert_eq!(m_number("M001 akt urodzenia.jpg"), Some(1));
        assert_eq!(m_number("m12.pdf"), Some(12));
        assert_eq!(m_number("Marianna.jpg"), None);
        assert_eq!(m_number("M01a.jpg"), None);
        assert_eq!(m_label(4), "M004");
    }
}
