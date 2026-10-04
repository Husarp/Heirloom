//! „To ta sama osoba?”: people of different archives of a set who look like one person, scored with the Import's
//! matching (RESEARCH §8 points and vetoes). Only each other's best match is offered, never one person for several.

use crate::derive::Derived;
use crate::import::matching::{self, Candidate, Incoming, first_word, incoming_from, levenshtein, score};
use crate::people;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

/// Below this nothing is offered (the Import's „review” threshold).
pub const MIN_SCORE: f64 = 4.0;
pub const SAME_UID: &str = "ten sam identyfikator (kopia tego samego wpisu)";

#[derive(Debug, Clone)]
pub struct Pair {
    pub a: String,
    pub b: String,
    pub score: f64,
    pub percent: u32,
    pub reasons: Vec<String>,
    /// Both people as the review card shows them, worked out when the pair was found.
    pub sides: [Value; 2],
}

impl Pair {
    pub fn json(&self) -> Value {
        json!({
            "a": self.a,
            "b": self.b,
            "percent": self.percent,
            "score": self.score,
            "status": if self.score >= 8.0 { "match" } else { "review" },
            "reasons": self.reasons,
            "sides": self.sides,
        })
    }
}

/// A person for the side-by-side card: the summary, parents, partners and children.
pub fn side(d: &Derived, i: usize) -> Value {
    let names = |list: &[usize]| list.iter().map(|&x| json!({ "id": d.xref(x), "name": d.info[x].name, "years": people::years_range(d, x) })).collect::<Vec<_>>();
    json!({
        "person": d.summary(i),
        "years": people::years_range(d, i),
        "parents": names(&d.info[i].parents),
        "partners": names(&d.info[i].partners),
        "children": names(&d.info[i].children),
    })
}

fn disjoint(d: &Derived, i: usize, j: usize) -> bool {
    let (a, b) = (&d.origins[i], &d.origins[j]);
    !a.iter().any(|k| b.contains(k))
}

/// The UID as the archive wrote it (the merged document puts the archive's key in front).
fn raw_uid(d: &Derived, i: usize) -> Option<&str> {
    let uid = d.view.model.persons[i].uid.as_deref()?;
    Some(uid.split_once('~').map_or(uid, |(_, u)| u))
}

fn percent(score: f64) -> u32 {
    ((score / 12.0) * 100.0).round().clamp(1.0, 99.0) as u32
}

/// Relatives two people share in the combined view can only be a linked person: then their family supports them.
fn family_boost(d: &Derived, i: usize, j: usize, c: &mut Candidate) {
    let given = (first_word(&d.info[i].given), first_word(&d.info[j].given));
    if given.0.is_empty() || given.0 != given.1 {
        return;
    }
    let shares = |a: &[usize], b: &[usize]| a.iter().any(|x| b.contains(x));
    let (x, y) = (&d.info[i], &d.info[j]);
    let reason = if shares(&x.children, &y.children) {
        "rodzic połączonej osoby"
    } else if shares(&x.parents, &y.parents) {
        "dziecko połączonej osoby"
    } else if shares(&x.partners, &y.partners) {
        "małżonek połączonej osoby"
    } else {
        return;
    };
    c.score += 3.0;
    c.percent = percent(c.score);
    c.reasons.push(reason.to_string());
}

/// Scores `j` for `i` (None below anything worth showing): the Import's points, the family boost, the same UID.
fn compare(d: &Derived, inc: &Incoming, keys_j: &[String], i: usize, j: usize, similar: &HashSet<&String>) -> Option<Candidate> {
    let same_uid = raw_uid(d, i).is_some_and(|u| raw_uid(d, j) == Some(u));
    let mut c = score(d, keys_j, j, inc, similar);
    if let Some(c) = &mut c {
        family_boost(d, i, j, c);
    }
    if same_uid {
        let mut c = c.unwrap_or(Candidate { xref: d.xref(j).to_string(), score: 0.0, percent: 0, reasons: Vec::new() });
        c.score = c.score.max(12.0);
        c.percent = 100;
        c.reasons.insert(0, SAME_UID.to_string());
        return Some(c);
    }
    c
}

/// Similar surname keys (up to 2 letters apart, at least 4 letters), worked out once over the distinct keys.
fn similar_keys(keys: &[Vec<String>]) -> HashMap<&String, Vec<&String>> {
    let mut distinct: Vec<(&String, usize)> = keys.iter().flatten().collect::<HashSet<_>>().into_iter().map(|k| (k, k.chars().count())).collect();
    distinct.sort_by_key(|&(k, n)| (n, k));
    let mut out: HashMap<&String, Vec<&String>> = HashMap::new();
    for (x, &(a, na)) in distinct.iter().enumerate() {
        if na < 4 {
            continue;
        }
        for &(b, _) in distinct[x + 1..].iter().take_while(|(_, nb)| *nb <= na + 2) {
            if levenshtein(a, b) <= 2 {
                out.entry(a).or_default().push(b);
                out.entry(b).or_default().push(a);
            }
        }
    }
    out
}

/// Every pair worth asking about, best first. `rejected` holds the pairs answered „nie” (ids of `d`, either order).
pub fn find(d: &Derived, rejected: &HashSet<(String, String)>) -> Vec<Pair> {
    let n = d.info.len();
    let keys = matching::surname_keys(d);
    // People by surname, then by first name: only first names that agree or are close are compared (people whose
    // first names differ are mostly siblings, and comparing everyone of a surname took far too long in big archives).
    let mut given_ids: HashMap<String, usize> = HashMap::new();
    let given: Vec<usize> = (0..n)
        .map(|i| {
            let next = given_ids.len();
            *given_ids.entry(first_word(&d.info[i].given)).or_insert(next)
        })
        .collect();
    let names: Vec<String> = {
        let mut v = vec![String::new(); given_ids.len()];
        for (g, &id) in &given_ids {
            v[id] = g.clone();
        }
        v
    };
    let mut close: HashMap<(usize, usize), bool> = HashMap::new();
    let mut is_close = |a: usize, b: usize| {
        *close.entry((a.min(b), a.max(b))).or_insert_with(|| {
            let (x, y) = (&names[a], &names[b]);
            x.is_empty() || y.is_empty() || x.starts_with(y.as_str()) || y.starts_with(x.as_str()) || levenshtein(x, y) <= 1
        })
    };
    let mut by_key: HashMap<&String, HashMap<usize, Vec<usize>>> = HashMap::new();
    for (i, list) in keys.iter().enumerate() {
        for k in list {
            by_key.entry(k).or_default().entry(given[i]).or_default().push(i);
        }
    }
    let mut by_uid: HashMap<&str, Vec<usize>> = HashMap::new();
    for i in 0..n {
        if let Some(uid) = raw_uid(d, i) {
            by_uid.entry(uid).or_default().push(i);
        }
    }
    let similar = similar_keys(&keys);
    let refused: HashSet<(usize, usize)> = rejected.iter().filter_map(|(a, b)| Some((d.index(a)?, d.index(b)?))).flat_map(|(a, b)| [(a, b), (b, a)]).collect();
    let mut best: Vec<Option<Candidate>> = vec![None; n];
    let mut seen = vec![usize::MAX; n];
    for i in 0..n {
        if keys[i].is_empty() && raw_uid(d, i).is_none() {
            continue;
        }
        let near: HashSet<&String> = keys[i].iter().filter_map(|k| similar.get(k)).flatten().copied().collect();
        let mut options: Vec<usize> = raw_uid(d, i).and_then(|u| by_uid.get(u)).cloned().unwrap_or_default();
        for k in keys[i].iter().chain(near.iter().copied()) {
            for (&g, list) in by_key.get(k).into_iter().flatten() {
                if is_close(given[i], g) {
                    options.extend(list);
                }
            }
        }
        let inc = incoming_from(d, i, keys[i].clone());
        for j in options {
            if seen[j] == i || j == i || !disjoint(d, i, j) || refused.contains(&(i, j)) {
                continue;
            }
            seen[j] = i;
            if let Some(c) = compare(d, &inc, &keys[j], i, j, &near) {
                if best[i].as_ref().is_none_or(|b| c.score > b.score) {
                    best[i] = Some(c);
                }
            }
        }
    }
    let mut pairs = Vec::new();
    for i in 0..n {
        let Some(c) = &best[i] else { continue };
        let Some(j) = d.index(&c.xref) else { continue };
        // Only each other's best (and each pair once).
        let mutual = best[j].as_ref().is_some_and(|b| b.xref == d.xref(i));
        if c.score >= MIN_SCORE && mutual && i < j {
            pairs.push(Pair { a: d.xref(i).to_string(), b: c.xref.clone(), score: c.score, percent: c.percent, reasons: c.reasons.clone(), sides: [side(d, i), side(d, j)] });
        }
    }
    sort(&mut pairs);
    pairs
}

pub fn sort(pairs: &mut [Pair]) {
    pairs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.a.cmp(&b.a)));
}

/// The points even when the surnames differ (a wife under her maiden name in one archive, her married one in the
/// other): the surname's points are taken off again and „inne nazwisko” said instead. The vetoes still hold.
fn score_any_surname(d: &Derived, i: usize, j: usize) -> Option<Candidate> {
    let (own, theirs) = (matching::surname_keys_of(d, i), matching::surname_keys_of(d, j));
    let mut c = score(d, &theirs, j, &incoming_from(d, i, theirs.clone()), &HashSet::new())?;
    if !own.iter().any(|k| theirs.contains(k)) {
        c.score -= 2.0;
        c.percent = percent(c.score.max(0.0));
        c.reasons[0] = "inne nazwisko".to_string();
    }
    Some(c)
}

/// Two chosen people („Połącz z osobą z innego archiwum…”): the points either way, the better one.
pub fn compare_two(d: &Derived, i: usize, j: usize) -> Option<Candidate> {
    let keys = matching::surname_keys(d);
    let similar = similar_keys(&keys);
    let near = |i: usize| keys[i].iter().filter_map(|k| similar.get(k)).flatten().copied().collect::<HashSet<&String>>();
    let one_way = |i: usize, j: usize| compare(d, &incoming_from(d, i, keys[i].clone()), &keys[j], i, j, &near(i));
    let best = match (one_way(i, j), one_way(j, i)) {
        (Some(x), Some(y)) => Some(if y.score > x.score { y } else { x }),
        (x, y) => x.or(y),
    };
    best.or_else(|| score_any_surname(d, i, j))
}

/// After `a` and `b` are linked: their parents, partners and children of the two archives with the same first name
/// come up next (the family boost), without working out every pair again. Uses `d` from before the link.
pub fn after_link(d: &Derived, pairs: &mut Vec<Pair>, a: usize, b: usize, rejected: &HashSet<(String, String)>) {
    let (x, y) = (&d.info[a], &d.info[b]);
    let reason = [(&x.parents, &y.parents, "rodzic połączonej osoby"), (&x.children, &y.children, "dziecko połączonej osoby"), (&x.partners, &y.partners, "małżonek połączonej osoby")];
    for (left, right, why) in reason {
        for &i in left {
            for &j in right {
                let given = first_word(&d.info[i].given);
                let refused = rejected.contains(&(d.xref(i).to_string(), d.xref(j).to_string())) || rejected.contains(&(d.xref(j).to_string(), d.xref(i).to_string()));
                if given.is_empty() || given != first_word(&d.info[j].given) || !disjoint(d, i, j) || refused {
                    continue;
                }
                let Some(mut c) = score_any_surname(d, i, j) else { continue };
                c.score += 3.0;
                c.percent = percent(c.score);
                c.reasons.push(why.to_string());
                let (ia, ib) = (d.xref(i), d.xref(j));
                let involved = |p: &Pair| [ia, ib].contains(&p.a.as_str()) || [ia, ib].contains(&p.b.as_str());
                if c.score < MIN_SCORE || pairs.iter().any(|p| involved(p) && p.score >= c.score) {
                    continue;
                }
                pairs.retain(|p| !involved(p));
                pairs.push(Pair { a: ia.to_string(), b: ib.to_string(), score: c.score, percent: c.percent, reasons: c.reasons, sides: [side(d, i), side(d, j)] });
            }
        }
    }
    sort(pairs);
}
