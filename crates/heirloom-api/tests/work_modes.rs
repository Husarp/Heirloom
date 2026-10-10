//! „Pracuj na archiwum” and „Nie ruszaj oryginału” (PLAN §11.2, design 0.7.0 part A): the question on first open, the
//! user's changes kept in their own folder while the original stays byte for byte as it was, the write-back into the
//! original (atomic, recoverable, undoable), backups and restore.

use heirloom_api::Api;
use heirloom_api::overlay::writeback;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// This computer: its settings and its own Documents folder.
fn computer(dir: &Path, name: &str) -> Api {
    let mut api = Api::new(Some(dir.join(name).join("ustawienia")), None);
    api.set_home(dir.join(name).join("dom"));
    api
}

fn photo(path: &Path, shade: u8) {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).unwrap();
    }
    image::RgbImage::from_pixel(4, 3, image::Rgb([shade, shade, shade])).save(path).unwrap();
}

/// An archive made by Heirloom on another computer: settings, history, Jan with a photo, a loose file in media/.
fn family(dir: &Path) -> PathBuf {
    let root = dir.join("Rodzina Kowalskich");
    let mut other = computer(dir, "inny-komputer");
    run(&mut other, "archive.create", json!({ "folder": path(&root), "name": "Rodzina Kowalskich" }));
    let jan = run(&mut other, "person.create", json!({ "given": "Jan", "surname": "Kowalski" }))["id"].as_str().unwrap().to_string();
    let source = dir.join("zrodla").join("jan.jpg");
    photo(&source, 10);
    run(&mut other, "media.add", json!({ "paths": [path(&source)], "person": jan }));
    run(&mut other, "archive.save", json!({ "author": "Ewa" }));
    photo(&root.join("media").join("luzem.jpg"), 20);
    run(&mut other, "archive.close", Value::Null);
    root
}

/// Every file under `root` with its bytes and time, and every folder.
fn snapshot(root: &Path) -> (BTreeMap<PathBuf, (Vec<u8>, Option<SystemTime>)>, Vec<PathBuf>) {
    let mut files = BTreeMap::new();
    let mut dirs = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        dirs.push(dir.clone());
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.insert(p.clone(), (fs::read(&p).unwrap(), fs::metadata(&p).unwrap().modified().ok()));
            }
        }
    }
    dirs.sort();
    (files, dirs)
}

fn import_answer() -> String {
    let json = heirloom_api::import::check::extract_blocks(IMPORT_FORMAT, "doc").into_iter().next().unwrap().text;
    format!("```json\n{json}\n```")
}

/// Loads the documented example (with its photo M002) and decides every person as new; returns the batch.
fn import(api: &mut Api, dir: &Path) -> String {
    let m002 = dir.join("paczka").join("M002 Józef i Marianna.jpg");
    photo(&m002, 30);
    let state = run(api, "import.load", json!({ "texts": [import_answer()], "paths": [path(&m002)] }));
    for p in state["persons"].as_array().unwrap() {
        run(api, "import.decide", json!({ "person": p["id"], "kind": "new" }));
    }
    run(api, "import.commit", json!({ "author": "Ewa" }))["batch"].as_str().unwrap().to_string()
}

fn overlay_mode(api: &mut Api, original: &Path) -> PathBuf {
    let status = run(api, "archive.open", json!({ "path": path(original) }));
    assert!(status["workMode"].is_null(), "asked first");
    let status = run(api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    assert_eq!(status["workMode"], "overlay");
    PathBuf::from(status["overlay"]["folder"].as_str().unwrap())
}

#[test]
fn archives_made_here_work_in_place_without_the_question() {
    let dir = tempfile::tempdir().unwrap();
    let mut api = computer(dir.path(), "ten");
    let status = run(&mut api, "archive.create", json!({ "folder": path(&dir.path().join("nowe")), "name": "Nowe" }));
    assert_eq!(status["workMode"], "inPlace");
    run(&mut api, "person.create", json!({ "given": "Jan" }));
    let new = run(&mut api, "archive.saveAsNew", json!({ "folder": path(&dir.path().join("trzecie")) }));
    assert_eq!(new["status"]["workMode"], "inPlace");
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // The registry knows the data file's hash after the save (for finding the archive again by its content).
    let config: Value = serde_json::from_slice(&fs::read(dir.path().join("ten").join("ustawienia").join("aplikacja.json")).unwrap()).unwrap();
    let record = config["archives"].as_object().unwrap().values().find(|r| r["opened"] == path(&dir.path().join("nowe")).as_str()).unwrap();
    assert_eq!(record["hash"], blake3::hash(&fs::read(dir.path().join("nowe").join("rodzina.ged")).unwrap()).to_hex().to_string().as_str());
    let zip = run(&mut api, "archive.backupAuto", json!({}))["path"].as_str().unwrap().to_string();
    let restored = run(&mut api, "archive.restore", json!({ "zip": zip, "folder": path(&dir.path().join("przywrocone")) }));
    assert_eq!((restored["workMode"].as_str(), restored["people"].as_u64()), (Some("inPlace"), Some(1)));
    assert!(restored["notice"].as_str().unwrap().contains("(z kopii "));
}

#[test]
fn the_first_open_asks_and_nothing_is_written_before_the_answer() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let before = snapshot(&original);
    let mut api = computer(dir.path(), "ten");
    let status = run(&mut api, "archive.open", json!({ "path": path(&original) }));
    assert!(status["workMode"].is_null());
    let info = run(&mut api, "archive.modeInfo", Value::Null);
    assert_eq!((info["usedBefore"].as_bool(), info["suggested"].as_str(), info["writable"].as_bool()), (Some(true), Some("inPlace"), Some(true)));
    assert_eq!(info["editors"][0]["name"], "Ewa", "who else works on it");
    assert!(info["folder"]["label"].as_str().unwrap().starts_with("Dokumenty › Heirloom › Moje zmiany › Rodzina Kowalskich"));
    let new_photo = dir.path().join("nowe.jpg");
    photo(&new_photo, 40);
    for (method, args) in [
        ("archive.save", json!({ "author": "Ewa" })),
        ("person.create", json!({ "given": "Anna" })),
        ("media.add", json!({ "paths": [path(&new_photo)] })),
        ("archive.setSettings", json!({ "display": { "x": 1 } })),
        ("import.commit", json!({ "author": "Ewa" })),
        ("history.undo", json!({ "index": 0 })),
        ("archive.saveAsCopy", Value::Null),
        ("settings.import", json!({ "path": path(&dir.path().join("x.json")) })),
        ("source.create", json!({})),
        ("place.rename", json!({})),
    ] {
        assert_eq!(code(&mut api, method, args), "mode_unset", "{method}");
    }
    // Reads work while the question waits.
    run(&mut api, "people.list", json!({}));
    run(&mut api, "media.list", Value::Null);
    assert_eq!(snapshot(&original), before, "nothing was written");
    run(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" }));
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    // Asked once per archive on this computer.
    let mut again = computer(dir.path(), "ten");
    assert_eq!(run(&mut again, "archive.open", json!({ "path": path(&original) }))["workMode"], "inPlace");
    // Another computer (a relative given the folder) is asked.
    let mut relative = computer(dir.path(), "krewny");
    assert!(run(&mut relative, "archive.open", json!({ "path": path(&original) }))["workMode"].is_null());
}

#[cfg(unix)]
#[test]
fn in_place_is_refused_for_a_folder_that_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let data = original.join("rodzina.ged");
    fs::set_permissions(&data, fs::Permissions::from_mode(0o444)).unwrap();
    fs::set_permissions(&original, fs::Permissions::from_mode(0o555)).unwrap();
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.open", json!({ "path": path(&original) }));
    let info = run(&mut api, "archive.modeInfo", Value::Null);
    assert_eq!((info["writable"].as_bool(), info["suggested"].as_str()), (Some(false), Some("overlay")));
    assert_eq!(code(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" })), "not_writable");
    // „Nie ruszaj oryginału” works on it.
    run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    fs::set_permissions(&original, fs::Permissions::from_mode(0o755)).unwrap();
}

/// The key promise of „Nie ruszaj oryginału”: whatever the user does, the original stays byte for byte as it was.
#[test]
fn overlay_mode_never_writes_into_the_original() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let before = snapshot(&original);
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    assert!(mine.starts_with(dir.path().join("ten").join("dom").join("Dokumenty").join("Heirloom").join("Moje zmiany")));
    assert!(mine.join("CO TO JEST.txt").is_file());

    let anna = run(&mut api, "person.create", json!({ "given": "Anna", "surname": "Kowalska" }))["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("anna.jpg");
    photo(&new_photo, 50);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": anna }));
    // A file the original already has: nothing copied (the same photo, and a loose file of its media folder).
    let same = run(&mut api, "media.add", json!({ "paths": [path(&original.join("media").join("jan.jpg"))], "person": anna }));
    assert_eq!(same[0]["duplicate"], true);
    run(&mut api, "media.add", json!({ "paths": [path(&original.join("media").join("luzem.jpg"))], "person": anna }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let batch = import(&mut api, dir.path());
    run(&mut api, "history.undo", json!({ "batch": batch }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    run(&mut api, "archive.setSettings", json!({ "display": { "treeColor": "generation" } }));
    let settings_file = dir.path().join("ustawienia.json");
    run(&mut api, "settings.export", json!({ "path": path(&settings_file) }));
    run(&mut api, "settings.import", json!({ "path": path(&settings_file) }));
    run(&mut api, "archive.backupAuto", json!({ "scope": "archive" }));
    run(&mut api, "archive.check", Value::Null);
    run(&mut api, "archive.exportGedzip", json!({ "path": path(&dir.path().join("eksport.gdz")) }));
    run(&mut api, "archive.saveAsCopy", Value::Null);

    // What the user did is in their folder: the new photo (only it), the import's answer, the settings.
    let media: Vec<String> = fs::read_dir(mine.join("media")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(media.len(), 2, "anna.jpg and the import's photo, nothing from the original: {media:?}");
    assert!(media.contains(&"anna.jpg".to_string()));
    assert!(mine.join("zrodla-ai").is_dir());
    assert!(fs::read_to_string(mine.join(".heirloom").join("ustawienia.json")).unwrap().contains("generation"));
    let status = run(&mut api, "archive.status", Value::Null);
    assert_eq!(status["root"], path(&original), "the screens see the original's place");
    assert!(status["overlay"]["pending"].as_u64().unwrap() >= 1);

    let set = dir.path().join("Razem.heirloom-zestaw");
    run(&mut api, "archive.saveAsNew", json!({ "folder": path(&dir.path().join("trzecie")) }));
    run(&mut api, "set.create", json!({ "path": path(&set), "name": "Razem", "archives": [path(&original), path(&dir.path().join("trzecie"))] }));
    run(&mut api, "archive.close", Value::Null);
    assert_eq!(snapshot(&original), before, "the original is exactly as it was");
}

#[test]
fn photos_of_both_folders_show_and_a_new_file_never_hides_an_original_one() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    let jan = run(&mut api, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    // Another jan.jpg (other content): it gets a name of its own, so the original's photo stays visible.
    let other = dir.path().join("inne").join("jan.jpg");
    photo(&other, 99);
    run(&mut api, "media.add", json!({ "paths": [path(&other)], "person": jan }));
    assert!(mine.join("media").join("jan (2).jpg").is_file());
    assert!(!mine.join("media").join("jan.jpg").exists());
    let list = run(&mut api, "media.list", Value::Null);
    let items = list["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["missing"] == false), "{items:?}");
    let layers: Vec<&str> = items.iter().map(|i| i["layer"].as_str().unwrap()).collect();
    assert_eq!(layers, ["archive", "mine"]);
    // The file server: the user's file, the original's file, nothing climbing out.
    let roots = api.media_roots();
    assert!(matches!(heirloom_api::media::serve(&roots, "file/media/jan.jpg"), heirloom_api::media::Served::Ok { .. }));
    assert!(matches!(heirloom_api::media::serve(&roots, "file/media/jan%20(2).jpg"), heirloom_api::media::Served::Ok { .. }));
    assert_eq!(heirloom_api::media::serve(&roots, "file/media/..%2F..%2Fustawienia"), heirloom_api::media::Served::BadRequest);
    assert!(run(&mut api, "media.missing", Value::Null)["rows"].as_array().unwrap().is_empty());
    // The import finds the original's photo already there and doesn't copy it again.
    let m002 = dir.path().join("paczka").join("M002 Józef i Marianna.jpg");
    fs::create_dir_all(m002.parent().unwrap()).unwrap();
    fs::copy(original.join("media").join("jan.jpg"), &m002).unwrap();
    let state = run(&mut api, "import.load", json!({ "texts": [import_answer()], "paths": [path(&m002)] }));
    assert!(state["issues"].as_array().unwrap().iter().any(|i| i["message"].as_str().unwrap().contains("jest już w archiwum")), "{}", state["issues"]);
}

#[test]
fn reopening_finds_the_users_changes() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // The original's path opens the user's version.
    let mut again = computer(dir.path(), "ten");
    let status = run(&mut again, "archive.open", json!({ "path": path(&original) }));
    assert_eq!((status["workMode"].as_str(), status["people"].as_u64()), (Some("overlay"), Some(2)));
    // So does the folder of the changes opened directly; the recent list keeps one entry, the original's.
    let status = run(&mut again, "archive.open", json!({ "path": path(&mine) }));
    assert_eq!((status["workMode"].as_str(), status["root"].as_str()), (Some("overlay"), Some(path(&original).as_str())));
    let recent = run(&mut again, "app.state", Value::Null)["recent"].clone();
    assert_eq!(recent.as_array().unwrap().len(), 1);
    assert_eq!(recent[0]["path"], path(&original));

    // aplikacja.json lost: the changes are found again.
    fs::remove_dir_all(dir.path().join("ten").join("ustawienia")).unwrap();
    let mut fresh = computer(dir.path(), "ten");
    let status = run(&mut fresh, "archive.open", json!({ "path": path(&original) }));
    assert_eq!(status["workMode"], "overlay");
    assert!(status["notice"].as_str().unwrap().contains("Znaleziono Twoje wcześniejsze zmiany"));

    // A copy of the folder (same id, the original still there) is another archive: asked the question.
    let copy = dir.path().join("Kopia rodziny");
    copy_dir(&original, &copy);
    assert!(run(&mut fresh, "archive.open", json!({ "path": path(&copy) }))["workMode"].is_null());

    // The original moved (another drive letter): found by its id.
    let moved = dir.path().join("Przeniesione");
    fs::rename(&original, &moved).unwrap();
    let status = run(&mut fresh, "archive.open", json!({ "path": path(&moved) }));
    assert_eq!((status["workMode"].as_str(), status["people"].as_u64()), (Some("overlay"), Some(2)));
    assert_eq!(status["root"], path(&moved));
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn a_missing_original_opens_the_users_version_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // The pendrive is unplugged.
    let away = dir.path().join("schowane");
    fs::rename(&original, &away).unwrap();
    let mut again = computer(dir.path(), "ten");
    let status = run(&mut again, "archive.open", json!({ "path": path(&original) }));
    assert_eq!((status["workMode"].as_str(), status["overlay"]["originalMissing"].as_bool()), (Some("overlay"), Some(true)));
    assert_eq!(code(&mut again, "archive.writeBack", json!({ "author": "Ewa" })), "original_missing");
    run(&mut again, "person.create", json!({ "given": "Piotr" }));
    run(&mut again, "archive.save", json!({ "author": "Ewa" }));
    // „Wskaż, gdzie jest teraz…”.
    let status = run(&mut again, "archive.relocateOriginal", json!({ "path": path(&away) }));
    assert_eq!((status["overlay"]["originalMissing"].as_bool(), status["root"].as_str()), (Some(false), Some(path(&away).as_str())));
}

/// Someone saves into the original (another computer working in place).
fn their_edit(dir: &Path, original: &Path, given: &str) {
    let mut other = computer(dir, "inny-komputer");
    run(&mut other, "archive.open", json!({ "path": path(original) }));
    let jan = run(&mut other, "people.list", json!({}))["people"].as_array().unwrap().iter().find(|p| p["given"].as_str().unwrap().starts_with("Jan")).unwrap()["id"].as_str().unwrap().to_string();
    run(&mut other, "person.update", json!({ "id": jan, "given": given, "surname": "Kowalski" }));
    run(&mut other, "archive.save", json!({ "author": "Adam" }));
}

#[test]
fn a_changed_original_is_taken_in_and_merged() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    // No own changes: taken in at the next open, silently.
    their_edit(dir.path(), &original, "Jan Paweł");
    let mut again = computer(dir.path(), "ten");
    let status = run(&mut again, "archive.open", json!({ "path": path(&original) }));
    assert_eq!(status["notice"], "Wczytano nowe zmiany z archiwum.");
    assert_eq!(status["overlay"]["pending"], 0);
    assert!(run(&mut again, "people.list", json!({}))["people"].as_array().unwrap().iter().any(|p| p["given"] == "Jan Paweł"));
    // Both changed: merged, a clash reported and chosen.
    let jan = run(&mut again, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    run(&mut again, "person.update", json!({ "id": jan, "given": "Jan Piotr", "surname": "Kowalski", "occupation": "Kowal" }));
    run(&mut again, "person.create", json!({ "given": "Anna" }));
    run(&mut again, "archive.save", json!({ "author": "Ewa" }));
    their_edit(dir.path(), &original, "Janek");
    assert_eq!(run(&mut again, "archive.status", Value::Null)["overlay"]["originalChanged"], true);
    assert_eq!(code(&mut again, "archive.writeBack", json!({ "author": "Ewa" })), "original_changed");
    let result = run(&mut again, "archive.rebase", json!({}))["result"].clone();
    assert_eq!(result["merged"], false);
    assert_eq!(result["conflicts"].as_array().unwrap().len(), 1);
    let key = result["conflicts"][0]["key"].as_str().unwrap().to_string();
    // „Pokaż różnice”: what each side changed from the base, worded like the history feed.
    let conflict = &result["conflicts"][0];
    let new_values = |side: &str| -> Vec<String> { conflict[side].as_array().unwrap().iter().map(|c| c["new"].as_str().unwrap_or("").to_string()).collect() };
    assert!(new_values("mineChanges").iter().any(|v| v.contains("Jan Piotr")), "{conflict}");
    assert!(new_values("theirChanges").iter().any(|v| v.contains("Janek")), "{conflict}");
    let preview = run(&mut again, "archive.rebase", json!({ "preview": true }))["result"].clone();
    assert!(preview["since"].as_str().is_some_and(|s| s.len() >= 10), "the date of the last sync: {preview}");
    let result = run(&mut again, "archive.rebase", json!({ "choices": { key: "theirs" } }));
    assert_eq!(result["result"]["merged"], true);
    assert_eq!(result["status"]["overlay"]["originalChanged"], false);
    let people = run(&mut again, "people.list", json!({}))["people"].clone();
    let names: Vec<&str> = people.as_array().unwrap().iter().map(|p| p["given"].as_str().unwrap()).collect();
    assert!(names.contains(&"Janek") && names.contains(&"Anna"), "{names:?}");
    // Choosing the archive's name does not drop the user's separate edit of the same person (the trade).
    let jan_now = people.as_array().unwrap().iter().find(|p| p["given"] == "Janek").unwrap()["id"].as_str().unwrap().to_string();
    assert!(run(&mut again, "person.get", json!({ "id": jan_now })).to_string().contains("Kowal\""), "my non-clashing edit survives choosing theirs");
    // The merge is one change of its own, from the archive.
    let history = fs::read_to_string(PathBuf::from(result["status"]["sidecar"].as_str().unwrap()).join("historia.jsonl")).unwrap();
    assert!(history.lines().last().unwrap().contains("\"kind\":\"fromArchive\""));
    let feed = run(&mut again, "history.feed", json!({ "filter": "all", "limit": 50 }));
    assert_eq!(feed["rows"][0]["who"], "Zmiany z archiwum", "{feed}");
}

#[test]
fn rebase_preview_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    their_edit(dir.path(), &original, "Janek");
    let before = snapshot(&PathBuf::from(run(&mut api, "archive.status", Value::Null)["overlay"]["folder"].as_str().unwrap()));
    let result = run(&mut api, "archive.rebase", json!({ "preview": true }))["result"].clone();
    assert_eq!((result["merged"].as_bool(), result["conflicts"].as_array().map(Vec::len)), (Some(false), Some(0)));
    assert_eq!((result["theirChanges"].as_u64(), result["myChanges"].as_u64()), (Some(1), Some(1)));
    let status = run(&mut api, "archive.status", Value::Null);
    assert_eq!((status["overlay"]["originalChanged"].as_bool(), status["unsavedChanges"].as_u64()), (Some(true), Some(0)));
    assert_eq!(snapshot(&PathBuf::from(status["overlay"]["folder"].as_str().unwrap())), before, "nothing was written");
    // The real thing then merges.
    assert_eq!(run(&mut api, "archive.rebase", json!({}))["result"]["merged"], true);
}

/// Moving files (Part B) is saved as a batch of kind "layout": not an import, and not undone line by line.
#[test]
fn layout_entries_are_not_imports_and_are_not_undone_one_by_one() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let history = original.join(".heirloom").join("historia.jsonl");
    let entry = heirloom_core::history::Entry {
        ts: "2026-10-10T10:00:00Z".into(),
        author: "Ewa".into(),
        batch: Some("porzadkowanie-1".into()),
        action: heirloom_core::history::Action::Update,
        record: "@O1@".into(),
        tag: "OBJE".into(),
        before: None,
        after: None,
        note: Some("Uporządkowano pliki".into()),
        kind: Some("layout".into()),
    };
    heirloom_core::history::append(&history, &[entry]).unwrap();
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.open", json!({ "path": path(&original) }));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" }));
    let imports = run(&mut api, "import.history", Value::Null);
    assert!(imports.as_array().unwrap().iter().all(|b| b["name"] != "porzadkowanie-1"), "{imports}");
    let feed = run(&mut api, "history.feed", json!({ "filter": "all", "limit": 50 }));
    let row = feed["rows"].as_array().unwrap().iter().find(|r| r["batch"] == "porzadkowanie-1").unwrap();
    assert_eq!((row["action"].as_str(), row["undo"].as_str()), (Some("Uporządkowano pliki"), Some("Cofnij porządkowanie")));
    assert_eq!(code(&mut api, "history.undo", json!({ "batch": "porzadkowanie-1" })), "layout_entry");
    let index = row["index"].clone();
    assert_eq!(code(&mut api, "history.undo", json!({ "index": index })), "layout_entry");
}

#[test]
fn the_write_back_moves_the_changes_into_the_archive() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let history_before = fs::read_to_string(original.join(".heirloom").join("historia.jsonl")).unwrap();
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    let anna = run(&mut api, "person.create", json!({ "given": "Anna", "surname": "Kowalska" }))["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("anna.jpg");
    photo(&new_photo, 50);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": anna }));
    run(&mut api, "archive.save", json!({ "author": "Zosia" }));
    let preview = run(&mut api, "archive.writeBackPreview", Value::Null);
    assert_eq!((preview["changes"].as_u64(), preview["files"].as_u64(), preview["writable"].as_bool()), (Some(2), Some(1), Some(true)));
    let before = fs::read(original.join("rodzina.ged")).unwrap();
    let done = run(&mut api, "archive.writeBack", json!({ "author": "Zosia" }));
    assert_eq!(done["status"]["overlay"]["pending"], 0);
    assert_eq!(done["status"]["overlay"]["canUndoWriteBack"], true);
    let data = fs::read_to_string(original.join("rodzina.ged")).unwrap();
    assert!(data.contains("Anna /Kowalska/") && data.contains("media/anna.jpg"));
    assert_eq!(fs::read(original.join("media").join("anna.jpg")).unwrap(), fs::read(&new_photo).unwrap());
    let history = fs::read_to_string(original.join(".heirloom").join("historia.jsonl")).unwrap();
    assert!(history.starts_with(&history_before) && history.len() > history_before.len());
    assert!(history.contains("\"author\":\"Zosia\""));
    let settings = fs::read_to_string(original.join(".heirloom").join("ustawienia.json")).unwrap();
    assert!(settings.contains("Zosia"), "the original's editors know who wrote");
    // A copy of the original as it was, kept by the user's folder.
    let copies: Vec<PathBuf> = fs::read_dir(mine.join(".heirloom").join("kopie")).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().contains("-oryginal-")).collect();
    assert_eq!(fs::read(&copies[0]).unwrap(), before);

    // Undo: the original's bytes and history come back, its new photo goes; the user's photo still shows.
    let status = run(&mut api, "archive.undoWriteBack", Value::Null);
    assert_eq!(fs::read(original.join("rodzina.ged")).unwrap(), before);
    assert_eq!(fs::read_to_string(original.join(".heirloom").join("historia.jsonl")).unwrap(), history_before);
    assert!(!original.join("media").join("anna.jpg").exists());
    assert_eq!(status["overlay"]["pending"], 2);
    assert!(run(&mut api, "media.list", Value::Null)["items"].as_array().unwrap().iter().all(|i| i["missing"] == false));

    // Again, then someone changes the archive: no undo any more, and the user's copies of what the archive has go.
    run(&mut api, "archive.writeBack", json!({ "author": "Zosia" }));
    their_edit(dir.path(), &original, "Janek");
    assert_eq!(code(&mut api, "archive.undoWriteBack", Value::Null), "cannot_undo");
    let mut again = computer(dir.path(), "ten");
    run(&mut again, "archive.open", json!({ "path": path(&original) }));
    assert!(!mine.join("media").join("anna.jpg").exists(), "settled: the archive has the photo");
    assert!(run(&mut again, "media.list", Value::Null)["items"].as_array().unwrap().iter().all(|i| i["missing"] == false));
}

#[test]
fn a_clashing_file_is_renamed_before_the_write_back() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    let jan = run(&mut api, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("slub.jpg");
    photo(&new_photo, 60);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": jan }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // Meanwhile the archive got another slub.jpg (put there by hand; the data doesn't change).
    photo(&original.join("media").join("slub.jpg"), 61);
    run(&mut api, "archive.writeBack", json!({ "author": "Ewa" }));
    assert_eq!(fs::read(original.join("media").join("slub (2).jpg")).unwrap(), fs::read(&new_photo).unwrap());
    assert_ne!(fs::read(original.join("media").join("slub.jpg")).unwrap(), fs::read(&new_photo).unwrap(), "the archive's file stays");
    assert!(fs::read_to_string(original.join("rodzina.ged")).unwrap().contains("media/slub%20(2).jpg"));
    assert!(mine.join("media").join("slub (2).jpg").is_file());
}

#[test]
fn another_programs_file_needs_a_yes_and_gets_no_heirloom_folder() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Pobrane");
    fs::create_dir_all(&folder).unwrap();
    let ged = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n0 TRLR\n";
    fs::write(folder.join("drzewo.ged"), ged).unwrap();
    let mut api = computer(dir.path(), "ten");
    let status = run(&mut api, "archive.open", json!({ "path": path(&folder.join("drzewo.ged")) }));
    assert!(status["workMode"].is_null());
    let info = run(&mut api, "archive.modeInfo", Value::Null);
    assert_eq!((info["foreign"].as_str(), info["suggested"].as_str()), (Some("MYHERITAGE"), Some("overlay")));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    run(&mut api, "person.create", json!({ "given": "Anna", "surname": "Nowak" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    assert_eq!(fs::read_to_string(folder.join("drzewo.ged")).unwrap(), ged);
    let preview = run(&mut api, "archive.writeBackPreview", Value::Null);
    assert_eq!((preview["foreign"]["origin"].as_str(), preview["foreign"]["version"].as_str()), (Some("MYHERITAGE"), Some("5.5.1")));
    assert_eq!(code(&mut api, "archive.writeBack", json!({ "author": "Ewa" })), "foreign");
    run(&mut api, "archive.writeBack", json!({ "author": "Ewa", "allowForeign": true }));
    assert!(fs::read_to_string(folder.join("drzewo.ged")).unwrap().contains("Anna /Nowak/"));
    let names: Vec<String> = fs::read_dir(&folder).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, ["drzewo.ged"], "nothing of Heirloom's in another program's folder");
}

#[test]
fn a_locked_original_refuses_the_write_back() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut owner = computer(dir.path(), "inny-komputer");
    run(&mut owner, "archive.open", json!({ "path": path(&original) }));
    run(&mut owner, "archive.setSettings", json!({ "readOnly": true }));
    run(&mut owner, "archive.close", Value::Null);
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    assert_eq!(run(&mut api, "archive.status", Value::Null)["readOnly"], false, "the user's version starts unlocked");
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    assert_eq!(run(&mut api, "archive.writeBackPreview", Value::Null)["lockedOriginal"], true);
    assert_eq!(code(&mut api, "archive.writeBack", json!({ "author": "Ewa" })), "locked_original");
}

/// A write-back cut off (the program closed, a pendrive pulled out) is finished or taken back at the next open.
#[test]
fn an_interrupted_write_back_is_finished_or_taken_back() {
    for (step, finished) in [("header", false), ("copy", false), ("written", true)] {
        let dir = tempfile::tempdir().unwrap();
        let original = family(dir.path());
        let before = fs::read(original.join("rodzina.ged")).unwrap();
        let history_before = fs::read(original.join(".heirloom").join("historia.jsonl")).unwrap();
        let mut api = computer(dir.path(), "ten");
        overlay_mode(&mut api, &original);
        let anna = run(&mut api, "person.create", json!({ "given": "Anna" }))["id"].as_str().unwrap().to_string();
        let new_photo = dir.path().join("zrodla").join("anna.jpg");
        photo(&new_photo, 50);
        run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": anna }));
        run(&mut api, "archive.save", json!({ "author": "Ewa" }));
        writeback::stop_after(Some(step));
        assert_eq!(code(&mut api, "archive.writeBack", json!({ "author": "Ewa" })), "stopped_for_test");
        writeback::stop_after(None);
        drop(api);
        let mut again = computer(dir.path(), "ten");
        let status = run(&mut again, "archive.open", json!({ "path": path(&original) }));
        if finished {
            assert_eq!(status["notice"], "Przenoszenie zmian do archiwum zostało teraz dokończone.", "{step}");
            assert_eq!(status["overlay"]["pending"], 0);
            assert!(fs::read_to_string(original.join("rodzina.ged")).unwrap().contains("Anna"));
            assert!(fs::read(original.join(".heirloom").join("historia.jsonl")).unwrap().len() > history_before.len());
        } else {
            assert_eq!(fs::read(original.join("rodzina.ged")).unwrap(), before, "{step}");
            assert!(!original.join("media").join("anna.jpg").exists(), "{step}: the copied photo goes again");
            assert!(status["overlay"]["interrupted"].is_string(), "{step}");
            assert_eq!(status["overlay"]["pending"], 2);
            // „Spróbuj ponownie”.
            run(&mut again, "archive.writeBack", json!({ "author": "Ewa" }));
            assert!(original.join("media").join("anna.jpg").is_file());
        }
    }
}

#[test]
fn switching_modes() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    run(&mut api, "person.create", json!({ "given": "Anna" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    assert_eq!(code(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" })), "pending_changes");
    // „Porzuć moje zmiany…”: renamed, never deleted; a fresh start from the archive.
    let status = run(&mut api, "archive.discardOverlay", Value::Null);
    assert_eq!((status["people"].as_u64(), status["overlay"]["pending"].as_u64()), (Some(1), Some(0)));
    let parent = mine.parent().unwrap();
    let kept: Vec<String> = fs::read_dir(parent).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    let day = kept.iter().find_map(|n| n.strip_prefix("Rodzina Kowalskich (porzucone ")?.strip_suffix(')')).unwrap_or_else(|| panic!("{kept:?}")).to_string();
    assert_eq!(day.len(), 10, "just the date, once: {kept:?}");
    // After a write-back, switching to in place retires the folder.
    run(&mut api, "person.create", json!({ "given": "Piotr" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    run(&mut api, "archive.writeBack", json!({ "author": "Ewa" }));
    let status = run(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" }));
    assert_eq!((status["workMode"].as_str(), status["people"].as_u64()), (Some("inPlace"), Some(2)));
    assert_eq!(status["notice"], "Od teraz zmiany zapisują się w archiwum.");
    let names: Vec<String> = fs::read_dir(parent).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(names.iter().any(|n| *n == format!("Rodzina Kowalskich (przeniesione do archiwum {day} – można usunąć)")), "{names:?}");
    assert!(!mine.exists());
    // And back: nothing is written into the archive.
    let before = snapshot(&original);
    run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    assert_eq!(snapshot(&original), before);
}

#[test]
fn archives_opened_together_show_the_users_version() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    let anna = run(&mut api, "person.create", json!({ "given": "Anna", "surname": "Kowalska" }))["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("anna.jpg");
    photo(&new_photo, 50);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": anna }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let other = dir.path().join("Nowakowie");
    run(&mut api, "archive.create", json!({ "folder": path(&other), "name": "Nowakowie" }));
    run(&mut api, "person.create", json!({ "given": "Zofia", "surname": "Nowak" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let set = dir.path().join("Razem.heirloom-zestaw");
    let status = run(&mut api, "set.create", json!({ "path": path(&set), "name": "Razem", "archives": [path(&original), path(&other)] }));
    assert_eq!(status["people"], 3, "Jan and Anna (the user's version) and Zofia");
    assert_eq!(status["combined"]["archives"][0]["withChanges"], true);
    assert_eq!(status["combined"]["archives"][0]["pending"], 2, "Anna and the photo link of her: the count behind „(z Twoimi zmianami: 2)”");
    assert_eq!(status["combined"]["archives"][1]["pending"], 0);
    assert_eq!(status["combined"]["archives"][0]["path"], path(&original), "the set names the original");
    let media = run(&mut api, "media.list", Value::Null);
    let items = media["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|i| i["missing"] == false), "{items:?}");
    let roots = api.media_roots();
    for file in ["file/~a/media/jan.jpg", "file/~a/media/anna.jpg"] {
        assert!(matches!(heirloom_api::media::serve(&roots, file), heirloom_api::media::Served::Ok { .. }), "{file}");
    }
    // A set file isn't written into the untouched original.
    let inside = original.join("Razem.heirloom-zestaw");
    assert_eq!(code(&mut api, "set.create", json!({ "path": path(&inside), "name": "R", "archives": [path(&original), path(&other)] })), "inside_archive");
}

fn zip_names(path: &Path) -> Vec<String> {
    let zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
    let mut names: Vec<String> = zip.file_names().map(str::to_string).collect();
    names.sort();
    names
}

#[test]
fn automatic_backups() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    // A 5.5.1-style file outside media/ that the data points to.
    photo(&original.join("zdjecia").join("dom.jpg"), 70);
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.open", json!({ "path": path(&original) }));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" }));
    let jan = run(&mut api, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    run(&mut api, "media.add", json!({ "paths": [path(&original.join("zdjecia").join("dom.jpg"))], "person": jan }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let info = run(&mut api, "archive.backupInfo", json!({}));
    assert!(info["lastZip"].is_null() && info["current"] == false);
    assert!(info["folderLabel"].as_str().unwrap().starts_with("Dokumenty › Heirloom › Kopie zapasowe › Rodzina Kowalskich"));
    let first = PathBuf::from(run(&mut api, "archive.backupAuto", json!({}))["path"].as_str().unwrap());
    assert!(first.starts_with(dir.path().join("ten").join("dom").join("Dokumenty").join("Heirloom").join("Kopie zapasowe").join("Rodzina Kowalskich")));
    let names = zip_names(&first);
    assert!(names.contains(&"zdjecia/dom.jpg".to_string()) && names.contains(&"media/jan.jpg".to_string()) && names.contains(&"rodzina.ged".to_string()), "{names:?}");
    assert!(!names.iter().any(|n| n.starts_with(".heirloom/kopie")));
    assert_eq!(run(&mut api, "archive.backupInfo", json!({}))["current"], true, "nothing changed since");
    // Rotation: the newest five automatic backups; a file Heirloom didn't make stays.
    let folder = first.parent().unwrap().to_path_buf();
    fs::write(folder.join("moja kopia.zip"), b"x").unwrap();
    let mut made = vec![first.clone()];
    for _ in 0..5 {
        made.push(PathBuf::from(run(&mut api, "archive.backupAuto", json!({}))["path"].as_str().unwrap()));
    }
    assert!(made[1].file_name().unwrap().to_string_lossy().contains(" (2)"), "a unique name within the minute");
    assert!(!first.exists(), "the oldest went");
    assert_eq!(made[1..].iter().filter(|p| p.exists()).count(), 5);
    assert!(folder.join("moja kopia.zip").exists());
    // Too little room.
    heirloom_api::backup::set_available_space(Some(1000));
    assert_eq!(code(&mut api, "archive.backupAuto", json!({})), "no_space");
    heirloom_api::backup::set_available_space(None);
    // Never inside the archive.
    run(&mut api, "app.setBackupFolder", json!({ "folder": path(&original.join("kopie")) }));
    assert_eq!(code(&mut api, "archive.backupAuto", json!({})), "inside_archive");
}

#[test]
fn backups_in_overlay_mode_and_restore() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    overlay_mode(&mut api, &original);
    let anna = run(&mut api, "person.create", json!({ "given": "Anna" }))["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("anna.jpg");
    photo(&new_photo, 50);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": anna }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // The user's version: a standalone archive with every photo.
    let mine_zip = run(&mut api, "archive.backupAuto", json!({ "scope": "archive" }))["path"].as_str().unwrap().to_string();
    let names = zip_names(Path::new(&mine_zip));
    assert!(names.contains(&"media/anna.jpg".to_string()) && names.contains(&"media/jan.jpg".to_string()), "{names:?}");
    assert!(!names.iter().any(|n| n.ends_with("oryginal.json") || n.ends_with("baza.ged")));
    // Never inside the user's folder, nor inside the original.
    let mine_folder = PathBuf::from(run(&mut api, "archive.status", Value::Null)["overlay"]["folder"].as_str().unwrap());
    for inside in [mine_folder.join("kopie"), original.join("kopie")] {
        run(&mut api, "app.setBackupFolder", json!({ "folder": path(&inside) }));
        assert_eq!(code(&mut api, "archive.backupAuto", json!({})), "inside_archive", "{inside:?}");
    }
    run(&mut api, "app.setBackupFolder", json!({ "folder": path(&dir.path().join("kopie-zapasowe")) }));
    // The original itself.
    let original_zip = run(&mut api, "archive.backupAuto", json!({ "scope": "original" }))["path"].as_str().unwrap().to_string();
    let names = zip_names(Path::new(&original_zip));
    assert!(!names.contains(&"media/anna.jpg".to_string()) && names.contains(&"media/jan.jpg".to_string()));
    // Restored: a new folder next to the archive, opened, in place; nothing overwritten.
    let restored = run(&mut api, "archive.restore", json!({ "zip": mine_zip }));
    assert_eq!((restored["workMode"].as_str(), restored["people"].as_u64(), restored["overlay"].is_null()), (Some("inPlace"), Some(2), true));
    let root = PathBuf::from(restored["root"].as_str().unwrap());
    assert_eq!(root.parent(), original.parent());
    assert!(root.join("media").join("anna.jpg").is_file() && root.join("media").join("jan.jpg").is_file());
    assert!(run(&mut api, "media.list", Value::Null)["items"].as_array().unwrap().iter().all(|i| i["missing"] == false));
    let again = run(&mut api, "archive.restore", json!({ "zip": mine_zip }));
    assert_ne!(again["root"], restored["root"], "a second restore gets a folder of its own");
}

#[test]
fn a_restore_refuses_paths_out_of_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("zly.zip");
    {
        let mut zip = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("rodzina.ged", options).unwrap();
        std::io::Write::write_all(&mut zip, b"0 HEAD\n0 TRLR\n").unwrap();
        zip.start_file("../poza.txt", options).unwrap();
        std::io::Write::write_all(&mut zip, b"x").unwrap();
        zip.finish().unwrap();
    }
    let mut api = computer(dir.path(), "ten");
    assert_eq!(code(&mut api, "archive.restore", json!({ "zip": path(&zip_path), "folder": path(&dir.path().join("cel")) })), "bad_file");
    assert!(!dir.path().join("poza.txt").exists());
    let empty = dir.path().join("pusty.zip");
    zip::ZipWriter::new(fs::File::create(&empty).unwrap()).finish().unwrap();
    assert_eq!(code(&mut api, "archive.restore", json!({ "zip": path(&empty), "folder": path(&dir.path().join("cel")) })), "bad_file");
}

/// A 5.5.1 file of another program: the user's version is GEDCOM 7 after the first save, the archive still 5.5.1.
#[test]
fn a_5_5_1_original_is_merged_after_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Pobrane");
    fs::create_dir_all(&folder).unwrap();
    let ged = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n0 @I2@ INDI\n1 NAME Ewa /Nowak/\n0 TRLR\n";
    fs::write(folder.join("drzewo.ged"), ged).unwrap();
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.open", json!({ "path": path(&folder.join("drzewo.ged")) }));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }));
    run(&mut api, "person.create", json!({ "given": "Anna", "surname": "Nowak" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    assert_eq!(run(&mut api, "archive.status", Value::Null)["gedcomVersion"], "7.0");
    // The other program changes its file (still 5.5.1).
    fs::write(folder.join("drzewo.ged"), ged.replace("Jan /Nowak/", "Janek /Nowak/")).unwrap();
    assert_eq!(run(&mut api, "archive.status", Value::Null)["overlay"]["originalChanged"], true);
    let preview = run(&mut api, "archive.rebase", json!({ "preview": true }))["result"].clone();
    assert_eq!((preview["conflicts"].as_array().map(Vec::len), preview["theirChanges"].as_u64()), (Some(0), Some(1)), "{preview}");
    assert_eq!(run(&mut api, "archive.rebase", json!({}))["result"]["merged"], true);
    let people = run(&mut api, "people.list", json!({}))["people"].clone();
    let names: Vec<&str> = people.as_array().unwrap().iter().map(|p| p["given"].as_str().unwrap()).collect();
    assert!(names.contains(&"Janek") && names.contains(&"Anna") && names.contains(&"Ewa") && !names.contains(&"Jan"), "{names:?}");
}

/// A set-aside folder opened directly (the discard text invites it) is only looked at: it never takes the place of the
/// current changes of the original (review finding F1).
#[test]
fn set_aside_changes_opened_directly_never_replace_the_current_ones() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let old = overlay_mode(&mut api, &original);
    run(&mut api, "person.create", json!({ "given": "Stara", "surname": "Zmiana" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let fresh = PathBuf::from(run(&mut api, "archive.discardOverlay", Value::Null)["overlay"]["folder"].as_str().unwrap());
    run(&mut api, "person.create", json!({ "given": "Nowa", "surname": "Zmiana" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    let kept: Vec<PathBuf> = fs::read_dir(old.parent().unwrap()).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().contains("porzucone")).collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    assert!(overlay_retired(&kept[0]), "marked before the rename");
    // Opened directly: looked at, with a notice, and nothing attached to the original.
    let status = run(&mut api, "archive.open", json!({ "path": path(&kept[0]) }));
    assert!(status["notice"].as_str().unwrap().starts_with("To są wcześniejsze, odłożone zmiany."), "{status}");
    assert!(status["overlay"].is_null());
    let names = |api: &mut Api| -> Vec<String> { run(api, "people.list", json!({}))["people"].as_array().unwrap().iter().map(|p| p["given"].as_str().unwrap().to_string()).collect() };
    assert!(names(&mut api).contains(&"Stara".to_string()));
    // The original opened again: the CURRENT changes, not the old ones.
    let mut again = computer(dir.path(), "ten");
    let _ = &mut again;
    let status = run(&mut api, "archive.open", json!({ "path": path(&original) }));
    assert_eq!(status["workMode"], "overlay");
    assert_eq!(PathBuf::from(status["overlay"]["folder"].as_str().unwrap()), fresh);
    let n = names(&mut api);
    assert!(n.contains(&"Nowa".to_string()) && !n.contains(&"Stara".to_string()), "{n:?}");
    // A retired folder is marked too when it is retired by a switch.
    run(&mut api, "archive.writeBack", json!({ "author": "Ewa" }));
    run(&mut api, "archive.setWorkMode", json!({ "mode": "inPlace" }));
    let moved: Vec<PathBuf> = fs::read_dir(fresh.parent().unwrap()).unwrap().flatten().map(|e| e.path()).filter(|p| p.to_string_lossy().contains("przeniesione")).collect();
    assert_eq!(moved.len(), 1);
    assert!(overlay_retired(&moved[0]));
}

fn overlay_retired(folder: &Path) -> bool {
    heirloom_api::overlay::read_manifest(folder).is_some_and(|m| m.retired)
}

/// A file that cannot be moved aside stops the merge before anything is saved (review finding F2).
#[cfg(unix)]
#[test]
fn a_file_that_cannot_move_aside_stops_the_rebase() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    let jan = run(&mut api, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    let new_photo = dir.path().join("zrodla").join("slub.jpg");
    photo(&new_photo, 60);
    run(&mut api, "media.add", json!({ "paths": [path(&new_photo)], "person": jan }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    photo(&original.join("media").join("slub.jpg"), 61);
    let history = fs::read(mine.join(".heirloom").join("historia.jsonl")).unwrap();
    let data = fs::read(mine.join("rodzina.ged")).unwrap();
    fs::set_permissions(mine.join("media"), fs::Permissions::from_mode(0o555)).unwrap();
    let failed = code(&mut api, "archive.rebase", json!({}));
    fs::set_permissions(mine.join("media"), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(failed, "io");
    assert_eq!(fs::read(mine.join("rodzina.ged")).unwrap(), data, "nothing saved");
    assert_eq!(fs::read(mine.join(".heirloom").join("historia.jsonl")).unwrap(), history);
    assert!(mine.join("media").join("slub.jpg").is_file());
    // With the folder writable again the merge goes through.
    assert_eq!(run(&mut api, "archive.rebase", json!({}))["result"]["merged"], true);
    assert!(mine.join("media").join("slub (2).jpg").is_file());
}

/// A refused merge leaves nothing behind in the open archive, not even the conversion to GEDCOM 7 (review finding F3).
#[test]
fn a_refused_merge_leaves_no_conversion_behind() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Pobrane");
    fs::create_dir_all(&folder).unwrap();
    let old = "0 HEAD\n1 SOUR MYHERITAGE\n1 GEDC\n2 VERS 5.5.1\n1 CHAR UTF-8\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n1 _UID AAA\n0 @I2@ INDI\n1 NAME Ewa /Nowak/\n1 _UID BBB\n0 TRLR\n";
    fs::write(folder.join("drzewo.ged"), old).unwrap();
    let mut api = computer(dir.path(), "ten");
    run(&mut api, "archive.open", json!({ "path": path(&folder.join("drzewo.ged")) }));
    let mine = PathBuf::from(run(&mut api, "archive.setWorkMode", json!({ "mode": "overlay" }))["overlay"]["folder"].as_str().unwrap());
    run(&mut api, "archive.close", Value::Null);
    // The user's version, still 5.5.1 (edited by hand), has a change of its own.
    fs::write(mine.join("drzewo.ged"), old.replace("Ewa /Nowak/", "Ewa /Nowak/\n1 SEX F")).unwrap();
    // The archive is replaced by an unrelated GEDCOM 7 file.
    let other = "0 HEAD\n1 GEDC\n2 VERS 7.0\n0 @I1@ INDI\n1 NAME Zofia /Inna/\n1 UID 11111111-1111-4111-8111-111111111111\n0 @I2@ INDI\n1 NAME Piotr /Inny/\n1 UID 22222222-2222-4222-8222-222222222222\n0 TRLR\n";
    fs::write(folder.join("drzewo.ged"), other).unwrap();
    let status = run(&mut api, "archive.open", json!({ "path": path(&folder.join("drzewo.ged")) }));
    assert_eq!(status["gedcomVersion"], "5.5.1", "{status}");
    assert_eq!(code(&mut api, "archive.rebase", json!({})), "too_different");
    let status = run(&mut api, "archive.status", Value::Null);
    assert_eq!((status["gedcomVersion"].as_str(), status["unsavedChanges"].as_u64()), (Some("5.5.1"), Some(0)), "{status}");
}

/// Every file under `root` with its bytes (no times).
fn contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    snapshot(root).0.into_iter().map(|(p, (bytes, _))| (p, bytes)).collect()
}

/// The whole story of ticket L02c: edit in „Nie ruszaj oryginału”, someone edits the original (the same field, and
/// another one), merge with one conflict, write back, undo: the original is as it was just before the write-back.
#[test]
fn scenario_conflict_merge_write_back_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let original = family(dir.path());
    let mut api = computer(dir.path(), "ten");
    let mine = overlay_mode(&mut api, &original);
    // The user changes Jan's given name and saves into his own folder.
    let jan = run(&mut api, "people.list", json!({}))["people"][0]["id"].as_str().unwrap().to_string();
    run(&mut api, "person.update", json!({ "id": jan, "given": "Jan Piotr", "surname": "Kowalski" }));
    run(&mut api, "archive.save", json!({ "author": "Ewa" }));
    // Someone else changes the same name in the original and, by a text edit, gives Jan a trade as well.
    their_edit(dir.path(), &original, "Janek");
    let ged = fs::read_to_string(original.join("rodzina.ged")).unwrap();
    let at = ged.find("1 NAME Janek").expect("the name was changed");
    let line_end = ged[at..].find('\n').unwrap() + at + 1;
    fs::write(original.join("rodzina.ged"), format!("{}1 OCCU Stolarz\n{}", &ged[..line_end], &ged[line_end..])).unwrap();
    // The write-back says the archive changed; the merge finds the one clash and keeps the user's version.
    assert_eq!(code(&mut api, "archive.writeBack", json!({ "author": "Ewa" })), "original_changed");
    let preview = run(&mut api, "archive.rebase", json!({ "preview": true }))["result"].clone();
    let conflicts = preview["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "{preview}");
    assert_eq!(conflicts[0]["mineChanges"][0]["new"].as_str().unwrap().contains("Jan Piotr"), true);
    assert_eq!(conflicts[0]["theirChanges"][0]["new"].as_str().unwrap().contains("Janek"), true);
    let theirs: Vec<String> = conflicts[0]["theirChanges"].as_array().unwrap().iter().map(|c| c["new"].as_str().unwrap_or("").to_string()).collect();
    assert!(theirs.iter().any(|v| v.contains("Stolarz")), "their other change is listed too: {theirs:?}");
    let fields: Vec<&str> = conflicts[0]["fields"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert!(!fields.is_empty() && fields.iter().all(|f| !f.contains("Stolarz")), "the clashing fields are named: {preview}");
    let key = conflicts[0]["key"].as_str().unwrap().to_string();
    let merged = run(&mut api, "archive.rebase", json!({ "choices": { key: "mine" } }));
    assert_eq!(merged["result"]["merged"], true);
    let people = run(&mut api, "people.list", json!({}))["people"].clone();
    assert!(people.as_array().unwrap().iter().any(|p| p["given"] == "Jan Piotr"));
    // Just before the write-back.
    let before = contents(&original);
    let done = run(&mut api, "archive.writeBack", json!({ "author": "Ewa" }));
    assert_eq!(done["status"]["overlay"]["canUndoWriteBack"], true);
    let data = fs::read_to_string(original.join("rodzina.ged")).unwrap();
    // The record is kept whole from the side chosen (A.9): the user's name; their trade shows in „Pokaż różnice” only.
    // The choice covers the clashing name only; the archive's separate edit of the same record (the trade) survives.
    assert!(data.contains("Jan Piotr /Kowalski/") && !data.contains("Janek /"), "{data}");
    assert!(data.contains("1 OCCU Stolarz"), "the archive's non-clashing edit survives choosing mine: {data}");
    run(&mut api, "archive.undoWriteBack", Value::Null);
    let after = contents(&original);
    // The data, the history and every photo are byte for byte as before. (The copy the write-back made in kopie\ stays
    // on purpose, and the editors' „last edited” in ustawienia.json is not taken back.)
    for (path, bytes) in &before {
        let rel = path.strip_prefix(&original).unwrap_or(path);
        if rel.ends_with("ustawienia.json") {
            eprintln!("ustawienia.json {} after the undo", if after.get(path) == Some(bytes) { "is the same" } else { "differs (editors)" });
            continue;
        }
        assert_eq!(after.get(path), Some(bytes), "{} differs after the undo", rel.display());
    }
    let extra: Vec<String> = after.keys().filter(|p| !before.contains_key(*p)).map(|p| p.strip_prefix(&original).unwrap().display().to_string()).collect();
    assert!(extra.iter().all(|p| p.contains("kopie")), "only the safety copy is new: {extra:?}");
    assert_eq!(run(&mut api, "archive.status", Value::Null)["overlay"]["pending"].as_u64().unwrap() > 0, true, "the user's changes wait again");
    assert!(mine.is_dir());
}
