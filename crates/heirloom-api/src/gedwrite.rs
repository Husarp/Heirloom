//! Small helpers for building and changing GEDCOM records in memory (the storage rules are in
//! docs/GEDCOM_EXTENSIONS.md).

use heirloom_core::gedcom::{Document, Node, Version};

pub const EXTENSION_URI: &str = "https://github.com/Husarp/Heirloom/blob/main/docs/GEDCOM_EXTENSIONS.md";

/// Every extension tag Heirloom writes; declared in HEAD.SCHMA before the first save (GEDCOM 7 rule).
pub const EXTENSION_TAGS: &[&str] = &[
    "_HLM_KIND", "_HLM_TITLE", "_HLM_DATE", "_HLM_PLAC", "_HLM_CERT", "_HLM_BASIS", "_HLM_JULIAN", "_HLM_AGE", "_HLM_ORIG",
    "_HLM_TAG", "_HLM_LINK", "_HLM_PROFILE", "_HLM_TRANSCRIPTION", "_HLM_TRANSLATION", "_HLM_PARISH", "_HLM_YEAR",
    "_HLM_AKT", "_HLM_URL", "_HLM_BATCH", "_HLM_NODATA",
];

const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// A new record id: the next free number for the prefix (`I`, `F`, `N`, `O`, `S`, `R`).
pub fn next_xref(doc: &Document, prefix: &str, taken: &[String]) -> String {
    let max = doc
        .records
        .iter()
        .filter_map(|r| r.xref.as_deref())
        .chain(taken.iter().map(String::as_str))
        .filter_map(|x| x.strip_prefix('@')?.strip_suffix('@')?.strip_prefix(prefix)?.parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    format!("@{prefix}{}@", max + 1)
}

/// `CHAN`/`CREA` with the current date and time (UTC).
pub fn stamp(tag: &str) -> Node {
    let now = heirloom_core::history::now();
    let year = now.get(0..4).unwrap_or("2026");
    let month = now.get(5..7).and_then(|m| m.parse::<usize>().ok()).unwrap_or(1);
    let day = now.get(8..10).and_then(|d| d.parse::<u32>().ok()).unwrap_or(1);
    let time = now.get(11..19).unwrap_or("00:00:00");
    Node::new(tag).with_child(
        Node::with_value("DATE", &format!("{day} {} {year}", MONTHS[month.clamp(1, 12) - 1])).with_child(Node::with_value("TIME", &format!("{time}Z"))),
    )
}

/// Marks a record as changed now.
pub fn touch(node: &mut Node) {
    node.children.retain(|c| c.tag != "CHAN");
    node.children.push(stamp("CHAN"));
}

pub fn new_uid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Sets (or removes, when `value` is empty) the first child with `tag`, keeping its substructures.
pub fn set_child_text(node: &mut Node, tag: &str, value: Option<&str>) {
    let value = value.map(str::trim).filter(|v| !v.is_empty());
    match (node.child_mut(tag), value) {
        (Some(child), Some(v)) => child.set_text(v, Version::V7),
        (Some(_), None) => {
            if let Some(pos) = node.children.iter().position(|c| c.tag == tag) {
                node.children.remove(pos);
            }
        }
        (None, Some(v)) => {
            let mut child = Node::new(tag);
            child.set_text(v, Version::V7);
            node.children.push(child);
        }
        (None, None) => {}
    }
}

/// A text node (`CONT` lines for line breaks).
pub fn text_node(tag: &str, text: &str) -> Node {
    let mut node = Node::new(tag);
    node.set_text(text, Version::V7);
    node
}

pub fn remove_children(node: &mut Node, tag: &str) {
    node.children.retain(|c| c.tag != tag);
}

/// Removes pointer lines `tag @xref@` (e.g. a CHIL from a family).
pub fn remove_pointer(node: &mut Node, tag: &str, xref: &str) {
    node.children.retain(|c| !(c.tag == tag && c.pointer() == Some(xref)));
}

pub fn has_pointer(node: &Node, tag: &str, xref: &str) -> bool {
    node.children.iter().any(|c| c.tag == tag && c.pointer() == Some(xref))
}

/// Inserts a child after the last child with one of the `after` tags (keeps records tidy: FAMS after FAMC…).
pub fn insert_after(node: &mut Node, after: &[&str], child: Node) {
    let pos = node.children.iter().rposition(|c| after.contains(&c.tag.as_str())).map_or(node.children.len(), |p| p + 1);
    node.children.insert(pos, child);
}

/// `Józef /Kowalski/`; slashes typed by the user are dropped (they mark the surname in GEDCOM).
pub fn name_value(given: &str, surname: &str) -> String {
    let given = given.replace('/', "").trim().to_string();
    let surname = surname.replace('/', "").trim().to_string();
    match (given.is_empty(), surname.is_empty()) {
        (_, true) => given,
        (true, false) => format!("/{surname}/"),
        (false, false) => format!("{given} /{surname}/"),
    }
}

/// GEDCOM 7 FILE values are URI references: spaces and a few reserved characters are percent-encoded; Polish
/// letters are kept as UTF-8.
pub fn file_uri(path: &str) -> String {
    let mut out = String::new();
    for c in path.replace('\\', "/").chars() {
        match c {
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            _ => out.push(c),
        }
    }
    out
}

/// The media type for a file name.
pub fn media_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "md" => "text/markdown",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

/// HEAD with every Heirloom extension tag declared (None when nothing is missing).
pub fn declared_head(doc: &Document) -> Option<Node> {
    let head = doc.head()?;
    let declared = |tag: &str| {
        head.child("SCHMA")
            .is_some_and(|s| s.children_tagged("TAG").any(|t| t.value.as_deref().and_then(|v| v.split_whitespace().next()) == Some(tag)))
    };
    if EXTENSION_TAGS.iter().all(|t| declared(t)) {
        return None;
    }
    let mut copy = Document::new_v7();
    copy.records = vec![head.clone()];
    for tag in EXTENSION_TAGS {
        copy.declare_extension(tag, &format!("{EXTENSION_URI}#{}", tag.to_ascii_lowercase()));
    }
    copy.records.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        let (doc, _) = Document::from_bytes(b"0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I7@ INDI\n0 @I12@ INDI\n0 @F2@ FAM\n0 TRLR\n");
        assert_eq!(next_xref(&doc, "I", &[]), "@I13@");
        assert_eq!(next_xref(&doc, "I", &["@I13@".into()]), "@I14@");
        assert_eq!(next_xref(&doc, "N", &[]), "@N1@");
        assert_eq!(name_value("Józef", "Kowalski"), "Józef /Kowalski/");
        assert_eq!(name_value("Józef", ""), "Józef");
        assert_eq!(file_uri("media\\Józef 1904 #2.jpg"), "media/Józef%201904%20%232.jpg");
        let head = declared_head(&doc).unwrap();
        assert_eq!(head.child("SCHMA").unwrap().children.len(), EXTENSION_TAGS.len());
        let stamped = stamp("CHAN");
        assert!(stamped.child("DATE").unwrap().child("TIME").unwrap().value.as_deref().unwrap().ends_with('Z'));
    }
}
