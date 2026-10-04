//! Data for Start, Nazwiska, Miejsca, Historie, Media, Źródła and the first-open screen (spec §4.18–§4.33).

use crate::derive::{DateInfo, Derived};
use crate::media_edit::FileRoots;
use crate::people::{self, domain, years_range};
use crate::ApiError;
use heirloom_core::fold::fold;
use heirloom_core::gedcom::view::{MediaKind, TextKind};
use heirloom_core::history::{Action, Entry};
use heirloom_core::polish;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

fn person_ref(d: &Derived, i: usize) -> Value {
    let mut v = json!({ "id": d.xref(i), "name": d.info[i].name, "initials": d.info[i].initials, "branch": d.info[i].branch, "photo": d.info[i].photo.as_ref().map(|p| &p.1) });
    if let Some(from) = d.from(i) {
        v["from"] = json!(from);
    }
    v
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

// ---------- Start ----------

fn month_day_now() -> (u8, u8, i32) {
    let now = heirloom_core::history::now();
    let m = now.get(5..7).and_then(|s| s.parse().ok()).unwrap_or(1);
    let d = now.get(8..10).and_then(|s| s.parse().ok()).unwrap_or(1);
    let y = now.get(0..4).and_then(|s| s.parse().ok()).unwrap_or(2026);
    (m, d, y)
}

/// Start (spec §4.19): stats, "W tym dniu", recently added and edited, from the history and CREA/CHAN stamps.
pub fn start(d: &Derived, history: &[Entry], title: &str) -> Value {
    let photos = d.view.media.values().filter(|m| m.kind == MediaKind::Photo).count();
    let documents = d.view.media.values().filter(|m| m.kind != MediaKind::Photo).count();
    let (month, day, year) = month_day_now();
    let on = |date: &Option<DateInfo>| date.as_ref().and_then(|dd| dd.value).filter(|v| v.start.month == Some(month) && v.start.day == Some(day) && !dd_uncertain(date)).map(|v| v.start.year);
    fn dd_uncertain(date: &Option<DateInfo>) -> bool {
        date.as_ref().is_some_and(|d| d.uncertain)
    }
    let mut birthdays = Vec::new();
    let mut deaths = Vec::new();
    for i in 0..d.info.len() {
        let info = &d.info[i];
        if let Some(y) = on(&info.birth) {
            let context = if info.living { format!("ur. {y} · żyje") } else { format!("ur. {y}{}", info.death.as_ref().map(|dd| format!(" · zm. {}", dd.year)).unwrap_or_default()) };
            let value = if info.living { crate::count_pl(usize::try_from(year - y).unwrap_or(0), "rok", "lata", "lat") } else { format!("{}. rocznica", year - y) };
            birthdays.push(json!({ "person": person_ref(d, i), "context": context, "value": value }));
        }
        if let Some(y) = on(&info.death) {
            let place = info.death_place.clone().map(|p| format!(", {p}")).unwrap_or_default();
            let ago = crate::count_pl(usize::try_from(year - y).unwrap_or(0), "rok", "lata", "lat");
            deaths.push(json!({ "person": person_ref(d, i), "context": format!("zm. {y}{place}"), "value": format!("{ago} temu") }));
        }
    }
    let mut weddings = Vec::new();
    for (f, details) in d.view.model.families.iter().zip(&d.view.families) {
        for fact in details.facts.iter().filter(|x| x.tag == "MARR") {
            let date = DateInfo::from_fact(fact);
            if let Some(y) = on(&date) {
                let names: Vec<usize> = f.partners.iter().filter_map(|p| d.index(p)).collect();
                if let Some(&first) = names.first() {
                    let label = match names.as_slice() {
                        [a, b] => format!("{} i {} {}", d.info[*a].given, d.info[*b].given, polish::family_plural(&d.info[*a].surname)),
                        _ => d.info[first].name.clone(),
                    };
                    let place = fact.place.clone().map(|p| format!(", {p}")).unwrap_or_default();
                    weddings.push(json!({ "person": person_ref(d, first), "label": label, "context": format!("ślub {y}{place}"), "value": format!("{}. rocznica", year - y) }));
                }
            }
        }
    }

    // Recently added: newest CREA first (or history "create" entries).
    let mut created: Vec<(String, usize)> = (0..d.info.len()).filter_map(|i| d.view.people[i].created.clone().map(|c| (c, i))).collect();
    created.sort_by(|a, b| b.0.cmp(&a.0));
    let origin_of = |xref: &str| -> String {
        match history.iter().find(|e| e.record == xref && e.action == Action::Create) {
            Some(e) => match &e.batch {
                Some(b) => format!("import „{b}”"),
                None => format!("ręcznie · {}", e.author),
            },
            None => "ręcznie".into(),
        }
    };
    let recently_added: Vec<Value> = created.iter().take(6).map(|(when, i)| json!({ "person": person_ref(d, *i), "origin": origin_of(d.xref(*i)), "when": when })).collect();

    // Recently edited: the last history entries about people.
    let mut edited = Vec::new();
    let mut seen = HashSet::new();
    for entry in history.iter().rev() {
        if entry.action != Action::Update {
            continue;
        }
        let Some(i) = d.index(&entry.record).or_else(|| owner_of_record(d, &entry.record)) else { continue };
        if !seen.insert(i) {
            continue;
        }
        let changes = crate::activity::changes(d, entry);
        let what: Vec<String> = changes.iter().filter_map(|c| c["field"].as_str().map(|f| f.to_lowercase())).take(3).collect();
        let who = entry.batch.as_ref().map_or(entry.author.clone(), |_| "import".into());
        edited.push(json!({ "person": person_ref(d, i), "what": if what.is_empty() { "zmiany".into() } else { what.join(", ") }, "who": who, "when": entry.ts }));
        if edited.len() >= 5 {
            break;
        }
    }

    json!({
        "title": title,
        "stats": { "people": d.info.len(), "photos": photos, "documents": documents, "generations": d.max_generation },
        "onThisDay": { "birthdays": birthdays, "deaths": deaths, "weddings": weddings, "day": day, "month": month },
        "recentlyAdded": recently_added,
        "recentlyEdited": edited,
        "lastBackup": Value::Null,
    })
}

fn owner_of_record(d: &Derived, record: &str) -> Option<usize> {
    d.view.people.iter().position(|p| {
        p.texts.iter().any(|t| matches!(t, heirloom_core::gedcom::view::TextRef::Shared(x) if x == record)) || p.media.iter().any(|m| m.object == record)
    })
}

// ---------- First open ----------

/// "Pierwsze otwarcie" (spec §4.18): what was found, what's missing, suggested start people.
pub fn first_open(d: &Derived, root: &Path) -> Value {
    let photos = d.view.media.values().filter(|m| m.kind == MediaKind::Photo).count();
    let missing = missing_files(d, &FileRoots::One(root.to_path_buf())).len();
    let other_fields: usize = d.view.people.iter().map(|p| p.other.len()).sum();
    // Suggestions: most connections, the youngest generation, the oldest with dates.
    let mut suggestions = Vec::new();
    let most = (0..d.info.len()).max_by_key(|&i| (d.info[i].parents.len() + d.info[i].children.len() + d.info[i].partners.len(), std::cmp::Reverse(i)));
    if let Some(i) = most {
        suggestions.push(json!({ "person": person_ref(d, i), "reason": "najwięcej powiązań" }));
    }
    let youngest = (0..d.info.len()).filter(|&i| d.info[i].birth.as_ref().and_then(|b| b.sort).is_some()).max_by_key(|&i| d.info[i].birth.as_ref().and_then(|b| b.sort));
    if let Some(i) = youngest.filter(|i| Some(*i) != most) {
        suggestions.push(json!({ "person": person_ref(d, i), "reason": "najmłodsze pokolenie" }));
    }
    let oldest = (0..d.info.len()).filter(|&i| d.info[i].birth.as_ref().and_then(|b| b.sort).is_some()).min_by_key(|&i| d.info[i].birth.as_ref().and_then(|b| b.sort));
    if let Some(i) = oldest.filter(|i| Some(*i) != most && Some(*i) != youngest) {
        suggestions.push(json!({ "person": person_ref(d, i), "reason": "najstarszy z datami" }));
    }
    json!({
        "people": d.info.len(),
        "families": d.view.model.families.len(),
        "photos": photos,
        "sources": d.view.sources.len(),
        "missingFiles": missing,
        "otherFields": other_fields,
        "suggestions": suggestions,
    })
}

// ---------- Surnames ----------

pub fn surnames(d: &Derived) -> Value {
    let groups: Vec<Value> = d
        .groups
        .iter()
        .map(|g| {
            let years: Vec<i32> = g.born.iter().filter_map(|&i| d.info[i].birth.as_ref().and_then(|b| b.value.map(|v| v.start.year))).collect();
            let places = top_places(d, &g.born, 2);
            json!({
                "key": g.key,
                "name": g.name,
                "plural": g.plural,
                "branch": g.branch,
                "born": g.born.len(),
                "married": g.married.len(),
                "from": years.iter().min(),
                "to": years.iter().max(),
                "places": places,
                "letter": g.name.chars().next().map(|c| c.to_uppercase().to_string()),
            })
        })
        .collect();
    let without = d.info.iter().filter(|i| i.group.is_none()).count();
    json!({ "groups": groups, "withoutSurname": without })
}

fn top_places(d: &Derived, people: &[usize], n: usize) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for &i in people {
        if let Some(place) = &d.info[i].birth_place {
            *counts.entry(place.split(',').next().unwrap_or(place).trim().to_string()).or_default() += 1;
        }
    }
    let mut list: Vec<(String, usize)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.into_iter().take(n).map(|(p, _)| p).collect()
}

fn parents_text(d: &Derived, i: usize) -> String {
    let names: Vec<&str> = d.info[i].parents.iter().map(|&p| d.info[p].given.as_str()).collect();
    if names.is_empty() { "—".into() } else { names.join(" i ") }
}

pub fn surname(d: &Derived, key: &str) -> Result<Value, ApiError> {
    let g = d.group_index.get(key).map(|&g| &d.groups[g]).ok_or_else(|| ApiError::new("not_found", "Nie ma takiego nazwiska."))?;
    let row = |i: usize| {
        let mut v = people::with_context(d, i);
        v["years"] = json!(years_range(d, i));
        v["parentsText"] = json!(parents_text(d, i));
        v
    };
    let mut born = g.born.clone();
    crate::kin::sort_by_birth(d, &mut born);
    let mut married = g.married.clone();
    crate::kin::sort_by_birth(d, &mut married);
    let years: Vec<i32> = g.born.iter().filter_map(|&i| d.info[i].birth.as_ref().and_then(|b| b.value.map(|v| v.start.year))).collect();
    let oldest = born.first().copied();
    // Similar surname groups that might be the same family ("Kowalewski" → "Kowalscy?").
    let similar: Vec<Value> = d
        .groups
        .iter()
        .filter(|o| o.key != g.key && levenshtein(&o.key, &g.key) <= 2 && o.key.len() >= 4)
        .map(|o| json!({ "key": o.key, "name": o.name, "plural": o.plural, "count": o.born.len() + o.married.len() }))
        .collect();
    let forms: Vec<Value> = g
        .forms
        .iter()
        .map(|(form, count)| {
            let note = if polish::masculine_form(form) != *form { Some("forma żeńska") } else if form.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c)) { Some("zapis ros.") } else { None };
            json!({ "form": form, "count": count, "note": note })
        })
        .collect();
    Ok(json!({
        "key": g.key,
        "name": g.name,
        "plural": g.plural,
        "genitive": polish::family_genitive(&g.name),
        "branch": g.branch,
        "count": g.born.len() + g.married.len(),
        "bornCount": g.born.len(),
        "marriedCount": g.married.len(),
        "from": years.iter().min(),
        "to": years.iter().max(),
        "places": top_places(d, &g.born, 3),
        "oldest": oldest.map(|i| json!({ "id": d.xref(i), "name": d.info[i].name, "year": d.info[i].birth.as_ref().map(|b| b.year.clone()) })),
        "forms": forms,
        "similar": similar,
        "born": born.iter().map(|&i| row(i)).collect::<Vec<_>>(),
        "married": married.iter().map(|&i| row(i)).collect::<Vec<_>>(),
    }))
}

// ---------- Places ----------

struct PlaceEvent {
    person: usize,
    kind: &'static str,
    date: Option<DateInfo>,
    relation: String,
    source: Option<String>,
}

fn place_kind(tag: &str) -> &'static str {
    match tag {
        "BIRT" => "born",
        "CHR" | "BAPM" => "baptised",
        "MARR" => "married",
        "DEAT" => "died",
        "BURI" | "CREM" => "buried",
        _ => "lived",
    }
}

/// Every place written in the archive, as a tree built from "Wólka, Łęczna, lubelskie" (smallest first).
fn place_events(d: &Derived) -> BTreeMap<Vec<String>, Vec<PlaceEvent>> {
    let mut map: BTreeMap<Vec<String>, Vec<PlaceEvent>> = BTreeMap::new();
    let mut add = |place: &str, event: PlaceEvent| {
        let mut parts: Vec<String> = place.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        parts.reverse();
        if !parts.is_empty() {
            map.entry(parts).or_default().push(event);
        }
    };
    for i in 0..d.info.len() {
        for f in &d.view.people[i].facts {
            if let Some(place) = &f.place {
                let relation = match &d.info[i].maiden {
                    Some(m) if matches!(f.tag.as_str(), "BIRT" | "CHR" | "BAPM") => format!("z d. {m}"),
                    _ => people::context(d, i),
                };
                let source = f.citations.first().and_then(|c| c.source.as_ref()).and_then(|s| d.view.sources.get(s)).and_then(|s| s.akt.clone().map(|a| format!("akt {a}")).or_else(|| s.title.clone()));
                add(place, PlaceEvent { person: i, kind: place_kind(&f.tag), date: DateInfo::from_fact(f), relation, source });
            }
        }
    }
    for (family, details) in d.view.model.families.iter().zip(&d.view.families) {
        for f in details.facts.iter().filter(|f| f.tag == "MARR") {
            if let Some(place) = &f.place {
                for p in family.partners.iter().filter_map(|x| d.index(x)) {
                    let other = family.partners.iter().filter_map(|x| d.index(x)).find(|&o| o != p);
                    let relation = other.map(|o| format!("ślub z: {}", d.info[o].name)).unwrap_or_default();
                    add(place, PlaceEvent { person: p, kind: "married", date: DateInfo::from_fact(f), relation, source: None });
                }
            }
        }
    }
    map
}

pub fn places(d: &Derived) -> Value {
    let events = place_events(d);
    // Count people per node, including sub-places.
    let mut nodes: BTreeMap<Vec<String>, HashSet<usize>> = BTreeMap::new();
    for (path, list) in &events {
        for depth in 1..=path.len() {
            let entry = nodes.entry(path[..depth].to_vec()).or_default();
            entry.extend(list.iter().map(|e| e.person));
        }
    }
    // The keys are sorted and every place's parents are keys too, so a place's sub-places come right after it
    // (looking through all keys for each place took most of a second on 10,000 places).
    let keys: Vec<&Vec<String>> = nodes.keys().collect();
    let rows: Vec<Value> = nodes
        .iter()
        .enumerate()
        .map(|(k, (path, people))| {
            let has_children = keys.get(k + 1).is_some_and(|next| next.len() > path.len() && next.starts_with(path));
            json!({ "path": path, "name": path.last(), "level": path.len() - 1, "count": people.len(), "leaf": !has_children })
        })
        .collect();
    let without = (0..d.info.len()).filter(|&i| d.view.people[i].facts.iter().all(|f| f.place.is_none())).count();
    json!({ "places": rows, "without": without, "total": nodes.len() })
}

pub fn place(d: &Derived, path: &[String]) -> Result<Value, ApiError> {
    let events = place_events(d);
    let matching: Vec<&PlaceEvent> = events.iter().filter(|(p, _)| p.starts_with(path)).flat_map(|(_, list)| list.iter()).collect();
    if matching.is_empty() {
        return Err(ApiError::new("not_found", "Nie ma takiego miejsca."));
    }
    let mut groups: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    let mut sorted: Vec<&&PlaceEvent> = matching.iter().collect();
    sorted.sort_by_key(|e| e.date.as_ref().and_then(|dd| dd.sort).unwrap_or(i64::MAX));
    for e in sorted {
        groups.entry(e.kind).or_default().push(json!({
            "date": e.date.as_ref().map(|dd| dd.short.clone()),
            "uncertain": e.date.as_ref().is_some_and(|dd| dd.uncertain),
            "person": person_ref(d, e.person),
            "relation": e.relation,
            "source": e.source,
        }));
    }
    let name = path.last().cloned().unwrap_or_default();
    let folded = fold(&name);
    let similar: Vec<Value> = events
        .keys()
        .filter(|p| p.len() == path.len() && p.last().is_some_and(|l| *l != name && folded.chars().count() >= 4 && levenshtein(&fold(l), &folded) <= 1))
        .map(|p| json!({ "path": p, "name": p.last(), "count": events[p].len() }))
        .collect();
    let children: Vec<String> = events.keys().filter(|p| p.len() == path.len() + 1 && p.starts_with(path)).filter_map(|p| p.last().cloned()).collect::<HashSet<_>>().into_iter().collect();
    Ok(json!({
        "name": name,
        "path": path,
        "groups": groups,
        "similar": similar,
        "children": children,
        "people": matching.iter().map(|e| e.person).collect::<HashSet<_>>().len(),
        "events": matching.len(),
    }))
}

// ---------- Stories ----------

pub fn stories(d: &Derived) -> Value {
    let mut items = Vec::new();
    for (xref, t) in &d.view.texts {
        if !matches!(t.kind, TextKind::Story | TextKind::Saying | TextKind::Trivia) {
            continue;
        }
        let owner = people::owner_of(d, t);
        let body = crate::text::plain(&crate::text::display_markdown(d, &t.body));
        let title = t.title.clone().unwrap_or_else(|| {
            let first: String = body.chars().take(90).collect();
            first
        });
        let excerpt = if t.title.is_some() { body.clone() } else { String::new() };
        let photo = owner.and_then(|o| d.info[o].photo.as_ref().map(|p| p.1.clone()));
        items.push(json!({
            "id": xref,
            "kind": t.kind.code(),
            "title": title,
            "excerpt": excerpt,
            "date": t.date.map(|v| polish::format_date_short(&v)).or_else(|| t.date_text.clone()),
            "sort": t.date.map(|v| v.sort_key()),
            "decade": t.date.map(|v| v.start.year / 10 * 10),
            "person": owner.map(|o| person_ref(d, o)),
            "branch": owner.map(|o| d.info[o].branch),
            "photo": photo,
            "search": fold(&format!("{title} {body}")),
        }));
    }
    items.sort_by(|a, b| a["sort"].as_i64().unwrap_or(i64::MAX).cmp(&b["sort"].as_i64().unwrap_or(i64::MAX)));
    json!({ "items": items })
}

pub fn story(d: &Derived, id: &str) -> Result<Value, ApiError> {
    let t = d.view.texts.get(id).ok_or_else(|| ApiError::new("not_found", "Nie ma takiej historii."))?;
    let owner = people::owner_of(d, t);
    let mut sources = Vec::new();
    for c in &t.citations {
        if let Some(s) = c.source.as_ref().and_then(|s| d.view.sources.get(s)) {
            sources.push(json!({ "id": s.xref, "title": s.title }));
        }
    }
    let photos: Vec<Value> = owner
        .map(|o| {
            d.view.people[o]
                .media
                .iter()
                .filter_map(|m| d.view.media.get(&m.object))
                .filter(|m| m.kind == MediaKind::Photo)
                .take(3)
                .map(|m| json!({ "id": m.id, "path": m.file, "caption": m.title }))
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "id": id,
        "kind": t.kind.code(),
        "title": t.title,
        "body": crate::text::display_markdown(d, &t.body),
        "date": t.date.map(|v| polish::format_date(&v)).or_else(|| t.date_text.clone()),
        "place": t.place,
        "certainty": t.certainty.map(|c| c.code()),
        "inferred": t.inferred,
        "people": people::text_people(d, t),
        "owner": owner.map(|o| d.xref(o).to_string()),
        "sources": sources,
        "photos": photos,
    }))
}

// ---------- Media ----------

pub fn missing_files(d: &Derived, roots: &FileRoots) -> Vec<String> {
    d.view
        .media_order
        .iter()
        .filter(|id| d.view.media.get(*id).and_then(|m| m.file.as_ref()).is_some_and(|f| !roots.resolve(f).is_file()))
        .cloned()
        .collect()
}

fn media_people(d: &Derived) -> HashMap<&str, Vec<usize>> {
    let mut map: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, p) in d.view.people.iter().enumerate() {
        for link in &p.media {
            map.entry(link.object.as_str()).or_default().push(i);
        }
    }
    map
}

fn media_json(d: &Derived, id: &str, roots: &FileRoots, people_of: &HashMap<&str, Vec<usize>>) -> Option<Value> {
    let m = d.view.media.get(id)?;
    let date = m.date_text.as_deref().and_then(heirloom_core::gedcom::date::parse);
    let path = m.file.as_ref().map(|f| roots.resolve(f));
    let meta = path.as_ref().and_then(|p| std::fs::metadata(p).ok());
    let people: Vec<Value> = people_of.get(id).map(|list| list.iter().map(|&i| person_ref(d, i)).collect()).unwrap_or_default();
    let profile_of: Vec<&str> = people_of.get(id).map(|list| list.iter().filter(|&&i| d.info[i].photo.as_ref().is_some_and(|p| p.0 == id)).map(|&i| d.xref(i)).collect()).unwrap_or_default();
    Some(json!({
        "id": id,
        "path": m.file,
        "kind": m.kind.code(),
        "documentType": m.document_type,
        "title": m.title,
        "date": date.map(|v| polish::format_date_short(&v)).or_else(|| m.date_text.clone()),
        "dateLong": date.map(|v| polish::format_date(&v)).or_else(|| m.date_text.clone()),
        "uncertain": date.is_some_and(|v| polish::is_uncertain(&v)),
        "sort": date.map(|v| v.sort_key()),
        "decade": date.map(|v| v.start.year / 10 * 10),
        "place": m.place,
        "transcription": m.transcription,
        "translation": m.translation,
        "note": m.note,
        "missing": meta.is_none() && m.file.is_some(),
        "size": meta.map(|x| x.len()),
        "absolute": path.map(|p| p.display().to_string()),
        "people": people,
        "profileOf": profile_of,
        "format": m.format,
        "sources": m.citations.iter().filter_map(|c| c.source.as_ref()).filter_map(|s| d.view.sources.get(s)).map(|s| json!({ "id": s.xref, "title": s.title })).collect::<Vec<_>>(),
    }))
}

pub fn media_list(d: &Derived, roots: &FileRoots) -> Value {
    let people_of = media_people(d);
    let items: Vec<Value> = d.view.media_order.iter().filter_map(|id| media_json(d, id, roots, &people_of)).collect();
    json!({ "items": items })
}

pub fn media_get(d: &Derived, id: &str, roots: &FileRoots) -> Result<Value, ApiError> {
    let people_of = media_people(d);
    media_json(d, id, roots, &people_of).ok_or_else(|| ApiError::new("not_found", "Nie ma takiego pliku w archiwum."))
}

/// "Brakujące pliki": looks for missing files by name in a folder (and its subfolders).
pub fn find_missing(d: &Derived, roots: &FileRoots, folder: &Path) -> Value {
    let missing = missing_files(d, roots);
    let wanted: HashMap<String, Vec<&String>> = missing.iter().fold(HashMap::new(), |mut acc, id| {
        if let Some(name) = d.view.media.get(id).and_then(|m| m.file.as_ref()).and_then(|f| f.rsplit(['/', '\\']).next()) {
            acc.entry(name.to_lowercase()).or_default().push(id);
        }
        acc
    });
    let mut found: HashMap<String, Vec<String>> = HashMap::new();
    let mut stack = vec![folder.to_path_buf()];
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 20_000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_lowercase()) {
                if wanted.contains_key(&name) {
                    found.entry(name).or_default().push(path.display().to_string());
                }
            }
        }
    }
    let rows: Vec<Value> = missing
        .iter()
        .filter_map(|id| {
            let m = d.view.media.get(id)?;
            let name = m.file.as_ref()?.rsplit(['/', '\\']).next()?.to_string();
            let hits = found.get(&name.to_lowercase()).cloned().unwrap_or_default();
            Some(json!({ "id": id, "file": name, "title": m.title, "found": hits }))
        })
        .collect();
    json!({ "rows": rows, "folder": folder.display().to_string() })
}

pub fn missing_list(d: &Derived, roots: &FileRoots) -> Value {
    let rows: Vec<Value> = missing_files(d, roots)
        .iter()
        .filter_map(|id| {
            let m = d.view.media.get(id)?;
            Some(json!({ "id": id, "file": m.file.as_ref()?.rsplit(['/', '\\']).next(), "path": m.file, "title": m.title, "found": [] }))
        })
        .collect();
    json!({ "rows": rows })
}

// ---------- Sources ----------

fn source_icon(kind: Option<&str>) -> &'static str {
    match kind {
        Some("index") | Some("website") => "search",
        Some("oral") | Some("letter") => "message-square-quote",
        Some("book") => "book",
        Some("photo") => "image",
        _ => "file-text",
    }
}

fn source_group(kind: Option<&str>) -> &'static str {
    match kind {
        Some("parish_record") | Some("civil_record") => "records",
        Some("index") => "indexes",
        Some("oral") | Some("letter") | Some("note") => "accounts",
        Some("website") => "web",
        _ => "other",
    }
}

/// Where each source is cited: (person, fact label, value, page, certainty).
fn citations_of(d: &Derived) -> HashMap<String, Vec<(usize, String, String, Option<String>, Option<String>)>> {
    let mut map: HashMap<String, Vec<_>> = HashMap::new();
    for i in 0..d.info.len() {
        for f in &d.view.people[i].facts {
            for c in &f.citations {
                let Some(s) = &c.source else { continue };
                let value = [DateInfo::from_fact(f).map(|x| x.short), f.place.clone(), f.value.clone()].into_iter().flatten().collect::<Vec<_>>().join(", ");
                let certainty = f.certainty.map(|x| x.code().to_string()).or_else(|| f.inferred.then(|| "inferred".to_string()));
                map.entry(s.clone()).or_default().push((i, people::event_name(&f.tag, f.kind.as_deref()), value, c.page.clone(), certainty));
            }
        }
        for c in &d.view.people[i].citations {
            if let Some(s) = &c.source {
                map.entry(s.clone()).or_default().push((i, "Osoba".into(), d.info[i].name.clone(), c.page.clone(), None));
            }
        }
    }
    for text in d.view.texts.values() {
        for c in &text.citations {
            if let (Some(s), Some(o)) = (&c.source, people::owner_of(d, text)) {
                let label = crate::activity::text_kind_word(text.kind).to_string();
                map.entry(s.clone()).or_default().push((o, label, text.title.clone().unwrap_or_default(), c.page.clone(), text.certainty.map(|x| x.code().to_string())));
            }
        }
    }
    map
}

pub fn sources(d: &Derived) -> Value {
    let cites = citations_of(d);
    let items: Vec<Value> = d
        .view
        .source_order
        .iter()
        .filter_map(|xref| {
            let s = d.view.sources.get(xref)?;
            let count = cites.get(xref).map_or(0, Vec::len);
            let mut meta = Vec::new();
            if let Some(p) = &s.parish {
                meta.push(format!("par. {p}"));
            }
            if let Some(r) = &s.repository {
                meta.push(r.clone());
            }
            if !s.media.is_empty() {
                meta.push("skan".into());
            }
            Some(json!({
                "id": xref,
                "title": s.title.clone().unwrap_or_else(|| xref.clone()),
                "kind": s.kind,
                "icon": source_icon(s.kind.as_deref()),
                "group": source_group(s.kind.as_deref()),
                "meta": meta.join(" · "),
                "uses": count,
            }))
        })
        .collect();
    json!({ "items": items })
}

pub fn source(d: &Derived, id: &str) -> Result<Value, ApiError> {
    let s = d.view.sources.get(id).ok_or_else(|| ApiError::new("not_found", "Nie ma takiego źródła."))?;
    let cites = citations_of(d);
    let rows: Vec<Value> = cites
        .get(id)
        .map(|list| {
            list.iter()
                .map(|(i, fact, value, page, certainty)| json!({ "fact": fact, "person": person_ref(d, *i), "value": value, "page": page, "certainty": certainty }))
                .collect()
        })
        .unwrap_or_default();
    let scans: Vec<Value> = s.media.iter().filter_map(|m| d.view.media.get(m)).map(|m| json!({ "id": m.id, "path": m.file, "title": m.title, "transcription": m.transcription, "translation": m.translation, "kind": m.kind.code() })).collect();
    let people_count = cites.get(id).map_or(0, |l| l.iter().map(|x| x.0).collect::<HashSet<_>>().len());
    Ok(json!({
        "id": id,
        "title": s.title,
        "kind": s.kind,
        "author": s.author,
        "publication": s.publication,
        "parish": s.parish,
        "year": s.year,
        "akt": s.akt,
        "url": s.url,
        "domain": s.url.as_deref().map(domain),
        "repository": s.repository,
        "callNumber": s.call_number,
        "text": s.text,
        "note": s.note,
        "scans": scans,
        "facts": rows,
        "peopleCount": people_count,
    }))
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance() {
        assert_eq!(levenshtein("wulka", "wolka"), 1);
        assert_eq!(levenshtein("kowalski", "kowalewski"), 2);
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn years_ago_are_counted_in_polish() {
        const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
        let (month, day, year) = month_day_now();
        let text = format!("0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 DEAT\n2 DATE {day} {} {}\n0 TRLR\n", MONTHS[month as usize - 1], year - 82);
        let d = crate::derive::build(&heirloom_core::gedcom::Document::from_bytes(text.as_bytes()).0);
        assert_eq!(start(&d, &[], "T")["onThisDay"]["deaths"][0]["value"], "82 lata temu");
        // A birth year typed wrong, in the future: no 20-digit age.
        let text = format!("0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 BIRT\n2 DATE {day} {} {}\n0 TRLR\n", MONTHS[month as usize - 1], year + 73);
        let d = crate::derive::build(&heirloom_core::gedcom::Document::from_bytes(text.as_bytes()).0);
        assert_eq!(start(&d, &[], "T")["onThisDay"]["birthdays"][0]["value"], "0 lat");
    }

    #[test]
    fn places_know_which_have_sub_places() {
        let text = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 BIRT\n2 PLAC Wólka, Łęczna, lubelskie\n\
0 @I2@ INDI\n1 BIRT\n2 PLAC Łęczna, lubelskie\n0 @I3@ INDI\n1 DEAT\n2 PLAC Łęczna A, lubelskie\n0 @I4@ INDI\n1 BIRT\n2 PLAC Lublin\n0 TRLR\n";
        let d = crate::derive::build(&heirloom_core::gedcom::Document::from_bytes(text.as_bytes()).0);
        let rows = places(&d)["places"].as_array().unwrap().clone();
        let leaf = |name: &str| rows.iter().find(|r| r["name"] == name).unwrap()["leaf"].as_bool().unwrap();
        assert_eq!((leaf("lubelskie"), leaf("Łęczna"), leaf("Wólka"), leaf("Łęczna A"), leaf("Lublin")), (false, false, true, true, true));
    }

    #[test]
    fn lists_of_a_generated_family() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("a");
        let options = heirloom_gen::Options { people: 200, media: 3, seed: 3, name: "T".into() };
        let archive = heirloom_gen::write_archive(&root, &options).unwrap();
        let d = crate::derive::build(&archive.doc);
        let s = surnames(&d);
        assert!(!s["groups"].as_array().unwrap().is_empty());
        let key = s["groups"][0]["key"].as_str().unwrap();
        assert!(surname(&d, key).unwrap()["born"].as_array().unwrap().len() > 0);
        let p = places(&d);
        let first = p["places"][0]["path"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert!(place(&d, &first).unwrap()["events"].as_u64().unwrap() > 0);
        let m = media_list(&d, &FileRoots::One(root.clone()));
        assert_eq!(m["items"].as_array().unwrap().len(), 3);
        assert!(missing_files(&d, &FileRoots::One(root.clone())).is_empty());
        let src = sources(&d);
        let first_source = src["items"][0]["id"].as_str().unwrap();
        assert!(source(&d, first_source).unwrap()["facts"].as_array().unwrap().len() > 0);
        let open = first_open(&d, &root);
        assert_eq!(open["people"], 200);
        assert!(!open["suggestions"].as_array().unwrap().is_empty());
        let _ = start(&d, &[], "T");
    }
}
