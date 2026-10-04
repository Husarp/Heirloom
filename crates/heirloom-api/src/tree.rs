//! Data for the tree screens. The focus views (Rodzina, Przodkowie, Potomkowie) get a neighbourhood of the
//! chosen person as people + unions, and the UI arranges it; the whole-family view ("Całe drzewo") is laid out
//! here, because it can hold thousands of people (spec §3, §4.1–§4.4).

use crate::derive::Derived;
use crate::kin;
use crate::people::not_found;
use crate::ApiError;
use heirloom_core::gedcom::model::Sex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet, VecDeque};

/// Everyone who descends from `i`, not counting `i`.
fn descendant_count(d: &Derived, i: usize) -> usize {
    let mut seen = HashSet::new();
    let mut queue: VecDeque<usize> = d.info[i].children.iter().copied().collect();
    while let Some(c) = queue.pop_front() {
        if seen.insert(c) {
            queue.extend(d.info[c].children.iter().copied());
        }
    }
    seen.len()
}

fn ancestor_count(d: &Derived, i: usize) -> usize {
    let mut seen = HashSet::new();
    let mut queue: VecDeque<usize> = d.info[i].parents.iter().copied().collect();
    while let Some(p) = queue.pop_front() {
        if seen.insert(p) {
            queue.extend(d.info[p].parents.iter().copied());
        }
    }
    seen.len()
}

/// The unions (families) between included people.
fn union_json(d: &Derived, fam: &str, included: &HashSet<usize>) -> Option<Value> {
    let (family, details) = d.view.family(fam)?;
    let partners: Vec<&str> = family.partners.iter().filter(|p| d.index(p).is_some_and(|i| included.contains(&i))).map(String::as_str).collect();
    let children: Vec<Value> = family
        .children
        .iter()
        .filter_map(|c| {
            let i = d.index(c)?;
            if !included.contains(&i) {
                return None;
            }
            let pedi = d.view.people[i].pedigree.iter().find(|(f, _)| f == fam).map(|(_, p)| p.clone());
            Some(json!({ "id": c, "pedi": pedi }))
        })
        .collect();
    if partners.is_empty() && children.is_empty() {
        return None;
    }
    let marriage = details.facts.iter().find(|f| f.tag == "MARR");
    Some(json!({
        "id": fam,
        "partners": partners,
        "children": children,
        "married": details.married,
        "year": marriage.and_then(|f| f.date.map(|dv| heirloom_core::polish::year_text(&dv))),
        "uncertain": details.certainty.is_some_and(|c| c != heirloom_core::gedcom::view::Certainty::High),
        "allChildren": family.children.len(),
    }))
}

/// People around `id`: ancestors `up` generations, descendants `down` generations (more under `expand`), the
/// siblings, and every partner of the people shown with their parents' count.
pub fn graph(d: &Derived, id: &str, up: usize, down: usize, expand: &[String]) -> Result<Value, ApiError> {
    let focus = d.index(id).ok_or_else(|| not_found(id))?;
    let mut included: HashSet<usize> = HashSet::from([focus]);
    // Ancestors, by generation.
    let mut frontier = vec![focus];
    for _ in 0..up {
        let mut next = Vec::new();
        for &p in &frontier {
            for &parent in &d.info[p].parents {
                if included.insert(parent) {
                    next.push(parent);
                }
            }
        }
        frontier = next;
    }
    // Descendants with their partners.
    // Each person carries how many more generations below them to show; an expanded person ("+12") gets the
    // full depth again.
    let expanded: HashSet<usize> = expand.iter().filter_map(|x| d.index(x)).collect();
    let mut queue: VecDeque<(usize, usize)> = VecDeque::from([(focus, down)]);
    let mut best: HashMap<usize, usize> = HashMap::new();
    while let Some((p, budget)) = queue.pop_front() {
        let budget = if expanded.contains(&p) { budget.max(down.max(1)) } else { budget };
        if best.get(&p).is_some_and(|&b| b >= budget) {
            continue;
        }
        best.insert(p, budget);
        for &partner in &d.info[p].partners {
            included.insert(partner);
        }
        if budget > 0 {
            for &c in &d.info[p].children {
                included.insert(c);
                queue.push_back((c, budget - 1));
            }
        }
    }
    // Siblings and the partners' parents (Rodzina).
    for s in kin::siblings(d, focus) {
        included.insert(s);
    }
    let partners: Vec<usize> = d.info[focus].partners.clone();
    for partner in &partners {
        for &pp in &d.info[*partner].parents {
            included.insert(pp);
        }
    }
    let mut people = serde_json::Map::new();
    let mut families: Vec<String> = Vec::new();
    for &i in &included {
        let mut v = d.summary(i);
        v["parents"] = json!(d.info[i].parents.iter().map(|&p| d.xref(p)).collect::<Vec<_>>());
        v["children"] = json!(d.info[i].children.iter().map(|&c| d.xref(c)).collect::<Vec<_>>());
        v["partners"] = json!(d.info[i].partners.iter().map(|&c| d.xref(c)).collect::<Vec<_>>());
        v["descendants"] = json!(descendant_count(d, i));
        v["ancestors"] = json!(ancestor_count(d, i));
        v["hasParents"] = json!(!d.info[i].parents.is_empty());
        let p = &d.view.model.persons[i];
        for fam in p.fams.iter().chain(&p.famc) {
            if !families.contains(fam) {
                families.push(fam.clone());
            }
        }
        people.insert(d.xref(i).to_string(), v);
    }
    let unions: Vec<Value> = families.iter().filter_map(|f| union_json(d, f, &included)).collect();
    Ok(json!({ "focus": id, "up": up, "down": down, "people": people, "unions": unions }))
}

/// [father, mother] as indices, either missing: the first man and the first woman among the parents, a parent of
/// unknown sex filling an empty place (the order `parentsOrdered` in the UI uses).
fn father_and_mother(d: &Derived, i: usize) -> [Option<usize>; 2] {
    let parents = &d.info[i].parents;
    let sex = |p: usize| d.view.model.persons[p].sex;
    let father = parents.iter().copied().find(|&p| sex(p) == Sex::Male);
    let mother = parents.iter().copied().find(|&p| sex(p) == Sex::Female);
    let mut rest = parents.iter().copied().filter(|&p| Some(p) != father && Some(p) != mother);
    let father = father.or_else(|| rest.next());
    [father, mother.or_else(|| rest.next())]
}

/// "Całe drzewo": everyone in generation bands, grouped into surname clusters (spec §3.11, §4.4). Positions are
/// world coordinates of card cells (204 × 72 cards on a 224 × 92 grid); the UI draws dots, blocks or cards
/// depending on the zoom.
pub fn overview(d: &Derived, focus: Option<&str>) -> Value {
    const CELL_W: f64 = 224.0;
    const CELL_H: f64 = 92.0;
    const BAND_PAD: f64 = 60.0;
    const CLUSTER_GAP: f64 = 160.0;
    let n = d.info.len();
    let bands = d.max_generation.max(1) as usize;
    let rows = ((1.52 * n as f64).sqrt() / bands as f64).ceil().clamp(1.0, 16.0) as usize;

    // Clusters: the surname group of the birth surname; people without one form their own "?" cluster.
    let cluster_of = |i: usize| d.info[i].group.map_or(usize::MAX, |g| g);
    let mut members: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        members.entry(cluster_of(i)).or_default().push(i);
    }
    let mut clusters: Vec<(usize, Vec<usize>)> = members.into_iter().collect();
    // Older families first (left), then bigger ones.
    clusters.sort_by_key(|(key, people)| {
        let first = people.iter().filter_map(|&i| d.info[i].generation).min().unwrap_or(u32::MAX);
        (first, std::cmp::Reverse(people.len()), *key)
    });

    let band_height = rows as f64 * CELL_H + BAND_PAD;
    let mut positions: Vec<(f64, f64)> = vec![(0.0, 0.0); n];
    let mut cluster_json = Vec::new();
    let mut x0 = 0.0;
    for (key, people) in &clusters {
        // Columns: enough for the busiest generation of this family.
        let mut by_gen: HashMap<usize, Vec<usize>> = HashMap::new();
        for &i in people {
            by_gen.entry(d.info[i].generation.unwrap_or(1) as usize).or_default().push(i);
        }
        let columns = by_gen.values().map(|v| v.len().div_ceil(rows)).max().unwrap_or(1).max(1);
        for (g, list) in &mut by_gen {
            list.sort_by_key(|&i| (d.info[i].birth.as_ref().and_then(|b| b.sort).unwrap_or(i64::MAX), i));
            let band_top = (*g as f64 - 1.0) * band_height + BAND_PAD / 2.0;
            let used_columns = list.len().div_ceil(rows);
            let offset = (columns - used_columns) as f64 * CELL_W / 2.0;
            for (k, &i) in list.iter().enumerate() {
                let (col, row) = (k / rows, k % rows);
                positions[i] = (x0 + offset + col as f64 * CELL_W, band_top + row as f64 * CELL_H);
            }
        }
        let gens: Vec<u32> = people.iter().filter_map(|&i| d.info[i].generation).collect();
        let (label, branch) = if *key == usize::MAX { ("Bez nazwiska".to_string(), 12) } else { (d.groups[*key].plural.clone(), d.groups[*key].branch) };
        cluster_json.push(json!({
            "label": label,
            "branch": branch,
            "count": people.len(),
            "fromGen": gens.iter().min(),
            "toGen": gens.iter().max(),
            "x": x0,
            "width": columns as f64 * CELL_W,
        }));
        x0 += columns as f64 * CELL_W + CLUSTER_GAP;
    }

    // Approximate years for the band labels ("ok. 1690").
    let mut band_years = Vec::new();
    for g in 1..=bands as u32 {
        let mut years: Vec<i32> = (0..n)
            .filter(|&i| d.info[i].generation == Some(g))
            .filter_map(|i| d.info[i].birth.as_ref().and_then(|b| b.value.map(|v| v.start.year)))
            .collect();
        years.sort_unstable();
        band_years.push(years.get(years.len() / 2).copied());
    }

    // The viewed person's direct line (their ancestors, one path per generation up).
    let mut line = Vec::new();
    if let Some(f) = focus.and_then(|x| d.index(x)) {
        let mut current = Some(f);
        while let Some(i) = current {
            line.push(d.xref(i).to_string());
            let parents = &d.info[i].parents;
            current = parents.iter().copied().find(|&p| d.view.model.persons[p].sex == Sex::Male).or(parents.first().copied());
            if line.len() > 200 {
                break;
            }
        }
    }

    // Flat arrays keep 10 000 people small: [id, x, y, branch, given, surname, birth, death, birth uncertain, death
    // uncertain, living, generation, surname branch, [father, mother] as indices] per person. The parents serve
    // „Koloruj wg: strona” until the overview has real family links.
    let people: Vec<Value> = (0..n)
        .map(|i| {
            let info = &d.info[i];
            let mut row = json!([
                d.xref(i),
                positions[i].0,
                positions[i].1,
                info.branch,
                info.given,
                info.surname,
                info.birth.as_ref().map(|b| b.year.clone()),
                info.death.as_ref().map(|b| b.year.clone()),
                info.birth.as_ref().is_some_and(|b| b.uncertain),
                info.death.as_ref().is_some_and(|b| b.uncertain),
                info.living,
                info.generation,
                info.surname_branch,
                father_and_mother(d, i),
            ]);
            // Archives opened together: the archives the person comes from, last, so the indexes above stay.
            if let (Some(from), Some(list)) = (d.from(i), row.as_array_mut()) {
                list.push(json!(from));
            }
            row
        })
        .collect();
    json!({
        "people": people,
        "clusters": cluster_json,
        "bands": bands,
        "bandHeight": band_height,
        "bandYears": band_years,
        "width": x0,
        "line": line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use heirloom_core::gedcom::Document;

    #[test]
    fn graph_and_overview_of_a_generated_family() {
        let options = heirloom_gen::Options { people: 300, media: 0, seed: 5, name: "T".into() };
        let (doc, _) = heirloom_gen::generate(&options);
        let bytes = doc.to_bytes();
        let d = crate::derive::build(&Document::from_bytes(&bytes).0);
        let someone = (0..d.info.len()).find(|&i| !d.info[i].parents.is_empty() && !d.info[i].children.is_empty()).unwrap();
        let g = graph(&d, d.xref(someone), 2, 2, &[]).unwrap();
        let people = g["people"].as_object().unwrap();
        assert!(people.contains_key(d.xref(someone)));
        for &p in &d.info[someone].parents {
            assert!(people.contains_key(d.xref(p)), "parents are included");
        }
        for &c in &d.info[someone].children {
            assert!(people.contains_key(d.xref(c)), "children are included");
        }
        let overview = overview(&d, Some(d.xref(someone)));
        assert_eq!(overview["people"].as_array().unwrap().len(), d.info.len());
        assert!(overview["line"].as_array().unwrap().len() >= 2);
        let row = &overview["people"][someone];
        assert_eq!(row[12], d.info[someone].surname_branch);
        let parents: Vec<usize> = row[13].as_array().unwrap().iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect();
        assert_eq!(parents.len(), d.info[someone].parents.len().min(2));
        assert!(parents.iter().all(|p| d.info[someone].parents.contains(p)));
        if let Some(father) = row[13][0].as_u64() {
            assert_ne!(d.view.model.persons[father as usize].sex, Sex::Female, "the father comes first");
        }
        // No two people share a cell.
        let mut cells = HashSet::new();
        for p in overview["people"].as_array().unwrap() {
            let key = (p[1].as_f64().unwrap() as i64, p[2].as_f64().unwrap() as i64);
            assert!(cells.insert(key), "overlap at {key:?}");
        }
    }
}
