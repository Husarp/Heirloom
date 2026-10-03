//! How two people are related, in Polish, always from the viewed person's side ("ojciec", "żona", "stryj",
//! "teściowa"). Only close relations: the full "Ścieżka pokrewieństwa" is a later feature (PLAN §4.3).

use crate::derive::Derived;
use heirloom_core::gedcom::model::Sex;

fn sex(d: &Derived, i: usize) -> Sex {
    d.view.model.persons[i].sex
}

/// Picks the word for a man, a woman, or someone whose sex is unknown.
fn by_sex(d: &Derived, i: usize, male: &str, female: &str, other: &str) -> String {
    match sex(d, i) {
        Sex::Male => male,
        Sex::Female => female,
        _ => other,
    }
    .to_string()
}

pub fn siblings(d: &Derived, me: usize) -> Vec<usize> {
    let mut out = Vec::new();
    // Children of the same families (also families without known parents).
    let person = &d.view.model.persons[me];
    for fam in &person.famc {
        if let Some((family, _)) = d.view.family(fam) {
            for child in &family.children {
                if let Some(c) = d.index(child) {
                    if c != me && !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
    }
    // Half-siblings: children of a parent with someone else.
    for &p in &d.info[me].parents {
        for &c in &d.info[p].children {
            if c != me && !out.contains(&c) {
                out.push(c);
            }
        }
    }
    sort_by_birth(d, &mut out);
    out
}

pub fn sort_by_birth(d: &Derived, list: &mut [usize]) {
    list.sort_by_key(|&i| (d.info[i].birth.as_ref().and_then(|b| b.sort).unwrap_or(i64::MAX), i));
}

fn is_half_sibling(d: &Derived, me: usize, other: usize) -> bool {
    let mine = &d.info[me].parents;
    let theirs = &d.info[other].parents;
    let shared = mine.iter().filter(|p| theirs.contains(p)).count();
    shared > 0 && (shared < mine.len() || shared < theirs.len())
}

/// Whether two partners are married (their family has a marriage), and the marriage year.
pub fn marriage(d: &Derived, a: usize, b: usize) -> (bool, Option<String>) {
    let (pa, pb) = (&d.view.model.persons[a], &d.view.model.persons[b]);
    for fam in pa.fams.iter().filter(|f| pb.fams.contains(f)) {
        if let Some((_, details)) = d.view.family(fam) {
            let marr = details.facts.iter().find(|f| f.tag == "MARR");
            let year = marr.and_then(|f| f.date.map(|dv| heirloom_core::polish::year_text(&dv)));
            return (details.married, year);
        }
    }
    (false, None)
}

/// The relation of `other` to `me`: "ojciec", "żona", "brat przyrodni", "wnuczka"…; None when they aren't close.
pub fn label(d: &Derived, me: usize, other: usize) -> Option<String> {
    if me == other {
        return None;
    }
    let info = &d.info;
    if info[me].parents.contains(&other) {
        let adopted = adoption(d, me, other);
        let word = by_sex(d, other, "ojciec", "matka", "rodzic");
        return Some(match adopted {
            Some(kind) => format!("{word} ({kind})"),
            None => word,
        });
    }
    if info[me].children.contains(&other) {
        let adopted = adoption(d, other, me);
        let word = by_sex(d, other, "syn", "córka", "dziecko");
        return Some(match adopted {
            Some(kind) => format!("{word} ({kind})"),
            None => word,
        });
    }
    if info[me].partners.contains(&other) {
        let (married, _) = marriage(d, me, other);
        return Some(if married { by_sex(d, other, "mąż", "żona", "małżonek") } else { by_sex(d, other, "partner", "partnerka", "partner") });
    }
    if siblings(d, me).contains(&other) {
        let base = by_sex(d, other, "brat", "siostra", "rodzeństwo");
        return Some(if is_half_sibling(d, me, other) {
            format!("{base} {}", by_sex(d, other, "przyrodni", "przyrodnia", "przyrodnie"))
        } else {
            base
        });
    }
    let grandparents: Vec<usize> = info[me].parents.iter().flat_map(|&p| info[p].parents.iter().copied()).collect();
    if grandparents.contains(&other) {
        return Some(by_sex(d, other, "dziadek", "babcia", "dziadek lub babcia"));
    }
    if grandparents.iter().any(|&g| info[g].parents.contains(&other)) {
        return Some(by_sex(d, other, "pradziadek", "prababcia", "pradziadek lub prababcia"));
    }
    let grandchildren: Vec<usize> = info[me].children.iter().flat_map(|&c| info[c].children.iter().copied()).collect();
    if grandchildren.contains(&other) {
        return Some(by_sex(d, other, "wnuk", "wnuczka", "wnuk"));
    }
    if grandchildren.iter().any(|&g| info[g].children.contains(&other)) {
        return Some(by_sex(d, other, "prawnuk", "prawnuczka", "prawnuk"));
    }
    // Parent's siblings: stryj (father's brother), wuj (mother's brother), ciotka.
    for &p in &info[me].parents {
        if siblings(d, p).contains(&other) {
            return Some(match sex(d, other) {
                Sex::Male if sex(d, p) == Sex::Male => "stryj".to_string(),
                Sex::Male => "wuj".to_string(),
                Sex::Female => "ciotka".to_string(),
                _ => "rodzeństwo rodzica".to_string(),
            });
        }
        // Step-parents: a parent's partner who isn't a parent.
        if info[p].partners.contains(&other) && !info[me].parents.contains(&other) {
            return Some(by_sex(d, other, "ojczym", "macocha", "partner rodzica"));
        }
    }
    // Siblings' children.
    for s in siblings(d, me) {
        if info[s].children.contains(&other) {
            return Some(match (sex(d, s), sex(d, other)) {
                (Sex::Male, Sex::Female) => "bratanica",
                (Sex::Male, _) => "bratanek",
                (_, Sex::Female) => "siostrzenica",
                _ => "siostrzeniec",
            }
            .to_string());
        }
        if info[s].partners.contains(&other) {
            return Some(match (sex(d, s), sex(d, other)) {
                (Sex::Male, _) => "bratowa",
                _ => "szwagier",
            }
            .to_string());
        }
    }
    // Cousins.
    for &p in &info[me].parents {
        for s in siblings(d, p) {
            if info[s].children.contains(&other) {
                return Some(by_sex(d, other, "kuzyn", "kuzynka", "kuzynostwo"));
            }
        }
    }
    // In-laws.
    for &partner in &info[me].partners {
        if info[partner].parents.contains(&other) {
            return Some(by_sex(d, other, "teść", "teściowa", "rodzic małżonka"));
        }
        if siblings(d, partner).contains(&other) {
            return Some(by_sex(d, other, "szwagier", "szwagierka", "rodzeństwo małżonka"));
        }
        if info[partner].children.contains(&other) && !info[me].children.contains(&other) {
            return Some(by_sex(d, other, "pasierb", "pasierbica", "pasierb"));
        }
    }
    for &c in &info[me].children {
        if info[c].partners.contains(&other) {
            return Some(by_sex(d, other, "zięć", "synowa", "partner dziecka"));
        }
    }
    None
}

/// "adoptowany" / "przybrany" when the child's link to that parent's family isn't by birth.
fn adoption(d: &Derived, child: usize, parent: usize) -> Option<&'static str> {
    let details = &d.view.people[child];
    let parent_fams = &d.view.model.persons[parent].fams;
    details.pedigree.iter().find(|(fam, _)| parent_fams.contains(fam)).map(|(_, pedi)| match pedi.as_str() {
        "ADOPTED" => "adopcja",
        "FOSTER" => "przybrany",
        _ => "nie biologiczny",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::gedcom::Document;

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @GF@ INDI\n1 NAME Jan /Kowalski/\n1 SEX M\n1 FAMS @F0@\n\
0 @A@ INDI\n1 NAME Antoni /Kowalski/\n1 SEX M\n1 FAMC @F0@\n1 FAMS @F1@\n\
0 @U@ INDI\n1 NAME Piotr /Kowalski/\n1 SEX M\n1 FAMC @F0@\n1 FAMS @F3@\n\
0 @C@ INDI\n1 NAME Ewa /Kowalska/\n1 SEX F\n1 FAMC @F3@\n\
0 @M@ INDI\n1 NAME Agnieszka /Mazur/\n1 SEX F\n1 FAMS @F1@\n\
0 @J@ INDI\n1 NAME Józef /Kowalski/\n1 SEX M\n1 FAMC @F1@\n1 FAMS @F2@\n\
0 @S@ INDI\n1 NAME Zofia /Kowalska/\n1 SEX F\n1 FAMC @F1@\n\
0 @W@ INDI\n1 NAME Marianna /Nowak/\n1 SEX F\n1 FAMS @F2@\n\
0 @K@ INDI\n1 NAME Stanisław /Kowalski/\n1 SEX M\n1 FAMC @F2@\n2 PEDI ADOPTED\n\
0 @F0@ FAM\n1 HUSB @GF@\n1 CHIL @A@\n1 CHIL @U@\n\
0 @F1@ FAM\n1 HUSB @A@\n1 WIFE @M@\n1 CHIL @J@\n1 CHIL @S@\n\
0 @F2@ FAM\n1 HUSB @J@\n1 WIFE @W@\n1 CHIL @K@\n1 MARR\n2 DATE 1904\n\
0 @F3@ FAM\n1 HUSB @U@\n1 CHIL @C@\n0 TRLR\n";

    #[test]
    fn close_relations_in_polish() {
        let d = crate::derive::build(&Document::from_bytes(FILE.as_bytes()).0);
        let rel = |a: &str, b: &str| label(&d, d.index(a).unwrap(), d.index(b).unwrap());
        assert_eq!(rel("@J@", "@A@").as_deref(), Some("ojciec"));
        assert_eq!(rel("@J@", "@M@").as_deref(), Some("matka"));
        assert_eq!(rel("@J@", "@W@").as_deref(), Some("żona"));
        assert_eq!(rel("@J@", "@S@").as_deref(), Some("siostra"));
        assert_eq!(rel("@J@", "@K@").as_deref(), Some("syn (adopcja)"));
        assert_eq!(rel("@J@", "@GF@").as_deref(), Some("dziadek"));
        assert_eq!(rel("@J@", "@U@").as_deref(), Some("stryj"));
        assert_eq!(rel("@J@", "@C@").as_deref(), Some("kuzynka"));
        assert_eq!(rel("@W@", "@A@").as_deref(), Some("teść"));
        assert_eq!(rel("@A@", "@W@").as_deref(), Some("synowa"));
        assert_eq!(rel("@GF@", "@K@").as_deref(), Some("prawnuk"));
        assert_eq!(marriage(&d, d.index("@J@").unwrap(), d.index("@W@").unwrap()), (true, Some("1904".into())));
    }
}
