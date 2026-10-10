//! Where photos lie in `media/` (design 0.7.0, part B): the scan, owners and folders of people, the „moved” map, the
//! journaled engine that switches the layout, and where new files go. Every test that moves files compares the
//! number and the hash of every file before and after: a lost or overwritten photo is the worst possible bug.

use heirloom_api::layout::engine;
use heirloom_api::media::{self, Served};
use heirloom_api::Api;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const IMPORT_FORMAT: &str = include_str!("../../../docs/IMPORT_FORMAT.md");

fn run(api: &mut Api, method: &str, args: Value) -> Value {
    api.call(method, args.clone()).unwrap_or_else(|e| panic!("{method} {args}: {} ({})", e.message, e.code))
}

fn code(api: &mut Api, method: &str, args: Value) -> String {
    match api.call(method, args) {
        Ok(v) => panic!("{method} should fail, gave {v}"),
        Err(e) => e.code.to_string(),
    }
}

fn path(p: &Path) -> String {
    p.display().to_string()
}

fn computer(dir: &Path, name: &str) -> Api {
    let mut api = Api::new(Some(dir.join(name).join("ustawienia")), None);
    api.set_home(dir.join(name).join("dom"));
    api
}

/// A file with its own contents (the contents are the name, so every file has a hash of its own).
fn put(root: &Path, rel: &str, text: &str) -> PathBuf {
    let file = rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part));
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, format!("zdjecie:{text}")).unwrap();
    file
}

/// Every file under `media/` (also the ones nothing references): relative path → hash.
fn tree(root: &Path) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let media = root.join("media");
    let mut stack = vec![media.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
                out.insert(rel, blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string());
            }
        }
    }
    out
}

fn folders_of(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root.join("media")).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

fn multiset(tree: &BTreeMap<String, String>) -> Vec<String> {
    let mut v: Vec<String> = tree.values().cloned().collect();
    v.sort();
    v
}

/// Object id → (FILE path, hash of the file it names). Panics when a file is missing.
fn objects(api: &mut Api) -> BTreeMap<String, (String, String)> {
    let list = run(api, "media.list", Value::Null);
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            assert_eq!(m["missing"], false, "{} is missing", m["path"]);
            let hash = blake3::hash(&fs::read(m["absolute"].as_str().unwrap()).unwrap()).to_hex().to_string();
            (m["id"].as_str().unwrap().to_string(), (m["path"].as_str().unwrap().to_string(), hash))
        })
        .collect()
}

fn hashes(api: &mut Api) -> BTreeMap<String, String> {
    objects(api).into_iter().map(|(id, (_, h))| (id, h)).collect()
}

fn paths(api: &mut Api) -> BTreeMap<String, String> {
    objects(api).into_iter().map(|(id, (p, _))| (id, p)).collect()
}

/// The object record whose FILE is `file`.
fn object_at(api: &mut Api, file: &str) -> String {
    let list = run(api, "media.list", Value::Null);
    list["items"].as_array().unwrap().iter().find(|m| m["path"] == file).unwrap_or_else(|| panic!("no object for {file}"))["id"].as_str().unwrap().to_string()
}

fn person(api: &mut Api, given: &str, surname: &str, birth: Option<&str>) -> String {
    let mut args = json!({ "given": given, "surname": surname });
    if let Some(b) = birth {
        args["events"] = json!({ "birth": { "date": b } });
    }
    run(api, "person.create", args)["id"].as_str().unwrap().to_string()
}

/// A file already inside the archive becomes an object record (and is linked to the person): referenced where it is.
fn add(api: &mut Api, root: &Path, rel: &str, person: Option<&str>) -> String {
    let file = put(root, rel, rel);
    let mut args = json!({ "paths": [path(&file)] });
    if let Some(p) = person {
        args["person"] = json!(p);
    }
    run(api, "media.add", args)[0]["id"].as_str().unwrap().to_string()
}

fn save(api: &mut Api) {
    run(api, "archive.save", json!({ "author": "Ewa" }));
}

fn scan(api: &mut Api) -> Value {
    run(api, "media.scan", Value::Null)
}

fn unknown_paths(scan: &Value) -> Vec<String> {
    scan["unknown"].as_array().unwrap().iter().map(|u| u["path"].as_str().unwrap().to_string()).collect()
}

fn unknown_row<'a>(scan: &'a Value, path: &str) -> &'a Value {
    scan["unknown"].as_array().unwrap().iter().find(|u| u["path"] == path).unwrap_or_else(|| panic!("{path} not in {:?}", unknown_paths(scan)))
}

/// A flat archive of the Kowalski family: Jan (1878), Anna, Józef (1905) and Maria; nothing unsaved.
struct Family {
    dir: tempfile::TempDir,
    api: Api,
    root: PathBuf,
    jan: String,
    anna: String,
    jozef: String,
    maria: String,
}

fn family() -> Family {
    let dir = tempfile::tempdir().unwrap();
    let mut api = computer(dir.path(), "ten");
    let root = dir.path().join("Rodzina Kowalskich");
    run(&mut api, "archive.create", json!({ "folder": path(&root), "name": "Rodzina Kowalskich" }));
    let jan = person(&mut api, "Jan", "Kowalski", Some("1878"));
    let anna = person(&mut api, "Anna", "Kowalska", None);
    let jozef = person(&mut api, "Józef", "Kowalski", Some("1905"));
    let maria = person(&mut api, "Maria", "Nowak", None);
    add(&mut api, &root, "media/jan1.jpg", Some(&jan));
    add(&mut api, &root, "media/jan2.jpg", Some(&jan));
    let anna_photo = add(&mut api, &root, "media/anna.jpg", Some(&anna));
    run(&mut api, "media.setProfile", json!({ "id": anna_photo, "person": anna }));
    let group = add(&mut api, &root, "media/grupa.jpg", Some(&jan));
    run(&mut api, "media.link", json!({ "id": group, "person": anna }));
    add(&mut api, &root, "media/jozef.jpg", Some(&jozef));
    add(&mut api, &root, "media/skan.jpg", None);
    save(&mut api);
    Family { dir, api, root, jan, anna, jozef, maria }
}

fn data_bytes(root: &Path) -> Vec<u8> {
    fs::read(root.join("rodzina.ged")).unwrap()
}

// ---------- detection ----------

#[test]
fn the_guess_follows_where_the_files_of_people_lie() {
    // Flat: everything in the media folder.
    let mut f = family();
    let report = scan(&mut f.api);
    assert_eq!(report["layout"]["guess"], "flat");
    assert_eq!(report["layout"]["setting"], Value::Null, "nothing was stored yet");
    assert_eq!(report["layout"]["personFolders"], 0);
    assert_eq!(report["layout"]["rootOwned"], 5);
    // The guess is stored, silently, in the archive's display.
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["display"]["mediaLayout"], "flat");
    assert_eq!(scan(&mut f.api)["layout"]["setting"], "flat");

    // Per person: every file in the folder of its owner.
    let dir = tempfile::tempdir().unwrap();
    let mut api = computer(dir.path(), "ten");
    let root = dir.path().join("A");
    run(&mut api, "archive.create", json!({ "folder": path(&root), "name": "A" }));
    let jan = person(&mut api, "Jan", "Kowalski", None);
    let anna = person(&mut api, "Anna", "Kowalska", None);
    for n in 1..=3 {
        add(&mut api, &root, &format!("media/Kowalski Jan/j{n}.jpg"), Some(&jan));
    }
    add(&mut api, &root, "media/Kowalska Anna/a1.jpg", Some(&anna));
    save(&mut api);
    let report = scan(&mut api);
    assert_eq!((report["layout"]["guess"].as_str(), report["layout"]["personFolders"].as_u64(), report["layout"]["folderOwned"].as_u64()), (Some("perPerson"), Some(2), Some(4)));

    // Mixed, more in the root: flat. Mixed, more in folders: per person.
    add(&mut api, &root, "media/l1.jpg", Some(&jan));
    save(&mut api);
    assert_eq!(scan(&mut api)["layout"]["guess"], "perPerson", "4 in folders, 1 loose");
    for n in 2..=5 {
        add(&mut api, &root, &format!("media/l{n}.jpg"), Some(&jan));
    }
    save(&mut api);
    let report = scan(&mut api);
    assert_eq!((report["layout"]["guess"].as_str(), report["layout"]["rootOwned"].as_u64()), (Some("flat"), Some(5)), "4 in folders, 5 loose");

    // An empty media folder counts as flat.
    let dir = tempfile::tempdir().unwrap();
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.create", json!({ "folder": path(&dir.path().join("B")), "name": "B" }));
    let report = scan(&mut api);
    assert_eq!(report["layout"]["guess"], "flat");
    assert_eq!(report["total"], 0);
    assert!(report["unknown"].as_array().unwrap().is_empty());
}

#[test]
fn the_owner_is_the_profile_photo_person_else_the_first_person_that_links_the_file() {
    let mut f = family();
    // The group photo links Jan first and Anna second: Jan owns it. Anna's profile photo makes her the owner of her photo.
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    let plan = run(&mut f.api, "media.restructurePreview", json!({ "to": "perPerson" }));
    let to_of = |name: &str| plan["moves"].as_array().unwrap().iter().find(|m| m["from"] == format!("media/{name}")).map(|m| m["to"].as_str().unwrap().to_string());
    assert_eq!(to_of("grupa.jpg").as_deref(), Some("media/Kowalski Jan/grupa.jpg"));
    assert_eq!(to_of("anna.jpg").as_deref(), Some("media/Kowalska Anna/anna.jpg"));
    assert_eq!(plan["groupFiles"], 1);
    // Making the group photo Anna's profile photo makes her its owner.
    let group = object_at(&mut f.api, "media/grupa.jpg");
    run(&mut f.api, "media.setProfile", json!({ "id": group, "person": f.anna }));
    let plan = run(&mut f.api, "media.restructurePreview", json!({ "to": "perPerson" }));
    assert_eq!(plan["moves"].as_array().unwrap().iter().find(|m| m["from"] == "media/grupa.jpg").unwrap()["to"], "media/Kowalska Anna/grupa.jpg");
    // A file linked to nobody stays in the media root: it is not in the plan at all.
    assert!(plan["moves"].as_array().unwrap().iter().all(|m| m["from"] != "media/skan.jpg"));
    assert_eq!(plan["unassigned"], 1);
}

// ---------- the folder of a person ----------

/// Where media.add puts a new file for the person: the folder, from the returned path.
fn new_file_folder(f: &mut Family, who: &str, name: &str) -> String {
    let who = match who {
        "jan" => f.jan.clone(),
        "anna" => f.anna.clone(),
        "jozef" => f.jozef.clone(),
        "maria" => f.maria.clone(),
        other => other.to_string(),
    };
    let who = who.as_str();
    let source = f.dir.path().join("nowe").join(name);
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, format!("nowy:{name}:{who}")).unwrap();
    let id = run(&mut f.api, "media.add", json!({ "paths": [path(&source)], "person": who }))[0]["id"].as_str().unwrap().to_string();
    let rel = run(&mut f.api, "media.get", json!({ "id": id }))["path"].as_str().unwrap().to_string();
    assert!(f.root.join(&rel).is_file(), "{rel} exists");
    rel.rsplit_once('/').unwrap().0.to_string()
}

#[test]
fn a_new_file_goes_to_the_folder_of_the_person() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    // (3) A new name, surname first.
    assert_eq!(new_file_folder(&mut f, "maria", "m1.jpg"), "media/Nowak Maria");
    // Next to it, the same folder again (the file is hers now: rule 1).
    assert_eq!(new_file_folder(&mut f, "maria", "m2.jpg"), "media/Nowak Maria");
    // Flat: always media/. A file without a person: media/ in both layouts.
    let source = f.dir.path().join("nowe").join("bez-osoby.jpg");
    fs::write(&source, "bez osoby").unwrap();
    let id = run(&mut f.api, "media.add", json!({ "paths": [path(&source)] }))[0]["id"].as_str().unwrap().to_string();
    assert_eq!(run(&mut f.api, "media.get", json!({ "id": id }))["path"], "media/bez-osoby.jpg");
    run(&mut f.api, "media.setLayout", json!({ "layout": "flat" }));
    assert_eq!(new_file_folder(&mut f, "jan", "j9.jpg"), "media");
}

#[test]
fn the_folder_that_holds_most_of_a_persons_files_wins() {
    let mut f = family();
    // Jan's files: two in „Dziadek”, one loose. A new file for him goes to „Dziadek”, not to „Kowalski Jan”.
    let root = f.root.clone();
    for (old, new) in [("media/jan1.jpg", "media/Dziadek/j1.jpg"), ("media/jan2.jpg", "media/Dziadek/j2.jpg")] {
        let id = object_at(&mut f.api, old);
        run(&mut f.api, "media.relink", json!({ "id": id, "path": path(&put(&root, new, new)) }));
    }
    save(&mut f.api);
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    assert_eq!(new_file_folder(&mut f, "jan", "j3.jpg"), "media/Dziadek");
}

#[test]
fn a_folder_named_after_the_person_is_adopted_unless_another_person_fits_it_too() {
    let mut f = family();
    fs::create_dir_all(f.root.join("media").join("Kowalska Anna")).unwrap();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    // Anna /Kowalska/ fits „Kowalska Anna”; so would Anna /Kowalski/, but she is not in the archive yet.
    assert_eq!(new_file_folder(&mut f, "anna", "a2.jpg"), "media/Kowalska Anna");
    // The same folder name, with the surname's other form: Kowalski/Kowalska are one surname.
    let anna2 = person(&mut f.api, "Anna", "Kowalski", None);
    // Now two people fit the folder (and Anna owns a file in it): neither Anna adopts it for a new file by name alone,
    // the first keeps it by owning a file there, the second gets a name of her own.
    let second = new_file_folder(&mut f, &anna2, "a3.jpg");
    assert_ne!(second, "media/Kowalska Anna");
    assert!(second.starts_with("media/Kowalski Anna"), "{second}");
}

#[test]
fn namesakes_get_the_year_of_birth_and_names_are_safe_for_windows() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    let j1 = person(&mut f.api, "Józef", "Kowalski", Some("1878"));
    let j2 = person(&mut f.api, "Józef", "Kowalski", Some("1950"));
    let j3 = person(&mut f.api, "Józef", "Kowalski", None);
    let a = new_file_folder(&mut f, &j1, "a.jpg");
    let b = new_file_folder(&mut f, &j2, "b.jpg");
    let c = new_file_folder(&mut f, &j3, "c.jpg");
    // The existing Józef (1905) already has „Kowalski Józef” through the profile of his own photo (rule 1).
    assert_eq!((a.as_str(), b.as_str(), c.as_str()), ("media/Kowalski Józef (1878)", "media/Kowalski Józef (1950)", "media/Kowalski Józef (2)"));
    // Forbidden characters, a device name and a very long name.
    let odd = person(&mut f.api, "Ja:n/?", "Co*n", None);
    assert_eq!(new_file_folder(&mut f, &odd, "d.jpg"), "media/Co n Ja n");
    let device = person(&mut f.api, "", "Con", None);
    assert_eq!(new_file_folder(&mut f, &device, "e.jpg"), "media/Con (folder)");
    let long = person(&mut f.api, &"Aleksander ".repeat(8), "Wiśniewski", None);
    let folder = new_file_folder(&mut f, &long, "f.jpg");
    assert!(folder.chars().count() <= "media/".len() + 60, "{folder}");
}

// ---------- unknown files ----------

#[test]
fn unknown_files_leave_out_deleted_ones_noise_and_what_was_left_alone() {
    let mut f = family();
    put(&f.root, "media/Thumbs.db", "szum");
    put(&f.root, "media/luzem.jpg", "luzem");
    put(&f.root, "media/skany/s1.jpg", "s1");
    put(&f.root, "media/skany/s2.jpg", "s2");
    put(&f.root, "media/skany/desktop.ini", "ini");
    // A file removed from Heirloom stays on disk and is not "unknown".
    let id = object_at(&mut f.api, "media/jozef.jpg");
    run(&mut f.api, "media.delete", json!({ "id": id }));
    save(&mut f.api);
    let report = scan(&mut f.api);
    assert_eq!(unknown_paths(&report), vec!["media/luzem.jpg", "media/skany/s1.jpg", "media/skany/s2.jpg"]);
    // Flat: the subfolder is a question; the folder row counts its files without the noise.
    let folder = report["folders"].as_array().unwrap().iter().find(|x| x["path"] == "media/skany").unwrap();
    assert_eq!((folder["files"].as_u64(), folder["unknown"].as_u64(), folder["question"].as_str()), (Some(2), Some(2), Some("unpack")));
    let before = report["fingerprint"].as_str().unwrap().to_string();

    // Left alone: a file and a folder disappear from the questions.
    run(&mut f.api, "media.leaveAlone", json!({ "path": "media/luzem.jpg", "kind": "file" }));
    run(&mut f.api, "media.leaveAlone", json!({ "path": "media/skany", "kind": "folder" }));
    let report = scan(&mut f.api);
    assert!(unknown_paths(&report).is_empty(), "{:?}", unknown_paths(&report));
    let folder = report["folders"].as_array().unwrap().iter().find(|x| x["path"] == "media/skany").unwrap();
    assert_eq!((folder["leftAlone"].as_bool(), folder["question"].is_null()), (Some(true), true));
    assert_ne!(report["fingerprint"].as_str().unwrap(), before, "the findings changed");
    assert_eq!(report["total"], 0);

    // A folder that changes is asked about again: a new file in it.
    put(&f.root, "media/skany/s3.jpg", "s3");
    let report = scan(&mut f.api);
    assert_eq!(unknown_paths(&report), vec!["media/skany/s1.jpg", "media/skany/s2.jpg", "media/skany/s3.jpg"]);
    let folder = report["folders"].as_array().unwrap().iter().find(|x| x["path"] == "media/skany").unwrap();
    assert_eq!((folder["leftAlone"].as_bool(), folder["changed"].as_bool(), folder["question"].as_str()), (Some(false), Some(true), Some("unpack")));
    // The file left alone stays quiet while it is unchanged.
    assert!(!unknown_paths(&report).contains(&"media/luzem.jpg".to_string()));
    // Leaving the folder alone again remembers the new state.
    run(&mut f.api, "media.leaveAlone", json!({ "path": "media/skany", "kind": "folder" }));
    assert!(unknown_paths(&scan(&mut f.api)).is_empty());
}

#[test]
fn the_banner_stays_away_until_the_findings_change() {
    let mut f = family();
    put(&f.root, "media/luzem.jpg", "luzem");
    let report = scan(&mut f.api);
    assert_eq!(report["seen"], false);
    let fingerprint = report["fingerprint"].as_str().unwrap().to_string();
    assert_eq!(scan(&mut f.api)["fingerprint"], fingerprint.as_str(), "the same findings, the same fingerprint");
    run(&mut f.api, "media.tidySeen", json!({ "fingerprint": fingerprint }));
    assert_eq!(scan(&mut f.api)["seen"], true);
    put(&f.root, "media/jeszcze.jpg", "jeszcze");
    assert_eq!(scan(&mut f.api)["seen"], false, "a new finding brings it back");
}

#[test]
fn a_name_suggests_a_person_but_nothing_is_assigned() {
    let mut f = family();
    let jozef2 = person(&mut f.api, "Józef", "Kowalski", Some("1878"));
    save(&mut f.api);
    for name in [
        "Maria Nowak 1950.jpg",
        "kowalski_jan_ślub.jpg",
        "Józef Kowalski.jpg",
        "Józef Kowalski 1878.jpg",
        "Józef Kowalski 1905 ślub.jpg",
        "Anna Kowalski.jpg",
        "bez nazwiska.jpg",
        "Jan.jpg",
        // The same name written the way a Mac does it: letters and accents apart (NFD).
        "Jo\u{301}zef Kowalski 1878 nfd.jpg",
    ] {
        put(&f.root, &format!("media/{name}"), name);
    }
    let report = scan(&mut f.api);
    let who = |name: &str| unknown_row(&report, &format!("media/{name}"))["suggestion"]["id"].as_str().map(str::to_string);
    let tag = |name: &str| unknown_row(&report, &format!("media/{name}"))["suggestion"]["why"].as_str().map(str::to_string);
    let candidates = |name: &str| unknown_row(&report, &format!("media/{name}"))["candidates"].as_array().unwrap().len();
    assert_eq!(who("Maria Nowak 1950.jpg"), Some(f.maria.clone()));
    assert_eq!(tag("Maria Nowak 1950.jpg").as_deref(), Some("pasuje nazwa pliku"));
    assert_eq!(who("kowalski_jan_ślub.jpg"), Some(f.jan.clone()), "any separators, any case");
    // Two Józef Kowalski: listed, nothing prefilled.
    assert_eq!((who("Józef Kowalski.jpg"), candidates("Józef Kowalski.jpg")), (None, 2));
    // A year of birth breaks the tie.
    assert_eq!(who("Józef Kowalski 1878.jpg"), Some(jozef2.clone()));
    assert_eq!(who("Józef Kowalski 1905 ślub.jpg"), Some(f.jozef.clone()));
    // Anna /Kowalska/ fits „Anna Kowalski” (one surname, two forms).
    assert_eq!(who("Anna Kowalski.jpg"), Some(f.anna.clone()));
    // No surname or no given name: nothing.
    assert_eq!((who("bez nazwiska.jpg"), candidates("bez nazwiska.jpg")), (None, 0));
    assert_eq!((who("Jan.jpg"), candidates("Jan.jpg")), (None, 0));
    assert_eq!(who("Jo\u{301}zef Kowalski 1878 nfd.jpg"), Some(jozef2));
    // Nothing was assigned or created.
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["unsavedChanges"], 0);
}

#[test]
fn a_file_in_a_person_folder_suggests_that_person() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    run(&mut f.api, "media.restructure", json!({ "to": "perPerson", "author": "Ewa" }));
    put(&f.root, "media/Kowalski Jan/niezapisane.jpg", "niezapisane");
    let report = scan(&mut f.api);
    let row = unknown_row(&report, "media/Kowalski Jan/niezapisane.jpg");
    assert_eq!((row["suggestion"]["id"].as_str(), row["suggestion"]["why"].as_str()), (Some(f.jan.as_str()), Some("w folderze osoby")));
}

// ---------- the moved map ----------

#[test]
fn a_file_moved_in_explorer_is_still_shown_and_the_path_can_be_fixed() {
    let mut f = family();
    // Someone moved jan1.jpg into a folder by hand.
    fs::create_dir_all(f.root.join("media").join("stare")).unwrap();
    fs::rename(f.root.join("media/jan1.jpg"), f.root.join("media/stare/jan1.jpg")).unwrap();
    // Two files of one name elsewhere: that one stays missing (nothing says which is meant).
    let list = run(&mut f.api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"] == "media/jan1.jpg").unwrap();
    assert_eq!(item["missing"], true, "before the scan nothing knows");
    let roots = f.api.media_roots();
    assert_eq!(media::serve(&roots, "file/media/jan1.jpg"), Served::NotFound);

    fs::create_dir_all(f.root.join("media").join("a")).unwrap();
    fs::create_dir_all(f.root.join("media").join("b")).unwrap();
    fs::rename(f.root.join("media/jan2.jpg"), f.root.join("media/a/jan2.jpg")).unwrap();
    fs::write(f.root.join("media/b/jan2.jpg"), "inny plik o tej samej nazwie").unwrap();
    let report = scan(&mut f.api);
    let moved: Vec<&str> = report["moved"].as_array().unwrap().iter().map(|m| m["file"].as_str().unwrap()).collect();
    assert_eq!(moved, vec!["media/jan1.jpg"], "jan2.jpg has two candidates and stays missing");
    assert_eq!(report["moved"][0]["found"], "media/stare/jan1.jpg");
    assert_eq!(report["counts"]["missing"], 1);
    // Shown: found, listed as „foundElsewhere”, and served by the file server.
    let list = run(&mut f.api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"] == "media/jan1.jpg").unwrap();
    assert_eq!((item["missing"].as_bool(), item["foundElsewhere"].as_bool()), (Some(false), Some(true)));
    assert_eq!(media::serve(&roots, "file/media/jan1.jpg"), Served::Ok { body: fs::read(f.root.join("media/stare/jan1.jpg")).unwrap(), mime: "image/jpeg" });
    // The truly missing files are the others.
    let missing = run(&mut f.api, "media.missing", Value::Null);
    assert_eq!(missing["rows"].as_array().unwrap().len(), 1);
    // Nothing outside the archive is served through the map.
    assert_eq!(media::serve(&roots, "file/../x.jpg"), Served::BadRequest);

    // „Popraw ścieżki”: an ordinary unsaved edit, and it can be undone.
    let fixed = run(&mut f.api, "media.fixMoved", Value::Null);
    assert_eq!(fixed["fixed"], 1);
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["unsavedChanges"], 1);
    let list = run(&mut f.api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"] == "media/stare/jan1.jpg").unwrap();
    assert_eq!(item["foundElsewhere"], false);
    save(&mut f.api);
    assert!(scan(&mut f.api)["moved"].as_array().unwrap().is_empty());
    run(&mut f.api, "archive.undo", Value::Null);
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["unsavedChanges"], 1, "the undo is an unsaved change too");
    let list = run(&mut f.api, "media.list", Value::Null);
    assert!(list["items"].as_array().unwrap().iter().any(|m| m["path"] == "media/jan1.jpg"));
}

// ---------- the engine ----------

/// Runs a restructure and returns its result.
fn restructure(api: &mut Api, to: &str) -> Value {
    run(api, "media.restructure", json!({ "to": to, "author": "Ewa" }))
}

fn journals_of(root: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(root.join(".heirloom").join("porzadkowanie")).into_iter().flatten().flatten().map(|e| e.path()).collect();
    v.sort();
    v
}

fn journal_state(path: &Path) -> Option<String> {
    fs::read_to_string(path).unwrap().lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter_map(|v| v.get("state").and_then(Value::as_str).map(str::to_string)).last()
}

fn feed_rows(api: &mut Api, action: &str) -> usize {
    run(api, "history.feed", json!({}))["rows"].as_array().unwrap().iter().filter(|r| r["action"] == action).count()
}

#[test]
fn switching_both_ways_loses_nothing_and_every_file_value_stays_right() {
    let mut f = family();
    put(&f.root, "media/luzem.jpg", "luzem");
    put(&f.root, "media/Thumbs.db", "szum");
    let tree0 = tree(&f.root);
    let objects0 = hashes(&mut f.api);
    let data0 = data_bytes(&f.root);
    let before = run(&mut f.api, "media.restructurePreview", json!({ "to": "perPerson" }));
    assert_eq!((before["count"].as_u64(), before["unassigned"].as_u64()), (Some(5), Some(1)));

    // Flat → per person.
    let result = restructure(&mut f.api, "perPerson");
    assert_eq!((result["moved"].as_u64(), result["held"].as_u64()), (Some(5), Some(0)));
    let tree1 = tree(&f.root);
    assert_eq!(tree1.len(), tree0.len(), "no file lost or added");
    assert_eq!(multiset(&tree1), multiset(&tree0), "every file has the bytes it had");
    assert_eq!(hashes(&mut f.api), objects0, "every object still names its own file");
    assert_eq!(folders_of(&f.root), vec!["Kowalska Anna", "Kowalski Jan", "Kowalski Józef"]);
    let at = paths(&mut f.api);
    assert!(at.values().any(|p| p == "media/Kowalski Jan/jan1.jpg") && at.values().any(|p| p == "media/Kowalska Anna/anna.jpg"));
    assert!(at.values().any(|p| p == "media/skan.jpg"), "a file of nobody stays");
    assert!(tree1.contains_key("media/luzem.jpg"), "an unknown file is untouched");
    // Saved in one go, setting stored, nothing left unsaved, the history has one row.
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert_eq!((status["unsavedChanges"].as_u64(), status["display"]["mediaLayout"].as_str(), status["canUndo"].as_bool()), (Some(0), Some("perPerson"), Some(false)), "no undo step leads back to old paths");
    assert_eq!(feed_rows(&mut f.api, "Uporządkowano pliki"), 1);
    let batch = result["batch"].as_str().unwrap();
    assert!(batch.starts_with("porzadkowanie-"));
    assert_eq!(code(&mut f.api, "history.undo", json!({ "batch": batch })), "layout_entry");
    assert_eq!(result["report"]["layout"]["setting"], "perPerson");
    assert_eq!(result["report"]["looseLinked"].as_array().unwrap().len(), 0);
    let journals = journals_of(&f.root);
    assert_eq!(journals.len(), 1);
    assert_eq!(journal_state(&journals[0]).as_deref(), Some("saved"));

    // Per person → flat; Jan's folder keeps a file Heirloom doesn't know.
    put(&f.root, "media/Kowalski Jan/notatki.txt", "notatki");
    put(&f.root, "media/Kowalska Anna/Thumbs.db", "szum");
    let result = restructure(&mut f.api, "flat");
    assert_eq!(result["moved"], 5);
    assert_eq!(hashes(&mut f.api), objects0);
    let tree2 = tree(&f.root);
    assert_eq!(multiset(&tree2), multiset(&{ let mut t = tree0.clone(); t.insert("media/Kowalski Jan/notatki.txt".into(), blake3::hash(&fs::read(f.root.join("media/Kowalski Jan/notatki.txt")).unwrap()).to_hex().to_string()); t }));
    // Emptied folders are gone (Thumbs.db with them); the one that holds an unknown file stays.
    assert_eq!(folders_of(&f.root), vec!["Kowalski Jan"]);
    assert!(f.root.join("media/Kowalski Jan/notatki.txt").is_file());
    let at = paths(&mut f.api);
    assert!(at.values().all(|p| p.matches('/').count() == 1), "{at:?}");
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["display"]["mediaLayout"], "flat");
    assert_eq!(feed_rows(&mut f.api, "Uporządkowano pliki"), 2);
    // FILE values were rewritten and back: the data file is byte for byte what it was (the history has grown).
    assert_eq!(data_bytes(&f.root), data0);
    assert_eq!(journals_of(&f.root).len(), 2);
}

#[test]
fn one_file_referenced_twice_moves_once_and_both_values_change() {
    let mut f = family();
    // A second record for jan1.jpg: media.relink points it at the same file.
    let second = add(&mut f.api, &f.root.clone(), "media/dwa.jpg", Some(&f.jan.clone()));
    run(&mut f.api, "media.relink", json!({ "id": second, "path": path(&f.root.join("media/jan1.jpg")) }));
    fs::remove_file(f.root.join("media/dwa.jpg")).unwrap();
    save(&mut f.api);
    let plan = run(&mut f.api, "media.restructurePreview", json!({ "to": "perPerson" }));
    assert_eq!(plan["shared"], 1);
    let moves = plan["moves"].as_array().unwrap();
    assert_eq!(moves.iter().filter(|m| m["from"] == "media/jan1.jpg").count(), 1);
    assert_eq!(moves.iter().find(|m| m["from"] == "media/jan1.jpg").unwrap()["objects"].as_array().unwrap().len(), 2);
    let tree0 = tree(&f.root);
    restructure(&mut f.api, "perPerson");
    assert_eq!(tree(&f.root).len(), tree0.len());
    let at = paths(&mut f.api);
    assert_eq!(at.values().filter(|p| *p == "media/Kowalski Jan/jan1.jpg").count(), 2, "both records follow the file");
    assert!(!f.root.join("media/Kowalski Jan/jan1 (2).jpg").exists());
}

#[test]
fn clashing_names_get_a_number_and_nothing_is_overwritten() {
    let mut f = family();
    // To flat: two folders each with a.jpg, and a.jpg already in the root.
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    restructure(&mut f.api, "perPerson");
    let root = f.root.clone();
    let (jan, anna) = (f.jan.clone(), f.anna.clone());
    add(&mut f.api, &root, "media/Kowalski Jan/a.jpg", Some(&jan));
    add(&mut f.api, &root, "media/Kowalska Anna/a.jpg", Some(&anna));
    put(&root, "media/a.jpg", "inne a");
    add(&mut f.api, &root, "media/b.jpg", None);
    save(&mut f.api);
    // An unknown file in the way of a move to B2: b.jpg is nobody's; give it to Jan and a clash in his folder.
    let tree0 = tree(&f.root);
    let objs0 = hashes(&mut f.api);
    restructure(&mut f.api, "flat");
    let tree1 = tree(&f.root);
    assert_eq!(multiset(&tree1), multiset(&tree0));
    assert_eq!(hashes(&mut f.api), objs0);
    assert!(tree1.contains_key("media/a.jpg") && tree1.contains_key("media/a (2).jpg") && tree1.contains_key("media/a (3).jpg"), "{:?}", tree1.keys().collect::<Vec<_>>());
    // The unknown a.jpg kept its name, since it was there first.
    assert_eq!(tree1["media/a.jpg"], blake3::hash(b"zdjecie:inne a").to_hex().to_string());
    // Back to per person: the root's a.jpg is nobody's, so it stays, and nothing in the folders is overwritten.
    restructure(&mut f.api, "perPerson");
    assert_eq!(multiset(&tree(&f.root)), multiset(&tree0));
    assert_eq!(hashes(&mut f.api), objs0);
}

#[test]
fn a_move_never_replaces_a_file_that_appeared_after_the_plan() {
    let mut f = family();
    let root = f.root.clone();
    // Just before Jan's first photo moves, a file of that name appears in his new folder.
    let intruder = root.join("media/Kowalski Jan/jan1.jpg");
    let mark = intruder.clone();
    engine_hook(Some(Box::new(move |_, to| {
        if to == mark {
            fs::create_dir_all(mark.parent().unwrap()).unwrap();
            fs::write(&mark, "ktos inny").unwrap();
        }
    })));
    let tree0 = tree(&root);
    let objs0 = hashes(&mut f.api);
    restructure(&mut f.api, "perPerson");
    engine_hook(None);
    assert_eq!(fs::read(&intruder).unwrap(), b"ktos inny", "the file that was there is untouched");
    assert!(root.join("media/Kowalski Jan/jan1 (2).jpg").is_file());
    assert_eq!(tree(&root).len(), tree0.len() + 1);
    assert_eq!(hashes(&mut f.api), objs0);
    assert!(paths(&mut f.api).values().any(|p| p == "media/Kowalski Jan/jan1 (2).jpg"));
}

fn engine_hook(hook: Option<Box<dyn Fn(usize, &Path)>>) {
    heirloom_api::layout::engine::before_move(hook);
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_moved_is_skipped_and_reported() {
    use std::os::unix::fs::PermissionsExt;
    let mut f = family();
    // Jan's folder exists and cannot be written: his files stay where they are (a file held by another program on Windows
    // is the same: skipped, its FILE value unchanged).
    let locked = f.root.join("media/Kowalski Jan");
    fs::create_dir_all(&locked).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(locked.join("probe"), "x").is_ok() {
        // Running as root: permissions don't hold, nothing to prove here.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let tree0 = tree(&f.root);
    let objs0 = hashes(&mut f.api);
    let result = restructure(&mut f.api, "perPerson");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(result["held"], 3, "jan1, jan2 and the group photo");
    assert_eq!(result["moved"], 2);
    assert!(result["skipped"].as_array().unwrap().iter().all(|s| s["reason"] == "locked"));
    assert_eq!(tree(&f.root).len(), tree0.len());
    assert_eq!(multiset(&tree(&f.root)), multiset(&tree0));
    assert_eq!(hashes(&mut f.api), objs0, "every photo is still where its FILE value says");
    let at = paths(&mut f.api);
    assert!(at.values().any(|p| p == "media/jan1.jpg") && at.values().any(|p| p == "media/Kowalska Anna/anna.jpg"));
    // What was left shows up as loose again: „Przenieś do folderów osób” can be tried later.
    let report = scan(&mut f.api);
    assert_eq!(report["looseLinked"].as_array().unwrap().len(), 3);
    let result = restructure(&mut f.api, "perPerson");
    assert_eq!((result["moved"].as_u64(), result["held"].as_u64()), (Some(3), Some(0)));
    assert_eq!(hashes(&mut f.api), objs0);
}

#[test]
fn a_save_conflict_reloads_and_applies_the_paths_again() {
    let mut f = family();
    let objs0 = hashes(&mut f.api);
    let tree0 = tree(&f.root);
    // Another program writes the data file while the files are moving: a person is added at the end.
    let data = f.root.join("rodzina.ged");
    let last = tree0.len();
    engine_hook(Some(Box::new(move |i, _| {
        if i + 1 == last - 1 {
            let text = fs::read_to_string(&data).unwrap();
            fs::write(&data, text.replacen("0 TRLR", "0 @I99@ INDI\n1 NAME Ktos /Inny/\n0 TRLR", 1)).unwrap();
        }
    })));
    let result = restructure(&mut f.api, "perPerson");
    engine_hook(None);
    assert_eq!(result["moved"], 5);
    // The other program's change is there, and the paths are right in the file on disk.
    let people = run(&mut f.api, "people.list", json!({}));
    assert!(people.to_string().contains("Ktos"), "their person survived the reload");
    assert_eq!(hashes(&mut f.api), objs0);
    let on_disk = String::from_utf8(data_bytes(&f.root)).unwrap();
    assert!(on_disk.contains("Kowalski%20Jan/jan1.jpg") && on_disk.contains("@I99@"));
    assert_eq!(multiset(&tree(&f.root)), multiset(&tree0));
    // The same with a plain failed first save.
    let mut g = family();
    let objs = hashes(&mut g.api);
    heirloom_api::layout::engine::fail_saves(1);
    let result = restructure(&mut g.api, "perPerson");
    assert_eq!(result["moved"], 5);
    assert_eq!(hashes(&mut g.api), objs);
    assert!(String::from_utf8(data_bytes(&g.root)).unwrap().contains("Kowalski%20Jan/jan1.jpg"));
}

#[test]
fn when_the_save_fails_twice_every_file_goes_back_and_the_bytes_are_identical() {
    let mut f = family();
    let tree0 = tree(&f.root);
    let objs0 = hashes(&mut f.api);
    let data0 = data_bytes(&f.root);
    let folders0 = folders_of(&f.root);
    heirloom_api::layout::engine::fail_saves(2);
    let error = f.api.call("media.restructure", json!({ "to": "perPerson", "author": "Ewa" })).unwrap_err();
    assert_eq!(error.code, "rolled_back");
    assert!(error.message.contains("pliki wróciły na swoje miejsca"), "{}", error.message);
    assert_eq!(tree(&f.root), tree0, "same names, same bytes");
    assert_eq!(folders_of(&f.root), folders0, "the folders it made are gone");
    assert_eq!(data_bytes(&f.root), data0, "the data file is untouched");
    assert_eq!(hashes(&mut f.api), objs0);
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert_eq!(status["unsavedChanges"], 0);
    let journals = journals_of(&f.root);
    assert_eq!(journal_state(&journals[0]).as_deref(), Some("rolledBack"));
    assert!(status["layoutInterrupted"].is_null(), "a rolled-back run is not an interrupted one");
    // The run can be started again, and works.
    let result = restructure(&mut f.api, "perPerson");
    assert_eq!(result["moved"], 5);
    assert_eq!(hashes(&mut f.api), objs0);
}

#[test]
fn a_cut_off_run_keeps_every_photo_visible_and_can_be_finished() {
    let mut f = family();
    let tree0 = tree(&f.root);
    let objs0 = hashes(&mut f.api);
    engine::stop_after(Some(2));
    let error = f.api.call("media.restructure", json!({ "to": "perPerson", "author": "Ewa" })).unwrap_err();
    engine::stop_after(None);
    assert_eq!(error.code, "stopped_for_test");
    assert_eq!(tree(&f.root).len(), tree0.len());
    let report = scan(&mut f.api);
    assert_eq!((report["interrupted"]["done"].as_u64(), report["interrupted"]["total"].as_u64()), (Some(2), Some(5)));
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert_eq!(status["layoutInterrupted"]["done"], 2);
    // The data was not touched yet, but the moved map shows both moved photos.
    assert_eq!(report["moved"].as_array().unwrap().len(), 2);
    assert_eq!(hashes(&mut f.api), objs0, "every photo is visible");
    // Finishing it, even if it is cut off again, is safe.
    engine::stop_after(Some(1));
    let again = f.api.call("media.finishRestructure", json!({ "author": "Ewa" })).unwrap_err();
    engine::stop_after(None);
    assert_eq!(again.code, "stopped_for_test");
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["layoutInterrupted"]["done"], 3);
    let result = run(&mut f.api, "media.finishRestructure", json!({ "author": "Ewa" }));
    assert_eq!(result["moved"], 5);
    assert_eq!(result["layout"], "perPerson");
    assert_eq!(hashes(&mut f.api), objs0);
    assert_eq!(multiset(&tree(&f.root)), multiset(&tree0));
    assert!(paths(&mut f.api).values().any(|p| p == "media/Kowalski Jan/jan1.jpg"));
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert!(status["layoutInterrupted"].is_null());
    assert_eq!(status["display"]["mediaLayout"], "perPerson");
    assert_eq!(feed_rows(&mut f.api, "Uporządkowano pliki"), 1);
    assert_eq!(code(&mut f.api, "media.finishRestructure", json!({ "author": "Ewa" })), "nothing_to_finish");
}

#[test]
fn a_cut_off_run_can_be_taken_back_and_taking_back_twice_is_safe() {
    let mut f = family();
    let tree0 = tree(&f.root);
    let data0 = data_bytes(&f.root);
    let objs0 = hashes(&mut f.api);
    engine::stop_after(Some(3));
    f.api.call("media.restructure", json!({ "to": "perPerson", "author": "Ewa" })).unwrap_err();
    engine::stop_after(None);
    let journal = scan(&mut f.api)["interrupted"]["journal"].as_str().unwrap().to_string();
    let result = run(&mut f.api, "media.undoRestructure", json!({ "journal": journal, "author": "Ewa" }));
    assert_eq!(result["movedBack"], 3);
    assert_eq!(tree(&f.root), tree0);
    assert_eq!(data_bytes(&f.root), data0, "nothing had been saved, nothing is saved now");
    assert_eq!(folders_of(&f.root), Vec::<String>::new());
    assert_eq!(hashes(&mut f.api), objs0);
    assert!(run(&mut f.api, "archive.status", Value::Null)["layoutInterrupted"].is_null());
    assert_eq!(code(&mut f.api, "media.undoRestructure", json!({ "journal": journal, "author": "Ewa" })), "cannot_undo");
}

#[test]
fn a_move_whose_line_was_lost_is_found_by_size_and_time_never_by_name_alone() {
    let mut f = family();
    let root = f.root.clone();
    // A file of Jan's photo's name appears in his new folder before the first move: the photo takes „jan1 (2).jpg”.
    let target = root.join("media/Kowalski Jan/jan1.jpg");
    let mark = target.clone();
    engine_hook(Some(Box::new(move |_, to| {
        if to == mark {
            fs::create_dir_all(mark.parent().unwrap()).unwrap();
            fs::write(&mark, "ktos inny").unwrap();
        }
    })));
    let tree0 = tree(&root);
    // The moves go in the order of the names: anna, grupa, jan1.
    engine::stop_after(Some(3));
    f.api.call("media.restructure", json!({ "to": "perPerson", "author": "Ewa" })).unwrap_err();
    engine::stop_after(None);
    engine_hook(None);
    // The program was closed before the lines of the move reached the journal.
    let journal = &journals_of(&root)[0];
    let kept: Vec<String> = fs::read_to_string(journal).unwrap().lines().filter(|l| !l.starts_with("{\"done\"")).map(str::to_string).collect();
    fs::write(journal, kept.join("\n") + "\n").unwrap();
    assert!(root.join("media/Kowalski Jan/jan1 (2).jpg").is_file());
    let result = run(&mut f.api, "media.finishRestructure", json!({ "author": "Ewa" }));
    assert_eq!(result["moved"], 5);
    // The FILE value names OUR photo, not the other file of the same name.
    let at = paths(&mut f.api);
    assert!(at.values().any(|p| p == "media/Kowalski Jan/jan1 (2).jpg"), "{at:?}");
    assert!(!at.values().any(|p| p == "media/Kowalski Jan/jan1.jpg"));
    assert_eq!(fs::read(&target).unwrap(), b"ktos inny");
    assert_eq!(tree(&root).len(), tree0.len() + 1);
}

#[test]
fn a_finished_run_is_taken_back_from_its_journal() {
    let mut f = family();
    put(&f.root, "media/luzem.jpg", "luzem");
    let tree0 = tree(&f.root);
    let objs0 = hashes(&mut f.api);
    let paths0 = paths(&mut f.api);
    let result = restructure(&mut f.api, "perPerson");
    let journal = result["journal"].as_str().unwrap().to_string();
    assert_eq!(run(&mut f.api, "media.scan", Value::Null)["journal"]["name"], journal.as_str());
    // A file the user moved by hand in between: it is not moved back, its value is left alone.
    let hand = f.root.join("media/Kowalski Jan/jan2.jpg");
    fs::rename(&hand, f.root.join("media/Kowalski Jan/reka.jpg")).unwrap();
    let back = run(&mut f.api, "media.undoRestructure", json!({ "journal": journal, "author": "Ewa" }));
    assert_eq!((back["movedBack"].as_u64(), back["notBack"].as_u64()), (Some(4), Some(0)));
    fs::rename(f.root.join("media/Kowalski Jan/reka.jpg"), f.root.join("media/jan2.jpg")).unwrap();
    assert_eq!(tree(&f.root), tree0);
    // jan2.jpg's value still names the folder: the moved map shows it until the paths are fixed.
    scan(&mut f.api);
    assert_eq!(hashes(&mut f.api), objs0);
    assert_eq!(paths(&mut f.api).len(), paths0.len());
    assert_eq!(paths(&mut f.api).values().filter(|p| p.starts_with("media/Kowalski Jan/")).count(), 1, "only the file moved by hand keeps the old path");
    assert_eq!(folders_of(&f.root), vec!["Kowalski Jan"], "the folders the run made are gone, except the one that held the file moved by hand");
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["display"]["mediaLayout"], "flat", "the layout is as before (it was not decided: the scan after the undo guessed flat)");
    assert_eq!(feed_rows(&mut f.api, "Uporządkowano pliki"), 2);
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert_eq!(status["unsavedChanges"], 0);
    assert_eq!(code(&mut f.api, "media.undoRestructure", json!({ "journal": journal, "author": "Ewa" })), "cannot_undo");
}

#[test]
fn only_the_last_five_journals_are_kept() {
    let mut f = family();
    for n in 0..4 {
        restructure(&mut f.api, if n % 2 == 0 { "perPerson" } else { "flat" });
    }
    restructure(&mut f.api, "perPerson");
    restructure(&mut f.api, "flat");
    restructure(&mut f.api, "perPerson");
    let journals = journals_of(&f.root);
    assert_eq!(journals.len(), 5);
    assert!(journals.iter().all(|j| journal_state(j).is_some()));
}

#[test]
fn moving_needs_a_writable_archive_with_nothing_unsaved() {
    let mut f = family();
    run(&mut f.api, "person.create", json!({ "given": "Nowy" }));
    assert_eq!(code(&mut f.api, "media.restructure", json!({ "to": "perPerson", "author": "Ewa" })), "unsaved");
    save(&mut f.api);
    run(&mut f.api, "archive.setSettings", json!({ "readOnly": true }));
    assert_eq!(code(&mut f.api, "media.restructure", json!({ "to": "perPerson", "author": "Ewa" })), "read_only");
    run(&mut f.api, "archive.setSettings", json!({ "readOnly": false }));
    assert_eq!(code(&mut f.api, "media.restructure", json!({ "to": "oops", "author": "Ewa" })), "bad_args");
    // The data file changed under us: no file moves.
    let data = f.root.join("rodzina.ged");
    let text = fs::read_to_string(&data).unwrap();
    fs::write(&data, text.replacen("0 TRLR", "0 @I98@ INDI\n1 NAME Ktos /Inny/\n0 TRLR", 1)).unwrap();
    let tree0 = tree(&f.root);
    assert_eq!(code(&mut f.api, "media.restructure", json!({ "to": "perPerson", "author": "Ewa" })), "conflict");
    assert_eq!(tree(&f.root), tree0);
}

#[test]
fn only_thumbs_db_and_desktop_ini_go_with_an_emptied_folder() {
    let mut f = family();
    restructure(&mut f.api, "perPerson");
    put(&f.root, "media/Kowalska Anna/Thumbs.db", "szum");
    put(&f.root, "media/Kowalska Anna/DESKTOP.INI", "ini");
    put(&f.root, "media/Kowalski Józef/~$robocze.doc", "praca w toku");
    let result = restructure(&mut f.api, "flat");
    assert!(!f.root.join("media/Kowalska Anna").exists(), "{result}");
    assert!(f.root.join("media/Kowalski Józef/~$robocze.doc").is_file(), "somebody's work in progress is not deleted");
    assert!(f.root.join("media/Kowalski Józef").is_dir());
}

#[test]
fn a_new_folder_name_is_shortened_when_the_path_would_pass_240_characters() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = computer(dir.path(), "ten");
    let root = dir.path().join("r".repeat(60));
    run(&mut api, "archive.create", json!({ "folder": path(&root), "name": "Długa" }));
    let long_given = "Aleksander Maksymilian Konstanty";
    let p = person(&mut api, long_given, "Wiśniewski-Kowalczykowski", None);
    let file_name = format!("{}.jpg", "x".repeat(110));
    add(&mut api, &root, &format!("media/{file_name}"), Some(&p));
    save(&mut api);
    let plan = run(&mut api, "media.restructurePreview", json!({ "to": "perPerson" }));
    assert_eq!(plan["shortened"].as_array().unwrap().len(), 1, "{plan}");
    let to = plan["moves"][0]["to"].as_str().unwrap();
    assert!(path(&root).chars().count() + 1 + to.chars().count() <= 240, "{to}");
    assert_eq!(plan["longPaths"], 0);
    let objs0 = hashes(&mut api);
    restructure(&mut api, "perPerson");
    assert_eq!(hashes(&mut api), objs0);
    assert!(root.join(to).is_file());
}

// ---------- where new files go ----------

fn import_answer() -> String {
    let json = heirloom_api::import::check::extract_blocks(IMPORT_FORMAT, "doc").into_iter().next().unwrap().text;
    format!("```json\n{json}\n```")
}

/// Loads the documented example with its photo M002 and decides every person as new (or the given ones as `merge`).
fn load_import(api: &mut Api, dir: &Path) {
    let m002 = dir.join("paczka").join("M002 Józef i Marianna.jpg");
    fs::create_dir_all(m002.parent().unwrap()).unwrap();
    fs::write(&m002, "paczka-m002").unwrap();
    let state = run(api, "import.load", json!({ "texts": [import_answer()], "paths": [path(&m002)] }));
    for p in state["persons"].as_array().unwrap() {
        run(api, "import.decide", json!({ "person": p["id"], "kind": "new" }));
    }
}

#[test]
fn an_import_puts_each_file_in_the_folder_of_its_first_person_even_a_new_one() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    load_import(&mut f.api, f.dir.path());
    // The example describes M002 as a photo of people of the batch: the first one decides.
    run(&mut f.api, "import.commit", json!({ "author": "Ewa" }));
    let list = run(&mut f.api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"].as_str().is_some_and(|p| p.contains("M002"))).unwrap_or_else(|| panic!("{list}"));
    let rel = item["path"].as_str().unwrap();
    let (folder, _) = rel.rsplit_once('/').unwrap();
    assert!(folder.starts_with("media/") && folder != "media", "{rel}: in a folder of a person");
    // The folder is named after the first person of the file, surname first (a new person has no record before the save).
    let names: Vec<String> = item["people"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect();
    assert!(!names.is_empty());
    let surname_first: Vec<String> = names.iter().map(|n| { let mut w: Vec<&str> = n.split(' ').collect(); let last = w.pop().unwrap(); format!("{last} {}", w.join(" ")) }).collect();
    assert!(surname_first.iter().any(|n| folder == format!("media/{n}")), "{folder} should be one of {surname_first:?}");
    assert!(f.root.join(rel).is_file());
    // Flat: the same import lands in media/.
    let mut g = family();
    run(&mut g.api, "media.setLayout", json!({ "layout": "flat" }));
    load_import(&mut g.api, g.dir.path());
    run(&mut g.api, "import.commit", json!({ "author": "Ewa" }));
    let list = run(&mut g.api, "media.list", Value::Null);
    assert!(list["items"].as_array().unwrap().iter().any(|m| m["path"] == "media/M002 Józef i Marianna.jpg"));
}

#[test]
fn replacing_a_file_goes_to_the_folder_of_the_owner_and_outside_files_are_copied_in() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    restructure(&mut f.api, "perPerson");
    // An object whose file lies outside the archive, linked to Jan.
    let outside = f.dir.path().join("poza").join("na-dysku.jpg");
    fs::create_dir_all(outside.parent().unwrap()).unwrap();
    fs::write(&outside, "poza archiwum").unwrap();
    let data = f.root.join("rodzina.ged");
    let text = fs::read_to_string(&data).unwrap();
    let text = text.replacen(&format!("0 {} INDI", f.jan), &format!("0 {} INDI\n1 OBJE @O50@", f.jan), 1).replacen("0 TRLR", &format!("0 @O50@ OBJE\n1 FILE {}\n2 FORM image/jpeg\n0 TRLR", outside.display()), 1);
    fs::write(&data, text).unwrap();
    run(&mut f.api, "archive.reload", Value::Null);
    let report = scan(&mut f.api);
    assert_eq!(report["outside"].as_array().unwrap().len(), 1);
    assert_eq!(report["outside"][0]["id"], "@O50@");
    let copied = run(&mut f.api, "media.relink", json!({ "id": "@O50@", "path": path(&outside) }));
    assert_eq!(copied["path"], "media/Kowalski Jan/na-dysku.jpg", "the owner's folder");
    assert_eq!(fs::read(f.root.join("media/Kowalski Jan/na-dysku.jpg")).unwrap(), b"poza archiwum");
    assert_eq!(fs::read(&outside).unwrap(), b"poza archiwum", "the original stays where it was");
    assert!(scan(&mut f.api)["outside"].as_array().unwrap().is_empty());
}

fn open_in_a1(dir: &Path, original: &Path) -> Api {
    let mut api = computer(dir, "krewny");
    run(&mut api, "archive.open", json!({ "path": path(original) }));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    api
}

/// A copy of the family archive as it lies on a stick: folders of people, a loose unknown file, a file of nobody.
fn per_person_original() -> (tempfile::TempDir, PathBuf, String, String) {
    let mut f = family();
    restructure(&mut f.api, "perPerson");
    put(&f.root, "media/luzem.jpg", "luzem");
    let (jan, anna) = (f.jan.clone(), f.anna.clone());
    run(&mut f.api, "archive.close", Value::Null);
    let Family { dir, root, .. } = f;
    (dir, root, jan, anna)
}

fn snapshot_all(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.strip_prefix(root).unwrap().display().to_string(), fs::read(&p).unwrap());
            }
        }
    }
    out
}

#[test]
fn working_on_a_copy_places_new_files_in_the_originals_layout_and_moves_nothing() {
    let (dir, original, jan, anna) = per_person_original();
    let before = snapshot_all(&original);
    let mut api = open_in_a1(dir.path(), &original);
    let report = scan(&mut api);
    assert_eq!(report["layout"]["setting"], "perPerson", "the original's layout");
    assert_eq!(unknown_paths(&report), vec!["media/luzem.jpg"]);
    // A new file for Jan: into the overlay, in a folder named like the one in the original.
    let new = dir.path().join("nowe").join("nowe-zdjecie.jpg");
    fs::create_dir_all(new.parent().unwrap()).unwrap();
    fs::write(&new, "nowe w kopii").unwrap();
    let id = run(&mut api, "media.add", json!({ "paths": [path(&new)], "person": jan }))[0]["id"].as_str().unwrap().to_string();
    let rel = run(&mut api, "media.get", json!({ "id": id }))["path"].as_str().unwrap().to_string();
    assert_eq!(rel, "media/Kowalski Jan/nowe-zdjecie.jpg");
    let status = run(&mut api, "archive.status", Value::Null);
    let overlay_root = PathBuf::from(status["overlay"]["folder"].as_str().unwrap());
    assert!(overlay_root.join(&rel).is_file(), "written into the folder of the changes");
    // Moving is refused in every form.
    for (method, args) in [
        ("media.restructure", json!({ "to": "flat", "author": "Ewa" })),
        ("media.restructurePreview", json!({ "to": "flat" })),
        ("media.finishRestructure", json!({ "author": "Ewa" })),
        ("media.undoRestructure", json!({ "journal": "x", "author": "Ewa" })),
        ("media.setLayout", json!({ "layout": "flat" })),
    ] {
        assert_eq!(code(&mut api, method, args), "overlay_mode", "{method}");
    }
    // Assigning a file of the original copies and moves nothing: the record points at it where it is.
    let assigned = run(&mut api, "media.assign", json!({ "path": "media/luzem.jpg", "person": anna }));
    assert_eq!(assigned["assigned"][0]["moved"], false);
    assert_eq!(assigned["assigned"][0]["path"], "media/luzem.jpg");
    let overlay_files = tree(&overlay_root);
    assert_eq!(overlay_files.keys().cloned().collect::<Vec<_>>(), vec!["media/Kowalski Jan/nowe-zdjecie.jpg"], "no copy of the assigned file");
    let list = run(&mut api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"] == "media/luzem.jpg").unwrap();
    assert_eq!((item["missing"].as_bool(), item["layer"].as_str()), (Some(false), Some("archive")));
    save(&mut api);
    assert_eq!(snapshot_all(&original), before, "the original is byte for byte what it was");
    // The files left in the original are still reported as the original's own (nothing else to tidy).
    assert!(unknown_paths(&scan(&mut api)).is_empty());
}

#[test]
fn the_guess_is_stored_in_the_changes_folder_when_the_original_has_no_setting() {
    let f = family();
    let (dir, original) = (f.dir, f.root);
    let before = snapshot_all(&original);
    let mut api = open_in_a1(dir.path(), &original);
    assert_eq!(scan(&mut api)["layout"]["setting"], Value::Null);
    let status = run(&mut api, "archive.status", Value::Null);
    assert_eq!(status["display"]["mediaLayout"], "flat");
    let overlay_settings: Value = serde_json::from_slice(&fs::read(PathBuf::from(status["overlay"]["folder"].as_str().unwrap()).join(".heirloom").join("ustawienia.json")).unwrap()).unwrap();
    assert_eq!(overlay_settings["display"]["mediaLayout"], "flat");
    assert_eq!(snapshot_all(&original), before);
}

#[test]
fn assigning_a_loose_file_moves_it_into_the_persons_folder_when_files_are_kept_in_folders() {
    let mut f = family();
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    put(&f.root, "media/Maria Nowak 1950.jpg", "maria");
    put(&f.root, "media/inne.jpg", "inne");
    let report = scan(&mut f.api);
    assert_eq!(unknown_row(&report, "media/Maria Nowak 1950.jpg")["suggestion"]["id"], f.maria.as_str());
    let result = run(&mut f.api, "media.assign", json!({ "path": "media/Maria Nowak 1950.jpg", "person": f.maria }));
    assert_eq!(result["assigned"][0]["path"], "media/Nowak Maria/Maria Nowak 1950.jpg");
    assert_eq!(result["assigned"][0]["moved"], true);
    assert!(f.root.join("media/Nowak Maria/Maria Nowak 1950.jpg").is_file() && !f.root.join("media/Maria Nowak 1950.jpg").exists());
    assert_eq!(run(&mut f.api, "archive.status", Value::Null)["unsavedChanges"], 2, "an unsaved edit: the new record and Maria's link");
    let list = run(&mut f.api, "media.list", Value::Null);
    let item = list["items"].as_array().unwrap().iter().find(|m| m["path"] == "media/Nowak Maria/Maria Nowak 1950.jpg").unwrap();
    assert_eq!(item["people"][0]["id"], f.maria.as_str());
    // Dropping the edit is harmless: the file stays in the folder, it is only unknown again.
    run(&mut f.api, "archive.undo", Value::Null);
    assert!(unknown_paths(&scan(&mut f.api)).contains(&"media/Nowak Maria/Maria Nowak 1950.jpg".to_string()));
    // A file in a subfolder is not moved; neither is a file when files are kept together.
    run(&mut f.api, "media.setLayout", json!({ "layout": "flat" }));
    let result = run(&mut f.api, "media.assign", json!({ "path": "media/inne.jpg", "person": f.maria }));
    assert_eq!(result["assigned"][0]["moved"], false);
    assert!(f.root.join("media/inne.jpg").is_file());
    // Paths that leave the media folder are refused.
    for bad in ["../rodzina.ged", "media/../rodzina.ged", "/etc/passwd", "rodzina.ged", "media/nie ma.jpg"] {
        assert!(f.api.call("media.assign", json!({ "path": bad, "person": f.maria })).is_err(), "{bad}");
    }
}

#[test]
fn a_folder_left_in_a_flat_archive_is_unpacked_and_its_new_files_go_to_the_person_it_is_named_after() {
    let mut f = family();
    let root = f.root.clone();
    let jozef = f.jozef.clone();
    add(&mut f.api, &root, "media/Kowalski Józef/ślub.jpg", Some(&jozef));
    put(&root, "media/Kowalski Józef/nowe1.jpg", "nowe1");
    put(&root, "media/Kowalski Józef/nowe2.jpg", "nowe2");
    put(&root, "media/skany/s1.jpg", "s1");
    save(&mut f.api);
    let report = scan(&mut f.api);
    assert_eq!(report["layout"]["effective"], "flat");
    let row = report["folders"].as_array().unwrap().iter().find(|x| x["path"] == "media/Kowalski Józef").unwrap();
    assert_eq!((row["files"].as_u64(), row["linked"].as_u64(), row["unknown"].as_u64(), row["question"].as_str(), row["person"]["id"].as_str()), (Some(3), Some(1), Some(2), Some("unpack"), Some(jozef.as_str())));
    assert_eq!(unknown_row(&report, "media/Kowalski Józef/nowe1.jpg")["suggestion"]["why"], "w folderze osoby");
    let plan = run(&mut f.api, "media.restructurePreview", json!({ "to": "flat", "folders": ["media/Kowalski Józef"], "only": true }));
    assert_eq!((plan["count"].as_u64(), plan["unknownMoves"].as_u64()), (Some(3), Some(2)));
    let tree0 = tree(&root);
    let objs0 = hashes(&mut f.api);
    let result = run(&mut f.api, "media.restructure", json!({ "to": "flat", "folders": ["media/Kowalski Józef"], "only": true, "setLayout": false, "assignTo": jozef, "author": "Ewa" }));
    assert_eq!((result["moved"].as_u64(), result["assigned"].as_u64()), (Some(3), Some(2)));
    assert_eq!(result["unknownMoved"].as_array().unwrap().len(), 2);
    assert!(!root.join("media/Kowalski Józef").exists());
    assert!(root.join("media/skany/s1.jpg").is_file(), "another folder is not touched");
    assert_eq!(tree(&root).len(), tree0.len());
    assert_eq!(multiset(&tree(&root)), multiset(&tree0));
    for (id, hash) in &objs0 {
        assert_eq!(hashes(&mut f.api)[id], *hash);
    }
    // The two new files now belong to Józef: an unsaved edit the user saves (or drops).
    let status = run(&mut f.api, "archive.status", Value::Null);
    assert_eq!((status["unsavedChanges"].as_u64(), status["display"]["mediaLayout"].as_str()), (Some(3), Some("flat")));
    let list = run(&mut f.api, "media.list", Value::Null);
    let of_jozef = list["items"].as_array().unwrap().iter().filter(|m| m["people"].as_array().unwrap().iter().any(|p| p["id"] == jozef.as_str())).count();
    assert_eq!(of_jozef, 4, "his photo, the wedding one and the two new files");
    assert!(unknown_paths(&scan(&mut f.api)).iter().all(|p| !p.contains("nowe")));
    // Without a person to assign to, the moved unknown files are reported for the assign list.
    let mut g = family();
    let groot = g.root.clone();
    put(&groot, "media/Kowalski Jan/obce.jpg", "obce");
    save(&mut g.api);
    let result = run(&mut g.api, "media.restructure", json!({ "to": "flat", "folders": ["media/Kowalski Jan"], "only": true, "setLayout": false, "author": "Ewa" }));
    assert_eq!(result["unknownMoved"][0], "media/obce.jpg");
    assert!(unknown_paths(&scan(&mut g.api)).contains(&"media/obce.jpg".to_string()));
}

#[test]
fn the_scan_stops_at_fifty_thousand_files() {
    let mut f = family();
    let bulk = f.root.join("media").join("duzo");
    fs::create_dir_all(&bulk).unwrap();
    for n in 0..50_010 {
        fs::write(bulk.join(format!("{n}.jpg")), n.to_string()).unwrap();
    }
    let report = scan(&mut f.api);
    assert_eq!(report["truncated"], true);
    assert!(report["counts"]["files"].as_u64().unwrap() <= 50_000);
}

// ---------- odd data ----------

#[test]
fn files_written_inside_other_records_or_with_odd_paths_are_never_unknown_and_never_moved() {
    let mut f = family();
    let root = f.root.clone();
    put(&root, "media/zagniezdzony.jpg", "w osobie");
    put(&root, "media/dziwna.jpg", "dziwna sciezka");
    let data = f.root.join("rodzina.ged");
    let text = fs::read_to_string(&data).unwrap();
    // An object written inside Jan, and a record whose path has `./` and a double slash.
    let text = text
        .replacen(&format!("0 {} INDI", f.jan), &format!("0 {} INDI\n1 OBJE\n2 FILE media/zagniezdzony.jpg\n3 FORM image/jpeg\n1 OBJE @O60@", f.jan), 1)
        .replacen("0 TRLR", "0 @O60@ OBJE\n1 FILE ./media//dziwna.jpg\n2 FORM image/jpeg\n0 TRLR", 1);
    fs::write(&data, text).unwrap();
    run(&mut f.api, "archive.reload", Value::Null);
    let report = scan(&mut f.api);
    assert!(unknown_paths(&report).is_empty(), "{:?}", unknown_paths(&report));
    run(&mut f.api, "media.setLayout", json!({ "layout": "perPerson" }));
    let plan = run(&mut f.api, "media.restructurePreview", json!({ "to": "perPerson" }));
    let from: Vec<&str> = plan["moves"].as_array().unwrap().iter().map(|m| m["from"].as_str().unwrap()).collect();
    assert!(!from.contains(&"media/zagniezdzony.jpg"), "{from:?}");
    assert!(from.contains(&"media/dziwna.jpg"), "the odd path is the same file");
    restructure(&mut f.api, "perPerson");
    assert!(root.join("media/zagniezdzony.jpg").is_file(), "its path is part of Jan's record");
    assert!(!root.join("media/dziwna.jpg").exists() && root.join("media/Kowalski Jan/dziwna.jpg").is_file());
    let objs = hashes(&mut f.api);
    assert!(objs.contains_key("@O60@"));
}

#[test]
fn when_every_file_is_held_nothing_is_saved_and_no_journal_is_left_to_undo() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut f = family();
        let root = f.root.clone();
        // Everything goes to folders that cannot be written.
        for folder in ["Kowalski Jan", "Kowalska Anna", "Kowalski Józef"] {
            fs::create_dir_all(root.join("media").join(folder)).unwrap();
            fs::set_permissions(root.join("media").join(folder), fs::Permissions::from_mode(0o555)).unwrap();
        }
        if fs::write(root.join("media/Kowalski Jan/probe"), "x").is_ok() {
            fs::remove_file(root.join("media/Kowalski Jan/probe")).unwrap();
            return;
        }
        let data0 = data_bytes(&root);
        let result = restructure(&mut f.api, "perPerson");
        for folder in ["Kowalski Jan", "Kowalska Anna", "Kowalski Józef"] {
            fs::set_permissions(root.join("media").join(folder), fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!((result["moved"].as_u64(), result["held"].as_u64()), (Some(0), Some(5)));
        assert_eq!(result["journal"], Value::Null);
        assert_eq!(data_bytes(&root), data0);
        assert_eq!(feed_rows(&mut f.api, "Uporządkowano pliki"), 0);
        assert!(run(&mut f.api, "media.scan", Value::Null)["journal"].is_null());
    }
}

#[test]
fn the_history_row_says_what_happened_and_the_questions_wait_for_the_mode() {
    let mut f = family();
    restructure(&mut f.api, "perPerson");
    let feed = run(&mut f.api, "history.feed", json!({}));
    let row = feed["rows"].as_array().unwrap().iter().find(|r| r["action"] == "Uporządkowano pliki").unwrap();
    assert_eq!((row["undo"].as_str(), row["note"].as_str()), (Some("Cofnij porządkowanie"), Some("Uporządkowano pliki")));
    // Another computer, not asked yet: reading works and writes nothing; every command that changes anything waits.
    let root = f.root.clone();
    let dir = f.dir.path().to_path_buf();
    let before = snapshot_all(&root);
    let mut other = computer(&dir, "krewny");
    run(&mut other, "archive.open", json!({ "path": path(&root) }));
    let report = run(&mut other, "media.scan", Value::Null);
    assert!(report["layout"]["setting"] == "perPerson");
    run(&mut other, "media.restructurePreview", json!({ "to": "flat" }));
    for (method, args) in [
        ("media.restructure", json!({ "to": "flat", "author": "Ewa" })),
        ("media.assign", json!({ "path": "media/skan.jpg", "person": "@I1@" })),
        ("media.leaveAlone", json!({ "path": "media/skan.jpg", "kind": "file" })),
        ("media.fixMoved", Value::Null),
        ("media.setLayout", json!({ "layout": "flat" })),
        ("media.finishRestructure", json!({ "author": "Ewa" })),
        ("media.undoRestructure", json!({ "journal": "x", "author": "Ewa" })),
    ] {
        assert_eq!(code(&mut other, method, args), "mode_unset", "{method}");
    }
    assert_eq!(snapshot_all(&root), before, "nothing was written");
}

#[test]
fn taking_back_a_run_whose_lines_were_lost_puts_back_our_files_and_leaves_the_others() {
    let mut f = family();
    let root = f.root.clone();
    let target = root.join("media/Kowalski Jan/jan1.jpg");
    let mark = target.clone();
    engine_hook(Some(Box::new(move |_, to| {
        if to == mark {
            fs::create_dir_all(mark.parent().unwrap()).unwrap();
            fs::write(&mark, "ktos inny").unwrap();
        }
    })));
    let tree0 = tree(&root);
    engine::stop_after(Some(3));
    f.api.call("media.restructure", json!({ "to": "perPerson", "author": "Ewa" })).unwrap_err();
    engine::stop_after(None);
    engine_hook(None);
    let journal = journals_of(&root)[0].clone();
    let kept: Vec<String> = fs::read_to_string(&journal).unwrap().lines().filter(|l| !l.starts_with("{\"done\"")).map(str::to_string).collect();
    fs::write(&journal, kept.join("\n") + "\n").unwrap();
    let name = journal.file_stem().unwrap().to_string_lossy().into_owned();
    let result = run(&mut f.api, "media.undoRestructure", json!({ "journal": name, "author": "Ewa" }));
    assert_eq!(result["movedBack"], 3);
    // Our three files are back; the file that was in the way is untouched and stays (its folder with it).
    assert_eq!(fs::read(&target).unwrap(), b"ktos inny");
    let tree1 = tree(&root);
    let mut expected = tree0.clone();
    expected.insert("media/Kowalski Jan/jan1.jpg".into(), blake3::hash(b"ktos inny").to_hex().to_string());
    assert_eq!(tree1, expected);
}
