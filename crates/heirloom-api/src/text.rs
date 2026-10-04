//! Texts written in simple Markdown with person mentions: `[Antoniego](person:<UID>)`. In the file mentions
//! use the person's UID (it survives renumbering by other programs); the interface works with record ids
//! (`person:@I12@`), so texts are translated on the way out and back.

use crate::derive::Derived;

/// Finds `](person:<id>)` links; returns (start of `[`, end after `)`, visible text, id).
fn links(body: &str) -> Vec<(usize, usize, String, String)> {
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let mut search = 0;
    while let Some(rel) = body[search..].find("](person:") {
        let close = search + rel;
        let id_start = close + "](person:".len();
        let Some(id_len) = body[id_start..].find(')') else { break };
        let id_end = id_start + id_len;
        // The matching `[` before the `]`.
        let open = body[..close].rfind('[');
        search = id_end + 1;
        let Some(open) = open else { continue };
        if body[open + 1..close].contains(']') || bytes.get(open.wrapping_sub(1)) == Some(&b'!') {
            continue;
        }
        out.push((open, id_end + 1, body[open + 1..close].to_string(), body[id_start..id_end].trim().to_string()));
    }
    out
}

pub(crate) fn rewrite(body: &str, map: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (start, end, text, id) in links(body) {
        out.push_str(&body[last..start]);
        match map(&id) {
            Some(new_id) => out.push_str(&format!("[{text}](person:{new_id})")),
            None => out.push_str(&text),
        }
        last = end;
    }
    out.push_str(&body[last..]);
    out
}

/// For display: UIDs become record ids. A mention of someone who isn't in the archive (any more) is marked
/// `person:!<id>`, so the screens can show that the link leads nowhere; saving keeps the original target.
pub fn display_markdown(d: &Derived, body: &str) -> String {
    rewrite(body, |id| {
        if let Some(&i) = d.view.uid_index.get(id) {
            return Some(d.xref(i).to_string());
        }
        // Already a record id (e.g. written by hand or by another tool).
        Some(d.index(id).map_or_else(|| format!("!{}", id.trim_start_matches('!')), |i| d.xref(i).to_string()))
    })
}

/// For storing: record ids become UIDs. People without a UID are returned, so the caller gives them one first.
pub fn storage_markdown(d: &Derived, body: &str, new_uids: &std::collections::HashMap<String, String>) -> String {
    rewrite(body, |id| {
        // A mention that led nowhere when the text was shown keeps its target, in case the person comes back.
        if let Some(missing) = id.strip_prefix('!') {
            return Some(missing.to_string());
        }
        if d.view.uid_index.contains_key(id) {
            return Some(id.to_string());
        }
        let i = d.index(id)?;
        d.view.model.persons[i].uid.clone().or_else(|| new_uids.get(d.xref(i)).cloned())
    })
}

/// Record ids mentioned in an interface text whose people have no UID yet.
pub fn mentioned_without_uid(d: &Derived, body: &str) -> Vec<String> {
    links(body)
        .into_iter()
        .filter_map(|(_, _, _, id)| {
            let i = d.index(&id)?;
            d.view.model.persons[i].uid.is_none().then(|| d.xref(i).to_string())
        })
        .collect()
}

/// The UIDs a stored text mentions.
pub fn mentioned_uids(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, _, _, id) in links(body) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// The text without Markdown marks and with mentions as their visible words.
pub fn plain(body: &str) -> String {
    let without_links = rewrite(body, |_| None);
    without_links.replace("**", "").replace(['*', '_'], "").replace("\r\n", "\n")
}

/// About `radius` characters of plain text around the first occurrence of `needle` (in the stored text):
/// "…Józef zaprzągł oba wozy dopiero zimą…".
pub fn excerpt(body: &str, needle: &str, radius: usize) -> String {
    let position = body.find(needle).unwrap_or(0);
    // Measure in the plain text: find the mention's visible text position approximately.
    let before = plain(&body[..body[..position].rfind('[').unwrap_or(position)]);
    let after_start = body[position..].find(')').map_or(position, |p| position + p + 1);
    let visible = links(body).into_iter().find(|(s, e, _, _)| *s <= position && position < *e).map(|(_, _, t, _)| t).unwrap_or_default();
    let after = plain(&body[after_start.min(body.len())..]);
    let before_chars: Vec<char> = before.chars().collect();
    let after_chars: Vec<char> = after.chars().collect();
    let start = before_chars.len().saturating_sub(radius);
    let end = after_chars.len().min(radius);
    let mut text = String::new();
    text.extend(&before_chars[start..]);
    text.push_str(&visible);
    text.extend(&after_chars[..end]);
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{}{text}{}", if start > 0 { "…" } else { "" }, if end < after_chars.len() { "…" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_rewrites_mentions() {
        let body = "Syn [Antoniego](person:u2) i [Agnieszki z Mazurów](person:@I3@), brat [Jana](person:zz).";
        let found: Vec<String> = links(body).into_iter().map(|l| l.3).collect();
        assert_eq!(found, ["u2", "@I3@", "zz"]);
        let rewritten = rewrite(body, |id| (id != "zz").then(|| format!("<{id}>")));
        assert_eq!(rewritten, "Syn [Antoniego](person:<u2>) i [Agnieszki z Mazurów](person:<@I3@>), brat Jana.");
        assert_eq!(plain("**Ważne:** syn [Antoniego](person:u2)."), "Ważne: syn Antoniego.");
    }

    #[test]
    fn a_mention_of_someone_missing_is_marked_and_kept() {
        let d = crate::derive::build(&heirloom_core::gedcom::Document::from_bytes(b"0 HEAD
0 @I1@ INDI
1 NAME Jan /Nowak/
1 UID u1
0 TRLR
").0);
        let shown = display_markdown(&d, "Syn [Jana](person:u1), brat [Piotra](person:u9).");
        assert_eq!(shown, "Syn [Jana](person:@I1@), brat [Piotra](person:!u9).");
        assert_eq!(storage_markdown(&d, &shown, &Default::default()), "Syn [Jana](person:u1), brat [Piotra](person:u9).");
    }

    #[test]
    fn excerpt_around_a_mention() {
        let body = "Zimą 1915 [Józef](person:u1) zaprzągł oba wozy i wyjechał na wschód.";
        assert_eq!(excerpt(body, "(person:u1)", 10), "Zimą 1915 Józef zaprzągł…");
        assert_eq!(excerpt(body, "(person:u1)", 4), "…915 Józef zap…");
    }
}
