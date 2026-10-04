use crate::Api;
use heirloom_core::Archive;
use heirloom_core::gedcom::Document;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Kowalscy: a Heirloom archive (GEDCOM 7, with `.heirloom/`).
const KOWALSCY: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Jan /Kowalski/\n1 SEX M\n1 UID u-jan\n1 BIRT\n2 DATE 12 MAR 1878\n2 PLAC Wólka\n1 FAMC @F1@\n1 FAMS @F2@\n1 OBJE @O1@\n\
0 @I2@ INDI\n1 NAME Antoni /Kowalski/\n1 SEX M\n1 UID u-antoni\n1 BIRT\n2 DATE 1850\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Agnieszka /Mazur/\n1 SEX F\n1 FAMS @F1@\n\
0 @I4@ INDI\n1 NAME Anna /Kowalska/\n1 SEX F\n1 FAMS @F2@\n\
0 @I5@ INDI\n1 NAME Piotr /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1905\n1 FAMC @F2@\n\
0 @F1@ FAM\n1 HUSB @I2@\n1 WIFE @I3@\n1 CHIL @I1@\n\
0 @F2@ FAM\n1 HUSB @I1@\n1 WIFE @I4@\n1 CHIL @I5@\n\
0 @O1@ OBJE\n1 FILE media/jan.png\n2 FORM image/png\n3 MEDI PHOTO\n\
0 @N1@ SNOTE Wspomnienie o [dziadku](person:u-jan).\n0 TRLR\n";

/// Nowakowie: another program's GEDCOM 5.5.1 file, no `.heirloom/`. The same Jan (and his parents), recorded again.
const NOWAKOWIE: &str = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n\
0 @I10@ INDI\n1 NAME Jan /Kowalski/\n1 SEX M\n1 BIRT\n2 DATE 1878\n2 PLAC Wólka Dolna\n1 OCCU kowal\n1 FAMC @F10@\n1 FAMS @F11@\n\
0 @I11@ INDI\n1 NAME Antoni /Kowalski/\n1 SEX M\n1 FAMS @F10@\n\
0 @I12@ INDI\n1 NAME Agnieszka /Kowalska/\n1 SEX F\n1 FAMS @F10@\n\
0 @I13@ INDI\n1 NAME Zofia /Nowak/\n1 SEX F\n1 FAMS @F11@\n\
0 @I14@ INDI\n1 NAME Józef /Nowak/\n1 SEX M\n1 BIRT\n2 DATE 1901\n\
0 @I15@ INDI\n1 NAME Janina /Kowalska/\n1 SEX F\n1 BIRT\n2 DATE 1878\n\
0 @F10@ FAM\n1 HUSB @I11@\n1 WIFE @I12@\n1 CHIL @I10@\n\
0 @F11@ FAM\n1 HUSB @I10@\n1 WIFE @I13@\n\
0 @N1@ NOTE Spisane z opowieści [Jana](person:@I10@).\n0 TRLR\n";

struct Family {
    _dir: tempfile::TempDir,
    root: PathBuf,
    kowalscy: PathBuf,
    nowakowie: PathBuf,
    set: PathBuf,
}

fn family() -> Family {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Rodzina");
    let kowalscy = root.join("Kowalscy");
    Archive::create_from_document(&kowalscy, "Kowalscy", Document::from_bytes(KOWALSCY.as_bytes()).0).unwrap();
    image::RgbImage::from_pixel(300, 200, image::Rgb([10, 20, 30])).save(kowalscy.join("media/jan.png")).unwrap();
    let folder = dir.path().join("Inne").join("Nowakowie z Ciechanek");
    std::fs::create_dir_all(&folder).unwrap();
    let nowakowie = folder.join("drzewo.ged");
    std::fs::write(&nowakowie, NOWAKOWIE).unwrap();
    let set = root.join("Rodzina razem.heirloom-zestaw");
    Family { root, kowalscy, nowakowie, set, _dir: dir }
}

fn api() -> Api {
    Api::new(None, None)
}

fn open_set(api: &mut Api, f: &Family) -> Value {
    let archives = [f.kowalscy.display().to_string(), f.nowakowie.display().to_string()];
    api.call("set.create", json!({ "path": f.set.to_str().unwrap(), "name": "Rodzina razem", "archives": archives })).unwrap()
}

/// Every file of a folder with a hash of its contents.
fn snapshot(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push((path.clone(), blake3::hash(&std::fs::read(&path).unwrap()).to_hex().to_string()));
            }
        }
    }
    out.sort();
    out
}

fn ids(list: &Value) -> Vec<String> {
    list["people"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect()
}

fn decide(api: &mut Api, a: &str, b: &str, answer: &str) -> Value {
    api.call("set.decide", json!({ "a": a, "b": b, "answer": answer, "by": "Ewa" })).unwrap()
}

#[test]
fn two_archives_open_as_one_and_stay_untouched() {
    let f = family();
    let before = (snapshot(&f.kowalscy), snapshot(f.nowakowie.parent().unwrap()));
    let mut api = api();
    let status = open_set(&mut api, &f);
    assert_eq!((status["readOnly"].as_bool(), status["people"].as_u64(), status["name"].as_str()), (Some(true), Some(11), Some("Rodzina razem")));
    let archives = &status["combined"]["archives"];
    assert_eq!((archives[0]["key"].as_str(), archives[1]["key"].as_str(), archives[1]["state"].as_str()), (Some("a"), Some("b"), Some("ok")));
    let list = api.call("people.list", Value::Null).unwrap();
    let all = ids(&list);
    assert!(all.contains(&"@a~I1@".to_string()) && all.contains(&"@b~I10@".to_string()));
    let jan = list["people"].as_array().unwrap().iter().find(|p| p["id"] == "@b~I10@").unwrap();
    assert_eq!(jan["from"], json!(["b"]));
    let jan_a = api.call("person.get", json!({ "id": "@a~I1@" })).unwrap();
    assert_eq!(jan_a["person"]["photo"], "~a/media/jan.png");
    assert_eq!(jan_a["uid"], "u-jan");
    assert_eq!(jan_a["combined"]["members"][0]["id"], "@I1@", "„Edytuj w jego archiwum” opens the record in its archive");
    assert_eq!(jan_a["combined"]["members"][0]["path"], f.kowalscy.display().to_string());
    assert_eq!(api.call("tree.overview", Value::Null).unwrap()["people"][0][14], json!(["a"]), "Całe drzewo: the archive last");
    let start = api.call("start.data", Value::Null).unwrap();
    assert_eq!((start["title"].as_str(), start["stats"]["people"].as_u64()), (Some("Rodzina razem"), Some(11)));
    assert_eq!(api.call("people.search", json!({ "q": "jozef nowak" })).unwrap()[0]["id"], "@b~I14@");
    // The photo is served from its archive by the key.
    let media = api.call("media.list", Value::Null).unwrap();
    assert_eq!((media["items"][0]["path"].as_str(), media["items"][0]["missing"].as_bool()), (Some("~a/media/jan.png"), Some(false)));
    assert!(matches!(crate::media::serve(&api.media_roots(), "thumb/128/~a/media/jan.png"), crate::media::Served::Ok { .. }));
    // A mention in another program's 5.5.1 note leads to the person.
    let stories = api.call("person.get", json!({ "id": "@b~I10@" })).unwrap();
    assert!(stories["brokenLinks"].as_array().unwrap().is_empty());

    // Linking, unlinking and a refresh write only the set file.
    let pairs = api.call("set.pairs", Value::Null).unwrap();
    assert!(pairs["pairs"].as_array().unwrap().iter().any(|p| p["a"] == "@a~I1@" && p["b"] == "@b~I10@"), "{pairs}");
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    api.call("set.unlink", json!({ "id": "@b~I10@" })).unwrap();
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    api.call("set.refresh", Value::Null).unwrap();
    api.call("archive.close", Value::Null).unwrap();
    assert_eq!((snapshot(&f.kowalscy), snapshot(f.nowakowie.parent().unwrap())), before, "the archives are byte for byte as they were");
    assert!(!f.nowakowie.parent().unwrap().join(".heirloom").exists(), "another program's folder gets no .heirloom");

    // An archive opened on its own shows nothing of the set.
    api.call("archive.open", json!({ "path": f.kowalscy.to_str().unwrap() })).unwrap();
    let alone = api.call("people.list", Value::Null).unwrap();
    assert!(alone["people"][0].get("from").is_none() && alone["people"][0]["id"].as_str().unwrap().starts_with("@I"));
    assert_eq!(api.call("person.get", json!({ "id": "@I1@" })).unwrap()["person"]["photo"], "media/jan.png");
    assert_eq!(api.call("tree.overview", Value::Null).unwrap()["people"][0].as_array().unwrap().len(), 14);
}

#[test]
fn nothing_that_could_change_an_archive_runs_in_the_combined_view() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    for method in [
        "archive.save", "archive.undo", "archive.redo", "archive.setSettings", "archive.saveAsCopy", "archive.saveAsNew", "archive.backup",
        "archive.exportGedzip", "archive.check", "archive.setEditor", "archive.firstOpen", "settings.import", "settings.export", "person.create",
        "person.update", "person.delete", "person.editData", "relation.add", "family.update", "text.save", "media.add", "media.findMissing",
        "source.save", "citation.add", "place.rename", "import.load", "import.commit", "history.feed", "history.undo",
    ] {
        let error = api.call(method, json!({ "id": "@a~I1@", "author": "Ewa", "path": "/tmp/x" })).unwrap_err();
        assert_eq!(error.code, "combined_read_only", "{method}");
    }
    for method in ["archive.status", "people.list", "surnames.list", "places.list", "stories.list", "sources.list", "media.missing", "app.state"] {
        api.call(method, Value::Null).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    }
    for method in ["person.panel", "person.relations", "person.hover", "tree.graph"] {
        api.call(method, json!({ "id": "@b~I10@" })).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    }
    // Opening an archive leaves the set: editing works there again.
    api.call("archive.open", json!({ "path": f.kowalscy.to_str().unwrap() })).unwrap();
    assert_eq!(api.call("archive.status", Value::Null).unwrap()["readOnly"], false);
    api.call("person.create", json!({ "given": "Ewa" })).unwrap();
}

#[test]
fn linked_people_are_one_person_with_both_records() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    let answer = decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    assert_eq!((answer["status"]["people"].as_u64(), answer["status"]["combined"]["linked"].as_u64()), (Some(10), Some(1)));
    let jan = api.call("person.get", json!({ "id": "@b~I10@" })).unwrap();
    assert_eq!(jan["person"]["id"], "@a~I1@", "the other record's id leads to the person");
    assert_eq!(jan["person"]["from"], json!(["a", "b"]));
    assert_eq!(jan["person"]["birth"]["text"], "12 marca 1878", "lists show the first archive's version");
    let members: Vec<(&str, &str)> = jan["combined"]["members"].as_array().unwrap().iter().map(|m| (m["archive"].as_str().unwrap(), m["id"].as_str().unwrap())).collect();
    assert_eq!(members, [("a", "@I1@"), ("b", "@I10@")]);
    let differences = &jan["combined"]["differences"];
    assert_eq!(differences[0]["label"], "Urodzenie");
    assert_eq!(differences[0]["values"][1]["text"], "1878, Wólka Dolna");
    assert!(jan.to_string().contains("kowal"), "the other archive's facts are there too");
    // Both parent couples and both wives show; Piotr and the new family are one tree.
    let panel = api.call("person.relations", json!({ "id": "@a~I1@" })).unwrap().to_string();
    assert!(panel.contains("@a~I2@") && panel.contains("@b~I11@") && panel.contains("@a~I4@") && panel.contains("@b~I13@"), "{panel}");
    let graph = api.call("tree.graph", json!({ "id": "@b~I10@", "up": 1, "down": 1 })).unwrap();
    assert!(graph["people"].get("@a~I5@").is_some() && graph["people"].get("@b~I13@").is_some());
    // A mention in archive b of Jan points at the person.
    let note = api.call("stories.list", Value::Null).unwrap().to_string();
    assert!(!note.contains("person:!"), "{note}");
    // Linking his parents too joins the parents' families: one family, Jan once among the children.
    decide(&mut api, "@a~I2@", "@b~I11@", "yes");
    decide(&mut api, "@a~I3@", "@b~I12@", "yes");
    let jan = api.call("person.panel", json!({ "id": "@a~I1@" })).unwrap().to_string();
    assert_eq!(jan.matches("@b~I11@").count(), 0, "{jan}");
    let antoni = api.call("person.relations", json!({ "id": "@b~I11@" })).unwrap().to_string();
    assert_eq!(antoni.matches("\"@a~I1@\"").count(), 1, "{antoni}");
    // Two people of one archive are never one.
    let error = api.call("set.decide", json!({ "a": "@a~I4@", "b": "@a~I5@", "answer": "yes" })).unwrap_err();
    assert_eq!(error.code, "same_archive");
    let error = api.call("set.decide", json!({ "a": "@a~I5@", "b": "@b~I10@", "answer": "yes" })).unwrap_err();
    assert_eq!(error.code, "same_archive", "Jan of b is already one with Jan of a");
    // „Rozłącz”: Jan of b is his own person again.
    let status = api.call("set.unlink", json!({ "id": "@b~I10@" })).unwrap();
    assert_eq!(status["combined"]["linked"], 2);
    assert_eq!(api.call("person.get", json!({ "id": "@b~I10@" })).unwrap()["person"]["id"], "@b~I10@");
    assert_eq!(api.call("set.unlink", json!({ "id": "@b~I14@" })).unwrap_err().code, "not_linked");
}

#[test]
fn the_same_person_is_suggested_and_a_no_is_remembered() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    let pairs = api.call("set.pairs", Value::Null).unwrap();
    let list = pairs["pairs"].as_array().unwrap();
    let jan = list.iter().find(|p| p["a"] == "@a~I1@").unwrap();
    assert_eq!((jan["b"].as_str(), jan["status"].as_str()), (Some("@b~I10@"), Some("match")));
    assert!(jan["score"].as_f64().unwrap() >= 8.0 && jan["reasons"].to_string().contains("imię ojca zgodne"), "{jan}");
    assert_eq!(jan["sides"][1]["parents"].as_array().unwrap().len(), 2);
    assert!(list.iter().all(|p| p["b"] != "@b~I15@" && p["a"] != "@b~I15@"), "Janina is a woman: vetoed");
    assert!(list.iter().all(|p| p["a"].as_str().unwrap().starts_with("@a~") && p["b"].as_str().unwrap().starts_with("@b~")), "never two of one archive");
    // Each person in one pair at most.
    let mut seen = std::collections::HashSet::new();
    assert!(list.iter().all(|p| seen.insert(p["a"].to_string()) && seen.insert(p["b"].to_string())));

    // After Jan: his father comes up with the family's support.
    let antoni_before = list.iter().find(|p| p["a"] == "@a~I2@").map(|p| p["score"].as_f64().unwrap());
    let answer = decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    let antoni = answer["pairs"].as_array().unwrap().iter().find(|p| p["a"] == "@a~I2@").unwrap().clone();
    assert!(antoni["reasons"].to_string().contains("rodzic połączonej osoby"), "{antoni}");
    assert!(antoni_before.is_none_or(|s| antoni["score"].as_f64().unwrap() > s));
    assert!(answer["pairs"].as_array().unwrap().iter().all(|p| p["a"] != "@a~I1@"), "a decided pair isn't asked again");
    // „Nie”: not offered again, also after a new search.
    decide(&mut api, "@a~I2@", "@b~I11@", "no");
    let again = api.call("set.pairs", json!({ "refresh": true })).unwrap();
    assert!(again["pairs"].as_array().unwrap().iter().all(|p| !(p["a"] == "@a~I2@" && p["b"] == "@b~I11@")), "{again}");
    let file: Value = serde_json::from_slice(&std::fs::read(&f.set).unwrap()).unwrap();
    assert_eq!((file["links"].as_array().unwrap().len(), file["rejected"].as_array().unwrap().len()), (1, 1));
    assert_eq!((file["links"][0]["by"].as_str(), file["links"][0]["b"]["xref"].as_str(), file["links"][0]["a"]["uid"].as_str()), (Some("Ewa"), Some("@I10@"), Some("u-jan")));
    // „Połącz z osobą z innego archiwum…”: any two people, side by side.
    let compared = api.call("set.compare", json!({ "a": "@a~I3@", "b": "@b~I12@" })).unwrap();
    assert!(compared["percent"].as_u64().unwrap() > 0 && compared["reasons"].to_string().contains("imię zgodne"), "{compared}");
    assert_eq!(api.call("set.compare", json!({ "a": "@a~I3@", "b": "@a~I4@" })).unwrap()["sameArchive"], true);
}

#[test]
fn copies_of_one_archive_pair_by_uid_and_link_in_one_go() {
    let dir = tempfile::tempdir().unwrap();
    let options = heirloom_gen::Options { people: 60, media: 0, seed: 9, name: "Kopia".into() };
    let first = dir.path().join("pierwsza");
    heirloom_gen::write_archive(&first, &options).unwrap();
    let second = dir.path().join("druga");
    std::fs::create_dir_all(&second).unwrap();
    std::fs::copy(first.join("rodzina.ged"), second.join("rodzina.ged")).unwrap();
    let mut api = api();
    let set = dir.path().join("Kopie");
    api.call("set.create", json!({ "path": set.to_str().unwrap(), "name": "Kopie", "archives": [first.to_str().unwrap(), second.to_str().unwrap()] })).unwrap();
    assert!(dir.path().join("Kopie.heirloom-zestaw").is_file(), "the extension is added");
    let pairs = api.call("set.pairs", Value::Null).unwrap();
    let with_uid = pairs["pairs"].as_array().unwrap().iter().filter(|p| p["reasons"][0] == super::pairs::SAME_UID).count();
    let uids = Document::from_bytes(&std::fs::read(first.join("rodzina.ged")).unwrap()).0.records.iter().filter(|r| r.child("UID").is_some()).count();
    assert!(uids == 0 || with_uid > 0);
    let certain = pairs["certain"].as_u64().unwrap();
    assert!(certain > 30, "{certain} of 60");
    let linked = api.call("set.linkAll", json!({ "minPercent": 90, "by": "Ewa" })).unwrap();
    assert_eq!(linked["linked"], certain);
    assert_eq!(linked["status"]["people"].as_u64(), Some(120 - certain));
    // Nobody is two people of one archive.
    let list = api.call("people.list", Value::Null).unwrap();
    assert!(list["people"].as_array().unwrap().iter().all(|p| p["from"].as_array().unwrap().len() <= 2));
}

#[test]
fn the_set_file_keeps_relative_paths_and_others_decisions() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    let file: Value = serde_json::from_slice(&std::fs::read(&f.set).unwrap()).unwrap();
    assert_eq!((file["format"].as_str(), file["version"].as_u64()), (Some("heirloom-zestaw"), Some(1)));
    assert_eq!(file["archives"][0]["path"], "Kowalscy");
    assert_eq!(file["archives"][1]["path"], "../Inne/Nowakowie z Ciechanek/drzewo.ged");
    assert_eq!(file["archives"][1]["id"], "", "a GEDCOM file Heirloom never saved has no lasting id");
    assert!(!file["archives"][0]["id"].as_str().unwrap().is_empty());

    // Another window decides meanwhile: its decision stays when this one decides.
    let mut other = Api::new(None, None);
    other.call("set.open", json!({ "path": f.set.to_str().unwrap() })).unwrap();
    decide(&mut other, "@a~I2@", "@b~I11@", "yes");
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    let file: Value = serde_json::from_slice(&std::fs::read(&f.set).unwrap()).unwrap();
    assert_eq!(file["links"].as_array().unwrap().len(), 2);
    assert_eq!(api.call("set.refresh", Value::Null).unwrap()["status"]["combined"]["linked"], 2, "the other window's link shows after a refresh");

    // The whole family folder moved: the relative paths still find both archives.
    api.call("archive.close", Value::Null).unwrap();
    let moved = f._dir.path().join("Przeniesione");
    std::fs::create_dir_all(&moved).unwrap();
    std::fs::rename(&f.root, moved.join("Rodzina")).unwrap();
    std::fs::rename(f._dir.path().join("Inne"), moved.join("Inne")).unwrap();
    let status = api.open_path(moved.join("Rodzina").join("Rodzina razem.heirloom-zestaw").to_str().unwrap()).unwrap();
    let states: Vec<&str> = status["combined"]["archives"].as_array().unwrap().iter().map(|a| a["state"].as_str().unwrap()).collect();
    assert_eq!(states, ["ok", "ok"]);
    assert_eq!((status["combined"]["linked"].as_u64(), status["people"].as_u64()), (Some(2), Some(9)));
}

#[test]
fn a_set_file_is_never_replaced_unasked() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    let saved = std::fs::read(&f.set).unwrap();
    api.call("archive.close", Value::Null).unwrap();
    let archives = [f.kowalscy.display().to_string(), f.nowakowie.display().to_string()];
    for path in [f.set.clone(), f.root.join("Rodzina razem")] {
        let err = api.call("set.create", json!({ "path": path.to_str().unwrap(), "name": "Nowy", "archives": archives })).unwrap_err();
        assert_eq!(err.code, "set_exists");
    }
    assert_eq!(std::fs::read(&f.set).unwrap(), saved, "the decisions are kept");
    // Chosen in the save dialog, which asked before replacing it.
    let status = api.call("set.create", json!({ "path": f.set.to_str().unwrap(), "name": "Nowy", "archives": archives, "replace": true })).unwrap();
    assert_eq!((status["name"].as_str(), status["combined"]["linked"].as_u64()), (Some("Nowy"), Some(0)));
}

#[test]
fn a_key_taken_over_in_another_window_gets_none_of_this_windows_decisions() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    let third = f._dir.path().join("Trzecie");
    std::fs::create_dir_all(&third).unwrap();
    std::fs::write(third.join("drzewo.ged"), "0 HEAD\n1 GEDC\n2 VERS 5.5.1\n0 @I10@ INDI\n1 NAME Zofia /Wrona/\n1 SEX F\n0 TRLR\n").unwrap();
    // Another window takes Nowakowie out and adds another archive, which gets the free key `b`.
    let mut other = Api::new(None, None);
    other.call("set.open", json!({ "path": f.set.to_str().unwrap() })).unwrap();
    other.call("set.removeArchive", json!({ "key": "b" })).unwrap();
    let added = other.call("set.addArchive", json!({ "path": third.join("drzewo.ged").to_str().unwrap() })).unwrap();
    assert_eq!(added["combined"]["archives"][1]["key"], "b");
    // This window still shows Nowakowie as `b`: its „tak” for Jan must not link Zofia.
    let err = api.call("set.decide", json!({ "a": "@a~I1@", "b": "@b~I10@", "answer": "yes", "by": "Ewa" })).unwrap_err();
    assert_eq!(err.code, "set_changed");
    let file: Value = serde_json::from_slice(&std::fs::read(&f.set).unwrap()).unwrap();
    assert!(file["links"].as_array().unwrap().is_empty());
    // The view was read again: `b` is the new archive now.
    let status = api.call("archive.status", Value::Null).unwrap();
    let path = PathBuf::from(status["combined"]["archives"][1]["path"].as_str().unwrap());
    assert_eq!(path.canonicalize().unwrap(), third.join("drzewo.ged").canonicalize().unwrap());
    assert_eq!(api.call("person.get", json!({ "id": "@b~I10@" })).unwrap()["person"]["name"], "Zofia Wrona");
}

#[test]
fn a_missing_archive_opens_with_the_rest_and_can_be_found_again() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    api.call("archive.close", Value::Null).unwrap();
    let elsewhere = f._dir.path().join("gdzie-indziej.ged");
    std::fs::rename(&f.nowakowie, &elsewhere).unwrap();
    let status = api.call("set.open", json!({ "path": f.set.to_str().unwrap() })).unwrap();
    assert_eq!((status["combined"]["archives"][1]["state"].as_str(), status["people"].as_u64()), (Some("missing"), Some(5)));
    assert!(status["warnings"][0]["message"].as_str().unwrap().contains("Nie znaleziono archiwum"));
    assert_eq!(status["combined"]["lost"], 0, "the link waits for its archive");
    // „Wskaż folder…”.
    let status = api.call("set.locate", json!({ "key": "b", "path": elsewhere.to_str().unwrap() })).unwrap();
    assert_eq!((status["combined"]["archives"][1]["state"].as_str(), status["people"].as_u64(), status["different"].as_bool()), (Some("ok"), Some(10), Some(false)));
    let file: Value = serde_json::from_slice(&std::fs::read(&f.set).unwrap()).unwrap();
    assert_eq!(file["archives"][1]["path"], "../gdzie-indziej.ged");
    // None left at all: an error, not an empty view.
    api.call("archive.close", Value::Null).unwrap();
    std::fs::rename(&elsewhere, f._dir.path().join("x.ged")).unwrap();
    std::fs::rename(&f.kowalscy, f._dir.path().join("Kowalscy-stare")).unwrap();
    assert_eq!(api.call("set.open", json!({ "path": f.set.to_str().unwrap() })).unwrap_err().code, "set_empty");
}

#[test]
fn a_renumbered_record_is_a_lost_link_and_a_refresh_sees_other_windows_saves() {
    let f = family();
    let mut api = api();
    open_set(&mut api, &f);
    decide(&mut api, "@a~I1@", "@b~I10@", "yes");
    decide(&mut api, "@a~I2@", "@b~I11@", "yes");
    // Another window (or program) saves archive a: Jan gets another record id (his UID finds him), Antoni's record
    // id now names someone else (another UID): that link is lost. And there is a new person.
    let text = KOWALSCY.replace("@I1@", "@I99@").replace("1 UID u-antoni", "1 UID inny").replace("0 TRLR", "0 @I50@ INDI\n1 NAME Ewa /Kowalska/\n0 TRLR");
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(f.kowalscy.join("rodzina.ged"), text).unwrap();
    assert_eq!(api.call("archive.status", Value::Null).unwrap()["changedOnDisk"], true);
    let refreshed = api.call("set.refresh", Value::Null).unwrap();
    assert_eq!(refreshed["changed"], json!(["Kowalscy"]));
    assert!(ids(&api.call("people.list", Value::Null).unwrap()).contains(&"@a~I50@".to_string()));
    assert_eq!(api.call("person.get", json!({ "id": "@b~I10@" })).unwrap()["person"]["id"], "@a~I99@");
    let pairs = api.call("set.pairs", Value::Null).unwrap();
    // The lost one: the set file still names it; „Usuń z zestawu” takes it out.
    assert_eq!((pairs["lost"].as_array().unwrap().len(), pairs["lost"][0]["a"]["xref"].as_str()), (1, Some("@I2@")));
    let status = api.call("set.forgetLost", json!({ "index": 0 })).unwrap();
    assert_eq!((status["combined"]["lost"].as_u64(), status["combined"]["linked"].as_u64()), (Some(0), Some(1)));
}

#[test]
fn the_recent_list_shows_sets_and_a_new_window_is_told_what_to_open() {
    let f = family();
    let config = f._dir.path().join("config");
    let mut api = Api::new(Some(config.clone()), None);
    open_set(&mut api, &f);
    let state = api.call("app.state", Value::Null).unwrap();
    assert_eq!((state["recent"][0]["kind"].as_str(), state["recent"][0]["archives"].as_u64()), (Some("set"), Some(2)));
    assert_eq!(state["archive"]["combined"]["archives"].as_array().unwrap().len(), 2);
    // A second window (another process, the same aplikacja.json) opens an archive: both stay in the list.
    let mut second = Api::new(Some(config.clone()), None);
    second.set_start(f.kowalscy.display().to_string(), Some("@I1@".into()));
    let start = second.call("app.takeStart", Value::Null).unwrap();
    assert_eq!((start["path"].as_str(), start["person"].as_str()), (Some(f.kowalscy.to_str().unwrap()), Some("@I1@")));
    assert!(second.call("app.takeStart", Value::Null).unwrap().is_null(), "given once");
    second.call("archive.open", json!({ "path": f.kowalscy.to_str().unwrap() })).unwrap();
    // The first window remembers a place; the second's archive stays in the list.
    let set_id = state["archive"]["archiveId"].as_str().unwrap().to_string();
    api.call("app.setPlace", json!({ "archiveId": set_id, "route": { "name": "people" } })).unwrap();
    let recent = crate::config::AppConfig::load(&config).recent;
    let kinds: Vec<&str> = recent.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds, ["archive", "set"]);
    // Reopened later from the list.
    let mut later = Api::new(Some(config), None);
    let path = later.call("app.state", Value::Null).unwrap()["recent"][1]["path"].as_str().unwrap().to_string();
    later.open_path(&path).unwrap();
    assert_eq!(later.call("app.state", Value::Null).unwrap()["place"]["route"]["name"], "people");
}

/// Two 10,000-person archives drawn from the same names (design §8: open ≤ 3 s, pairs ≤ 3 s, the list ≤ 0.5 s).
/// `cargo test --release -p heirloom-api big_sets -- --ignored --nocapture`
#[test]
#[ignore]
fn big_sets_open_and_pair_in_time() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a");
    heirloom_gen::write_archive(&a, &heirloom_gen::Options { people: 10_000, media: 0, seed: 1, name: "A".into() }).unwrap();
    let b = dir.path().join("b");
    heirloom_gen::write_archive(&b, &heirloom_gen::Options { people: 10_000, media: 0, seed: 2, name: "B".into() }).unwrap();
    let mut api = api();
    let t = std::time::Instant::now();
    api.call("set.create", json!({ "path": dir.path().join("Duzy").to_str().unwrap(), "name": "Duży", "archives": [a.to_str().unwrap(), b.to_str().unwrap()] })).unwrap();
    api.call("people.list", Value::Null).unwrap();
    let open = t.elapsed();
    let t = std::time::Instant::now();
    let pairs = api.call("set.pairs", Value::Null).unwrap();
    let pairing = t.elapsed();
    let t = std::time::Instant::now();
    api.call("people.list", Value::Null).unwrap();
    let list = t.elapsed();
    // A decision rebuilds the view on the next read, not at once.
    let first = &pairs["pairs"][0];
    let t = std::time::Instant::now();
    decide(&mut api, first["a"].as_str().unwrap(), first["b"].as_str().unwrap(), "yes");
    let deciding = t.elapsed();
    let t = std::time::Instant::now();
    api.call("people.list", Value::Null).unwrap();
    let rebuild = t.elapsed();
    let peak = std::fs::read_to_string("/proc/self/status").ok().and_then(|s| s.lines().find(|l| l.starts_with("VmHWM")).map(str::to_string));
    println!("open {open:?}, pairs {pairing:?} ({} found), list {list:?}, decide {deciding:?}, rebuild {rebuild:?}, {peak:?}", pairs["pairs"].as_array().unwrap().len());
    assert!(open.as_secs_f64() <= 3.0 && pairing.as_secs_f64() <= 3.0 && list.as_secs_f64() <= 0.5 && deciding.as_secs_f64() <= 0.5);
}
