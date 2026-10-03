//! Reading people: the catalogue, search, the tree's side panel, hover cards and the full profile.

use crate::derive::{DateInfo, Derived, date_json};
use crate::kin;
use crate::text::{display_markdown, excerpt};
use crate::ApiError;
use heirloom_core::fold::fold;
use heirloom_core::gedcom::model::Sex;
use heirloom_core::gedcom::view::{Citation, Fact, MediaKind, Text, TextKind};
use heirloom_core::polish;
use serde_json::{Value, json};
use std::collections::HashMap;

pub fn not_found(id: &str) -> ApiError {
    ApiError::new("not_found", format!("Nie ma osoby {id} w tym archiwum."))
}

/// Links from a person (and in their families) to records that are no longer in the archive: an undone import, a
/// deleted text, a file from another program. The profile lists them („prowadzi donikąd”) instead of leaving them out
/// without a word.
pub fn broken_links(doc: &heirloom_core::gedcom::Document, id: &str) -> Value {
    fn word(tag: &str) -> &'static str {
        match tag {
            "FAMS" | "FAMC" => "rodzina",
            "HUSB" => "mąż",
            "WIFE" => "żona",
            "CHIL" => "dziecko",
            "OBJE" => "zdjęcie lub dokument",
            "SOUR" => "źródło",
            "SNOTE" | "NOTE" => "tekst",
            "ASSO" | "ALIA" => "osoba",
            "REPO" => "archiwum",
            _ => "wpis",
        }
    }
    fn walk(node: &heirloom_core::gedcom::Node, doc: &heirloom_core::gedcom::Document, record: &str, out: &mut Vec<Value>) {
        for c in &node.children {
            if let Some(target) = c.pointer().filter(|t| *t != "@VOID@") {
                if doc.record(target).is_none() && !out.iter().any(|o| o["record"] == record && o["xref"] == target) {
                    out.push(json!({ "record": record, "tag": c.tag, "xref": target, "what": word(&c.tag) }));
                }
            }
            walk(c, doc, record, out);
        }
    }
    let mut out = Vec::new();
    let Some(indi) = doc.record(id) else { return json!(out) };
    walk(indi, doc, id, &mut out);
    for fam in indi.children.iter().filter(|c| c.tag == "FAMS" || c.tag == "FAMC").filter_map(|c| c.pointer()) {
        if let Some(record) = doc.record(fam) {
            walk(record, doc, fam, &mut out);
        }
    }
    json!(out)
}

/// Every person, for the Osoby table and the tree's jump search (the UI filters and sorts them).
pub fn list(d: &Derived) -> Value {
    let people: Vec<Value> = (0..d.info.len()).map(|i| d.summary(i)).collect();
    let groups: Vec<Value> = d
        .groups
        .iter()
        .map(|g| json!({ "key": g.key, "name": g.name, "plural": g.plural, "branch": g.branch, "born": g.born.len(), "married": g.married.len() }))
        .collect();
    // Each person's groups by birth and by marriage, for "Nazwiska: w obu grupach" (one pass over the groups; asking
    // every group about every person was quadratic).
    let mut married: Vec<Vec<&str>> = vec![Vec::new(); d.info.len()];
    for g in &d.groups {
        for &i in &g.married {
            married[i].push(g.key.as_str());
        }
    }
    let memberships: Vec<Value> = married
        .into_iter()
        .enumerate()
        .map(|(i, married)| json!({ "birth": d.info[i].group.map(|g| d.groups[g].key.clone()), "married": married }))
        .collect();
    // Moved in, not through `json!`, which would copy the 10,000 finished summaries once more.
    let mut out = serde_json::Map::new();
    out.insert("people".into(), Value::Array(people));
    out.insert("groups".into(), Value::Array(groups));
    out.insert("memberships".into(), Value::Array(memberships));
    out.insert("generations".into(), json!(d.max_generation));
    Value::Object(out)
}

/// The other gender's form of a whole surname word (folded): "wisniewska" ↔ "wisniewski", "zawadzka" ↔ "zawadzki".
fn other_form(word: &str) -> Option<String> {
    for (female, male) in [("dzka", "dzki"), ("ska", "ski"), ("cka", "cki")] {
        if let Some(stem) = word.strip_suffix(female) {
            return Some(format!("{stem}{male}"));
        }
        if let Some(stem) = word.strip_suffix(male) {
            return Some(format!("{stem}{female}"));
        }
    }
    None
}

/// Search by any name form, nickname or maiden name; with or without Polish letters; a surname also finds its other
/// gender's form (Wiśniewska finds Franciszek Wiśniewski, design 17e). Prefix match on words.
pub fn search(d: &Derived, query: &str, limit: usize) -> Value {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return json!([]);
    }
    let others: Vec<Option<String>> = words.iter().map(|w| other_form(w)).collect();
    let mut hits: Vec<(i64, usize)> = Vec::new();
    for (i, info) in d.info.iter().enumerate() {
        let tokens: Vec<&str> = info.search.split_whitespace().collect();
        let mut score = 0i64;
        let mut all = true;
        for (w, other) in words.iter().zip(&others) {
            match tokens.iter().position(|t| t.starts_with(w.as_str())) {
                Some(pos) => score += if tokens[pos] == w { 3 } else { 2 } - (pos as i64 / 4),
                None => match other.as_ref().and_then(|o| tokens.iter().position(|t| t == o)) {
                    Some(pos) => score += 1 - (pos as i64 / 4),
                    None => {
                        all = false;
                        break;
                    }
                },
            }
        }
        if all {
            hits.push((score, i));
        }
    }
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| d.info[a.1].name.cmp(&d.info[b.1].name)));
    Value::Array(hits.into_iter().take(limit).map(|(_, i)| with_context(d, i)).collect())
}

/// A summary plus a hint that tells apart people with the same name: "syn Antoniego i Agnieszki".
pub fn with_context(d: &Derived, i: usize) -> Value {
    let mut v = d.summary(i);
    v["context"] = Value::String(context(d, i));
    v
}

pub fn context(d: &Derived, i: usize) -> String {
    let info = &d.info[i];
    let female = d.view.model.persons[i].sex == Sex::Female;
    let parents: Vec<String> = info.parents.iter().map(|&p| polish::given_genitive(&d.info[p].given, d.view.model.persons[p].sex == Sex::Female)).collect();
    let mut parts = Vec::new();
    if !parents.is_empty() {
        parts.push(format!("{} {}", if female { "córka" } else { "syn" }, parents.join(" i ")));
    }
    if let Some(maiden) = &info.maiden {
        parts.insert(0, format!("z d. {maiden}"));
    }
    if parts.is_empty() {
        if let Some(place) = &info.birth_place {
            parts.push(place.clone());
        }
    }
    parts.join(" · ")
}

fn relative(d: &Derived, me: usize, other: usize) -> Value {
    let mut v = d.summary(other);
    v["label"] = json!(kin::label(d, me, other));
    v
}

fn place_with_note(fact: &Fact) -> Option<String> {
    let place = fact.place.clone()?;
    Some(match &fact.place_note {
        Some(note) => format!("{place} ({})", note.replace("parafia ", "par. ")),
        None => place,
    })
}

/// "12 marca 1878 · Wólka, par. Łęczna".
fn when_where(fact: &Fact, separator: &str) -> String {
    let date = DateInfo::from_fact(fact).map(|d| d.text);
    let place = fact.place.clone().map(|p| match &fact.place_note {
        Some(note) => format!("{p}, {}", note.replace("parafia ", "par. ")),
        None => p,
    });
    match (date, place) {
        (Some(d), Some(p)) => format!("{d}{separator}{p}"),
        (Some(d), None) => d,
        (None, Some(p)) => p,
        (None, None) if fact.only_happened => "tak (bez daty)".into(),
        (_, None) => String::new(),
    }
}

/// Age in whole years between two dates, with "ok." when either is approximate.
pub fn age_between(from: &Option<DateInfo>, to: &Option<DateInfo>) -> Option<(i32, bool)> {
    let (a, b) = (from.as_ref()?.value?, to.as_ref()?.value?);
    let mut years = b.start.year - a.start.year;
    if let (Some(am), Some(bm)) = (a.start.month, b.start.month) {
        let (ad, bd) = (a.start.day.unwrap_or(1), b.start.day.unwrap_or(1));
        if (bm, bd) < (am, ad) {
            years -= 1;
        }
    }
    let approximate = from.as_ref()?.uncertain || to.as_ref()?.uncertain || a.start.month.is_none() || b.start.month.is_none();
    (years >= 0).then_some((years, approximate))
}

fn now_date() -> Option<DateInfo> {
    let now = heirloom_core::history::now();
    let (y, m, day) = (now.get(0..4)?.parse().ok()?, now.get(5..7)?.parse().ok()?, now.get(8..10)?.parse().ok()?);
    let value = heirloom_core::gedcom::date::DateValue {
        qualifier: heirloom_core::gedcom::date::Qualifier::Exact,
        start: heirloom_core::gedcom::date::Ymd { year: y, month: Some(m), day: Some(day) },
        end: None,
        calendar: heirloom_core::gedcom::date::Calendar::Gregorian,
    };
    Some(DateInfo { text: String::new(), short: String::new(), year: String::new(), uncertain: false, sort: None, value: Some(value) })
}

fn years_text(age: (i32, bool)) -> String {
    let (n, approx) = age;
    let word = match n {
        1 => "rok",
        n if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) => "lata",
        _ => "lat",
    };
    if approx { format!("ok. {n} {word}") } else { format!("{n} {word}") }
}

/// The age at death, or now for the living: "72 lata".
pub fn age_text(d: &Derived, i: usize) -> Option<String> {
    let info = &d.info[i];
    let end = if info.death.is_some() { info.death.clone() } else if info.living { now_date() } else { None };
    age_between(&info.birth, &end).map(years_text)
}

/// The tree's side panel ("Wybrana osoba").
pub fn panel(d: &Derived, id: &str) -> Result<Value, ApiError> {
    let i = d.index(id).ok_or_else(|| not_found(id))?;
    let (p, details, info) = d.person(i);
    let female = p.sex == Sex::Female;
    let fact = |tags: &[&str]| details.facts.iter().find(|f| tags.contains(&f.tag.as_str()));
    let occupation: Vec<String> = details.facts.iter().filter(|f| f.tag == "OCCU").filter_map(|f| f.value.clone()).collect();
    let mut relatives = Vec::new();
    for &parent in &info.parents {
        relatives.push(relative(d, i, parent));
    }
    for &partner in &info.partners {
        let mut v = relative(d, i, partner);
        if let (_, Some(year)) = kin::marriage(d, i, partner) {
            v["label"] = json!(format!("{} · {year}", v["label"].as_str().unwrap_or("")));
        }
        relatives.push(v);
    }
    let mut kids = info.children.clone();
    kin::sort_by_birth(d, &mut kids);
    for child in kids {
        relatives.push(relative(d, i, child));
    }
    let photo = info.photo.as_ref().and_then(|(id, path)| {
        let object = d.view.media.get(id)?;
        Some(json!({ "path": path, "caption": object.title, "date": object.date_text }))
    });
    Ok(json!({
        "person": d.summary(i),
        "photo": photo,
        "generationBranch": generation_branch(d, i),
        "birth": fact(&["BIRT", "CHR", "BAPM"]).map(|f| when_where(f, " · ")).filter(|s| !s.is_empty()),
        "death": fact(&["DEAT", "BURI"]).map(|f| when_where(f, " · ")).filter(|s| !s.is_empty()),
        "deathAge": if info.death.is_some() { age_text(d, i) } else { None },
        "occupation": (!occupation.is_empty()).then(|| occupation.join(", ")),
        "relatives": relatives,
        "sexWord": if female { "F" } else { "M" },
    }))
}

/// "Pokolenie VII · gałąź Kowalskich".
pub fn generation_branch(d: &Derived, i: usize) -> String {
    let info = &d.info[i];
    let mut parts = Vec::new();
    if let Some(g) = info.generation {
        parts.push(format!("Pokolenie {}", roman(g)));
    }
    if let Some(g) = info.group {
        parts.push(format!("gałąź {}", polish::family_genitive(&d.groups[g].name)));
    }
    parts.join(" · ")
}

pub fn roman(n: u32) -> String {
    let table = [(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")];
    let mut out = String::new();
    let mut rest = n;
    for (value, letters) in table {
        while rest >= value {
            out.push_str(letters);
            rest -= value;
        }
    }
    out
}

/// A hover card for a person mention (Heirloom 2b), with the relation to the person being viewed.
pub fn hover(d: &Derived, id: &str, from: Option<&str>) -> Result<Value, ApiError> {
    let i = d.index(id).ok_or_else(|| not_found(id))?;
    let mut v = d.summary(i);
    let places: Vec<String> = {
        let mut list: Vec<String> = Vec::new();
        for f in &d.view.people[i].facts {
            if let Some(place) = &f.place {
                let short = place.split(',').next().unwrap_or(place).trim().to_string();
                if !list.contains(&short) {
                    list.push(short);
                }
            }
        }
        list
    };
    v["places"] = json!(places.join(" → "));
    if let Some(from) = from.and_then(|f| d.index(f)) {
        let label = kin::label(d, from, i);
        v["relation"] = json!(label.map(|l| {
            let viewer = &d.info[from];
            let genitive = polish::given_genitive(&viewer.given, d.view.model.persons[from].sex == Sex::Female);
            let mut text = format!("{} {genitive}", capitalize(&l));
            if d.info[from].partners.contains(&i) {
                if let (_, Some(year)) = kin::marriage(d, from, i) {
                    text.push_str(&format!(" · ślub {year}"));
                }
            }
            text
        }));
    }
    v["mentionCount"] = json!(mentions_of(d, i).len());
    Ok(v)
}

pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The texts that mention a person (`[…](person:<UID>)`), for "Wspomniany w…".
pub fn mentions_of(d: &Derived, i: usize) -> Vec<(&Text, String)> {
    let Some(uid) = d.view.model.persons[i].uid.as_deref() else { return Vec::new() };
    let needle = format!("(person:{uid})");
    let own: Vec<&str> = d.view.people[i]
        .texts
        .iter()
        .filter_map(|t| match t {
            heirloom_core::gedcom::view::TextRef::Shared(x) => Some(x.as_str()),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (xref, text) in &d.view.texts {
        if own.contains(&xref.as_str()) || !text.body.contains(&needle) {
            continue;
        }
        out.push((text, excerpt(&text.body, &needle, 80)));
    }
    out.sort_by(|a, b| a.0.title.cmp(&b.0.title));
    out
}

struct SourceNumbers {
    order: Vec<String>,
    numbers: HashMap<String, usize>,
    pages: HashMap<String, Option<String>>,
}

impl SourceNumbers {
    fn cite(&mut self, citations: &[Citation]) -> Vec<usize> {
        let mut out = Vec::new();
        for c in citations {
            let Some(source) = &c.source else { continue };
            let n = *self.numbers.entry(source.clone()).or_insert_with(|| {
                self.order.push(source.clone());
                self.order.len()
            });
            self.pages.entry(source.clone()).or_insert_with(|| c.page.clone());
            if !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
}

fn fact_key(tag: &str, kind: Option<&str>, female: bool) -> String {
    let gendered = |m: &str, f: &str| if female { f.to_string() } else { m.to_string() };
    match tag {
        "BIRT" => gendered("Urodzony", "Urodzona"),
        "CHR" | "BAPM" => gendered("Ochrzczony", "Ochrzczona"),
        "DEAT" => gendered("Zmarł", "Zmarła"),
        "BURI" => gendered("Pochowany", "Pochowana"),
        "CREM" => "Kremacja".into(),
        "OCCU" => "Zawód".into(),
        "RELI" => "Wyznanie".into(),
        "RESI" => "Miejsca zamieszkania".into(),
        "EDUC" => "Wykształcenie".into(),
        "EMIG" => "Emigracja".into(),
        "IMMI" => "Imigracja".into(),
        "NATU" => "Naturalizacja".into(),
        "CONF" => "Bierzmowanie".into(),
        "FCOM" => "Pierwsza komunia".into(),
        "GRAD" => "Ukończenie szkoły".into(),
        "RETI" => "Emerytura".into(),
        "CENS" => "Spis ludności".into(),
        "TITL" => "Tytuł".into(),
        "NATI" => "Narodowość".into(),
        "DSCR" => "Wygląd".into(),
        "PROP" => "Majątek".into(),
        "ADOP" => "Adopcja".into(),
        "ORDN" => "Święcenia".into(),
        "WILL" => "Testament".into(),
        "PROB" => "Spadek".into(),
        "_MILT" | "_MILI" => "Służba wojskowa".into(),
        "MARR" => "Ślub".into(),
        "MARB" => "Zapowiedzi".into(),
        "DIV" => "Rozwód".into(),
        "ENGA" => "Zaręczyny".into(),
        _ => kind.map_or_else(|| "Wydarzenie".to_string(), capitalize),
    }
}

/// The timeline name of an event: "Urodzenie", "Chrzest", "Zgon", …
pub fn event_name(tag: &str, kind: Option<&str>) -> String {
    match tag {
        "BIRT" => "Urodzenie".into(),
        "CHR" | "BAPM" => "Chrzest".into(),
        "DEAT" => "Zgon".into(),
        "BURI" => "Pogrzeb".into(),
        "RESI" => "Zamieszkanie".into(),
        "OCCU" => "Praca".into(),
        "EDUC" => "Nauka".into(),
        "RELI" => "Wyznanie".into(),
        other => fact_key(other, kind, false),
    }
}

/// The whole profile (spec §4.6–§4.7).
/// A fact's date for the profile; a double-dated record (Russian partition) also shows its Julian date when that is
/// switched on in the settings: „12 marca 1878 (jul. 28 lutego 1878)”.
fn fact_date(d: &Derived, f: &Fact) -> Option<String> {
    let text = DateInfo::from_fact(f)?.text;
    match f.julian.as_deref().and_then(heirloom_core::gedcom::date::parse).filter(|_| d.julian_dates) {
        Some(julian) => Some(format!("{text} (jul. {})", polish::format_date(&julian))),
        None => Some(text),
    }
}

pub fn profile(d: &Derived, id: &str, history: &[heirloom_core::history::Entry]) -> Result<Value, ApiError> {
    let i = d.index(id).ok_or_else(|| not_found(id))?;
    let (p, details, info) = d.person(i);
    let female = p.sex == Sex::Female;
    let mut sources = SourceNumbers { order: Vec::new(), numbers: HashMap::new(), pages: HashMap::new() };

    // "W skrócie": the key facts grid.
    let mut facts = Vec::new();
    let mut fact_row = |key: String, value: String, uncertain: bool, cites: Vec<usize>| {
        if !value.is_empty() {
            facts.push(json!({ "key": key, "value": value, "uncertain": uncertain, "sources": cites }));
        }
    };
    let dated = |f: &Fact| DateInfo::from_fact(f).is_some_and(|d| d.uncertain);
    for tag in ["BIRT", "CHR", "BAPM"] {
        for f in details.facts.iter().filter(|f| f.tag == tag) {
            let value = [fact_date(d, f), place_with_note(f)].into_iter().flatten().collect::<Vec<_>>().join(", ");
            let cites = sources.cite(&f.citations);
            fact_row(fact_key(tag, None, female), value, dated(f), cites);
        }
    }
    let mut partner_list = info.partners.clone();
    kin::sort_by_birth(d, &mut partner_list);
    for &partner in &partner_list {
        let (pa, pb) = (&d.view.model.persons[i], &d.view.model.persons[partner]);
        for fam in pa.fams.iter().filter(|f| pb.fams.contains(f)) {
            if let Some((_, fd)) = d.view.family(fam) {
                for f in fd.facts.iter().filter(|f| f.tag == "MARR") {
                    let value = [fact_date(d, f), f.place.clone()].into_iter().flatten().collect::<Vec<_>>().join(", ");
                    let key = if partner_list.len() > 1 {
                        let with = polish::given_instrumental(&d.info[partner].given, d.view.model.persons[partner].sex == Sex::Female);
                        format!("Ślub {} {with}", polish::z_or_ze(&with))
                    } else {
                        "Ślub".into()
                    };
                    let cites = sources.cite(&f.citations);
                    fact_row(key, value, dated(f), cites);
                }
            }
        }
    }
    for tag in ["DEAT", "BURI", "CREM"] {
        for f in details.facts.iter().filter(|f| f.tag == tag) {
            let mut value = [fact_date(d, f), place_with_note(f)].into_iter().flatten().collect::<Vec<_>>().join(", ");
            if value.is_empty() && f.only_happened {
                value = "tak (bez daty)".into();
            }
            if let Some(cause) = &f.cause {
                value.push_str(&format!(" — {cause}"));
            }
            let cites = sources.cite(&f.citations);
            fact_row(fact_key(tag, None, female), value, dated(f), cites);
        }
    }
    let mut grouped: Vec<(String, Vec<&Fact>)> = Vec::new();
    for f in details.facts.iter().filter(|f| !matches!(f.tag.as_str(), "BIRT" | "CHR" | "BAPM" | "DEAT" | "BURI" | "CREM")) {
        let key = fact_key(&f.tag, f.kind.as_deref(), female);
        match grouped.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(f),
            None => grouped.push((key, vec![f])),
        }
    }
    for (key, list) in grouped {
        let arrow = key == "Miejsca zamieszkania";
        let parts: Vec<String> = list
            .iter()
            .map(|f| {
                let what = f.value.clone().or_else(|| f.place.clone()).unwrap_or_default();
                let when = DateInfo::from_fact(f).map(|d| d.text);
                match (what.is_empty(), when) {
                    (false, Some(w)) => format!("{what} ({w})"),
                    (false, None) => what,
                    (true, Some(w)) => w,
                    (true, None) => String::new(),
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        let cites: Vec<usize> = list.iter().flat_map(|f| sources.cite(&f.citations)).collect();
        let uncertain = list.iter().any(|f| dated(f) || matches!(f.certainty, Some(heirloom_core::gedcom::view::Certainty::Low)));
        fact_row(key, parts.join(if arrow { " → " } else { "; " }), uncertain, cites);
    }

    // Texts, in the person's order.
    let texts: Vec<&Text> = d.view.person_texts(details).collect();
    let text_json = |t: &Text, sources: &mut SourceNumbers| {
        json!({
            "id": t.xref,
            "kind": t.kind.code(),
            "title": t.title,
            "body": display_markdown(d, &t.body),
            "date": t.date.map(|dv| polish::format_date(&dv)).or_else(|| t.date_text.clone()),
            "place": t.place,
            "certainty": t.certainty.map(|c| c.code()),
            "inferred": t.inferred,
            "sources": sources.cite(&t.citations),
            "people": text_people(d, t),
        })
    };
    let summary = texts.iter().find(|t| t.kind == TextKind::Summary).map(|t| text_json(t, &mut sources));
    let section = |kind: TextKind, sources: &mut SourceNumbers| -> Vec<Value> {
        texts.iter().filter(|t| t.kind == kind).map(|t| text_json(t, sources)).collect()
    };
    let bio = section(TextKind::Bio, &mut sources);
    let stories = section(TextKind::Story, &mut sources);
    let sayings = section(TextKind::Saying, &mut sources);
    let trivia = section(TextKind::Trivia, &mut sources);
    let notes = section(TextKind::Note, &mut sources);
    let person_cites = sources.cite(&details.citations);

    // Gallery and documents.
    let mut gallery = Vec::new();
    let mut documents = Vec::new();
    for link in &details.media {
        let Some(object) = d.view.media.get(&link.object) else { continue };
        let date = object.date_text.as_deref().and_then(heirloom_core::gedcom::date::parse);
        match object.kind {
            MediaKind::Photo => gallery.push(json!({
                "id": object.id,
                "path": object.file,
                "caption": link.title.clone().or_else(|| object.title.clone()),
                "date": date.map(|dv| polish::format_date_short(&dv)).or_else(|| object.date_text.clone()),
                "uncertain": date.is_some_and(|dv| polish::is_uncertain(&dv)),
                "profile": info.photo.as_ref().is_some_and(|(id, _)| *id == object.id),
            })),
            _ => {
                let mut tags = Vec::new();
                if object.transcription.is_some() {
                    tags.push("transkrypcja");
                }
                if object.translation.is_some() {
                    tags.push("tłumaczenie");
                }
                documents.push(json!({
                    "id": object.id,
                    "path": object.file,
                    "kind": object.document_type.clone().unwrap_or_else(|| "dokument".into()),
                    "title": object.title.clone().unwrap_or_else(|| file_name(object.file.as_deref())),
                    "meta": object.place,
                    "tags": tags,
                    "year": date.map(|dv| dv.start.year.to_string()).or_else(|| object.date_text.clone()),
                }))
            }
        }
    }

    // Sources, numbered in the order they were first cited.
    let source_list: Vec<Value> = sources
        .order
        .iter()
        .enumerate()
        .map(|(n, xref)| {
            let s = d.view.sources.get(xref);
            json!({
                "n": n + 1,
                "id": xref,
                "title": s.and_then(|s| s.title.clone()).unwrap_or_else(|| xref.clone()),
                "page": sources.pages.get(xref).cloned().flatten(),
                "repository": s.and_then(|s| s.repository.clone()),
            })
        })
        .collect();

    let links: Vec<Value> = details
        .links
        .iter()
        .map(|l| json!({ "url": l.url, "title": l.title.clone().unwrap_or_else(|| l.url.clone()), "kind": l.kind, "domain": domain(&l.url) }))
        .collect();

    let family = family_groups(d, i);
    let timeline = timeline(d, i, &mut sources);
    let mentioned: Vec<Value> = mentions_of(d, i)
        .into_iter()
        .map(|(t, quote)| {
            let owner = owner_of(d, t);
            json!({
                "id": t.xref,
                "kind": t.kind.code(),
                "title": t.title.clone().unwrap_or_else(|| match (t.kind, owner) {
                    (TextKind::Bio, Some(o)) => format!("Życiorys: {}", d.info[o].name),
                    (_, Some(o)) => format!("{}: {}", kind_word(t.kind), d.info[o].name),
                    _ => kind_word(t.kind).to_string(),
                }),
                "quote": quote,
                "owner": owner.map(|o| d.xref(o).to_string()),
            })
        })
        .collect();

    let portrait = info.photo.as_ref().and_then(|(id, path)| {
        let object = d.view.media.get(id)?;
        Some(json!({ "path": path, "caption": object.title, "date": object.date_text }))
    });
    let names: Vec<Value> = p
        .names
        .iter()
        .enumerate()
        .map(|(n, name)| {
            let orig = details.name_originals.get(n).cloned().flatten();
            json!({
                "kind": name.kind.code(),
                "given": name.given,
                "surname": name.surname,
                "nickname": name.nickname,
                "orig": orig.as_ref().map(|o| o.0.clone()),
                "origLang": orig.and_then(|o| o.1),
            })
        })
        .collect();
    let other: Vec<Value> = details
        .other
        .iter()
        .map(|text| {
            let first = text.lines().next().unwrap_or("");
            let mut parts = first.splitn(3, ' ');
            let _level = parts.next();
            let tag = parts.next().unwrap_or("").to_string();
            let value = parts.next().unwrap_or("").to_string();
            let rest: Vec<&str> = text.lines().skip(1).collect();
            json!({ "tag": tag, "value": value, "more": rest.join("\n"), "origin": d.view.version.map(|_| ()).and(None::<String>) })
        })
        .collect();

    Ok(json!({
        "person": d.summary(i),
        "uid": p.uid,
        "names": names,
        "age": age_text(d, i),
        "generationBranch": generation_branch(d, i),
        "tags": details.tags,
        "portrait": portrait,
        "summary": summary,
        "facts": facts,
        "family": family,
        "bio": bio,
        "stories": stories,
        "sayings": sayings,
        "trivia": trivia,
        "notes": notes,
        "gallery": gallery,
        "documents": documents,
        "sources": source_list,
        "personSources": person_cites,
        "links": links,
        "timeline": timeline.0,
        "undated": timeline.1,
        "mentionedIn": mentioned,
        "history": crate::activity::person_history(d, history, p.xref.as_str()),
        "other": other,
        "created": details.created,
        "changed": details.changed,
        "batch": details.batch,
        "noData": details.no_data,
    }))
}

fn kind_word(kind: TextKind) -> &'static str {
    match kind {
        TextKind::Summary => "W skrócie",
        TextKind::Bio => "Życiorys",
        TextKind::Story => "Historia",
        TextKind::Saying => "Powiedzonko",
        TextKind::Trivia => "Ciekawostka",
        TextKind::Note => "Uwaga",
    }
}

/// The first person who links a text (its owner).
pub fn owner_of(d: &Derived, text: &Text) -> Option<usize> {
    let xref = text.xref.as_deref()?;
    d.view.people.iter().position(|p| {
        p.texts.iter().any(|t| matches!(t, heirloom_core::gedcom::view::TextRef::Shared(x) if x == xref))
    })
}

/// Everyone a shared text is linked from, and everyone it mentions.
pub fn text_people(d: &Derived, text: &Text) -> Vec<Value> {
    let mut list: Vec<usize> = Vec::new();
    if let Some(xref) = text.xref.as_deref() {
        for (i, p) in d.view.people.iter().enumerate() {
            if p.texts.iter().any(|t| matches!(t, heirloom_core::gedcom::view::TextRef::Shared(x) if x == xref)) {
                list.push(i);
            }
        }
    }
    for uid in crate::text::mentioned_uids(&text.body) {
        if let Some(&i) = d.view.uid_index.get(&uid) {
            if !list.contains(&i) {
                list.push(i);
            }
        }
    }
    list.into_iter().map(|i| json!({ "id": d.xref(i), "name": d.info[i].name, "initials": d.info[i].initials, "branch": d.info[i].branch })).collect()
}

fn file_name(path: Option<&str>) -> String {
    path.and_then(|p| p.rsplit(['/', '\\']).next()).unwrap_or("plik").to_string()
}

pub fn domain(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let host = rest.split('/').next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

/// Rodzice, Rodzeństwo, Małżonek i dzieci (per partner), Dziadkowie, Wnuki.
fn family_groups(d: &Derived, i: usize) -> Vec<Value> {
    let info = &d.info[i];
    let tile = |other: usize, extra: Option<String>| {
        let mut v = relative(d, i, other);
        let years = card_years(d, other);
        let label = v["label"].as_str().unwrap_or("").to_string();
        let mut parts = vec![label];
        if let Some(extra) = extra {
            parts.push(extra);
        }
        if let Some(m) = &d.info[other].maiden {
            parts.push(format!("z d. {m}"));
        }
        if !years.is_empty() {
            parts.push(years);
        }
        v["line"] = json!(parts.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "));
        v
    };
    let mut groups = Vec::new();
    let mut parents = info.parents.clone();
    parents.sort_by_key(|&p| d.view.model.persons[p].sex != Sex::Male);
    if !parents.is_empty() {
        groups.push(json!({ "title": "Rodzice", "people": parents.iter().map(|&p| tile(p, None)).collect::<Vec<_>>() }));
    }
    let siblings = kin::siblings(d, i);
    if !siblings.is_empty() {
        groups.push(json!({ "title": "Rodzeństwo", "people": siblings.iter().map(|&s| tile(s, None)).collect::<Vec<_>>() }));
    }
    let mut partners = info.partners.clone();
    kin::sort_by_birth(d, &mut partners);
    let mut shown_children: Vec<usize> = Vec::new();
    for &partner in &partners {
        let mut people = vec![tile(partner, kin::marriage(d, i, partner).1.map(|y| format!("ślub {y}")))];
        let mut kids: Vec<usize> = info.children.iter().copied().filter(|c| d.info[*c].parents.contains(&partner)).collect();
        kin::sort_by_birth(d, &mut kids);
        for c in kids {
            shown_children.push(c);
            people.push(tile(c, None));
        }
        let title = match (d.view.model.persons[partner].sex, people.len() > 1) {
            (Sex::Female, true) => "Żona i dzieci",
            (Sex::Female, false) => "Żona",
            (Sex::Male, true) => "Mąż i dzieci",
            (Sex::Male, false) => "Mąż",
            (_, true) => "Partner i dzieci",
            (_, false) => "Partner",
        };
        let title = if partners.len() > 1 { format!("{title} ({})", d.info[partner].given) } else { title.to_string() };
        groups.push(json!({ "title": title, "people": people }));
    }
    let mut other_kids: Vec<usize> = info.children.iter().copied().filter(|c| !shown_children.contains(c)).collect();
    if !other_kids.is_empty() {
        kin::sort_by_birth(d, &mut other_kids);
        groups.push(json!({ "title": "Dzieci", "people": other_kids.iter().map(|&c| tile(c, None)).collect::<Vec<_>>() }));
    }
    // Once each: parents who are cousins or siblings share grandparents.
    let mut grandparents: Vec<usize> = Vec::new();
    for g in info.parents.iter().flat_map(|&p| d.info[p].parents.iter().copied()) {
        if !grandparents.contains(&g) {
            grandparents.push(g);
        }
    }
    if !grandparents.is_empty() {
        groups.push(json!({ "title": "Dziadkowie", "people": grandparents.iter().map(|&g| tile(g, None)).collect::<Vec<_>>() }));
    }
    let mut grandchildren: Vec<usize> = info.children.iter().flat_map(|&c| d.info[c].children.iter().copied()).collect();
    grandchildren.sort_unstable();
    grandchildren.dedup();
    kin::sort_by_birth(d, &mut grandchildren);
    if !grandchildren.is_empty() {
        groups.push(json!({ "title": "Wnuki", "people": grandchildren.iter().map(|&g| tile(g, None)).collect::<Vec<_>>() }));
    }
    groups
}

/// Years on person cards and tiles (design v2, A9): "1878 † 1951", "ok. 1850 † przed 1910", "1931 · żyje".
pub fn card_years(d: &Derived, i: usize) -> String {
    let info = &d.info[i];
    let birth = info.birth.as_ref().map(|b| b.year.clone());
    let death = info.death.as_ref().map(|b| b.year.clone());
    match (birth, death) {
        (Some(b), Some(dd)) => format!("{b} † {dd}"),
        (Some(b), None) if info.living => format!("{b} · żyje"),
        (Some(b), None) => format!("{b} † ?"),
        (None, Some(dd)) => format!("? † {dd}"),
        (None, None) if info.living => "żyje".into(),
        (None, None) => String::new(),
    }
}

/// "ok. 1850 – przed 1910", "1853–1921", "ur. 1931" (lists and tables).
pub fn years_range(d: &Derived, i: usize) -> String {
    let info = &d.info[i];
    let birth = info.birth.as_ref().map(|b| b.year.clone());
    let death = info.death.as_ref().map(|b| b.year.clone());
    match (birth, death) {
        (Some(b), Some(dd)) => {
            let spaced = b.contains(' ') || dd.contains(' ');
            if spaced { format!("{b} – {dd}") } else { format!("{b}–{dd}") }
        }
        (Some(b), None) if info.living => format!("ur. {b}"),
        (Some(b), None) => format!("{b} – ?"),
        (None, Some(dd)) => format!("? – {dd}"),
        (_, None) => String::new(),
    }
}

/// The person's own events plus the family's (births of children, deaths of parents…), in date order; and the
/// facts without a date ("BEZ DATY").
fn timeline(d: &Derived, i: usize, sources: &mut SourceNumbers) -> (Vec<Value>, Vec<Value>) {
    let info = &d.info[i];
    let female = d.view.model.persons[i].sex == Sex::Female;
    let mut rows: Vec<(i64, Value)> = Vec::new();
    let mut undated = Vec::new();
    let birth = info.birth.clone();
    let age_at = |date: &Option<DateInfo>| -> Option<String> {
        let (years, approx) = age_between(&birth, date)?;
        if years == 0 {
            return None;
        }
        Some(if approx { format!("ok. {years}") } else { format!("lat {years}") })
    };
    let push = |date: Option<DateInfo>, kind: String, place: Option<String>, description: Option<String>, source: Vec<usize>, family: bool, rows: &mut Vec<(i64, Value)>| {
        let Some(date_info) = date else { return };
        let sort = date_info.sort.unwrap_or(i64::MAX);
        let age = age_at(&Some(date_info.clone()));
        rows.push((
            sort,
            json!({
                "date": date_info.short,
                "uncertain": date_info.uncertain,
                "age": age,
                "type": kind,
                "place": place,
                "description": description,
                "sources": source,
                "family": family,
            }),
        ));
    };
    for f in &d.view.people[i].facts {
        let date = DateInfo::from_fact(f);
        let cites = sources.cite(&f.citations);
        let description = match f.tag.as_str() {
            "BIRT" => {
                let parents: Vec<String> = info.parents.iter().map(|&p| polish::given_genitive(&d.info[p].given, d.view.model.persons[p].sex == Sex::Female)).collect();
                (!parents.is_empty()).then(|| format!("{} {}", if female { "córka" } else { "syn" }, parents.join(" i ")))
            }
            _ => f.value.clone().or_else(|| f.note.clone()),
        };
        if date.is_none() {
            if f.value.is_some() || f.place.is_some() {
                undated.push(json!({ "type": event_name(&f.tag, f.kind.as_deref()), "value": f.value.clone().or_else(|| f.place.clone()) }));
            }
            continue;
        }
        push(date, event_name(&f.tag, f.kind.as_deref()), f.place.clone(), description, cites, false, &mut rows);
    }
    for &partner in &info.partners {
        let (pa, pb) = (&d.view.model.persons[i], &d.view.model.persons[partner]);
        for fam in pa.fams.iter().filter(|f| pb.fams.contains(f)) {
            if let Some((_, fd)) = d.view.family(fam) {
                for f in &fd.facts {
                    let partner_female = pb.sex == Sex::Female;
                    let partner_birth_surname = pb.birth_name().map(|n| n.surname.clone()).unwrap_or_default();
                    let name = format!(
                        "{} {}",
                        polish::given_instrumental(&d.info[partner].given, partner_female),
                        polish::surname_instrumental(&partner_birth_surname, partner_female)
                    );
                    let kind = match f.tag.as_str() {
                        "MARR" => format!("Ślub {} {}", polish::z_or_ze(&name), name.trim()),
                        "DIV" => format!("Rozwód {} {}", polish::z_or_ze(&name), name.trim()),
                        "ENGA" => format!("Zaręczyny {} {}", polish::z_or_ze(&name), name.trim()),
                        "MARB" => "Zapowiedzi".to_string(),
                        _ => fact_key(&f.tag, f.kind.as_deref(), female),
                    };
                    let cites = sources.cite(&f.citations);
                    push(DateInfo::from_fact(f), kind, f.place.clone(), None, cites, false, &mut rows);
                }
            }
        }
        if let Some(death) = &d.info[partner].death {
            // „Śmierć żony, Tekli”, like „Śmierć ojca, Antoniego” below.
            let word = kin::label(d, i, partner).unwrap_or_default();
            let whose = match word.as_str() {
                "żona" => Some("żony"),
                "mąż" => Some("męża"),
                "partnerka" => Some("partnerki"),
                "partner" => Some("partnera"),
                _ => None,
            };
            let given = &d.info[partner].given;
            let text = match whose {
                Some(whose) => format!("Śmierć {whose}, {}", polish::given_genitive(given, d.view.model.persons[partner].sex == Sex::Female)),
                None => format!("Śmierć: {word}, {given}"),
            };
            push(Some(death.clone()), text, d.info[partner].death_place.clone(), None, Vec::new(), true, &mut rows);
        }
    }
    for &c in &info.children {
        let son = d.view.model.persons[c].sex != Sex::Female;
        let label = if son { "syna" } else { "córki" };
        let given = polish::given_genitive(&d.info[c].given, !son);
        push(d.info[c].birth.clone(), format!("Narodziny {label} {given}"), None, None, Vec::new(), true, &mut rows);
    }
    for &p in &info.parents {
        let father = d.view.model.persons[p].sex == Sex::Male;
        let given = polish::given_genitive(&d.info[p].given, !father);
        push(d.info[p].death.clone(), format!("Śmierć {}, {given}", if father { "ojca" } else { "matki" }), d.info[p].death_place.clone(), None, Vec::new(), true, &mut rows);
    }
    // Family events after the person's death are not part of their life.
    let end = info.death.as_ref().and_then(|dd| dd.sort).unwrap_or(i64::MAX);
    rows.retain(|(sort, v)| !v["family"].as_bool().unwrap_or(false) || *sort <= end);
    rows.sort_by_key(|(sort, _)| *sort);
    (rows.into_iter().map(|(_, v)| v).collect(), undated)
}

/// A date as it can be typed back into the smart date field ("12.03.1878", "ok. 1850", "między 1850 a 1855"). The
/// Polish form only when it reads back as the same date; otherwise the date as written (EST, INT with its phrase,
/// "1750/51", other calendars), which the field reads too, so saving the form unchanged changes nothing.
fn date_input(fact: &Fact) -> String {
    match (fact.date, fact.date_text.as_deref()) {
        (Some(d), Some(raw)) => {
            let short = polish::format_date_short(&d);
            if polish::parse_date_input(&short) == polish::parse_date_input(raw) { short } else { raw.to_string() }
        }
        (None, Some(raw)) => raw.trim_matches(['(', ')']).to_string(),
        (_, None) => String::new(),
    }
}

/// The values the edit form starts with (spec §4.8), exactly as they can be typed back.
pub fn edit_data(d: &Derived, id: &str) -> Result<Value, ApiError> {
    let i = d.index(id).ok_or_else(|| not_found(id))?;
    let (p, details, info) = d.person(i);
    let birth_name = p.names.iter().find(|n| n.kind == heirloom_core::gedcom::model::NameKind::Birth).or(p.names.first());
    let married = p.married_name();
    let event = |tags: &[&str]| {
        details.facts.iter().find(|f| tags.contains(&f.tag.as_str())).map(|f| json!({ "date": date_input(f), "place": f.place.clone().unwrap_or_default(), "certainty": f.certainty.map(|c| c.code()) }))
    };
    let dead = details.facts.iter().any(|f| matches!(f.tag.as_str(), "DEAT" | "BURI" | "CREM"));
    Ok(json!({
        "given": birth_name.map(|n| n.given.clone()).unwrap_or_default(),
        "surname": married.or(birth_name).map(|n| n.surname.clone()).unwrap_or_default(),
        "birthSurname": if married.is_some() { birth_name.map(|n| n.surname.clone()).unwrap_or_default() } else { String::new() },
        "nickname": info.nickname.clone().unwrap_or_default(),
        "sex": p.sex.code(),
        "status": if dead { "deceased" } else if info.living { "living" } else { "unknown" },
        "otherNames": p.names.iter().filter(|n| n.kind == heirloom_core::gedcom::model::NameKind::Other).map(|n| json!({ "given": n.given, "surname": n.surname, "note": n.phrase.clone().unwrap_or_default() })).collect::<Vec<_>>(),
        "events": {
            "birth": event(&["BIRT"]),
            "baptism": event(&["BAPM", "CHR"]),
            "death": event(&["DEAT"]),
            "burial": event(&["BURI"]),
        },
        "occupation": details.facts.iter().find(|f| f.tag == "OCCU").and_then(|f| f.value.clone()).unwrap_or_default(),
        "religion": details.facts.iter().find(|f| f.tag == "RELI").and_then(|f| f.value.clone()).unwrap_or_default(),
        "noData": details.no_data,
    }))
}

/// The ASSO role codes in Polish, for relations outside the family.
pub fn role_word(role: &str, phrase: Option<&str>) -> String {
    if let Some(p) = phrase.filter(|p| !p.trim().is_empty()) {
        return p.trim().to_string();
    }
    match role {
        "FRIEND" => "przyjaciel",
        "NGHBR" => "sąsiad",
        "GODP" => "chrzestny",
        "WITN" => "świadek",
        "CLERGY" | "OFFICIATOR" => "duchowny",
        _ => "inna relacja",
    }
    .to_string()
}

/// Every direct relation of a person with what the edit screen needs to change it: the family that links them and
/// the kind of link (spec §4.8 „Relacje”).
pub fn relations(d: &Derived, id: &str) -> Result<Value, ApiError> {
    let i = d.index(id).ok_or_else(|| not_found(id))?;
    let p = &d.view.model.persons[i];
    let mut out = Vec::new();
    let row = |other: usize, role: &str, family: Option<&str>, detail: String| {
        let mut v = d.summary(other);
        v["role"] = json!(role);
        v["family"] = json!(family);
        v["label"] = json!(kin::label(d, i, other).unwrap_or_else(|| role.to_string()));
        v["detail"] = json!(detail);
        v["years"] = json!(card_years(d, other));
        v
    };
    for fam in &p.famc {
        let Some((family, _)) = d.view.family(fam) else { continue };
        let pedi = d.view.people[i].pedigree.iter().find(|(f, _)| f == fam).map(|(_, x)| x.as_str());
        let kind = match pedi {
            Some("ADOPTED") => "adoptowany",
            Some("FOSTER") => "przybrany",
            Some(_) => "nieznany",
            None => "biologiczny",
        };
        for parent in family.partners.iter().filter_map(|x| d.index(x)) {
            let female = d.view.model.persons[parent].sex == Sex::Female;
            let kind = if female && kind.ends_with('y') { format!("{}a", &kind[..kind.len() - 1]) } else { kind.to_string() };
            let mut v = row(parent, "parent", Some(fam), kind);
            v["pedi"] = json!(pedi.map_or("birth", |x| match x { "ADOPTED" => "adopted", "FOSTER" => "foster", _ => "unknown" }));
            out.push(v);
        }
    }
    for fam in &p.fams {
        let Some((family, details)) = d.view.family(fam) else { continue };
        let year = details.facts.iter().find(|f| f.tag == "MARR").and_then(|f| f.date.map(|v| polish::year_text(&v)));
        for partner in family.partners.iter().filter_map(|x| d.index(x)).filter(|&x| x != i) {
            let detail = match (&year, details.married) {
                (Some(y), _) => format!("ślub {y}"),
                (None, true) => "małżeństwo".to_string(),
                (None, false) => "związek".to_string(),
            };
            let mut v = row(partner, "partner", Some(fam), detail);
            v["married"] = json!(details.married);
            out.push(v);
        }
        let mut kids: Vec<usize> = family.children.iter().filter_map(|x| d.index(x)).collect();
        kin::sort_by_birth(d, &mut kids);
        for child in kids {
            let pedi = d.view.people[child].pedigree.iter().find(|(f, _)| f == fam).map(|(_, x)| x.as_str());
            let female = d.view.model.persons[child].sex == Sex::Female;
            let kind = match pedi {
                Some("ADOPTED") => "adoptowan",
                Some("FOSTER") => "przybran",
                Some(_) => "nieznan",
                None => "biologiczn",
            };
            let mut v = row(child, "child", Some(fam), format!("{kind}{}", if female { "a" } else { "y" }));
            v["pedi"] = json!(pedi.map_or("birth", |x| match x { "ADOPTED" => "adopted", "FOSTER" => "foster", _ => "unknown" }));
            out.push(v);
        }
    }
    for s in kin::siblings(d, i) {
        out.push(row(s, "sibling", None, String::new()));
    }
    for a in &d.view.people[i].associates {
        let Some(other) = d.index(&a.person) else { continue };
        let mut v = row(other, "associate", None, role_word(&a.role, a.phrase.as_deref()));
        v["label"] = json!(role_word(&a.role, a.phrase.as_deref()));
        v["assoRole"] = json!(a.role);
        out.push(v);
    }
    // Relations recorded on the other person only (they named this one as their godchild, say).
    for (j, other) in d.view.people.iter().enumerate() {
        for a in other.associates.iter().filter(|a| a.person == p.xref) {
            if !d.view.people[i].associates.iter().any(|mine| d.index(&mine.person) == Some(j)) {
                let word = role_word(&a.role, a.phrase.as_deref());
                let mut v = row(j, "associate", None, word.clone());
                v["label"] = json!(format!("{word} (z drugiej strony)"));
                v["assoRole"] = json!(a.role);
                out.push(v);
            }
        }
    }
    Ok(Value::Array(out))
}

/// Years for cards: birth and death with their uncertainty (so the UI can underline each side).
pub fn life_span(d: &Derived, i: usize) -> Value {
    let info = &d.info[i];
    json!({ "birth": date_json(&info.birth), "death": date_json(&info.death), "living": info.living })
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::gedcom::Document;

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 UID u1\n1 NAME Józef /Kowalski/\n2 NICK Dziadek Józek\n1 SEX M\n\
1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka\n3 NOTE parafia Łęczna\n2 SOUR @S1@\n\
1 DEAT\n2 DATE 3 FEB 1951\n2 PLAC Lublin\n1 OCCU rolnik\n1 OCCU kolejarz\n2 DATE FROM 1905\n\
1 FAMC @F1@\n1 FAMS @F2@\n1 SNOTE @N1@\n1 SNOTE @N2@\n1 SNOTE @N3@\n1 _HLM_TAG kolejarz\n\
0 @I2@ INDI\n1 UID u2\n1 NAME Antoni /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE ABT 1850\n1 DEAT\n2 DATE BEF 1910\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 UID u3\n1 NAME Marianna /Nowak/\n2 TYPE BIRTH\n1 NAME Marianna /Kowalska/\n2 TYPE MARRIED\n1 SEX F\n\
1 BIRT\n2 DATE ABT 1882\n1 FAMS @F2@\n1 SNOTE @N4@\n\
0 @I4@ INDI\n1 UID u4\n1 NAME Stanisław /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1905\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 CHIL @I1@\n\
0 @F2@ FAM\n1 HUSB @I1@\n1 WIFE @I3@\n1 CHIL @I4@\n1 MARR\n2 DATE 14 FEB 1904\n2 PLAC Łęczna\n\
0 @N1@ SNOTE Syn rolnika z Wólki.\n1 _HLM_KIND summary\n\
0 @N2@ SNOTE Józef urodził się jako syn [Antoniego](person:u2).\n1 _HLM_KIND bio\n1 _HLM_TITLE Dzieciństwo\n1 SOUR @S1@\n\
0 @N3@ SNOTE „Pociąg nie czeka.”\n1 _HLM_KIND saying\n\
0 @N4@ SNOTE Wyszła za [Józefa](person:u1) w 1904.\n1 _HLM_KIND bio\n1 _HLM_TITLE Ślub\n\
0 @S1@ SOUR\n1 TITL Akt urodzenia nr 45/1878\n0 TRLR\n";

    fn derived() -> Derived {
        crate::derive::build(&Document::from_bytes(FILE.as_bytes()).0)
    }

    #[test]
    fn profile_has_facts_family_texts_and_sources() {
        let d = derived();
        let p = profile(&d, "@I1@", &[]).unwrap();
        assert_eq!(p["facts"][0]["key"], "Urodzony");
        assert_eq!(p["facts"][0]["value"], "12 marca 1878, Wólka (par. Łęczna)");
        assert_eq!(p["facts"][0]["sources"][0], 1);
        assert_eq!(p["facts"][1]["key"], "Ślub");
        assert_eq!(p["facts"][1]["value"], "14 lutego 1904, Łęczna");
        assert_eq!(p["facts"][2]["key"], "Zmarł");
        assert!(p["facts"].as_array().unwrap().iter().any(|f| f["key"] == "Zawód" && f["value"] == "rolnik; kolejarz (od 1905)"));
        assert_eq!(p["age"], "72 lata");
        assert_eq!(p["summary"]["body"], "Syn rolnika z Wólki.");
        assert_eq!(p["bio"][0]["title"], "Dzieciństwo");
        assert_eq!(p["bio"][0]["body"], "Józef urodził się jako syn [Antoniego](person:@I2@).", "mentions point to records");
        assert_eq!(p["bio"][0]["sources"][0], 1, "the same source keeps its number");
        assert_eq!(p["sayings"][0]["body"], "„Pociąg nie czeka.”");
        assert_eq!(p["sources"][0]["title"], "Akt urodzenia nr 45/1878");
        assert_eq!(p["family"][0]["title"], "Rodzice");
        assert_eq!(p["family"][0]["people"][0]["line"], "ojciec · ok. 1850 † przed 1910");
        assert_eq!(p["family"][1]["title"], "Żona i dzieci");
        assert_eq!(p["family"][1]["people"][0]["line"], "żona · ślub 1904 · z d. Nowak · ok. 1882 † ?");
        assert_eq!(p["mentionedIn"][0]["title"], "Ślub");
        assert_eq!(p["generationBranch"], "Pokolenie II · gałąź Kowalskich");
        let types: Vec<&str> = p["timeline"].as_array().unwrap().iter().map(|t| t["type"].as_str().unwrap()).collect();
        assert_eq!(types, ["Urodzenie", "Ślub z Marianną Nowak", "Praca", "Narodziny syna Stanisława", "Śmierć ojca, Antoniego", "Zgon"]);
        assert_eq!(p["undated"][0]["value"], "rolnik");
    }

    #[test]
    fn search_finds_every_name_form_without_polish_letters() {
        let d = derived();
        let ids = |q: &str| search(&d, q, 10).as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids("jozef"), ["@I1@"]);
        assert_eq!(ids("dziadek"), ["@I1@"]);
        assert_eq!(ids("nowak"), ["@I3@"], "maiden names are found");
        assert_eq!(ids("mar kow"), ["@I3@"]);
        assert!(ids("stanislaw kow").contains(&"@I4@".to_string()));
    }

    #[test]
    fn a_surname_finds_its_other_gender_form() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @A@ INDI\n1 NAME Helena /Wiśniewska/\n1 SEX F\n0 @B@ INDI\n1 NAME Franciszek /Wiśniewski/\n1 SEX M\n\
0 @C@ INDI\n1 NAME Jan /Zawadzki/\n1 SEX M\n0 @D@ INDI\n1 NAME Anna /Wiśniewicz/\n1 SEX F\n0 TRLR\n";
        let d = crate::derive::build(&Document::from_bytes(text.as_bytes()).0);
        let ids = |q: &str| search(&d, q, 10).as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids("wisniewska"), ["@A@", "@B@"], "the exact form first");
        assert_eq!(ids("Wiśniewski"), ["@B@", "@A@"]);
        assert_eq!(ids("zawadzka"), ["@C@"]);
        let mut prefix = ids("wisniew");
        prefix.sort();
        assert_eq!(prefix, ["@A@", "@B@", "@D@"], "a word being typed matches every name that starts with it");
    }

    #[test]
    fn nonsense_years_do_not_break_the_profile() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 BIRT\n2 DATE 5 BCE\n1 DEAT\n2 DATE 2147483647\n0 TRLR\n";
        let d = crate::derive::build(&Document::from_bytes(text.as_bytes()).0);
        assert!(profile(&d, "@I1@", &[]).is_ok());
    }

    #[test]
    fn a_grandparent_of_both_parents_is_listed_once() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @GF@ INDI\n1 NAME Jan /Nowak/\n1 SEX M\n\
0 @M1@ INDI\n1 NAME Anna /Mazur/\n1 SEX F\n0 @M2@ INDI\n1 NAME Ewa /Wójcik/\n1 SEX F\n\
0 @A@ INDI\n1 NAME Piotr /Nowak/\n1 SEX M\n0 @B@ INDI\n1 NAME Zofia /Nowak/\n1 SEX F\n0 @C@ INDI\n1 NAME Adam /Nowak/\n1 SEX M\n\
0 @F1@ FAM\n1 HUSB @GF@\n1 WIFE @M1@\n1 CHIL @A@\n0 @F2@ FAM\n1 HUSB @GF@\n1 WIFE @M2@\n1 CHIL @B@\n\
0 @F3@ FAM\n1 HUSB @A@\n1 WIFE @B@\n1 CHIL @C@\n0 TRLR\n";
        let d = crate::derive::build(&Document::from_bytes(text.as_bytes()).0);
        let groups = family_groups(&d, d.index("@C@").unwrap());
        let grandparents = groups.iter().find(|g| g["title"] == "Dziadkowie").unwrap()["people"].as_array().unwrap();
        let ids: Vec<&str> = grandparents.iter().map(|p| p["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["@GF@", "@M1@", "@M2@"], "the half-siblings' father once");
    }

    #[test]
    fn hover_card_relation() {
        let d = derived();
        let card = hover(&d, "@I3@", Some("@I1@")).unwrap();
        assert_eq!(card["relation"], "Żona Józefa · ślub 1904");
    }
}
