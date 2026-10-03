//! The AI trial: packages written the way a chat AI answers `docs/AI_INSTRUCTIONS.md` (fictional family, see
//! `tests/fixtures/ai-trial/README.md`), taken through the same commands the Import screens use.

use heirloom_api::Api;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ai-trial")
}

/// The packages are kept out of Git (test data, never published): without them these tests are skipped.
fn have_fixtures() -> bool {
    let found = fixtures().join("paczka-1").is_dir();
    if !found {
        eprintln!("No tests/fixtures/ai-trial packages here: these tests are skipped.");
    }
    found
}

fn text(file: &str) -> String {
    std::fs::read_to_string(fixtures().join(file)).unwrap()
}

/// The images in a package folder (the researcher drops the folder, or its files).
fn files(folder: &str) -> Vec<String> {
    let mut list: Vec<String> = std::fs::read_dir(fixtures().join(folder))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg" || e == "png"))
        .map(|p| p.display().to_string())
        .collect();
    list.sort();
    list
}

fn run(api: &mut Api, method: &str, args: Value) -> Value {
    api.call(method, args.clone()).unwrap_or_else(|e| panic!("{method} {args}: {} ({})", e.message, e.code))
}

fn new_archive(dir: &Path) -> Api {
    let mut api = Api::new(None, None);
    run(&mut api, "archive.create", json!({ "folder": dir.join("archiwum").display().to_string(), "name": "Próba AI" }));
    api
}

fn issues(state: &Value, level: &str) -> Vec<String> {
    state["issues"].as_array().unwrap().iter().filter(|i| i["level"] == level).map(|i| i["message"].as_str().unwrap().to_string()).collect()
}

fn person<'a>(state: &'a Value, name: &str) -> &'a Value {
    state["persons"].as_array().unwrap().iter().find(|p| p["name"] == name).unwrap_or_else(|| panic!("no {name} in the batch"))
}

/// Everyone in the archive with this name.
fn in_archive(api: &mut Api, name: &str) -> Vec<Value> {
    run(api, "people.list", json!({}))["people"].as_array().unwrap().iter().filter(|p| p["name"] == name).cloned().collect()
}

fn fact<'a>(profile: &'a Value, key: &str) -> Option<&'a str> {
    profile["facts"].as_array().unwrap().iter().find(|f| f["key"] == key).and_then(|f| f["value"].as_str())
}

/// Package 1 as the careful answer: both parts, the five files.
fn import_package_1(api: &mut Api) -> Value {
    let state = run(api, "import.load", json!({ "texts": [text("paczka-1/odpowiedz-czesc-1.txt"), text("paczka-1/odpowiedz-czesc-2.txt")], "paths": files("paczka-1") }));
    assert_eq!(issues(&state, "error"), Vec::<String>::new());
    state
}

#[test]
fn the_careful_package_comes_in_whole() {
    if !have_fixtures() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    let state = import_package_1(&mut api);
    let counts = &state["batch"]["counts"];
    assert_eq!((counts["persons"].as_u64(), counts["events"].as_u64(), counts["texts"].as_u64(), counts["files"].as_u64()), (Some(13), Some(10), Some(5), Some(5)));
    assert_eq!(state["batch"]["parts"], json!([1, 2]));
    assert!(state["files"].as_array().unwrap().iter().all(|f| f["status"] == "ok"), "every file found: {}", state["files"]);
    assert!(state["persons"].as_array().unwrap().iter().all(|p| p["decision"] == "new"), "an empty archive: everyone is new");
    eprintln!("warnings: {:#?}\ninfo: {:#?}", issues(&state, "warning"), issues(&state, "info"));

    run(&mut api, "import.answer", json!({ "question": "Q1", "answer": "unknown", "note": "Trzeba sprawdzić akt urodzenia Katarzyny." }));
    run(&mut api, "import.answer", json!({ "question": "Q2", "answer": "no" }));
    let result = run(&mut api, "import.commit", json!({ "author": "Próba" }));
    assert_eq!(result["people"], 13);
    assert_eq!(run(&mut api, "archive.status", json!({}))["unsavedChanges"], 0, "the import is saved");

    // Stanisław: dates with the Julian ones, both occupations, texts, photo, documents and sources.
    let stanislaw = in_archive(&mut api, "Stanisław Sadowski");
    assert_eq!(stanislaw.len(), 1);
    let profile = run(&mut api, "person.get", json!({ "id": stanislaw[0]["id"] }));
    assert_eq!(fact(&profile, "Urodzony"), Some("13 marca 1884 (jul. 1 marca 1884), Ratoszyn (par. Chodel)"));
    assert!(fact(&profile, "Ochrzczony").is_some_and(|v| v.starts_with("15 marca 1884")), "{}", profile["facts"]);
    assert!(fact(&profile, "Zmarł").is_some_and(|v| v.starts_with("7 listopada 1932")), "{}", profile["facts"]);
    let facts = profile["facts"].to_string();
    assert!(facts.contains("rolnik") && facts.contains("kowal"), "both occupations: {facts}");
    let bio: Vec<&str> = profile["bio"].as_array().unwrap().iter().filter_map(|t| t["title"].as_str()).collect();
    assert_eq!(bio, ["Dzieciństwo", "Rodzina i praca"]);
    assert_eq!(profile["stories"].as_array().unwrap().len(), 1);
    assert_eq!(profile["sayings"].as_array().unwrap().len(), 1);
    assert!(profile["gallery"].as_array().unwrap().iter().any(|g| g["profile"] == true), "the wedding photo is his profile photo");
    assert!(profile["documents"].as_array().unwrap().len() >= 3, "birth, marriage and death records: {}", profile["documents"]);
    assert!(profile["sources"].as_array().unwrap().len() >= 3);

    // The family: parents, wife, three children, godparents and witnesses as people of their own.
    let relations = run(&mut api, "person.relations", json!({ "id": stanislaw[0]["id"] })).to_string();
    for name in ["Wojciech Sadowski", "Katarzyna Sadowska", "Józefa Sadowska", "Jan Sadowski", "Maria Sadowska", "Helena Sadowska"] {
        assert!(relations.contains(name), "{name} in {relations}");
    }
    assert_eq!(in_archive(&mut api, "Marianna Zając").len(), 1);
    // Ages became estimated births: Wojciech was 35 in 1884.
    let wojciech = &in_archive(&mut api, "Wojciech Sadowski")[0];
    assert_eq!(wojciech["birth"]["year"], "ok. 1849");
    // The mentions in the texts point at the imported people.
    let childhood = profile["bio"][0]["body"].as_str().unwrap_or_default();
    assert!(childhood.contains("person:@"), "mentions are linked: {childhood}");
}

#[test]
fn everything_dropped_at_once_is_listed_with_what_it_is() {
    if !have_fixtures() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    // The researcher's folder as the family would drop it: the answers, the photos, a second copy of part 1, a note,
    // a photo the package doesn't describe and a file of an unknown kind.
    let drop = dir.path().join("od-badacza");
    std::fs::create_dir_all(&drop).unwrap();
    for entry in std::fs::read_dir(fixtures().join("paczka-1")).unwrap().flatten() {
        std::fs::copy(entry.path(), drop.join(entry.file_name())).unwrap();
    }
    std::fs::copy(drop.join("odpowiedz-czesc-1.txt"), drop.join("wersja-kopia.txt")).unwrap();
    std::fs::write(drop.join("notatki babci.txt"), "Zima 1915: wyjazd na wschód, pociągiem z Lublina.").unwrap();
    std::fs::write(drop.join("skan.xyz"), b"?").unwrap();
    let mut photo = std::fs::read(drop.join(std::fs::read_dir(&drop).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).find(|n| n.starts_with("M001")).unwrap())).unwrap();
    photo.push(0);
    std::fs::write(drop.join("IMG_2291.jpg"), photo).unwrap();

    let state = run(&mut api, "import.load", json!({ "texts": [], "paths": [drop.display().to_string()] }));
    let inputs = state["inputs"].as_array().unwrap();
    let row = |name: &str| inputs.iter().find(|i| i["name"] == name).unwrap_or_else(|| panic!("no {name} in {inputs:#?}"));
    let part1 = row("odpowiedz-czesc-1.txt");
    assert_eq!((part1["kind"].as_str(), part1["status"].as_str()), (Some("answer"), Some("ok")));
    assert!(part1["detail"].as_str().unwrap().starts_with("Część 1 z 2 · "), "{part1}");
    assert_eq!(row("wersja-kopia.txt")["status"], "skipped", "the same answer twice is read once");
    assert_eq!(row("wersja-kopia.txt")["detail"], "Taki sam jak odpowiedz-czesc-1.txt");
    let m001 = inputs.iter().find(|i| i["m"] == "M001").unwrap();
    assert_eq!((m001["kind"].as_str(), m001["status"].as_str()), (Some("photo"), Some("ok")));
    assert!(m001["detail"].as_str().unwrap().contains("Sadowsk"), "whom the photo shows: {m001}");
    assert_eq!((row("IMG_2291.jpg")["status"].as_str(), row("IMG_2291.jpg")["detail"].as_str()), (Some("assign"), Some("Nie ma go w paczce — wskaż osobę")));
    assert_eq!(row("notatki babci.txt")["kind"], "note");
    assert_eq!((row("skan.xyz")["kind"].as_str(), row("skan.xyz")["detail"].as_str()), (Some("other"), Some("Wybierz rodzaj pliku")));

    // Taken off the list; a kind chosen by hand.
    let state = run(&mut api, "import.load", json!({ "texts": [], "paths": [drop.display().to_string()], "exclude": [drop.join("skan.xyz").display().to_string()] }));
    assert!(!state["inputs"].as_array().unwrap().iter().any(|i| i["name"] == "skan.xyz"));
    let key = state["inputs"].as_array().unwrap().iter().find(|i| i["name"] == "notatki babci.txt").unwrap()["file"].as_str().unwrap().to_string();
    let state = run(&mut api, "import.file", json!({ "file": key, "kind": "document" }));
    assert_eq!(state["inputs"].as_array().unwrap().iter().find(|i| i["name"] == "notatki babci.txt").unwrap()["kind"], "document");
}

#[test]
fn the_list_keeps_order_names_and_choices() {
    if !have_fixtures() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    // Two answers with the same file name, from two folders; a photo dropped twice under two names; many files of
    // one size, only two of them the same.
    let (a, b) = (dir.path().join("od-Ewy"), dir.path().join("od-Adama"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::copy(fixtures().join("paczka-1/odpowiedz-czesc-1.txt"), a.join("odpowiedz.txt")).unwrap();
    std::fs::copy(fixtures().join("paczka-1/odpowiedz-czesc-2.txt"), b.join("odpowiedz.txt")).unwrap();
    let photos = dir.path().join("zdjecia");
    std::fs::create_dir_all(&photos).unwrap();
    for n in 0..40u8 {
        std::fs::write(photos.join(format!("skan-{n:02}.png")), [n; 64]).unwrap();
    }
    std::fs::write(photos.join("skan-99.png"), [7u8; 64]).unwrap();
    let first = photos.join("portret.jpg");
    std::fs::write(&first, b"a photo").unwrap();
    let second = dir.path().join("portret (kopia).jpg");
    std::fs::write(&second, b"a photo").unwrap();
    let paths = [a.join("odpowiedz.txt"), b.join("odpowiedz.txt"), first.clone(), second.clone(), photos.clone()].map(|p| p.display().to_string());
    let state = run(&mut api, "import.load", json!({ "texts": [], "paths": paths }));
    let inputs = state["inputs"].as_array().unwrap();
    let names: Vec<&str> = inputs.iter().filter(|i| i["kind"] == "answer").map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["odpowiedz.txt", "odpowiedz.txt (2)"], "each answer keeps its own row");
    let details: Vec<&str> = inputs.iter().filter(|i| i["kind"] == "answer").map(|i| i["detail"].as_str().unwrap()).collect();
    assert!(details[0].starts_with("Część 1") && details[1].starts_with("Część 2"), "{details:?}");
    let copy = inputs.iter().find(|i| i["name"] == "portret (kopia).jpg").unwrap();
    assert_eq!((copy["status"].as_str(), copy["detail"].as_str()), (Some("skipped"), Some("Taki sam jak portret.jpg")), "the one dropped first stays");
    let skipped: Vec<&str> = inputs.iter().filter(|i| i["status"] == "skipped").map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(skipped, ["portret (kopia).jpg", "skan-99.png"], "of 41 same-size scans only the true copy is left out");

    // A kind chosen by hand survives a reload of the list.
    let key = inputs.iter().find(|i| i["name"] == "portret.jpg").unwrap()["path"].as_str().unwrap().to_string();
    let state = run(&mut api, "import.load", json!({ "texts": [], "paths": paths, "kinds": { key.clone(): "document" } }));
    assert_eq!(state["inputs"].as_array().unwrap().iter().find(|i| i["name"] == "portret.jpg").unwrap()["kind"], "document");

    // Adding the missing part later reads the answer files as files again, not as pasted text.
    let state = run(&mut api, "import.load", json!({ "texts": ["tylko notatka"], "add": true }));
    let names: Vec<String> = state["inputs"].as_array().unwrap().iter().filter(|i| i["kind"] == "answer").map(|i| i["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"odpowiedz.txt".to_string()) && names.contains(&"odpowiedz.txt (2)".to_string()), "{names:?}");
}

#[test]
fn a_sloppy_package_is_repaired_or_explained() {
    if !have_fixtures() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    // Both answers pasted at once, part 2 cut off by the answer length limit.
    let state = run(&mut api, "import.load", json!({ "texts": [text("paczka-1-niechlujna/odpowiedz.txt")], "paths": files("paczka-1") }));
    let errors = issues(&state, "error");
    assert!(errors.iter().any(|e| e.starts_with("Część 2 jest ucięta")), "{errors:?}");
    assert_eq!(state["batch"]["parts"], json!([1]), "part 1 parsed despite the trailing comma and the curly quotes");
    let date = state["issues"].as_array().unwrap().iter().find(|i| i["message"].as_str().unwrap_or_default().contains("11.02.1907")).expect("the Polish date is flagged");
    assert_eq!(date["action"], "fix");
    let event = date["target"].as_str().unwrap().to_string();

    // Part 2 sent again in full: the error goes, the decisions stay.
    let state = run(&mut api, "import.load", json!({ "texts": [text("paczka-1-niechlujna/czesc-2-ponownie.txt")], "add": true }));
    assert_eq!(issues(&state, "error"), Vec::<String>::new());
    assert_eq!(state["batch"]["parts"], json!([1, 2]));
    // The date fixed by hand, typed the Polish way.
    let state = run(&mut api, "import.fixDate", json!({ "event": event, "date": "11.02.1907" }));
    assert!(!state["issues"].to_string().contains("11.02.1907"));
    assert_eq!(person(&state, "Józefa Sadowska")["sex"], "F", "„K” (kobieta) read as female");

    run(&mut api, "import.commit", json!({ "author": "Próba" }));
    let jozefa = in_archive(&mut api, "Józefa Sadowska");
    assert_eq!((jozefa.len(), jozefa[0]["sex"].as_str()), (1, Some("F")));
    let stanislaw = in_archive(&mut api, "Stanisław Sadowski");
    let profile = run(&mut api, "person.get", json!({ "id": stanislaw[0]["id"] }));
    assert!(profile["facts"].to_string().contains("11 lutego 1907"), "the fixed wedding date: {}", profile["facts"]);
}

#[test]
fn a_later_package_joins_the_people_already_there() {
    if !have_fixtures() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut api = new_archive(dir.path());
    import_package_1(&mut api);
    run(&mut api, "import.commit", json!({ "author": "Próba" }));

    let state = run(&mut api, "import.load", json!({ "texts": [text("paczka-2/odpowiedz.txt")], "paths": files("paczka-2") }));
    assert_eq!(issues(&state, "error"), Vec::<String>::new());
    let report: Vec<String> = state["persons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| format!("{} → {} {:?}", p["name"], p["status"], p["candidates"].as_array().unwrap().iter().map(|c| format!("{} {}%", c["name"], c["percent"])).collect::<Vec<_>>()))
        .collect();
    eprintln!("{report:#?}");
    // Jan, his parents: the right archive person comes first; Anna and Tadeusz are new.
    for name in ["Jan Sadowski", "Stanisław Sadowski", "Józefa Sadowska"] {
        let p = person(&state, name);
        assert_eq!(p["candidates"][0]["name"], name, "{name}: {}", p["candidates"]);
        assert_eq!(p["status"], "match", "{name}: a clear match (Jan by his data, his parents also through him)");
        run(&mut api, "import.decide", json!({ "person": p["id"], "kind": "merge", "target": p["candidates"][0]["id"] }));
    }
    for name in ["Anna Sadowska", "Tadeusz Sadowski"] {
        assert_eq!(person(&state, name)["status"], "new", "{name}");
    }
    run(&mut api, "import.answer", json!({ "question": "Q1", "answer": "yes" }));
    let result = run(&mut api, "import.commit", json!({ "author": "Próba" }));
    assert_eq!((result["people"].as_u64(), result["merged"].as_u64()), (Some(2), Some(3)));

    for name in ["Jan Sadowski", "Stanisław Sadowski", "Józefa Sadowska"] {
        assert_eq!(in_archive(&mut api, name).len(), 1, "{name} is not doubled");
    }
    let jan = &in_archive(&mut api, "Jan Sadowski")[0];
    let profile = run(&mut api, "person.get", json!({ "id": jan["id"] }));
    let facts = profile["facts"].to_string();
    assert!(facts.contains("20 czerwca 1908"), "the exact birth date from Lens: {facts}");
    assert!(facts.contains("górnik"), "{facts}");
    let relations = run(&mut api, "person.relations", json!({ "id": jan["id"] })).to_string();
    for name in ["Anna Sadowska", "Tadeusz Sadowski", "Stanisław Sadowski", "Józefa Sadowska"] {
        assert!(relations.contains(name), "{name} in {relations}");
    }
}
