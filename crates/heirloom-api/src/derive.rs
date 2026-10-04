//! What the screens show about every person, worked out once per version of the data: display names, Polish dates,
//! the branch colour (the person's birth family), the generation number, the profile photo, and folded search text.

use heirloom_core::fold::fold;
use heirloom_core::gedcom::date::DateValue;
use heirloom_core::gedcom::model::Person;
use heirloom_core::gedcom::view::{self, Fact, MediaKind, PersonDetails, View};
use heirloom_core::gedcom::Document;
use heirloom_core::polish;
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};

/// A date ready for display.
#[derive(Debug, Clone, PartialEq)]
pub struct DateInfo {
    pub text: String,
    pub short: String,
    pub year: String,
    pub uncertain: bool,
    pub sort: Option<i64>,
    pub value: Option<DateValue>,
}

impl DateInfo {
    pub fn from_fact(fact: &Fact) -> Option<DateInfo> {
        let raw = fact.date_text.as_deref()?;
        let uncertain_fact = matches!(fact.certainty, Some(view::Certainty::Low | view::Certainty::Medium));
        Some(match fact.date {
            Some(d) => DateInfo {
                text: polish::format_date(&d),
                short: polish::format_date_short(&d),
                year: polish::year_text(&d),
                uncertain: polish::is_uncertain(&d) || uncertain_fact,
                sort: Some(d.sort_key()),
                value: Some(d),
            },
            // A date phrase GEDCOM can't read ("zima 1915"): shown as written.
            None => DateInfo {
                text: raw.trim_matches(['(', ')']).to_string(),
                short: raw.trim_matches(['(', ')']).to_string(),
                year: raw.trim_matches(['(', ')']).to_string(),
                uncertain: true,
                sort: None,
                value: None,
            },
        })
    }

    pub fn json(&self) -> Value {
        json!({ "text": self.text, "short": self.short, "year": self.year, "uncertain": self.uncertain, "sort": self.sort })
    }
}

pub fn date_json(d: &Option<DateInfo>) -> Value {
    d.as_ref().map_or(Value::Null, DateInfo::json)
}

#[derive(Debug, Clone)]
pub struct Info {
    /// As lists show it: „Jan Kowalski”, or „Kowalski Jan” when that order is chosen in the settings.
    pub name: String,
    /// Always from the first name: „JK”.
    pub initials: String,
    pub given: String,
    pub surname: String,
    pub maiden: Option<String>,
    pub nickname: Option<String>,
    pub birth: Option<DateInfo>,
    pub death: Option<DateInfo>,
    pub birth_place: Option<String>,
    pub death_place: Option<String>,
    pub living: bool,
    /// Index into [`Derived::groups`]: the surname group of the birth surname.
    pub group: Option<usize>,
    /// 1–12 (`--b1` …).
    pub branch: u8,
    /// The colour of the surname borne now („Koloruj wg: nazwisko”): a wife who took her husband's name gets his
    /// family's colour. The same as `branch` when the surname didn't change.
    pub surname_branch: u8,
    pub generation: Option<u32>,
    /// The profile photo: (media id, file path relative to the archive).
    pub photo: Option<(String, String)>,
    pub photo_count: usize,
    pub search: String,
    pub parents: Vec<usize>,
    pub children: Vec<usize>,
    pub partners: Vec<usize>,
}

/// People who share a surname in any of its forms (Kowalski, Kowalska, Kowalskiego…).
#[derive(Debug, Clone)]
pub struct SurnameGroup {
    pub key: String,
    /// The most common masculine form: "Kowalski".
    pub name: String,
    /// "Kowalscy".
    pub plural: String,
    /// Every form used, with how many people use it.
    pub forms: Vec<(String, usize)>,
    /// People born with the surname.
    pub born: Vec<usize>,
    /// People who took it by marriage.
    pub married: Vec<usize>,
    pub branch: u8,
}

pub struct Derived {
    pub view: View,
    pub info: Vec<Info>,
    pub groups: Vec<SurnameGroup>,
    pub group_index: HashMap<String, usize>,
    pub max_generation: u32,
    /// Julian dates of double-dated records are shown next to the Gregorian ones.
    pub julian_dates: bool,
    /// Archives opened together („Otwórz razem…”): per person the archives the person comes from, as indices into
    /// `archive_keys`. Empty for an archive opened on its own (filled after building, by `combined`).
    pub origins: Vec<Vec<u8>>,
    pub archive_keys: Vec<String>,
}

/// Display choices stored with the archive (Ustawienia › Osoby i daty; „Dołącz” on the Nazwiska screen).
#[derive(Debug, Clone)]
pub struct Display {
    /// „Nazwisko Imię” instead of „Imię Nazwisko”.
    pub surname_first: bool,
    /// „12.03.1878” instead of „12 marca 1878”.
    pub numeric_dates: bool,
    pub julian: bool,
    /// A surname group key → the group it was joined to.
    pub joins: HashMap<String, String>,
}

impl Default for Display {
    fn default() -> Display {
        Display { surname_first: false, numeric_dates: false, julian: true, joins: HashMap::new() }
    }
}

impl Display {
    pub fn from_settings(display: &serde_json::Map<String, Value>) -> Display {
        Display {
            surname_first: display.get("nameOrder").and_then(Value::as_str) == Some("surname"),
            numeric_dates: display.get("dateFormat").and_then(Value::as_str) == Some("numeric"),
            julian: display.get("julian").and_then(Value::as_bool).unwrap_or(true),
            joins: display
                .get("surnameJoins")
                .and_then(Value::as_object)
                .map(|m| m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())
                .unwrap_or_default(),
        }
    }
}

impl Derived {
    pub fn index(&self, xref: &str) -> Option<usize> {
        self.view.person_index.get(xref).copied()
    }

    pub fn person(&self, i: usize) -> (&Person, &PersonDetails, &Info) {
        (&self.view.model.persons[i], &self.view.people[i], &self.info[i])
    }

    pub fn xref(&self, i: usize) -> &str {
        &self.view.model.persons[i].xref
    }

    /// The keys of the archives a person comes from, in archives opened together; None for a single archive.
    pub fn from(&self, i: usize) -> Option<Vec<&str>> {
        let origins = self.origins.get(i)?;
        Some(origins.iter().filter_map(|&k| self.archive_keys.get(k as usize).map(String::as_str)).collect())
    }

    /// The texts linked from a person, in order.
    pub fn person_texts_of(&self, xref: &str) -> Vec<&heirloom_core::gedcom::view::Text> {
        match self.index(xref) {
            Some(i) => self.view.person_texts(&self.view.people[i]).collect(),
            None => Vec::new(),
        }
    }

    /// The group label with genitive: "gałąź Kowalskich".
    pub fn branch_name(&self, i: usize) -> String {
        self.info[i].group.map(|g| self.groups[g].plural.clone()).unwrap_or_default()
    }

    /// The fields every list and card needs (`PersonSummary` in `src/api/types.ts`).
    pub fn summary(&self, i: usize) -> Value {
        let (p, d, info) = self.person(i);
        let mut v = json!({
            "id": p.xref,
            "name": info.name,
            "given": info.given,
            "surname": info.surname,
            "maiden": info.maiden,
            "nickname": info.nickname,
            "sex": p.sex.code(),
            "birth": date_json(&info.birth),
            "death": date_json(&info.death),
            "birthPlace": info.birth_place,
            "deathPlace": info.death_place,
            "living": info.living,
            "branch": info.branch,
            "surnameBranch": info.surname_branch,
            "branchName": self.branch_name(i),
            "generation": info.generation,
            "initials": info.initials,
            "photo": info.photo.as_ref().map(|(_, path)| path),
            "photoCount": info.photo_count,
            "changed": d.changed,
            "created": d.created,
        });
        if let Some(from) = self.from(i) {
            v["from"] = json!(from);
        }
        v
    }
}

pub fn initials(name: &str) -> String {
    name.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

fn first_fact<'a>(details: &'a PersonDetails, tags: &[&str]) -> Option<&'a Fact> {
    // The first one that has a date, else the first one at all.
    let facts = || details.facts.iter().filter(|f| tags.contains(&f.tag.as_str()));
    facts().find(|f| f.date_text.is_some()).or_else(|| facts().next())
}

pub fn build(doc: &Document) -> Derived {
    build_with(doc, &Display::default())
}

pub fn build_with(doc: &Document, display: &Display) -> Derived {
    let view = view::build(doc);
    let persons = &view.model.persons;
    let n = persons.len();

    // Relations as indices.
    let mut parents = vec![Vec::new(); n];
    let mut children = vec![Vec::new(); n];
    let mut partners = vec![Vec::new(); n];
    for family in &view.model.families {
        let members: Vec<usize> = family.partners.iter().filter_map(|x| view.person_index.get(x).copied()).collect();
        let kids: Vec<usize> = family.children.iter().filter_map(|x| view.person_index.get(x).copied()).collect();
        for &a in &members {
            for &b in &members {
                if a != b && !partners[a].contains(&b) {
                    partners[a].push(b);
                }
            }
            for &c in &kids {
                if !children[a].contains(&c) {
                    children[a].push(c);
                }
                if !parents[c].contains(&a) {
                    parents[c].push(a);
                }
            }
        }
    }

    // Surname groups: birth surnames make the families; married surnames add "przez małżeństwo".
    let mut groups: Vec<SurnameGroup> = Vec::new();
    let mut group_index: HashMap<String, usize> = HashMap::new();
    let mut forms: Vec<HashMap<String, usize>> = Vec::new();
    let mut person_group = vec![None; n];
    // A surname joined to another group (Kowalewski → Kowalski) counts there; joins can chain.
    let resolve = |surname: &str| {
        let mut key = polish::surname_key(surname);
        for _ in 0..8 {
            match display.joins.get(&key) {
                Some(target) if *target != key => key = target.clone(),
                _ => break,
            }
        }
        key
    };
    let mut add = |surname: &str, person: usize, married: bool, groups: &mut Vec<SurnameGroup>, forms: &mut Vec<HashMap<String, usize>>| {
        let key = resolve(surname);
        if key.is_empty() {
            return None;
        }
        let g = *group_index.entry(key.clone()).or_insert_with(|| {
            groups.push(SurnameGroup { key, name: String::new(), plural: String::new(), forms: Vec::new(), born: Vec::new(), married: Vec::new(), branch: 0 });
            forms.push(HashMap::new());
            groups.len() - 1
        });
        let list = if married { &mut groups[g].married } else { &mut groups[g].born };
        if !list.contains(&person) {
            list.push(person);
        }
        *forms[g].entry(surname.trim().to_string()).or_default() += 1;
        Some(g)
    };
    for (i, p) in persons.iter().enumerate() {
        if let Some(birth) = p.birth_name().filter(|b| !b.surname.is_empty()) {
            person_group[i] = add(&birth.surname, i, false, &mut groups, &mut forms);
        }
        for name in p.names.iter().filter(|nm| nm.kind == heirloom_core::gedcom::model::NameKind::Married) {
            if !name.surname.is_empty() && Some(resolve(&name.surname)) != person_group[i].map(|g| groups[g].key.clone()) {
                add(&name.surname, i, true, &mut groups, &mut forms);
            }
        }
    }
    for (g, group) in groups.iter_mut().enumerate() {
        let mut list: Vec<(String, usize)> = forms[g].iter().map(|(k, v)| (k.clone(), *v)).collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        // The dictionary form: the most common masculine (or unmarked) form.
        let masculine = list
            .iter()
            .map(|(f, c)| (polish::masculine_form(f), *c))
            .fold(HashMap::<String, usize>::new(), |mut acc, (f, c)| {
                *acc.entry(f).or_default() += c;
                acc
            });
        let mut best: Vec<(String, usize)> = masculine.into_iter().collect();
        best.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        group.name = best.first().map(|(f, _)| f.clone()).unwrap_or_default();
        group.plural = polish::family_plural(&group.name);
        group.forms = list;
    }
    // Colours by size: the biggest family is b1.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| groups[b].born.len().cmp(&groups[a].born.len()).then_with(|| groups[a].name.cmp(&groups[b].name)));
    for (rank, &g) in order.iter().enumerate() {
        groups[g].branch = (rank % 12) as u8 + 1;
    }

    let generations = generations(n, &parents, &children, &partners, persons, &view);
    let max_generation = generations.iter().flatten().copied().max().unwrap_or(0);

    let mut info = Vec::with_capacity(n);
    for (i, p) in persons.iter().enumerate() {
        let d = &view.people[i];
        let birth_fact = first_fact(d, &["BIRT", "CHR", "BAPM"]);
        let death_fact = first_fact(d, &["DEAT", "BURI", "CREM"]);
        let nickname = p.names.iter().find_map(|nm| nm.nickname.clone()).filter(|s| !s.is_empty());
        let photos: Vec<&view::MediaLink> = d
            .media
            .iter()
            .filter(|l| view.media.get(&l.object).is_some_and(|m| m.kind == MediaKind::Photo))
            .collect();
        let profile = photos.iter().find(|l| l.profile).or(photos.first());
        let photo = profile.and_then(|l| Some((l.object.clone(), view.media.get(&l.object)?.file.clone()?)));
        let given_first = p.display_name();
        let name = if display.surname_first && !p.surname().is_empty() && !p.given().is_empty() { format!("{} {}", p.surname(), p.given()) } else { given_first.clone() };
        let mut search = fold(&name);
        for nm in &p.names {
            search.push(' ');
            search.push_str(&fold(&format!("{} {} {}", nm.given, nm.surname, nm.nickname.as_deref().unwrap_or(""))));
        }
        let group = person_group[i];
        // The group of the surname borne now (the married one when recorded), with joins followed.
        let current = Some(p.surname()).filter(|s| !s.is_empty()).and_then(|s| group_index.get(&resolve(s)).copied()).or(group);
        info.push(Info {
            given: p.given().to_string(),
            surname: p.surname().to_string(),
            maiden: p.maiden_name().map(str::to_string),
            nickname,
            birth: birth_fact.and_then(DateInfo::from_fact),
            death: death_fact.and_then(DateInfo::from_fact),
            birth_place: birth_fact.and_then(|f| f.place.clone()),
            death_place: death_fact.and_then(|f| f.place.clone()),
            living: d.living,
            branch: group.map_or(12, |g| groups[g].branch),
            surname_branch: current.map_or(12, |g| groups[g].branch),
            group,
            generation: generations[i],
            photo,
            photo_count: photos.len(),
            search,
            parents: std::mem::take(&mut parents[i]),
            children: std::mem::take(&mut children[i]),
            partners: std::mem::take(&mut partners[i]),
            initials: initials(&given_first),
            name: if name.is_empty() { "(bez imienia)".to_string() } else { name },
        });
    }
    Derived { view, info, groups, group_index, max_generation, julian_dates: display.julian, origins: Vec::new(), archive_keys: Vec::new() }
}

/// Generation numbers: partners share a generation, children are one below their parents. Each connected family
/// starts at I with its oldest generation. With cousin marriages the first path found wins.
fn generations(
    n: usize,
    parents: &[Vec<usize>],
    children: &[Vec<usize>],
    partners: &[Vec<usize>],
    persons: &[Person],
    view: &View,
) -> Vec<Option<u32>> {
    let mut level: Vec<Option<i64>> = vec![None; n];
    // Start each component from its earliest-born person, so the numbering is stable between saves.
    let mut seeds: Vec<usize> = (0..n).collect();
    let birth_year = |i: usize| {
        view.people[i]
            .facts
            .iter()
            .find(|f| matches!(f.tag.as_str(), "BIRT" | "CHR" | "BAPM") && f.date.is_some())
            .and_then(|f| f.date.map(|d| d.start.year))
            .unwrap_or(i32::MAX)
    };
    seeds.sort_by_key(|&i| (birth_year(i), persons[i].xref.clone()));
    let mut result = vec![None; n];
    for seed in seeds {
        if level[seed].is_some() {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([seed]);
        level[seed] = Some(0);
        while let Some(i) = queue.pop_front() {
            component.push(i);
            let g = level[i].unwrap_or(0);
            let next = partners[i].iter().map(|&p| (p, g)).chain(children[i].iter().map(|&c| (c, g + 1))).chain(parents[i].iter().map(|&p| (p, g - 1)));
            for (j, gj) in next.collect::<Vec<_>>() {
                if level[j].is_none() {
                    level[j] = Some(gj);
                    queue.push_back(j);
                }
            }
        }
        let min = component.iter().filter_map(|&i| level[i]).min().unwrap_or(0);
        for i in component {
            result[i] = level[i].map(|g| (g - min + 1) as u32);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Antoni /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE ABT 1850\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Agnieszka /Mazur/\n2 TYPE BIRTH\n1 NAME Agnieszka /Kowalska/\n2 TYPE MARRIED\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Józef /Kowalski/\n2 NICK Dziadek Józek\n1 SEX M\n1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka\n1 DEAT\n2 DATE 3 FEB 1951\n\
1 FAMC @F1@\n1 FAMS @F2@\n1 OBJE @O1@\n\
0 @I4@ INDI\n1 NAME Marianna /Nowak/\n2 TYPE BIRTH\n1 NAME Marianna /Kowalska/\n2 TYPE MARRIED\n1 SEX F\n1 FAMS @F2@\n\
0 @I5@ INDI\n1 NAME Helena /Kowalska/\n1 SEX F\n1 BIRT\n2 DATE 1908\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n\
0 @F2@ FAM\n1 HUSB @I3@\n1 WIFE @I4@\n1 CHIL @I5@\n\
0 @O1@ OBJE\n1 FILE media/j.jpg\n2 FORM image/jpeg\n3 MEDI PHOTO\n0 TRLR\n";

    fn mazur_of(d: &Derived) -> usize {
        d.group_index[&polish::surname_key("Mazur")]
    }

    fn derived() -> Derived {
        build(&Document::from_bytes(FILE.as_bytes()).0)
    }

    #[test]
    fn generations_follow_the_family() {
        let d = derived();
        let level_of = |x: &str| d.info[d.index(x).unwrap()].generation;
        assert_eq!((level_of("@I1@"), level_of("@I2@"), level_of("@I3@"), level_of("@I4@"), level_of("@I5@")), (Some(1), Some(1), Some(2), Some(2), Some(3)));
    }

    #[test]
    fn branches_come_from_the_birth_family() {
        let d = derived();
        let kowalscy = d.group_index[&polish::surname_key("Kowalski")];
        assert_eq!(d.groups[kowalscy].plural, "Kowalscy");
        assert_eq!(d.groups[kowalscy].branch, 1, "the biggest family is b1");
        let marianna = d.index("@I4@").unwrap();
        assert_ne!(d.info[marianna].branch, 1, "Marianna keeps her own family's colour");
        assert!(d.groups[kowalscy].married.contains(&marianna), "she is a Kowalska by marriage");
        let helena = d.index("@I5@").unwrap();
        assert_eq!(d.info[helena].branch, 1);
        // „Koloruj wg: nazwisko”: the surname borne now.
        assert_eq!(d.info[marianna].surname_branch, 1, "Marianna Kowalska takes her husband's colour");
        assert_eq!(d.info[helena].surname_branch, 1);
        assert_eq!(d.summary(marianna)["surnameBranch"], 1);
    }

    #[test]
    fn summary_fields() {
        let d = derived();
        let jozef = d.summary(d.index("@I3@").unwrap());
        assert_eq!(jozef["name"], "Józef Kowalski");
        assert_eq!(jozef["nickname"], "Dziadek Józek");
        assert_eq!(jozef["birth"]["text"], "12 marca 1878");
        assert_eq!(jozef["birth"]["short"], "12.03.1878");
        assert_eq!(jozef["photo"], "media/j.jpg");
        assert_eq!(jozef["photoCount"], 1);
        let antoni = d.summary(d.index("@I1@").unwrap());
        assert_eq!((antoni["birth"]["year"].as_str(), antoni["birth"]["uncertain"].as_bool()), (Some("ok. 1850"), Some(true)));
        let marianna = d.summary(d.index("@I4@").unwrap());
        assert_eq!((marianna["name"].as_str(), marianna["maiden"].as_str()), (Some("Marianna Kowalska"), Some("Nowak")));
    }

    #[test]
    fn display_settings_order_names_and_join_surnames() {
        let settings = json!({ "nameOrder": "surname", "surnameJoins": { polish::surname_key("Nowak"): polish::surname_key("Mazur") } });
        let d = build_with(&Document::from_bytes(FILE.as_bytes()).0, &Display::from_settings(settings.as_object().unwrap()));
        let jozef = d.summary(d.index("@I3@").unwrap());
        assert_eq!((jozef["name"].as_str(), jozef["initials"].as_str()), (Some("Kowalski Józef"), Some("JK")));
        // Marianna (born Nowak) now counts among the Mazurs, and there is no Nowak group left.
        let mazur = d.group_index[&polish::surname_key("Mazur")];
        assert!(d.groups[mazur].born.contains(&d.index("@I4@").unwrap()));
        assert!(!d.group_index.contains_key(&polish::surname_key("Nowak")));
        // A married surname joined to another group takes that group's colour.
        let settings = json!({ "surnameJoins": { polish::surname_key("Kowalski"): polish::surname_key("Mazur") } });
        let d = build_with(&Document::from_bytes(FILE.as_bytes()).0, &Display::from_settings(settings.as_object().unwrap()));
        let marianna = d.index("@I4@").unwrap();
        assert_eq!(d.info[marianna].surname_branch, d.groups[mazur_of(&d)].branch);
        assert!(d.julian_dates, "Julian dates are shown unless switched off");
    }
}
