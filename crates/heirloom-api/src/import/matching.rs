//! Import step 3: who in the batch is already in the archive. Points per signal follow RESEARCH §8: names, birth
//! year, place, parents and spouse, with vetoes for a sex conflict and a birth year more than 10 years apart.
//! Nothing is merged automatically; a suggested match still needs a click.

use super::format::{Part, Person};
use crate::derive::Derived;
use heirloom_core::fold::fold;
use heirloom_core::gedcom::model::Sex;
use heirloom_core::polish;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// The person's record id: the archive can change while the import waits, and an index into the view built
    /// then would point at someone else (or past the end).
    pub xref: String,
    pub score: f64,
    pub percent: u32,
    pub reasons: Vec<String>,
}

/// ≥ 8 points: suggested match; 4–8: needs a decision; below 4: a new person.
pub fn status_for(best: Option<&Candidate>) -> &'static str {
    match best.map(|c| c.score) {
        Some(s) if s >= 8.0 => "match",
        Some(s) if s >= 4.0 => "review",
        _ => "new",
    }
}

/// What the batch says about one incoming person.
struct Incoming {
    given: String,
    surnames: Vec<String>,
    sex: Option<Sex>,
    birth_year: Option<i32>,
    birth_place: Option<String>,
    father: Option<String>,
    mother: Option<String>,
    spouses: Vec<String>,
}

fn first_word(s: &str) -> String {
    fold(s.split_whitespace().next().unwrap_or(""))
}

fn sex_of(code: Option<&str>) -> Option<Sex> {
    match code {
        Some("M") => Some(Sex::Male),
        Some("F") => Some(Sex::Female),
        _ => None,
    }
}

fn incoming(batch: &Part, person: &Person) -> Incoming {
    let given = person.names.iter().find_map(|n| n.given.clone()).unwrap_or_default();
    let surnames = person.names.iter().filter_map(|n| n.surname.as_deref()).map(polish::surname_key).filter(|k| !k.is_empty()).collect();
    let birth = batch
        .events
        .iter()
        .filter(|e| matches!(e.kind.as_deref(), Some("birth" | "baptism")))
        .find(|e| e.people.iter().any(|r| r.p == person.id && r.role.as_deref().is_none_or(|x| x == "principal")));
    let birth_year = birth.and_then(|e| e.date.as_deref()).and_then(|d| d.split_whitespace().find_map(|t| t.get(..4)?.parse().ok()));
    let birth_place = birth.and_then(|e| e.place.clone()).map(|p| fold(p.split(',').next().unwrap_or(&p).trim()));
    let given_of = |id: &str| batch.persons.iter().find(|p| p.id == id).and_then(|p| p.names.iter().find_map(|n| n.given.clone())).map(|g| first_word(&g));
    let sex_of_id = |id: &str| batch.persons.iter().find(|p| p.id == id).and_then(|p| sex_of(p.sex.as_deref()));
    let mut father = None;
    let mut mother = None;
    let mut spouses = Vec::new();
    for r in &batch.relationships {
        match r.relation.as_deref() {
            Some("parent") if r.child.as_deref() == Some(person.id.as_str()) => {
                let parent = r.parent.as_deref().unwrap_or("");
                match sex_of_id(parent) {
                    Some(Sex::Female) => mother = given_of(parent),
                    _ => father = given_of(parent),
                }
            }
            Some("partners") => {
                let other = if r.a.as_deref() == Some(person.id.as_str()) { r.b.as_deref() } else if r.b.as_deref() == Some(person.id.as_str()) { r.a.as_deref() } else { None };
                if let Some(g) = other.and_then(given_of) {
                    spouses.push(g);
                }
            }
            _ => {}
        }
    }
    Incoming { given: first_word(&given), surnames, sex: sex_of(person.sex.as_deref()), birth_year, birth_place, father, mother, spouses }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut row = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            row.push((prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(row[j] + 1));
        }
        prev = row;
    }
    prev[b.len()]
}

/// Every archive person's surname keys (all forms of all names), worked out once for a whole batch: for each incoming
/// person again, they took most of the time of a load into 10,000 people.
pub fn surname_keys(d: &Derived) -> Vec<Vec<String>> {
    d.view.model.persons.iter().map(|p| p.names.iter().map(|n| polish::surname_key(&n.surname)).filter(|k| !k.is_empty()).collect()).collect()
}

/// The best existing people for an incoming one, highest score first (at most 3). `keys` from [`surname_keys`].
pub fn candidates(d: &Derived, keys: &[Vec<String>], batch: &Part, person: &Person) -> Vec<Candidate> {
    let inc = incoming(batch, person);
    if inc.given.is_empty() && inc.surnames.is_empty() {
        return Vec::new();
    }
    // Similar surnames (up to 2 letters apart), compared once per surname of the archive rather than once per person.
    let distinct: HashSet<&String> = keys.iter().flatten().collect();
    let similar: HashSet<&String> = distinct
        .into_iter()
        .filter(|k| inc.surnames.iter().any(|s| s.chars().count() >= 4 && s.chars().count().abs_diff(k.chars().count()) <= 2 && levenshtein(s, k) <= 2))
        .collect();
    let mut out = Vec::new();
    for (i, keys) in keys.iter().enumerate().take(d.view.model.persons.len()) {
        let (existing, info) = (&d.view.model.persons[i], &d.info[i]);
        let mut score: f64 = 0.0;
        let mut reasons = Vec::new();
        // Surname: any form of any of the person's names.
        if inc.surnames.iter().any(|s| keys.contains(s)) {
            score += 2.0;
            reasons.push("nazwisko zgodne".to_string());
        } else if keys.iter().any(|k| similar.contains(k)) {
            score += 1.0;
            reasons.push("nazwisko podobne".to_string());
        } else {
            continue;
        }
        // Sex conflict: a veto.
        if let (Some(a), b) = (inc.sex, existing.sex) {
            if matches!(b, Sex::Male | Sex::Female) && a != b {
                continue;
            }
        }
        let given = first_word(&info.given);
        if !inc.given.is_empty() && !given.is_empty() {
            if inc.given == given {
                score += 3.0;
                reasons.push("imię zgodne".to_string());
            } else if inc.given.starts_with(&given) || given.starts_with(&inc.given) || levenshtein(&inc.given, &given) <= 1 {
                score += 1.5;
                reasons.push("imię podobne".to_string());
            } else {
                score -= 3.0;
                reasons.push("inne imię".to_string());
            }
        }
        let year = info.birth.as_ref().and_then(|b| b.value.map(|v| v.start.year));
        if let (Some(a), Some(b)) = (inc.birth_year, year) {
            let diff = (a - b).abs();
            if diff > 10 {
                continue;
            }
            let (points, reason) = match diff {
                0 => (2.0, "ten sam rok urodzenia".to_string()),
                1..=2 => (1.0, format!("rok urodzenia ±{diff}")),
                3..=5 => (0.0, format!("rok urodzenia ±{diff}")),
                _ => (-2.0, format!("rok urodzenia różni się o {diff} lat")),
            };
            score += points;
            reasons.push(reason);
        }
        if let (Some(a), Some(b)) = (&inc.birth_place, &info.birth_place) {
            if *a == fold(b.split(',').next().unwrap_or(b).trim()) {
                score += 1.5;
                reasons.push("to samo miejsce urodzenia".to_string());
            }
        }
        let parent_given = |female: bool| {
            info.parents.iter().find(|&&p| (d.view.model.persons[p].sex == Sex::Female) == female).map(|&p| first_word(&d.info[p].given))
        };
        if let (Some(a), Some(b)) = (&inc.father, parent_given(false)) {
            if *a == b {
                score += 2.0;
                reasons.push("imię ojca zgodne".to_string());
            } else {
                score -= 2.0;
                reasons.push("inne imię ojca".to_string());
            }
        }
        if let (Some(a), Some(b)) = (&inc.mother, parent_given(true)) {
            if *a == b {
                score += 2.5;
                reasons.push("imię matki zgodne".to_string());
            } else {
                score -= 2.0;
                reasons.push("inne imię matki".to_string());
            }
        }
        if !inc.spouses.is_empty() && !info.partners.is_empty() {
            let theirs: Vec<String> = info.partners.iter().map(|&p| first_word(&d.info[p].given)).collect();
            if inc.spouses.iter().any(|s| theirs.contains(s)) {
                score += 2.5;
                reasons.push("małżonek zgodny".to_string());
            } else {
                score -= 1.0;
                reasons.push("inny małżonek".to_string());
            }
        }
        if score >= 2.0 {
            let percent = ((score / 12.0) * 100.0).round().clamp(1.0, 99.0) as u32;
            out.push(Candidate { xref: d.xref(i).to_string(), score, percent, reasons });
        }
    }
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(3);
    out
}

/// A clear match lends weight to its family: the parents, partners and children the batch gives someone who clearly
/// matches (≥ 8 points) get 3 more points for the candidates who are that person's parents, partners and children in
/// the archive. One pass, so a boosted match doesn't boost further.
pub fn family_boost(d: &Derived, batch: &Part, all: &mut std::collections::HashMap<String, Vec<Candidate>>) {
    let mut hints: Vec<(String, Vec<String>, String)> = Vec::new();
    for p in &batch.persons {
        let Some(i) = all.get(&p.id).and_then(|l| l.first()).filter(|c| c.score >= 8.0).and_then(|c| d.index(&c.xref)) else { continue };
        let xrefs = |list: &[usize]| list.iter().map(|&x| d.xref(x).to_string()).collect::<Vec<_>>();
        let name = &d.info[i].name;
        for r in &batch.relationships {
            let me = Some(p.id.as_str());
            let (relative, expected, reason) = match r.relation.as_deref() {
                Some("parent") if r.child.as_deref() == me => (r.parent.clone(), xrefs(&d.info[i].parents), format!("rodzic: {name} pasuje")),
                Some("parent") if r.parent.as_deref() == me => (r.child.clone(), xrefs(&d.info[i].children), format!("dziecko: {name} pasuje")),
                Some("partners") if r.a.as_deref() == me => (r.b.clone(), xrefs(&d.info[i].partners), format!("małżonek: {name} pasuje")),
                Some("partners") if r.b.as_deref() == me => (r.a.clone(), xrefs(&d.info[i].partners), format!("małżonek: {name} pasuje")),
                _ => continue,
            };
            if let Some(relative) = relative {
                hints.push((relative, expected, reason));
            }
        }
    }
    for (id, expected, reason) in hints {
        let Some(list) = all.get_mut(&id) else { continue };
        for c in list.iter_mut().filter(|c| expected.contains(&c.xref) && !c.reasons.contains(&reason)) {
            c.score += 3.0;
            c.percent = ((c.score / 12.0) * 100.0).round().clamp(1.0, 99.0) as u32;
            c.reasons.push(reason.clone());
        }
        list.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::gedcom::Document;

    #[test]
    fn scores_follow_the_evidence() {
        let archive = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Marianna /Nowak/\n2 TYPE BIRTH\n1 NAME Marianna /Kowalska/\n2 TYPE MARRIED\n1 SEX F\n1 BIRT\n2 DATE ABT 1882\n2 PLAC Ciechanki\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Józef /Kowalski/\n1 SEX M\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Marianna /Nowak/\n1 SEX F\n1 BIRT\n2 DATE 1860\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I1@\n0 TRLR\n";
        let d = crate::derive::build(&Document::from_bytes(archive.as_bytes()).0);
        let batch: Part = serde_json::from_str(
            r#"{ "persons": [ { "id": "P1", "sex": "F", "names": [ { "type": "birth", "given": "Marianna", "surname": "Nowak" } ] },
                              { "id": "P2", "sex": "M", "names": [ { "given": "Józef", "surname": "Kowalski" } ] } ],
                 "events": [ { "id": "E1", "type": "birth", "date": "1881", "place": "Ciechanki", "people": [ { "p": "P1", "role": "principal" } ] } ],
                 "relationships": [ { "type": "partners", "a": "P1", "b": "P2", "kind": "marriage" } ] }"#,
        )
        .unwrap();
        let list = candidates(&d, &surname_keys(&d), &batch, &batch.persons[0]);
        assert_eq!(list[0].xref, "@I1@", "the married Marianna born 1882 in Ciechanki with the same husband");
        assert!(list[0].score >= 8.0, "{:?}", list[0]);
        assert_eq!(status_for(list.first()), "match");
        assert!(list.iter().all(|c| c.xref != "@I3@"), "born 21 years apart: vetoed");
    }
}
