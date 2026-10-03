//! The Import: the built-in converter from the researcher's AI answers and files into the archive (PLAN §4.6,
//! §11.2 rule 8). Five steps, nothing written before the last one: Wczytaj → Sprawdź → Dopasuj osoby → Zdjęcia i
//! pliki → Podsumowanie. The batch being reviewed is kept here between the steps.

pub mod check;
mod commit;
pub mod format;
pub mod matching;

use crate::derive::{DateInfo, Derived, initials};
use crate::{ApiError, ApiResult, Session};
use check::{InputFile, Issue, m_label, m_number};
use format::Part;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// The AI instructions for the researcher, always matching this app's format version.
pub const AI_INSTRUCTIONS: &str = include_str!("../../../../docs/AI_INSTRUCTIONS.md");
pub const FORMAT_VERSION: &str = "1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Undecided,
    New,
    Merge,
    Skip,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub kind: Kind,
    pub target: Option<String>,
    /// Per-field choices for a merge: `keep`, `replace`, `variant`, `add`, `skip`.
    pub fields: HashMap<String, String>,
    /// Unticked in the summary: not saved at all.
    pub excluded: bool,
    pub excluded_fields: HashSet<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FileChoice {
    pub kind: Option<String>,
    pub caption: Option<String>,
    /// People added by hand (record ids of batch persons: P1…).
    pub people: Vec<String>,
    pub skip: bool,
}

pub struct Draft {
    pub batch: check::Batch,
    pub issues: Vec<Issue>,
    pub files: Vec<InputFile>,
    pub raw: Vec<String>,
    pub candidates: HashMap<String, Vec<matching::Candidate>>,
    pub decisions: HashMap<String, Decision>,
    pub answers: HashMap<String, (String, String)>,
    pub file_choices: HashMap<String, FileChoice>,
    /// The profile photo per person (M label), starting with the AI's suggestion.
    pub profile: HashMap<String, Option<String>>,
    /// M label → the existing media record with the same contents.
    pub duplicates: HashMap<String, String>,
    /// Changes with every load, so file previews from an earlier batch aren't reused.
    pub load_id: u64,
    /// Choices made by hand that a reload (adding the missing part or files) must keep: files attached to an
    /// M-number whatever their name, and dates fixed in step 2 (event id → date in the format).
    pub attached: Vec<(u32, PathBuf)>,
    pub date_fixes: HashMap<String, String>,
    /// Step 1's list (design 17d): each AI answer with what it held, the copies left out, and the files taken off it.
    pub answer_inputs: Vec<Value>,
    /// What was given as text, and the answer files: adding the missing part reads them again as they came.
    pub pasted: Vec<String>,
    pub answer_paths: Vec<PathBuf>,
    pub skipped: Vec<check::SkippedInput>,
    /// Dropped paths that are gone: shown in step 1 so they can be taken off the list.
    pub missing: Vec<PathBuf>,
    pub exclude: Vec<PathBuf>,
}

/// A file picked for M-number `n` (it replaces any other file with that number).
fn attach(d: &mut Draft, n: u32, path: PathBuf) -> Result<(), ApiError> {
    let size = std::fs::metadata(&path).map_err(|_| ApiError::new("not_found", "Nie ma takiego pliku."))?.len();
    let name = path.file_name().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
    let label = m_label(n);
    d.files.retain(|f| f.m != Some(n) && f.path != path);
    d.files.push(InputFile { path: path.clone(), name, m: Some(n), size });
    d.issues.retain(|i| !(i.target.as_deref() == Some(label.as_str()) && i.message.contains("nie został dołączony")));
    d.attached.retain(|(m, _)| *m != n);
    d.attached.push((n, path));
    Ok(())
}

fn fix_date(d: &mut Draft, event: &str, value: &str) -> Result<(), ApiError> {
    let e = d.batch.merged.events.iter_mut().find(|e| e.id == event).ok_or_else(|| ApiError::bad_args("event"))?;
    e.date = Some(value.to_string());
    d.issues.retain(|i| !(i.action == Some("fix") && i.target.as_deref() == Some(event)));
    d.date_fixes.insert(event.to_string(), value.to_string());
    Ok(())
}

fn req(args: &Value, key: &str) -> Result<String, ApiError> {
    args.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| ApiError::bad_args(key))
}

fn no_draft() -> ApiError {
    ApiError::new("no_import", "Nie ma rozpoczętego importu.")
}

pub fn call(s: &mut Session, draft: &mut Option<Draft>, method: &str, args: &Value) -> ApiResult {
    match method {
        "import.instructions" => Ok(json!({ "text": AI_INSTRUCTIONS, "version": FORMAT_VERSION })),
        "import.load" => {
            let texts: Vec<String> = args.get("texts").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
            let paths: Vec<PathBuf> = args.get("paths").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect()).unwrap_or_default();
            // Files taken off the list in step 1 (`×`), even when they came in a dropped folder.
            let mut exclude: Vec<PathBuf> = args.get("exclude").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(PathBuf::from).collect()).unwrap_or_default();
            // Adding to a batch already loaded (the missing part, more files) keeps the decisions made so far.
            let (mut all_texts, mut all_paths) = match (draft.as_ref(), args.get("add").and_then(Value::as_bool).unwrap_or(false)) {
                // Attached files come back below, under their M-number.
                (Some(old), true) => (
                    old.pasted.clone(),
                    old.answer_paths.iter().cloned().chain(old.files.iter().filter(|f| !old.attached.iter().any(|(_, p)| *p == f.path)).map(|f| f.path.clone())).collect(),
                ),
                _ => (Vec::new(), Vec::new()),
            };
            all_texts.extend(texts);
            all_paths.extend(paths);
            if args.get("add").and_then(Value::as_bool).unwrap_or(false) {
                if let Some(old) = draft.as_ref() {
                    exclude.extend(old.exclude.iter().cloned());
                }
            }
            let previous = draft.take();
            let mut new = load(s, &all_texts, &all_paths, &exclude);
            if let Some(old) = previous.filter(|_| args.get("add").and_then(Value::as_bool).unwrap_or(false)) {
                new.answers = old.answers;
                new.file_choices = old.file_choices;
                new.profile.extend(old.profile);
                for (id, decision) in old.decisions {
                    if new.decisions.contains_key(&id) {
                        new.decisions.insert(id, decision);
                    }
                }
                for (n, path) in old.attached {
                    let _ = attach(&mut new, n, path);
                }
                for (event, value) in old.date_fixes {
                    let _ = fix_date(&mut new, &event, &value);
                }
            }
            // Kinds chosen by hand in step 1 (by path): a reload (a file taken off, another dropped) keeps them.
            if let Some(kinds) = args.get("kinds").and_then(Value::as_object) {
                for f in &new.files {
                    if let Some(kind) = kinds.get(&f.path.display().to_string()).and_then(Value::as_str) {
                        let key = f.m.map(m_label).unwrap_or_else(|| f.name.clone());
                        new.file_choices.entry(key).or_default().kind = Some(kind.to_string());
                    }
                }
            }
            *draft = Some(new);
            state(s, draft.as_ref().expect("just set"))
        }
        "import.state" => match draft.as_ref() {
            Some(d) => state(s, d),
            None => Ok(Value::Null),
        },
        "import.cancel" => {
            *draft = None;
            Ok(Value::Null)
        }
        "import.history" => Ok(history(s)),
        _ => {
            let d = draft.as_mut().ok_or_else(no_draft)?;
            match method {
                "import.fileDetail" => {
                    // A described file's texts, read on demand: a transcription can be long, and the state is sent
                    // again after every click.
                    let label = req(args, "file")?;
                    let m = d.batch.merged.media.iter().find(|m| m_number(&m.id).map(m_label).as_deref() == Some(label.as_str())).ok_or_else(|| ApiError::bad_args("file"))?;
                    return Ok(json!({
                        "file": label,
                        "caption": m.caption,
                        "documentType": m.document_type,
                        "date": date_info(m.date.as_deref()).map(|x| x.text),
                        "place": m.place,
                        "transcription": m.transcription,
                        "translation": m.translation,
                        "note": m.note,
                    }));
                }
                "import.answer" => {
                    let id = req(args, "question")?;
                    let answer = args.get("answer").and_then(Value::as_str).unwrap_or("").to_string();
                    let note = args.get("note").and_then(Value::as_str).unwrap_or("").to_string();
                    d.answers.insert(id, (answer, note));
                }
                "import.decide" => {
                    let id = req(args, "person")?;
                    let kind = match req(args, "kind")?.as_str() {
                        "new" => Kind::New,
                        "merge" => Kind::Merge,
                        "skip" => Kind::Skip,
                        _ => Kind::Undecided,
                    };
                    let target = args.get("target").and_then(Value::as_str).map(str::to_string);
                    let current = d.decisions.get(&id).ok_or_else(|| ApiError::bad_args("person"))?;
                    if kind == Kind::Merge {
                        let fallback = d.candidates.get(&id).and_then(|c| c.first()).map(|c| c.xref.clone());
                        let target = target.or(current.target.clone()).or(fallback).ok_or_else(|| ApiError::new("bad_args", "Wybierz osobę do połączenia."))?;
                        if let Some(why) = (!current.excluded).then(|| same_target_conflict(d, &id, &target)).flatten() {
                            return Err(ApiError::new("same_target", why));
                        }
                        d.decisions.get_mut(&id).expect("checked").target = Some(target);
                    }
                    d.decisions.get_mut(&id).expect("checked").kind = kind;
                }
                "import.field" => {
                    let id = req(args, "person")?;
                    let field = req(args, "field")?;
                    let choice = req(args, "choice")?;
                    d.decisions.get_mut(&id).ok_or_else(|| ApiError::bad_args("person"))?.fields.insert(field, choice);
                }
                "import.include" => {
                    let id = req(args, "person")?;
                    let include = args.get("include").and_then(Value::as_bool).unwrap_or(true);
                    let current = d.decisions.get(&id).ok_or_else(|| ApiError::bad_args("person"))?;
                    // Ticked again in the summary: the same rule as joining in step 3.
                    if include && args.get("field").is_none() && current.kind == Kind::Merge {
                        if let Some(why) = current.target.as_deref().and_then(|t| same_target_conflict(d, &id, t)) {
                            return Err(ApiError::new("same_target", why));
                        }
                    }
                    let decision = d.decisions.get_mut(&id).expect("checked");
                    match args.get("field").and_then(Value::as_str) {
                        Some(field) if include => {
                            decision.excluded_fields.remove(field);
                        }
                        Some(field) => {
                            decision.excluded_fields.insert(field.to_string());
                        }
                        None => decision.excluded = !include,
                    }
                }
                "import.file" => {
                    let m = req(args, "file")?;
                    let choice = d.file_choices.entry(m).or_default();
                    if let Some(kind) = args.get("kind").and_then(Value::as_str) {
                        choice.kind = Some(kind.to_string());
                    }
                    if let Some(caption) = args.get("caption").and_then(Value::as_str) {
                        choice.caption = Some(caption.to_string());
                    }
                    if let Some(people) = args.get("people").and_then(Value::as_array) {
                        choice.people = people.iter().filter_map(Value::as_str).map(str::to_string).collect();
                    }
                    if let Some(skip) = args.get("skip").and_then(Value::as_bool) {
                        choice.skip = skip;
                    }
                }
                "import.attach" => {
                    // A file the batch describes (M004) but that wasn't delivered, picked by hand whatever its name.
                    let label = req(args, "file")?;
                    let n = m_number(&label).ok_or_else(|| ApiError::bad_args("file"))?;
                    attach(d, n, PathBuf::from(req(args, "path")?))?;
                }
                "import.fixDate" => {
                    // A date the AI wrote as free text, set by hand in step 2.
                    let id = req(args, "event")?;
                    let date = req(args, "date")?;
                    // Typed in Polish like any date in the app ("ok. 1915", "12.03.1878"), or in the format itself.
                    let value = format::gedcom_date(&date)
                        .map(|_| date.trim().to_string())
                        .or_else(|| heirloom_core::polish::parse_date_input(&date).and_then(|g| format::from_gedcom(&g)))
                        .ok_or_else(|| ApiError::new("bad_date", format!("Nie rozumiem daty „{}”. Wpisz np. 12.03.1878, ok. 1850 albo między 1850 a 1855.", date.trim())))?;
                    fix_date(d, &id, &value)?;
                }
                "import.profile" => {
                    let person = req(args, "person")?;
                    let file = args.get("file").and_then(Value::as_str).map(str::to_string);
                    d.profile.insert(person, file);
                }
                "import.commit" => {
                    let author = req(args, "author")?;
                    let note = args.get("note").and_then(Value::as_str).map(str::to_string);
                    // The first save into another program's file is confirmed, as for archive.save.
                    let allow_foreign = args.get("allowForeign").and_then(Value::as_bool).unwrap_or(false);
                    let result = commit::commit(s, d, &author, note.as_deref(), allow_foreign)?;
                    *draft = None;
                    return Ok(result);
                }
                _ => return Err(ApiError::new("unknown_method", format!("Nieznane polecenie: {method}"))),
            }
            state(s, draft.as_ref().expect("still there"))
        }
    }
}

fn load(s: &mut Session, texts: &[String], paths: &[PathBuf], exclude: &[PathBuf]) -> Draft {
    let inputs = check::read_inputs_with(texts, paths, exclude);
    let pasted: Vec<String> = texts.iter().filter(|t| !t.trim().is_empty()).cloned().collect();
    let answer_paths: Vec<PathBuf> = inputs.answers.iter().filter_map(|a| a.path.clone()).collect();
    let (blocks, files, raw) = (inputs.blocks, inputs.files, inputs.raw);
    let parsed = check::parse_blocks(&blocks);
    // What each answer held: its part and counts (why it can't be read is added after the check below).
    let mut answer_inputs: Vec<Value> = inputs
        .answers
        .iter()
        .map(|a| {
            let mine: Vec<&check::Block> = blocks.iter().filter(|b| b.origin == a.origin).collect();
            let parts: Vec<&format::Part> = parsed.parts.iter().filter(|(_, text, _)| mine.iter().any(|b| b.text == *text)).map(|(p, _, _)| p).collect();
            let name = match a.origin.strip_prefix("wklejony tekst") {
                Some(rest) if a.path.is_none() => format!("Wklejony tekst{rest}"),
                _ => a.origin.clone(),
            };
            json!({
                "name": name,
                "kind": "answer",
                "path": a.path.as_ref().map(|p| p.display().to_string()),
                "size": a.size,
                "parts": parts.iter().filter_map(|p| p.part).collect::<Vec<_>>(),
                "persons": parts.iter().map(|p| p.persons.len()).sum::<usize>(),
                "events": parts.iter().map(|p| p.events.len()).sum::<usize>(),
                "questions": parts.iter().map(|p| p.questions.len()).sum::<usize>(),
                "found": !mine.is_empty(),
                "problem": null,
            })
        })
        .collect();
    let skipped = inputs.skipped;
    let (batch, mut issues) = check::check(parsed, &files);
    // An issue names its part and origin („Część 2 · odpowiedz.txt”): the first error is shown at its answer, whether
    // the part can't be read or isn't an answer for Heirloom at all.
    for (a, row) in inputs.answers.iter().zip(answer_inputs.iter_mut()) {
        if let Some(i) = issues.iter().find(|i| i.level == "error" && i.reference.ends_with(&format!(" · {}", a.origin))) {
            row["problem"] = json!(i.message);
        }
    }
    if blocks.is_empty() && texts.iter().any(|t| !t.trim().is_empty()) {
        issues.insert(0, Issue {
            level: "error",
            message: "Nie znaleziono paczki w tym tekście. Wklej całą odpowiedź AI, razem z blokiem ```json.".into(),
            reference: "Wklejony tekst".into(),
            action: None,
            target: None,
        });
    }
    let root = s.archive.root().to_path_buf();
    let d = s.derived();
    // Files already in the archive (same contents) are linked, not copied again.
    let mut duplicates = HashMap::new();
    for f in &files {
        let Some(n) = f.m else { continue };
        if let Some(existing) = duplicate_in_archive(d, &root, &f.path, f.size) {
            let what = if check::is_image(&f.path) { "Zdjęcie" } else { "Plik" };
            issues.push(Issue {
                level: "info",
                message: format!("{what} {} jest już w archiwum — nie zostanie zapisane drugi raz.", m_label(n)),
                reference: format!("Plik {}", f.name),
                action: None,
                target: None,
            });
            duplicates.insert(m_label(n), existing);
        }
    }
    let mut candidates = HashMap::new();
    let mut decisions = HashMap::new();
    let keys = matching::surname_keys(d);
    for p in &batch.merged.persons {
        candidates.insert(p.id.clone(), matching::candidates(d, &keys, &batch.merged, p));
    }
    matching::family_boost(d, &batch.merged, &mut candidates);
    for p in &batch.merged.persons {
        let list = &candidates[&p.id];
        let kind = if matching::status_for(list.first()) == "new" { Kind::New } else { Kind::Undecided };
        let target = list.first().map(|c| c.xref.clone());
        decisions.insert(p.id.clone(), Decision { kind, target, fields: HashMap::new(), excluded: false, excluded_fields: HashSet::new() });
    }
    // The AI's profile-photo suggestions.
    let mut profile = HashMap::new();
    for media in &batch.merged.media {
        for p in &media.profile_for {
            profile.entry(p.clone()).or_insert_with(|| m_number(&media.id).map(m_label));
        }
    }
    // Ages written in events become estimated birth dates (info only; saved at the end).
    for e in &batch.merged.events {
        let Some(year) = e.date.as_deref().and_then(format::gedcom_date).and_then(|g| g.split_whitespace().last().and_then(|y| y.parse::<i32>().ok())) else { continue };
        for role in &e.people {
            let Some(age) = role.age_orig.as_deref().and_then(|a| age_at(a, year)) else { continue };
            let has_birth = batch.merged.events.iter().any(|x| matches!(x.kind.as_deref(), Some("birth" | "baptism")) && x.people.iter().any(|r| r.p == role.p && r.role.as_deref().is_none_or(|q| q == "principal")));
            if !has_birth && role.role.as_deref() != Some("principal") {
                let who = batch.merged.persons.iter().find(|p| p.id == role.p).and_then(|p| p.names.first()).map(|n| format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default())).unwrap_or_default();
                issues.push(Issue {
                    level: "info",
                    message: format!("Wiek „{}” zamieniono na szacowaną datę urodzenia: ok. {} (wywnioskowane).", role.age_orig.clone().unwrap_or_default(), year - age),
                    reference: format!("Osoba {} · {}", role.p, who.trim()),
                    action: None,
                    target: Some(role.p.clone()),
                });
            }
        }
    }
    let load_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |t| t.as_millis() as u64);
    Draft {
        batch,
        issues,
        files,
        raw,
        candidates,
        decisions,
        answers: HashMap::new(),
        file_choices: HashMap::new(),
        profile,
        duplicates,
        load_id,
        attached: Vec::new(),
        date_fixes: HashMap::new(),
        answer_inputs,
        pasted,
        answer_paths,
        skipped,
        missing: inputs.missing,
        exclude: exclude.to_vec(),
    }
}

/// Step 1's list of everything dropped or pasted (design 17d): each AI answer with its part and counts, each file with
/// its kind, whom it shows (from the batch) or what is still to point at, and the copies left out.
fn input_list(draft: &Draft, files: &[Value]) -> Vec<Value> {
    let total = draft.batch.total_parts;
    let mut out = Vec::new();
    for a in &draft.answer_inputs {
        let parts: Vec<u64> = a["parts"].as_array().map(|p| p.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        let problem = a["problem"].as_str().map(str::to_string).or_else(|| (!a["found"].as_bool().unwrap_or(false)).then(|| "Nie znaleziono paczki w tym tekście.".to_string()));
        let part = match (parts.as_slice(), total) {
            ([n], Some(t)) if t > 1 => Some(format!("Część {n} z {t}")),
            ([n], _) => Some(format!("Część {n}")),
            ([], _) => None,
            (many, _) => Some(format!("Części {}", many.iter().map(u64::to_string).collect::<Vec<_>>().join(", "))),
        };
        let persons = a["persons"].as_u64().unwrap_or(0);
        let events = a["events"].as_u64().unwrap_or(0);
        let questions = a["questions"].as_u64().unwrap_or(0);
        let mut counts = Vec::new();
        if persons > 0 {
            counts.push(crate::count_pl(persons as usize, "osoba", "osoby", "osób"));
        }
        if events > 0 {
            counts.push(crate::count_pl(events as usize, "fakt", "fakty", "faktów"));
        }
        if questions > 0 {
            counts.push(crate::count_pl(questions as usize, "pytanie", "pytania", "pytań"));
        }
        let detail = match &problem {
            Some(p) => p.clone(),
            None => [part, (!counts.is_empty()).then(|| counts.join(", "))].into_iter().flatten().collect::<Vec<_>>().join(" · "),
        };
        out.push(json!({
            "name": a["name"],
            "kind": "answer",
            "path": a["path"],
            "size": a["size"],
            "m": null,
            "status": if problem.is_some() { "error" } else { "ok" },
            "detail": detail,
            "file": null,
        }));
    }
    for f in files.iter().filter(|f| f["path"].is_string()) {
        let key = f["file"].as_str().unwrap_or_default();
        let path = PathBuf::from(f["path"].as_str().unwrap_or_default());
        let choice = draft.file_choices.get(key).cloned().unwrap_or_default();
        let described = m_number(key).is_some_and(|n| draft.batch.merged.media.iter().any(|m| m_number(&m.id) == Some(n)));
        let guessed = check::file_kind(&path);
        let kind = choice.kind.clone().unwrap_or_else(|| match (described, f["kind"].as_str()) {
            (true, Some("photo")) => "photo".into(),
            (true, _) if guessed == "photo" => "photo".into(),
            (true, _) => "document".into(),
            (false, _) => guessed.into(),
        });
        let names: Vec<String> = f["people"].as_array().map(|p| p.iter().filter_map(|x| x["name"].as_str().map(str::to_string)).collect()).unwrap_or_default();
        // Short enough for a row: the first person and how many more, the caption cut to a few words.
        let people = match names.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            [a, b] => Some(format!("{a} i {b}")),
            [first, rest @ ..] => Some(format!("{first} i {}", crate::count_pl(rest.len(), "inna osoba", "inne osoby", "innych osób"))),
        };
        let caption = f["caption"].as_str().filter(|c| !c.is_empty()).map(|c| {
            let short: String = c.chars().take(48).collect();
            if short.len() < c.len() { format!("{}…", short.trim_end()) } else { short }
        });
        let who = [people, caption].into_iter().flatten().collect::<Vec<_>>().join(" · ");
        let (status, detail) = if choice.skip {
            ("skipped", "Pominięty — nie zostanie zapisany".to_string())
        } else if kind == "other" {
            ("assign", "Wybierz rodzaj pliku".to_string())
        } else if described || !names.is_empty() {
            let dup = if f["status"] == "dup" { " · już jest w archiwum" } else { "" };
            ("ok", format!("{}{dup}", if who.is_empty() { "Opisany w paczce".to_string() } else { who }))
        } else {
            ("assign", "Nie ma go w paczce — wskaż osobę".to_string())
        };
        out.push(json!({
            "name": f["name"],
            "kind": kind,
            "path": f["path"],
            "size": f["size"],
            "m": m_number(key).map(m_label),
            "status": status,
            "detail": detail,
            "file": key,
        }));
    }
    for path in &draft.missing {
        out.push(json!({
            "name": path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string()),
            "kind": check::file_kind(path),
            "path": path.display().to_string(),
            "size": null,
            "m": null,
            "status": "error",
            "detail": "Pliku nie ma już w tym miejscu — usuń go z listy albo upuść jeszcze raz.",
            "file": null,
        }));
    }
    for s in &draft.skipped {
        out.push(json!({
            "name": s.name,
            "kind": if s.answer { "answer" } else { check::file_kind(s.path.as_deref().unwrap_or(std::path::Path::new(""))) },
            "path": s.path.as_ref().map(|p| p.display().to_string()),
            "size": s.size,
            "m": null,
            "status": "skipped",
            "detail": format!("Taki sam jak {}", s.same_as),
            "file": null,
        }));
    }
    out
}


pub fn first_number(text: &str) -> Option<u32> {
    let digits: String = text.chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// An age as written ("lat 30", "30 лѣтъ") at an event in `year`; None when the number can't be one (a year written
/// by mistake would give a birth "ok. 0").
pub fn age_at(text: &str, year: i32) -> Option<i32> {
    first_number(text).filter(|age| *age <= 120).map(|age| age as i32).filter(|age| *age < year)
}

fn duplicate_in_archive(d: &Derived, root: &std::path::Path, path: &std::path::Path, size: u64) -> Option<String> {
    let mut hash = None;
    // Only records can be pointed to (not an object written inside a person, `@I1@#0`); an object without a file is
    // passed over, not the end of the search.
    for id in d.view.media_order.iter().filter(|id| !id.contains('#')) {
        let Some(file) = d.view.media.get(id).and_then(|m| m.file.clone()) else { continue };
        let existing = crate::media_edit::resolve(root, &file);
        if std::fs::metadata(&existing).ok().map(|m| m.len()) != Some(size) {
            continue;
        }
        let ours = hash.get_or_insert_with(|| std::fs::read(path).ok().map(|b| blake3::hash(&b)));
        if ours.is_some() && *ours == std::fs::read(&existing).ok().map(|b| blake3::hash(&b)) {
            return Some(id.clone());
        }
    }
    None
}

/// Joining `pid` with the archive person `target` when another person of the batch is already joined with them and the
/// two are relatives in the batch (a parent, a child, a partner, a sibling, the other half of a wedding): saving would
/// make one person their own relative, so it is refused here, in step 3, rather than at the end. Unrelated people may
/// be joined with one person (the AI can describe one human twice). Links the commit leaves out (a relative set to
/// „Pomiń” in the comparison) don't count.
pub fn same_target_conflict(draft: &Draft, pid: &str, target: &str) -> Option<String> {
    let batch = &draft.batch.merged;
    let name = |id: &str| batch.persons.iter().find(|p| p.id == id).map(batch_name).unwrap_or_else(|| id.to_string());
    for other in &batch.persons {
        let Some(decision) = draft.decisions.get(&other.id) else { continue };
        if other.id == pid || decision.excluded || decision.kind != Kind::Merge || decision.target.as_deref() != Some(target) {
            continue;
        }
        if let Some(word) = relation_word(draft, pid, &other.id) {
            return Some(format!(
                "Ta osoba z archiwum jest już połączona z: {} ({}), a w paczce to {word} osoby {}. Jedna osoba nie może być swoim własnym krewnym — wybierz kogoś innego albo „Nowa”.",
                name(&other.id),
                other.id,
                name(pid)
            ));
        }
    }
    None
}

/// What `b` is to `a` in the batch („ojciec”, „żona”, „brat”…), counting only the links the commit would write
/// (commit.rs: a merged person's relative set to „Pomiń” is not linked; `a` is taken as being merged).
fn relation_word(draft: &Draft, a: &str, b: &str) -> Option<&'static str> {
    let batch = &draft.batch.merged;
    let sex = |id: &str| batch.persons.iter().find(|p| p.id == id).and_then(|p| p.sex.clone());
    let skipped = |who: &str, field: &str| {
        draft.decisions.get(who).is_some_and(|d| (who == a || d.kind == Kind::Merge) && (d.excluded_fields.contains(field) || d.fields.get(field).map(String::as_str) == Some("skip")))
    };
    let word = |id: &str, male: &'static str, female: &'static str, other: &'static str| match sex(id).as_deref() {
        Some("M") => male,
        Some("F") => female,
        _ => other,
    };
    let partner = word(b, "mąż", "żona", "małżonek");
    for r in &batch.relationships {
        let (parent, child, x, y) = (r.parent.as_deref(), r.child.as_deref(), r.a.as_deref(), r.b.as_deref());
        match r.relation.as_deref() {
            Some("parent") if parent == Some(b) && child == Some(a) && !skipped(a, if sex(b).as_deref() == Some("F") { "mother" } else { "father" }) => {
                return Some(word(b, "ojciec", "matka", "rodzic"));
            }
            Some("parent") if parent == Some(a) && child == Some(b) && !skipped(b, if sex(a).as_deref() == Some("F") { "mother" } else { "father" }) => {
                return Some(word(b, "syn", "córka", "dziecko"));
            }
            Some("partners") if (x, y) == (Some(a), Some(b)) || (x, y) == (Some(b), Some(a)) => {
                if !skipped(a, "spouse") && !skipped(b, "spouse") {
                    return Some(partner);
                }
            }
            Some("sibling") if (x, y) == (Some(a), Some(b)) || (x, y) == (Some(b), Some(a)) => return Some(word(b, "brat", "siostra", "rodzeństwo")),
            _ => {}
        }
    }
    // A wedding (banns, divorce) links its two principals as partners whatever was chosen.
    for e in batch.events.iter().filter(|e| matches!(e.kind.as_deref(), Some("marriage" | "banns" | "divorce"))) {
        let explicit: Vec<&str> = e.people.iter().filter(|r| r.role.as_deref() == Some("principal")).map(|r| r.p.as_str()).collect();
        let principals: Vec<&str> = if explicit.is_empty() { e.people.iter().filter(|r| r.role.as_deref().is_none_or(|x| x == "spouse")).map(|r| r.p.as_str()).collect() } else { explicit };
        if principals.contains(&a) && principals.contains(&b) {
            return Some(partner);
        }
    }
    None
}

/// The name as the app shows people: the married name when there is one (the maiden name goes with „z d.”).
fn batch_name(p: &format::Person) -> String {
    p.names.iter().find(|n| n.kind.as_deref() == Some("married")).or(p.names.first()).map(|n| format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default()).trim().to_string()).unwrap_or_else(|| p.id.clone())
}

fn event_date(batch: &Part, person: &str, kinds: &[&str]) -> Option<DateInfo> {
    let e = batch.events.iter().filter(|e| kinds.contains(&e.kind.as_deref().unwrap_or(""))).find(|e| e.people.iter().any(|r| r.p == person && r.role.as_deref().is_none_or(|x| x == "principal" || x == "spouse")))?;
    date_info(e.date.as_deref())
}

fn date_info(text: Option<&str>) -> Option<DateInfo> {
    let text = text?;
    let gedcom = format::gedcom_date(text);
    let value = gedcom.as_deref().and_then(heirloom_core::gedcom::date::parse);
    Some(match value {
        Some(v) => DateInfo {
            text: heirloom_core::polish::format_date(&v),
            short: heirloom_core::polish::format_date_short(&v),
            year: heirloom_core::polish::year_text(&v),
            uncertain: heirloom_core::polish::is_uncertain(&v),
            sort: Some(v.sort_key()),
            value: Some(v),
        },
        None => DateInfo { text: text.into(), short: text.into(), year: text.into(), uncertain: true, sort: None, value: None },
    })
}

fn years(birth: &Option<DateInfo>, death: &Option<DateInfo>) -> String {
    match (birth.as_ref().map(|b| b.year.clone()), death.as_ref().map(|d| d.year.clone())) {
        (Some(b), Some(d)) => format!("{b}–{d}"),
        (Some(b), None) => format!("{b} – ?"),
        (None, Some(d)) => format!("? – {d}"),
        _ => String::new(),
    }
}

/// One row of the comparison table: what the archive has and what the batch brings.
fn compare_rows(s: &mut Session, draft: &Draft, pid: &str, target: &str) -> Vec<Value> {
    let d = s.derived();
    let Some(i) = d.index(target) else { return Vec::new() };
    let batch = &draft.batch.merged;
    let Some(person) = batch.persons.iter().find(|p| p.id == pid) else { return Vec::new() };
    let decision = draft.decisions.get(pid);
    let chosen = |field: &str, default: &str| decision.and_then(|x| x.fields.get(field).cloned()).unwrap_or_else(|| default.to_string());
    let mut rows = Vec::new();
    let archive_name = format!("{}{}", d.info[i].name, d.info[i].maiden.as_ref().map(|m| format!(" z d. {m}")).unwrap_or_default());
    let maiden = person.names.iter().find(|n| n.kind.as_deref() == Some("birth")).and_then(|n| n.surname.clone()).filter(|_| person.names.iter().any(|n| n.kind.as_deref() == Some("married")));
    let incoming_name = format!("{}{}", batch_name(person), maiden.map(|m| format!(" z d. {m}")).unwrap_or_default());
    // The same when every name form of the batch is already in the archive (a missing one is added as a variant).
    let archive_names: Vec<String> = d.view.model.persons[i].names.iter().map(|n| heirloom_core::fold::fold(&format!("{} {}", n.given, n.surname))).collect();
    let same_name = person.names.iter().all(|n| archive_names.contains(&heirloom_core::fold::fold(&format!("{} {}", n.given.clone().unwrap_or_default(), n.surname.clone().unwrap_or_default()))));
    rows.push(json!({ "field": "name", "label": "Imię i nazwisko", "archive": archive_name, "import": incoming_name, "same": same_name, "choice": if same_name { "same".into() } else { chosen("name", "variant") }, "options": ["keep", "variant"] }));
    let evidence = |kinds: &[&str]| -> Value {
        let e = batch.events.iter().filter(|e| kinds.contains(&e.kind.as_deref().unwrap_or(""))).find(|e| e.people.iter().any(|r| r.p == pid));
        match e {
            Some(e) => {
                let source = e.src.first().and_then(|s| batch.sources.iter().find(|x| x.id == *s).and_then(|x| x.title.clone()).or_else(|| batch.media.iter().find(|m| m.id == *s).and_then(|m| m.caption.clone())));
                json!({ "source": source, "basis": e.basis })
            }
            None => Value::Null,
        }
    };
    for (field, label, kinds, tags) in [
        ("birth", "Urodzenie", &["birth"][..], &["BIRT"][..]),
        ("baptism", "Chrzest", &["baptism"][..], &["BAPM", "CHR"][..]),
        ("death", "Zgon", &["death"][..], &["DEAT"][..]),
        ("burial", "Pogrzeb", &["burial"][..], &["BURI"][..]),
    ] {
        let fact = d.view.people[i].facts.iter().find(|f| tags.contains(&f.tag.as_str()));
        let archive_value = fact.map(|f| [DateInfo::from_fact(f).map(|x| x.text), f.place.clone()].into_iter().flatten().collect::<Vec<_>>().join(", "));
        let e = batch.events.iter().filter(|e| kinds.contains(&e.kind.as_deref().unwrap_or(""))).find(|e| e.people.iter().any(|r| r.p == pid && r.role.as_deref().is_none_or(|x| x == "principal")));
        let Some(e) = e else {
            if archive_value.is_some() {
                rows.push(json!({ "field": field, "label": label, "archive": archive_value, "import": Value::Null, "same": true, "choice": "same", "options": [] }));
            }
            continue;
        };
        let import_value = [date_info(e.date.as_deref()).map(|x| x.text), e.place.clone()].into_iter().flatten().collect::<Vec<_>>().join(", ");
        let same = archive_value.as_deref() == Some(import_value.as_str());
        let (default, options): (&str, Vec<&str>) = if archive_value.is_none() { ("add", vec!["skip", "add"]) } else { ("variant", vec!["keep", "replace", "variant"]) };
        rows.push(json!({ "field": field, "label": label, "archive": archive_value, "import": import_value, "same": same, "choice": if same { "same".into() } else { chosen(field, default) }, "options": options, "evidence": evidence(kinds) }));
    }
    let occupation: Vec<String> = batch.events.iter().filter(|e| e.kind.as_deref() == Some("occupation") && e.people.iter().any(|r| r.p == pid)).filter_map(|e| e.value.clone()).collect();
    if !occupation.is_empty() {
        let archive: Vec<String> = d.view.people[i].facts.iter().filter(|f| f.tag == "OCCU").filter_map(|f| f.value.clone()).collect();
        let same = occupation.iter().all(|o| archive.contains(o));
        rows.push(json!({ "field": "occupation", "label": "Zawód", "archive": (!archive.is_empty()).then(|| archive.join("; ")), "import": occupation.join("; "), "same": same, "choice": if same { "same".into() } else { chosen("occupation", "add") }, "options": ["skip", "add"], "evidence": evidence(&["occupation"]) }));
    }
    // Relatives the batch gives: the father, the mother, the spouse.
    let related = |relation: &str| -> Vec<(String, String)> {
        batch.relationships.iter().filter_map(|r| match (r.relation.as_deref(), relation) {
            (Some("parent"), "father" | "mother") if r.child.as_deref() == Some(pid) => {
                let parent = r.parent.clone()?;
                let female = batch.persons.iter().find(|p| p.id == parent).and_then(|p| p.sex.clone()) == Some("F".into());
                ((relation == "mother") == female).then_some((parent, r.basis.clone().unwrap_or_default()))
            }
            (Some("partners"), "spouse") => {
                let other = if r.a.as_deref() == Some(pid) { r.b.clone() } else if r.b.as_deref() == Some(pid) { r.a.clone() } else { None };
                other.map(|o| (o, r.basis.clone().unwrap_or_default()))
            }
            _ => None,
        }).collect()
    };
    for (field, label) in [("father", "Ojciec"), ("mother", "Matka"), ("spouse", "Małżonek")] {
        let incoming = related(field);
        if incoming.is_empty() {
            continue;
        }
        let archive: Vec<String> = match field {
            "spouse" => d.info[i].partners.iter().map(|&p| d.info[p].name.clone()).collect(),
            _ => d.info[i].parents.iter().filter(|&&p| (d.view.model.persons[p].sex == heirloom_core::gedcom::model::Sex::Female) == (field == "mother")).map(|&p| d.info[p].name.clone()).collect(),
        };
        let names: Vec<String> = incoming
            .iter()
            .map(|(id, _)| {
                let name = batch.persons.iter().find(|p| p.id == *id).map(batch_name).unwrap_or_else(|| id.clone());
                let state = match draft.decisions.get(id).map(|x| x.kind) {
                    Some(Kind::Merge) => "połączona",
                    Some(Kind::Skip) => "pominięta",
                    _ => "nowa osoba",
                };
                format!("{name} ({state})")
            })
            .collect();
        let same = incoming.iter().all(|(id, _)| {
            draft.decisions.get(id).and_then(|x| x.target.clone().filter(|_| x.kind == Kind::Merge)).and_then(|t| d.index(&t)).is_some_and(|t| d.info[i].parents.contains(&t) || d.info[i].partners.contains(&t))
        });
        rows.push(json!({ "field": field, "label": label, "archive": (!archive.is_empty()).then(|| archive.join(", ")), "import": names.join(", "), "same": same, "choice": if same { "same".into() } else { chosen(field, "add") }, "options": ["skip", "add"], "evidence": { "source": Value::Null, "basis": incoming.first().map(|x| x.1.clone()) } }));
    }
    rows
}

fn state(s: &mut Session, draft: &Draft) -> ApiResult {
    let batch = &draft.batch.merged;
    let errors = draft.issues.iter().filter(|i| i.level == "error").count();
    let warnings = draft.issues.iter().filter(|i| i.level == "warning").count();
    let questions: Vec<Value> = batch
        .questions
        .iter()
        .map(|q| {
            let (answer, note) = draft.answers.get(&q.id).cloned().unwrap_or_default();
            let about: Vec<Value> = q.about.iter().map(|p| json!({ "id": p, "name": batch.persons.iter().find(|x| x.id == *p).map(batch_name) })).collect();
            json!({ "id": q.id, "text": q.text, "about": about, "answer": (!answer.is_empty()).then_some(answer), "note": note })
        })
        .collect();

    let mut persons = Vec::new();
    for p in &batch.persons {
        let decision = draft.decisions.get(&p.id);
        let candidates: Vec<Value> = {
            let d = s.derived();
            draft
                .candidates
                .get(&p.id)
                .map(|list| {
                    // A candidate deleted since the batch was loaded is left out.
                    list.iter()
                        .filter_map(|c| {
                            let i = d.index(&c.xref)?;
                            let mut v = crate::people::with_context(d, i);
                            v["percent"] = json!(c.percent);
                            v["score"] = json!(c.score);
                            v["reasons"] = json!(c.reasons);
                            v["years"] = json!(crate::people::years_range(d, i));
                            Some(v)
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let best = draft.candidates.get(&p.id).and_then(|c| c.first());
        let status = match decision.map(|x| x.kind) {
            Some(Kind::New) => "new",
            Some(Kind::Merge) => "merged",
            Some(Kind::Skip) => "skipped",
            _ => matching::status_for(best),
        };
        let birth = event_date(batch, &p.id, &["birth", "baptism"]);
        let death = event_date(batch, &p.id, &["death", "burial"]);
        let maiden = p.names.iter().find(|n| n.kind.as_deref() == Some("birth")).and_then(|n| n.surname.clone()).filter(|_| p.names.iter().any(|n| n.kind.as_deref() == Some("married")));
        let facts = batch.events.iter().filter(|e| e.people.iter().any(|r| r.p == p.id)).count();
        let files: Vec<String> = batch.media.iter().filter(|m| m.depicts.contains(&p.id) || m.about.contains(&p.id)).filter_map(|m| m_number(&m.id).map(m_label)).collect();
        let target = decision.and_then(|x| x.target.clone());
        // Others of the batch joined with the same archive person (allowed when they aren't relatives): their data
        // goes into that one person.
        let same_target: Vec<Value> = match (decision, &target) {
            (Some(x), Some(t)) if x.kind == Kind::Merge && !x.excluded => batch
                .persons
                .iter()
                .filter(|o| o.id != p.id && draft.decisions.get(&o.id).is_some_and(|y| y.kind == Kind::Merge && !y.excluded && y.target.as_ref() == Some(t)))
                .map(|o| json!({ "id": o.id, "name": batch_name(o) }))
                .collect(),
            _ => Vec::new(),
        };
        let compare = match (decision.map(|x| x.kind), &target) {
            (Some(Kind::Merge) | Some(Kind::Undecided), Some(t)) => compare_rows(s, draft, &p.id, t),
            _ => Vec::new(),
        };
        persons.push(json!({
            "id": p.id,
            "name": batch_name(p),
            "sex": p.sex,
            "maiden": maiden,
            "years": years(&birth, &death),
            "initials": initials(&batch_name(p)),
            "status": status,
            "decision": decision.map(|x| match x.kind { Kind::New => "new", Kind::Merge => "merge", Kind::Skip => "skip", Kind::Undecided => "undecided" }),
            "target": target,
            "sameTarget": same_target,
            "candidates": candidates,
            "compare": compare,
            "facts": facts,
            "files": files,
            "profile": draft.profile.get(&p.id).cloned().flatten(),
            "excluded": decision.is_some_and(|x| x.excluded),
            "excludedFields": decision.map(|x| x.excluded_fields.iter().cloned().collect::<Vec<_>>()).unwrap_or_default(),
            "summary": p.summary,
        }));
    }

    // Files: described ones (M001…) and dropped ones without a description.
    let described: HashSet<String> = batch.media.iter().filter_map(|m| m_number(&m.id).map(m_label)).collect();
    let mut files = Vec::new();
    // The files the commit will really save or link (not skipped, delivered, described or given a person), and how
    // many of them are already in the archive.
    let (mut saving, mut saving_dups) = (0, 0);
    for m in &batch.media {
        let Some(label) = m_number(&m.id).map(m_label) else { continue };
        let input = check::file_by_m(&draft.files, &m.id);
        let choice = draft.file_choices.get(&label).cloned().unwrap_or_default();
        let status = if choice.skip {
            "skipped"
        } else if draft.duplicates.contains_key(&label) {
            "dup"
        } else if input.is_none() {
            "miss"
        } else {
            "ok"
        };
        if matches!(status, "ok" | "dup") {
            saving += 1;
            saving_dups += usize::from(status == "dup");
        }
        let people: Vec<String> = m.depicts.iter().chain(&m.about).chain(&choice.people).cloned().collect::<Vec<_>>().into_iter().fold(Vec::new(), |mut acc, x| {
            if !acc.contains(&x) {
                acc.push(x);
            }
            acc
        });
        files.push(json!({
            "file": label,
            "index": input.and_then(|f| draft.files.iter().position(|x| x.path == f.path)),
            "name": input.map(|f| f.name.clone()),
            "path": input.map(|f| f.path.display().to_string()),
            "size": input.map(|f| f.size),
            "status": status,
            "kind": choice.kind.clone().or(m.kind.clone()).unwrap_or_else(|| "photo".into()),
            "documentType": m.document_type,
            "caption": choice.caption.clone().or(m.caption.clone()),
            "people": people.iter().map(|id| json!({ "id": id, "name": batch.persons.iter().find(|x| x.id == *id).map(batch_name) })).collect::<Vec<_>>(),
            "transcription": m.transcription.is_some(),
            "image": input.is_some_and(|f| check::is_image(&f.path)),
        }));
    }
    for f in &draft.files {
        let label = f.m.map(m_label);
        if label.as_ref().is_some_and(|l| described.contains(l)) {
            continue;
        }
        let key = label.clone().unwrap_or_else(|| f.name.clone());
        let choice = draft.file_choices.get(&key).cloned().unwrap_or_default();
        if !choice.skip && !choice.people.is_empty() {
            saving += 1;
            saving_dups += usize::from(draft.duplicates.contains_key(&key));
        }
        files.push(json!({
            "file": key,
            "index": draft.files.iter().position(|x| x.path == f.path),
            "name": f.name,
            "path": f.path.display().to_string(),
            "size": f.size,
            "status": if choice.skip { "skipped" } else if draft.duplicates.contains_key(&key) { "dup" } else if !choice.people.is_empty() { "ok" } else { "und" },
            "kind": choice.kind.clone().unwrap_or_else(|| if check::is_image(&f.path) { "photo".into() } else { "document".into() }),
            "caption": choice.caption,
            "people": choice.people.iter().map(|id| json!({ "id": id, "name": batch.persons.iter().find(|x| x.id == *id).map(batch_name) })).collect::<Vec<_>>(),
            "transcription": false,
            "image": check::is_image(&f.path),
        }));
    }

    let undecided = persons.iter().filter(|p| p["decision"] == "undecided").count();
    let mentions_plain = draft.issues.iter().filter(|i| i.message.starts_with("Wzmianka o")).count();
    let unanswered = batch.questions.iter().filter(|q| !draft.answers.get(&q.id).is_some_and(|a| !a.0.is_empty())).count();
    let count_kind = |k: Kind| draft.decisions.values().filter(|x| x.kind == k && !x.excluded).count();
    let inputs = input_list(draft, &files);
    Ok(json!({
        "loadId": draft.load_id,
        "inputs": inputs,
        "batch": {
            "name": draft.batch.name,
            "author": draft.batch.author,
            "created": draft.batch.created,
            "parts": draft.batch.parts_seen,
            "totalParts": draft.batch.total_parts,
            "counts": {
                "persons": batch.persons.len(),
                "events": batch.events.len(),
                "relationships": batch.relationships.len(),
                "texts": batch.texts.len(),
                "files": files.len(),
                "sources": batch.sources.len(),
            },
        },
        "issues": draft.issues.iter().map(Issue::json).collect::<Vec<_>>(),
        "result": if errors > 0 { "errors" } else if warnings > 0 { "warnings" } else { "ok" },
        "errors": errors,
        "warnings": warnings,
        "questions": questions,
        "persons": persons,
        "relationships": batch.relationships.iter().map(|r| json!({ "type": r.relation, "parent": r.parent, "child": r.child, "a": r.a, "b": r.b, "kind": r.kind })).collect::<Vec<_>>(),
        "files": files,
        "undecided": undecided,
        "unresolved": { "mentions": mentions_plain, "questions": unanswered },
        "summary": {
            "new": count_kind(Kind::New),
            "merged": count_kind(Kind::Merge),
            "skipped": count_kind(Kind::Skip) + draft.decisions.values().filter(|x| x.excluded).count(),
            "events": batch.events.len(),
            "relationships": batch.relationships.len(),
            "texts": batch.texts.len(),
            "files": saving,
            "duplicates": saving_dups,
            "sources": batch.sources.len(),
        },
    }))
}

/// Previous imports, from the change history (newest first). `active`: something of the import is still as it saved
/// it, so „Cofnij import” would take it back (false once it was undone, or everything of it was changed since).
fn history(s: &Session) -> Value {
    let entries = s.history();
    let mut batches: Vec<(String, String, String, usize, usize, bool)> = Vec::new();
    let mut now: HashMap<&str, Option<String>> = HashMap::new();
    for e in &entries {
        let Some(batch) = &e.batch else { continue };
        let current = now.entry(e.record.as_str()).or_insert_with(|| s.archive.doc.record(&e.record).map(heirloom_core::history::record_text));
        let active = e.tag != "HEAD" && e.after.is_some() && *current == e.after;
        match batches.iter_mut().find(|b| b.0 == *batch) {
            Some(b) => {
                if e.tag == "INDI" {
                    b.3 += 1;
                }
                if e.tag == "OBJE" {
                    b.4 += 1;
                }
                b.5 |= active;
            }
            None => batches.push((batch.clone(), e.ts.clone(), e.author.clone(), usize::from(e.tag == "INDI"), usize::from(e.tag == "OBJE"), active)),
        }
    }
    batches.reverse();
    Value::Array(
        batches
            .into_iter()
            .map(|(name, ts, author, people, files, active)| json!({ "name": name, "ts": ts, "author": author, "people": people, "files": files, "active": active }))
            .collect(),
    )
}
