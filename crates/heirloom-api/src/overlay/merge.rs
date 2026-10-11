//! Three-way merge of the user's version („mine”) and the archive's („theirs”) against the base they both started from
//! (A.9). Records are compared by their key (xref, or tag and occurrence); a record changed on both sides is merged
//! child by child, and only what really clashes is a conflict, for the user to choose.

use crate::gedwrite::next_xref;
use heirloom_core::gedcom::{Document, Node};
use heirloom_core::history::{Key, keyed};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    /// The record's key (`@I12@`, or `HEAD`).
    pub key: String,
    pub tag: String,
    pub mine: Option<Node>,
    pub theirs: Option<Node>,
    pub base: Option<Node>,
    /// What is kept unless the user chooses otherwise: mine, or for a record deleted on one side the changed one.
    pub default: Side,
    /// The clashing places inside a record changed on both sides (`BIRT/DATE`); empty for a record deleted on one side.
    /// The choice applies to these only: every other change of both sides is kept either way.
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Mine,
    Theirs,
}

#[derive(Debug, Default)]
pub struct Merge {
    /// The merged records, with each conflict resolved by `choices` (or its default).
    pub records: Vec<Node>,
    pub conflicts: Vec<Conflict>,
    /// Records of mine renumbered because the archive added another record under the same xref: old → new.
    pub renumbered: Vec<(String, String)>,
}

/// The archive looks like another family altogether: under half of the base's people are still in it.
#[derive(Debug, PartialEq)]
pub struct TooDifferent {
    pub shared_percent: usize,
}

fn uid(node: &Node) -> Option<&str> {
    node.child_value("UID").or_else(|| node.child_value("_UID")).map(str::trim)
}

/// How many of the base's people are still in `theirs` (the same xref with the same UID, or the same UID elsewhere).
pub fn shared_people(base: &[Node], theirs: &[Node]) -> Option<usize> {
    let people: Vec<&Node> = base.iter().filter(|r| r.tag == "INDI").collect();
    if people.is_empty() {
        return None;
    }
    let by_xref: HashMap<&str, &Node> = theirs.iter().filter(|r| r.tag == "INDI").filter_map(|r| Some((r.xref.as_deref()?, r))).collect();
    let uids: HashSet<&str> = theirs.iter().filter(|r| r.tag == "INDI").filter_map(uid).collect();
    let kept = people
        .iter()
        .filter(|p| match (p.xref.as_deref().and_then(|x| by_xref.get(x)), uid(p)) {
            (Some(t), Some(u)) => uid(t) == Some(u) || uids.contains(u),
            (Some(_), None) => true,
            (None, Some(u)) => uids.contains(u),
            (None, None) => false,
        })
        .count();
    Some(kept * 100 / people.len())
}

/// Merges `mine` and `theirs` over `base` (all in one GEDCOM version). `choices`: key → side, for conflicts.
pub fn merge3(base: &[Node], mine: &[Node], theirs: &[Node], choices: &HashMap<String, Side>) -> Result<Merge, TooDifferent> {
    if let Some(shared) = shared_people(base, theirs) {
        if shared < 50 {
            return Err(TooDifferent { shared_percent: shared });
        }
    }
    let mut mine = mine.to_vec();
    let renumbered = renumber_collisions(base, &mut mine, theirs);
    let (kb, km, kt) = (keyed(base), keyed(&mine), keyed(theirs));
    let b: HashMap<&Key, &Node> = kb.iter().map(|(k, n)| (k, *n)).collect();
    let m: HashMap<&Key, &Node> = km.iter().map(|(k, n)| (k, *n)).collect();
    let t: HashMap<&Key, &Node> = kt.iter().map(|(k, n)| (k, *n)).collect();
    let mut out: Vec<Node> = Vec::new();
    let mut conflicts = Vec::new();
    let mut resolve = |key: &Key, out: &mut Vec<Node>| {
        let (bn, mn, tn) = (b.get(key).copied(), m.get(key).copied(), t.get(key).copied());
        let pick = if mn == bn {
            tn.cloned()
        } else if tn == bn || mn == tn {
            mn.cloned()
        } else {
            let head = mn.or(tn).is_some_and(|n| n.tag == "HEAD");
            let side = if head { Side::Mine } else { choices.get(&key.0).copied().unwrap_or(Side::Mine) };
            let mut fields = Vec::new();
            let merged = match (bn, mn, tn) {
                (Some(bn), Some(mn), Some(tn)) => merge_node(bn, mn, tn, side, &mut fields),
                _ => None,
            };
            match merged {
                Some(node) => {
                    // HEAD: mine, silently (no question for the user).
                    if !fields.is_empty() && !head {
                        conflicts.push(Conflict { key: key.0.clone(), tag: node.tag.clone(), mine: mn.cloned(), theirs: tn.cloned(), base: bn.cloned(), default: Side::Mine, fields });
                    }
                    Some(node)
                }
                None => {
                    // A record deleted on one side and changed on the other: keep the changed one, so nothing is lost.
                    let default = if mn.is_none() { Side::Theirs } else { Side::Mine };
                    let tag = mn.or(tn).map(|n| n.tag.clone()).unwrap_or_default();
                    // HEAD: mine, silently.
                    if tag == "HEAD" {
                        mn.cloned()
                    } else {
                        let side = choices.get(&key.0).copied().unwrap_or(default);
                        conflicts.push(Conflict { key: key.0.clone(), tag, mine: mn.cloned(), theirs: tn.cloned(), base: bn.cloned(), default, fields: Vec::new() });
                        match side {
                            Side::Mine => mn.cloned(),
                            Side::Theirs => tn.cloned(),
                        }
                    }
                }
            }
        };
        if let Some(node) = pick {
            out.push(node);
        }
    };
    // Theirs' order, then mine's new records (before TRLR) in mine's order.
    let mut done: HashSet<&Key> = HashSet::new();
    let mut trailer: Option<&Key> = None;
    for (key, node) in &kt {
        if node.tag == "TRLR" {
            trailer = Some(key);
            continue;
        }
        done.insert(key);
        resolve(key, &mut out);
    }
    for (key, node) in &km {
        if node.tag == "TRLR" || done.contains(key) {
            continue;
        }
        // In mine only: added by the user (kept), or deleted from theirs (resolved above as gone or a conflict).
        if t.contains_key(key) {
            continue;
        }
        done.insert(key);
        resolve(key, &mut out);
    }
    if let Some(key) = trailer.or_else(|| km.iter().find(|(_, n)| n.tag == "TRLR").map(|(k, _)| k)) {
        resolve(key, &mut out);
    }
    Ok(Merge { records: out, conflicts, renumbered })
}

/// Records both sides added under the same xref (each side numbers new people max + 1): mine's get a new number, and
/// every pointer to them in mine follows.
fn renumber_collisions(base: &[Node], mine: &mut [Node], theirs: &[Node]) -> Vec<(String, String)> {
    let base_xrefs: HashSet<&str> = base.iter().filter_map(|r| r.xref.as_deref()).collect();
    let theirs_by: HashMap<&str, &Node> = theirs.iter().filter_map(|r| Some((r.xref.as_deref()?, r))).collect();
    let clashes: Vec<String> = mine
        .iter()
        .filter_map(|r| r.xref.as_deref())
        .filter(|x| !base_xrefs.contains(x))
        .filter(|x| theirs_by.get(x).is_some_and(|t| mine.iter().find(|m| m.xref.as_deref() == Some(x)) != Some(*t)))
        .map(str::to_string)
        .collect();
    if clashes.is_empty() {
        return Vec::new();
    }
    let all = Document { records: base.iter().chain(mine.iter()).chain(theirs.iter()).cloned().collect(), ..Document::new_v7() };
    let mut taken: Vec<String> = Vec::new();
    let mut map = Vec::new();
    for old in clashes {
        let prefix: String = old.trim_matches('@').chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let new = next_xref(&all, &prefix, &taken);
        taken.push(new.clone());
        map.push((old, new));
    }
    let lookup: HashMap<&str, &str> = map.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    for record in mine.iter_mut() {
        rewrite_pointers(record, &lookup);
    }
    map
}

/// Changes the xref and every pointer named in `map`.
pub fn rewrite_pointers(node: &mut Node, map: &HashMap<&str, &str>) {
    if let Some(new) = node.xref.as_deref().and_then(|x| map.get(x)) {
        node.xref = Some(new.to_string());
    }
    if let Some(new) = node.pointer().and_then(|p| map.get(p)) {
        node.value = Some(new.to_string());
    }
    for child in &mut node.children {
        rewrite_pointers(child, map);
    }
}

/// One record (or child) changed on both sides, merged child by child. Where the two changes clash, `side` wins in that
/// place only and the place is added to `clashes`; None only when the two are not the same kind of record.
fn merge_node(base: &Node, mine: &Node, theirs: &Node, side: Side, clashes: &mut Vec<String>) -> Option<Node> {
    merge_at(base, mine, theirs, side, clashes, "")
}

fn merge_at(base: &Node, mine: &Node, theirs: &Node, side: Side, clashes: &mut Vec<String>, path: &str) -> Option<Node> {
    if mine.tag != theirs.tag || mine.xref != theirs.xref {
        return None;
    }
    let value = merge_value(&base.value, &mine.value, &theirs.value, side, clashes, path);
    // CHAN is never a conflict: the later change wins.
    let strip = |n: &Node| n.children.iter().filter(|c| c.tag != "CHAN").cloned().collect::<Vec<_>>();
    let mut children = merge_children(&strip(base), &strip(mine), &strip(theirs), side, clashes, path);
    let chan = later_chan(mine.child("CHAN"), theirs.child("CHAN"));
    if let Some(chan) = chan {
        let at = mine.children.iter().position(|c| c.tag == "CHAN").unwrap_or(children.len()).min(children.len());
        children.insert(at, chan.clone());
    }
    Some(Node { value, children, ..mine.clone() })
}

fn merge_value(base: &Option<String>, mine: &Option<String>, theirs: &Option<String>, side: Side, clashes: &mut Vec<String>, path: &str) -> Option<String> {
    if mine == base {
        theirs.clone()
    } else if theirs == base || mine == theirs {
        mine.clone()
    } else {
        clashes.push(path.to_string());
        if side == Side::Mine { mine.clone() } else { theirs.clone() }
    }
}

fn later_chan<'a>(mine: Option<&'a Node>, theirs: Option<&'a Node>) -> Option<&'a Node> {
    let when = |n: &Node| {
        let date = n.child("DATE");
        let sort = date.and_then(|d| d.value.as_deref()).and_then(heirloom_core::gedcom::date::parse).map(|v| v.sort_key()).unwrap_or(i64::MIN);
        let time = date.and_then(|d| d.child_value("TIME")).unwrap_or("").to_string();
        (sort, time)
    };
    match (mine, theirs) {
        (Some(m), Some(t)) => Some(if when(t) > when(m) { t } else { m }),
        (m, t) => m.or(t),
    }
}

/// A run of the base replaced by a side: base[start..end] → `with` (an insertion when start == end).
#[derive(Debug, Clone, PartialEq)]
struct Hunk {
    start: usize,
    end: usize,
    with: Vec<Node>,
}

/// What one side changed in a list of children, as hunks over the base (along a longest common subsequence).
fn hunks(base: &[Node], side: &[Node]) -> Vec<Hunk> {
    let map = lcs(base, side);
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    loop {
        let next = (i..base.len()).find(|&b| map[b].is_some());
        let (bend, send) = match next {
            Some(b) => (b, map[b].expect("found")),
            None => (base.len(), side.len()),
        };
        if bend > i && send > j {
            refine(&base[i..bend], &side[j..send], i, &mut out);
        } else if bend > i || send > j {
            out.push(Hunk { start: i, end: bend, with: side[j..send].to_vec() });
        }
        match next {
            Some(b) => (i, j) = (b + 1, send + 1),
            None => return out,
        }
    }
}

/// A run replaced by another run, split by tag: a fact edited and a neighbouring fact added are two changes, not one,
/// so a clash over the edited fact never swallows the added one. `at`: the run's place in the base.
fn refine(base: &[Node], side: &[Node], at: usize, out: &mut Vec<Hunk>) {
    let (n, m) = (base.len(), side.len());
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for x in (0..n).rev() {
        for y in (0..m).rev() {
            table[x][y] = if base[x].tag == side[y].tag { table[x + 1][y + 1] + 1 } else { table[x + 1][y].max(table[x][y + 1]) };
        }
    }
    let (mut x, mut y) = (0, 0);
    let (mut px, mut py) = (0, 0);
    let mut pairs = Vec::new();
    while x < n && y < m {
        if base[x].tag == side[y].tag {
            pairs.push((x, y));
            x += 1;
            y += 1;
        } else if table[x + 1][y] >= table[x][y + 1] {
            x += 1;
        } else {
            y += 1;
        }
    }
    for (x, y) in pairs {
        if x > px || y > py {
            out.push(Hunk { start: at + px, end: at + x, with: side[py..y].to_vec() });
        }
        out.push(Hunk { start: at + x, end: at + x + 1, with: vec![side[y].clone()] });
        (px, py) = (x + 1, y + 1);
    }
    if n > px || m > py {
        out.push(Hunk { start: at + px, end: at + n, with: side[py..m].to_vec() });
    }
}

/// Two changes touch the same children: overlapping runs, or two insertions at one place.
fn overlap(a: &Hunk, b: &Hunk) -> bool {
    match (a.start == a.end, b.start == b.end) {
        (true, true) => a.start == b.start,
        (true, false) => b.start < a.start && a.start < b.end,
        (false, true) => a.start < b.start && b.start < a.end,
        (false, false) => a.start < b.end && b.start < a.end,
    }
}

/// diff3 over children: a change made on one side only is taken; changes on both sides to different children are
/// both taken (two new facts added at one place: mine, then theirs); the same change on both is taken once; one child
/// changed on each side (same tag) is merged further down. Anything else clashes: there `side` wins and the place is
/// added to `clashes`; nothing outside the clash is ever dropped.
fn merge_children(base: &[Node], mine: &[Node], theirs: &[Node], side: Side, clashes: &mut Vec<String>, path: &str) -> Vec<Node> {
    if mine == base {
        return theirs.to_vec();
    }
    if theirs == base || mine == theirs {
        return mine.to_vec();
    }
    let (hm, ht) = (hunks(base, mine), hunks(base, theirs));
    let mut resolved: Vec<Hunk> = Vec::new();
    let mut used_theirs = vec![false; ht.len()];
    // The tag the hunks are about, for naming the clash.
    let place = |hs: &[&Hunk]| {
        let tag = hs.iter().find_map(|h| base[h.start..h.end].iter().chain(h.with.iter()).map(|n| n.tag.clone()).next()).unwrap_or_default();
        if path.is_empty() { tag } else { format!("{path}/{tag}") }
    };
    for m in &hm {
        let clashing: Vec<usize> = (0..ht.len()).filter(|&k| overlap(m, &ht[k])).collect();
        match clashing.as_slice() {
            [] => resolved.push(m.clone()),
            [k] if !used_theirs[*k] && m == &ht[*k] => {
                used_theirs[*k] = true;
                resolved.push(m.clone());
            }
            [k] if !used_theirs[*k] && m.start == m.end && ht[*k].start == ht[*k].end => {
                used_theirs[*k] = true;
                resolved.push(Hunk { start: m.start, end: m.end, with: m.with.iter().chain(&ht[*k].with).cloned().collect() });
            }
            [k] if !used_theirs[*k] && {
                let t = &ht[*k];
                (m.start, m.end) == (t.start, t.end) && m.end == m.start + 1 && m.with.len() == 1 && t.with.len() == 1 && m.with[0].tag == t.with[0].tag && base[m.start].tag == m.with[0].tag
            } =>
            {
                let t = &ht[*k];
                used_theirs[*k] = true;
                let child = format!("{}{}{}", path, if path.is_empty() { "" } else { "/" }, m.with[0].tag);
                let node = merge_at(&base[m.start], &m.with[0], &t.with[0], side, clashes, &child).unwrap_or_else(|| if side == Side::Mine { m.with[0].clone() } else { t.with[0].clone() });
                resolved.push(Hunk { start: m.start, end: m.end, with: vec![node] });
            }
            ks => {
                let mut all: Vec<&Hunk> = vec![m];
                all.extend(ks.iter().map(|&k| &ht[k]));
                clashes.push(place(&all));
                if side == Side::Mine {
                    resolved.push(m.clone());
                    for &k in ks {
                        used_theirs[k] = true;
                    }
                }
            }
        }
    }
    for (k, t) in ht.into_iter().enumerate() {
        if used_theirs[k] {
            continue;
        }
        if resolved.iter().any(|r| overlap(r, &t)) {
            // Only reachable after a widened merge above: the side chosen wins that place.
            clashes.push(place(&[&t]));
            if side == Side::Mine {
                continue;
            }
            resolved.retain(|r| !overlap(r, &t));
        }
        resolved.push(t);
    }
    // Insertions at a place come before a run replaced from that place.
    resolved.sort_by_key(|h| (h.start, h.end != h.start));
    let mut out = Vec::new();
    let mut at = 0;
    for h in resolved {
        out.extend_from_slice(&base[at..h.start.max(at)]);
        out.extend(h.with);
        at = at.max(h.end);
    }
    out.extend_from_slice(&base[at.min(base.len())..]);
    out
}

/// For each element of `a`, its index in `b` along a longest common subsequence.
fn lcs(a: &[Node], b: &[Node]) -> Vec<Option<usize>> {
    let (n, m) = (a.len(), b.len());
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for x in (0..n).rev() {
        for y in (0..m).rev() {
            table[x][y] = if a[x] == b[y] { table[x + 1][y + 1] + 1 } else { table[x + 1][y].max(table[x][y + 1]) };
        }
    }
    let mut out = vec![None; n];
    let (mut x, mut y) = (0, 0);
    while x < n && y < m {
        if a[x] == b[y] {
            out[x] = Some(y);
            x += 1;
            y += 1;
        } else if table[x + 1][y] >= table[x][y + 1] {
            x += 1;
        } else {
            y += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<Node> {
        Document::from_bytes(format!("0 HEAD\n1 GEDC\n2 VERS 7.0\n{text}0 TRLR\n").as_bytes()).0.records
    }

    fn text(records: &[Node]) -> String {
        records.iter().filter(|r| r.tag != "HEAD" && r.tag != "TRLR").map(heirloom_core::history::record_text).collect::<Vec<_>>().join("\n")
    }

    fn none() -> HashMap<String, Side> {
        HashMap::new()
    }

    const BASE: &str = "0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 BIRT\n2 DATE 1878\n0 @I2@ INDI\n1 NAME Anna /Nowak/\n0 @O1@ OBJE\n1 FILE media/a.jpg\n2 TITL Ślub\n";

    #[test]
    fn each_sides_changes_are_kept() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("Jan /Nowak/", "Jan Paweł /Nowak/"));
        let theirs = parse(&format!("{BASE}0 @I3@ INDI\n1 NAME Ewa /Nowak/\n").replace("Anna /Nowak/", "Anna /Kowalska/"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert!(merged.conflicts.is_empty());
        let out = text(&merged.records);
        assert!(out.contains("Jan Paweł") && out.contains("Anna /Kowalska/") && out.contains("Ewa /Nowak/"), "{out}");
        assert_eq!(merged.records.last().unwrap().tag, "TRLR");
    }

    #[test]
    fn a_birth_date_on_one_side_and_a_photo_link_on_the_other_merge() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("2 DATE 1878", "2 DATE 12 MAR 1878"));
        let theirs = parse(&BASE.replace("1 NAME Jan /Nowak/\n", "1 NAME Jan /Nowak/\n1 OBJE @O1@\n"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert!(merged.conflicts.is_empty());
        let jan = heirloom_core::history::record_text(&merged.records[1]);
        assert_eq!(jan, "0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 OBJE @O1@\n1 BIRT\n2 DATE 12 MAR 1878");
    }

    #[test]
    fn a_file_value_and_its_title_merge() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("1 FILE media/a.jpg", "1 FILE media/b.jpg"));
        let theirs = parse(&BASE.replace("2 TITL Ślub", "2 TITL Ślub Jana i Anny"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert!(merged.conflicts.is_empty());
        assert!(text(&merged.records).contains("1 FILE media/b.jpg\n2 TITL Ślub Jana i Anny"));
    }

    #[test]
    fn chan_is_never_a_conflict() {
        let base = parse(&BASE.replace("1 NAME Anna /Nowak/\n", "1 NAME Anna /Nowak/\n1 CHAN\n2 DATE 1 JAN 2026\n"));
        let mine = parse(&BASE.replace("1 NAME Anna /Nowak/\n", "1 NAME Anna /Nowak/\n1 SEX F\n1 CHAN\n2 DATE 3 OCT 2026\n"));
        let theirs = parse(&BASE.replace("1 NAME Anna /Nowak/\n", "1 NAME Anna /Nowak/\n1 CHAN\n2 DATE 5 OCT 2026\n1 NOTE x\n"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert!(merged.conflicts.is_empty(), "{:?}", merged.conflicts);
        let anna = heirloom_core::history::record_text(&merged.records[2]);
        assert!(anna.contains("1 SEX F") && anna.contains("1 NOTE x") && anna.contains("2 DATE 5 OCT 2026") && !anna.contains("3 OCT"), "{anna}");
    }

    #[test]
    fn a_real_conflict_is_reported_and_both_choices_apply() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("2 DATE 1878", "2 DATE 1879"));
        let theirs = parse(&BASE.replace("2 DATE 1878", "2 DATE 1880"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!((merged.conflicts[0].key.as_str(), merged.conflicts[0].default), ("@I1@", Side::Mine));
        assert!(text(&merged.records).contains("DATE 1879"));
        let theirs_chosen = merge3(&base, &mine, &theirs, &HashMap::from([("@I1@".to_string(), Side::Theirs)])).unwrap();
        assert!(text(&theirs_chosen.records).contains("DATE 1880"));
    }

    /// Both changed Jan's birth date (a clash); mine also added a SEX, theirs an OCCU: both are kept, whatever is chosen.
    #[test]
    fn a_choice_covers_only_the_clashing_field() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("2 DATE 1878", "2 DATE 1879").replace("1 NAME Jan /Nowak/\n", "1 NAME Jan /Nowak/\n1 SEX M\n"));
        let theirs = parse(&BASE.replace("2 DATE 1878", "2 DATE 1880").replace("1 BIRT", "1 OCCU Stolarz\n1 BIRT"));
        for (side, date, other) in [(Side::Mine, "DATE 1879", "DATE 1880"), (Side::Theirs, "DATE 1880", "DATE 1879")] {
            let merged = merge3(&base, &mine, &theirs, &HashMap::from([("@I1@".to_string(), side)])).unwrap();
            assert_eq!(merged.conflicts.len(), 1);
            assert_eq!(merged.conflicts[0].fields, ["BIRT/DATE"]);
            let out = text(&merged.records);
            assert!(out.contains(date) && !out.contains(other), "{out}");
            assert!(out.contains("1 SEX M") && out.contains("1 OCCU Stolarz"), "both sides' other edits are kept: {out}");
            // Merging the result again changes nothing.
            let again = merge3(&theirs, &merged.records, &theirs, &none()).unwrap();
            assert_eq!(again.records, merged.records);
            assert!(again.conflicts.is_empty());
        }
    }

    #[test]
    fn a_clash_of_a_whole_run_of_children_follows_the_choice() {
        let base = parse(BASE);
        // Both rewrote the name; each also added a separate fact.
        let mine = parse(&BASE.replace("Jan /Nowak/", "Jan Paweł /Nowak/").replace("0 @I2@ INDI", "1 NOTE moja\n0 @I2@ INDI"));
        let theirs = parse(&BASE.replace("Jan /Nowak/", "Janek /Nowak/").replace("0 @I2@ INDI", "1 OCCU Stolarz\n0 @I2@ INDI"));
        for side in [Side::Mine, Side::Theirs] {
            let merged = merge3(&base, &mine, &theirs, &HashMap::from([("@I1@".to_string(), side)])).unwrap();
            let out = text(&merged.records);
            assert!(out.contains("NOTE moja") && out.contains("OCCU Stolarz"), "{out}");
            assert_eq!(out.contains("Jan Paweł"), side == Side::Mine, "{out}");
            assert_eq!(out.contains("Janek"), side == Side::Theirs, "{out}");
        }
    }

    #[test]
    fn a_fact_deleted_on_one_side_and_edited_on_the_other_follows_the_choice_and_keeps_the_rest() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("2 DATE 1878", "2 DATE 1879").replace("0 @I2@ INDI", "1 NOTE moja\n0 @I2@ INDI"));
        let theirs = parse(&BASE.replace("1 BIRT\n2 DATE 1878\n", "").replace("1 NAME Jan /Nowak/", "1 NAME Janek /Nowak/"));
        let kept = merge3(&base, &mine, &theirs, &none()).unwrap();
        let out = text(&kept.records);
        assert!(out.contains("DATE 1879") && out.contains("NOTE moja") && out.contains("Janek"), "{out}");
        let gone = merge3(&base, &mine, &theirs, &HashMap::from([("@I1@".to_string(), Side::Theirs)])).unwrap();
        let out = text(&gone.records);
        assert!(!out.contains("BIRT") && out.contains("NOTE moja") && out.contains("Janek"), "{out}");
    }

    #[test]
    fn a_deletion_against_a_change_keeps_the_record() {
        let base = parse(BASE);
        let mine = parse(&BASE.replace("0 @I2@ INDI\n1 NAME Anna /Nowak/\n", ""));
        let theirs = parse(&BASE.replace("Anna /Nowak/", "Anna /Kowalska/"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!(merged.conflicts[0].default, Side::Theirs);
        assert!(text(&merged.records).contains("Anna /Kowalska/"));
        // Deleted in mine, untouched in theirs: simply gone.
        let merged = merge3(&base, &mine, &base, &none()).unwrap();
        assert!(!text(&merged.records).contains("Anna"));
    }

    #[test]
    fn a_record_both_sides_added_renumbers_mine_and_its_pointers() {
        let base = parse(BASE);
        let mine = parse(&format!("{BASE}0 @I3@ INDI\n1 NAME Ewa /Nowak/\n1 FAMS @F1@\n0 @F1@ FAM\n1 WIFE @I3@\n"));
        let theirs = parse(&format!("{BASE}0 @I3@ INDI\n1 NAME Piotr /Nowak/\n"));
        let merged = merge3(&base, &mine, &theirs, &none()).unwrap();
        assert!(merged.conflicts.is_empty());
        assert_eq!(merged.renumbered, [("@I3@".to_string(), "@I4@".to_string())]);
        let out = text(&merged.records);
        assert!(out.contains("0 @I3@ INDI\n1 NAME Piotr") && out.contains("0 @I4@ INDI\n1 NAME Ewa") && out.contains("1 WIFE @I4@"), "{out}");
        // Merging the result again changes nothing.
        let again = merge3(&theirs, &merged.records, &theirs, &none()).unwrap();
        assert_eq!(again.records, merged.records);
        assert!(again.renumbered.is_empty());
    }

    #[test]
    fn a_completely_different_archive_is_refused() {
        let base = parse(BASE);
        let theirs = parse("0 @I7@ INDI\n1 NAME Zofia /Wiśniewska/\n");
        assert_eq!(merge3(&base, &base, &theirs, &none()).unwrap_err(), TooDifferent { shared_percent: 0 });
    }
}
