//! The AI instructions (`docs/AI_INSTRUCTIONS.md`, the text „Kopiuj instrukcję dla AI” copies) against the real
//! importer: their own example, and a fresh two-part answer written by following Part B rule by rule, must come in
//! without a single error or warning.

use heirloom_api::Api;
use serde_json::{Value, json};
use std::path::Path;

const INSTRUCTIONS: &str = include_str!("../../../docs/AI_INSTRUCTIONS.md");

fn run(api: &mut Api, method: &str, args: Value) -> Value {
    api.call(method, args.clone()).unwrap_or_else(|e| panic!("{method} {args}: {} ({})", e.message, e.code))
}

fn new_archive(dir: &Path) -> Api {
    let mut api = Api::new(None, None);
    run(&mut api, "archive.create", json!({ "folder": dir.join("archiwum").display().to_string(), "name": "Instrukcja" }));
    api
}

fn issues(state: &Value, level: &str) -> Vec<String> {
    state["issues"].as_array().unwrap().iter().filter(|i| i["level"] == level).map(|i| i["message"].as_str().unwrap().to_string()).collect()
}

fn photo(path: &Path, shade: u8) {
    image::RgbImage::from_pixel(4, 3, image::Rgb([shade, shade, shade])).save(path).unwrap();
}

#[test]
fn the_instructions_example_passes_the_check() {
    let start = INSTRUCTIONS.find("## Przykład").expect("Part B has an example");
    let example = &INSTRUCTIONS[start..];
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    let m001 = dir.path().join("M001 akt urodzenia Józef 1878.jpg");
    photo(&m001, 3);
    let state = run(&mut api, "import.load", json!({ "texts": [example], "paths": [m001.display().to_string()] }));
    assert_eq!(issues(&state, "error"), Vec::<String>::new());
    assert_eq!(issues(&state, "warning"), Vec::<String>::new());
    assert_eq!(state["batch"]["counts"]["persons"], 2);
}

#[test]
fn the_prompt_revision_and_format_version_agree_everywhere() {
    let rev = INSTRUCTIONS.split("Wersja instrukcji: ").nth(1).unwrap().split(' ').next().unwrap();
    assert_eq!(INSTRUCTIONS.matches(&format!("\"prompt_rev\": \"{rev}\"")).count(), 2, "the format line and the example");
    assert!(INSTRUCTIONS.contains(&format!("wersja instrukcji {rev}")), "Part B's header");
    let format = include_str!("../../../docs/IMPORT_FORMAT.md");
    assert!(format.contains(&format!("\"prompt_rev\": \"{rev}\"")), "IMPORT_FORMAT's example uses the current revision");
    assert!(INSTRUCTIONS.contains("heirloom-import 1.0") && format.starts_with("# Heirloom import format: `heirloom-import` 1.0"));
}

/// A chat's whole answer, written by following Part B exactly: two parts, a pasted note (N1), a Latin record with an
/// age in words, a date that can't be written in the format, an emigration, a link, a question.
const ANSWER: &str = r#"Oto część 1 z 2.

```json
{
  "format": "heirloom-import",
  "version": "1.0",
  "prompt_rev": "2026-10-05",
  "batch": "Nowakowie-Ciechanki-2026-10-05",
  "part": 1,
  "created": "2026-10-05",
  "lang": "pl",
  "manifest": ["P1", "P2", "P3", "P4"],
  "persons": [
    {
      "id": "P1", "sex": "M", "living": false,
      "names": [ { "type": "birth", "given": "Jan", "surname": "Nowak", "orig": "Joannes Nowak", "lang": "la", "src": ["S1"] } ],
      "summary": "Jan Nowak ożenił się 12 listopada 1890 roku w Ciechankach z Marianną z domu Wiśniewską.",
      "tags": ["rolnik"]
    },
    {
      "id": "P2", "sex": "F",
      "names": [
        { "type": "birth", "given": "Marianna", "surname": "Wiśniewska", "orig": "Marianna de domo Wiśniewska", "lang": "la", "src": ["S1"] },
        { "type": "married", "given": "Marianna", "surname": "Nowak", "src": ["S1"] }
      ]
    },
    {
      "id": "P3", "sex": "M",
      "names": [ { "type": "birth", "given": "Wojciech", "surname": "Nowak", "orig": "Adalbertus Nowak", "lang": "la", "src": ["S1"] } ]
    }
  ],
  "events": [
    {
      "id": "E1", "type": "marriage", "date": "1890-11-12", "date_orig": "die duodecima Novembris 1890",
      "place": "Ciechanki", "place_note": "parafia Łęczna",
      "people": [
        { "p": "P1", "role": "spouse" },
        { "p": "P2", "role": "spouse" },
        { "p": "P3", "role": "witness", "age_orig": "quinquaginta annorum (50)" }
      ],
      "src": ["S1"], "basis": "stated", "certainty": "high"
    },
    {
      "id": "E2", "type": "occupation", "value": "rolnik", "date": "1890",
      "people": [ { "p": "P1", "role": "principal" } ],
      "src": ["S1"], "basis": "stated", "certainty": "high"
    }
  ],
  "relationships": [
    { "type": "partners", "a": "P1", "b": "P2", "kind": "marriage", "src": ["S1"], "basis": "stated", "certainty": "high" },
    { "type": "parent", "parent": "P3", "child": "P1", "kind": "birth", "src": ["N1"], "basis": "stated", "certainty": "medium" }
  ],
  "texts": [
    { "id": "T1", "person": "P1", "kind": "bio", "title": "Ślub", "md": "Jan ożenił się w 1890 roku w Ciechankach z [Marianną](person:P2). Świadkiem był jego ojciec [Wojciech](person:P3).", "src": ["S1", "N1"], "basis": "stated", "certainty": "high" }
  ],
  "media": [
    { "id": "M001", "kind": "document", "document_type": "akt małżeństwa", "caption": "Akt małżeństwa Jana Nowaka i Marianny Wiśniewskiej, Łęczna 1890, nr 31", "about": ["P1", "P2", "P3"], "transcription": "Anno Domini 1890 die duodecima Novembris […]", "translation": "Roku Pańskiego 1890, dnia dwunastego listopada […]" },
    { "id": "M002", "kind": "photo", "caption": "Jan i Marianna Nowakowie", "depicts": ["P1", "P2"], "profile_for": ["P1"] }
  ],
  "sources": [
    { "id": "S1", "kind": "parish_record", "title": "Akt małżeństwa nr 31/1890, parafia rzymskokatolicka Łęczna", "parish": "Łęczna", "year": 1890, "akt": "31", "archive": "Archiwum Archidiecezjalne w Lublinie", "call_number": "Łęczna, księga małżeństw 1890", "media": ["M001"] }
  ],
  "coverage": [
    { "m": "M001", "status": "extracted" },
    { "m": "M002", "status": "extracted" }
  ],
  "end": { "part": 1, "final": false, "counts": { "persons": 3, "events": 2, "relationships": 2, "texts": 1, "media": 2, "sources": 1, "coverage": 2 }, "marker": "END-HEIRLOOM-IMPORT" }
}
```

Napisz: dalej

Oto część 2 z 2.

```json
{
  "format": "heirloom-import",
  "version": "1.0",
  "prompt_rev": "2026-10-05",
  "batch": "Nowakowie-Ciechanki-2026-10-05",
  "part": 2,
  "created": "2026-10-05",
  "lang": "pl",
  "persons": [
    {
      "id": "P4", "sex": "M",
      "names": [ { "type": "birth", "given": "Józef", "surname": "Nowak", "nickname": "Józek", "src": ["N1"] } ],
      "summary": "Józef Nowak, syn [Jana](person:P1) i Marianny, w 1912 roku wyjechał do Ameryki."
    }
  ],
  "events": [
    {
      "id": "E3", "type": "birth", "date_orig": "zimą 1892",
      "people": [ { "p": "P4", "role": "principal" }, { "p": "P1", "role": "father" }, { "p": "P2", "role": "mother" } ],
      "src": ["N1"], "basis": "stated", "certainty": "medium"
    },
    {
      "id": "E4", "type": "emigration", "date": "1912", "place": "Chicago", "note": "Wypłynął z Hamburga.",
      "people": [ { "p": "P4", "role": "principal" } ],
      "src": ["N1"], "basis": "stated", "certainty": "medium"
    }
  ],
  "relationships": [
    { "type": "parent", "parent": "P1", "child": "P4", "kind": "birth", "src": ["N1"], "basis": "stated", "certainty": "medium" },
    { "type": "parent", "parent": "P2", "child": "P4", "kind": "birth", "src": ["N1"], "basis": "stated", "certainty": "medium" }
  ],
  "texts": [
    { "id": "T2", "person": "P4", "kind": "story", "title": "Wyjazd do Ameryki", "md": "Według relacji wnuczki Józef w 1912 roku pożegnał się z [ojcem](person:P1) i wypłynął z Hamburga do Ameryki.", "date": "1912", "place": "Hamburg", "src": ["N1"], "basis": "stated", "certainty": "medium" },
    { "id": "T3", "person": "P4", "kind": "saying", "md": "„Kto pyta, nie błądzi.”", "src": ["N1"], "basis": "stated", "certainty": "medium" }
  ],
  "links": [
    { "person": "P4", "url": "https://www.familysearch.org/ark:/61903/1:1:TEST-123", "title": "Lista pasażerów", "kind": "familysearch" }
  ],
  "coverage": [
    { "m": "N1", "status": "extracted" }
  ],
  "questions": [
    { "id": "Q1", "about": ["P4"], "text": "Czy Józef miał rodzeństwo? Notatka (N1) wspomina „braci”, ale bez imion." }
  ],
  "end": { "part": 2, "final": true, "counts": { "persons": 1, "events": 2, "relationships": 2, "texts": 2, "links": 1, "coverage": 1, "questions": 1 }, "marker": "END-HEIRLOOM-IMPORT" }
}
```"#;

#[test]
fn a_fresh_answer_that_follows_the_instructions_comes_in_whole() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    let (m001, m002) = (dir.path().join("M001 akt malzenstwa 1890.jpg"), dir.path().join("M002 Jan i Marianna.jpg"));
    photo(&m001, 1);
    photo(&m002, 2);
    let state = run(&mut api, "import.load", json!({ "texts": [ANSWER], "paths": [m001.display().to_string(), m002.display().to_string()] }));
    assert_eq!(issues(&state, "error"), Vec::<String>::new());
    assert_eq!(issues(&state, "warning"), Vec::<String>::new());
    assert_eq!(state["batch"]["parts"], json!([1, 2]));
    assert_eq!(issues(&state, "info"), ["Wiek „quinquaginta annorum (50)” zamieniono na szacowaną datę urodzenia: ok. 1840 (wywnioskowane)."]);

    run(&mut api, "import.answer", json!({ "question": "Q1", "answer": "unknown" }));
    let result = run(&mut api, "import.commit", json!({ "author": "Test" }));
    assert_eq!(result["people"], 4);

    let people = run(&mut api, "people.list", json!({}))["people"].as_array().unwrap().clone();
    let id = |name: &str| people.iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no {name}: {people:#?}"))["id"].clone();
    let wojciech = people.iter().find(|p| p["name"] == "Wojciech Nowak").unwrap();
    assert_eq!(wojciech["birth"]["year"], "ok. 1840", "the age in words with its number");

    let jan = run(&mut api, "person.get", json!({ "id": id("Jan Nowak") }));
    let facts = jan["facts"].to_string();
    assert!(facts.contains("12 listopada 1890") && facts.contains("rolnik"), "the marriage and the occupation: {facts}");
    assert!(jan["gallery"].as_array().unwrap().iter().any(|g| g["profile"] == true), "M002 is his profile photo");
    assert!(jan["sources"].to_string().contains("Akt małżeństwa nr 31/1890"));
    let relations = run(&mut api, "person.relations", json!({ "id": id("Jan Nowak") })).to_string();
    for name in ["Marianna Nowak", "Wojciech Nowak", "Józef Nowak"] {
        assert!(relations.contains(name), "{name} in {relations}");
    }

    let jozef = run(&mut api, "person.get", json!({ "id": id("Józef Nowak") }));
    let facts = jozef["facts"].to_string();
    assert!(facts.contains("zimą 1892"), "a date only in words is kept as its wording: {facts}");
    assert!(facts.contains("Chicago"), "the emigration with its place: {facts}");
    assert_eq!(jozef["stories"].as_array().unwrap().len(), 1);
    assert_eq!(jozef["sayings"].as_array().unwrap().len(), 1);
    assert!(jozef["notes"].to_string().contains("Odpowiedź: nie wiem."), "the answered question: {}", jozef["notes"]);
    assert!(jozef.to_string().contains("familysearch.org"), "the link");
    assert!(jozef["sources"].to_string().contains("Notatka N1 wklejona do czatu"), "{}", jozef["sources"]);
}
