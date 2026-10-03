//! The screens' read commands on the official GEDCOM 7 test files from gedcom.io (see
//! `heirloom-core/tests/gedcom_io.rs`): unusual but legal data must not make any of them fail.

use heirloom_api::Api;
use serde_json::{Value, json};
use std::path::PathBuf;

#[test]
fn every_screen_reads_the_official_test_files() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-files/gedcom.io");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("No test-files/gedcom.io: run scripts/fetch-gedcom-test-files.ps1 to include this test.");
        return;
    };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ged")).collect();
    files.sort();
    let mut failures = Vec::new();
    let (mut opened, mut checked) = (0, 0);
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // A copy, so opening it can't leave anything next to the downloaded files.
        let folder = tempfile::tempdir().unwrap();
        let copy = folder.path().join(&name);
        std::fs::copy(&path, &copy).unwrap();
        let mut api = Api::new(None, None);
        let mut check = |api: &mut Api, method: &str, args: Value| -> Option<Value> {
            match api.call(method, args.clone()) {
                Ok(v) => Some(v),
                Err(e) => {
                    failures.push(format!("{name}: {method} {args} → {} ({})", e.message, e.code));
                    None
                }
            }
        };
        if check(&mut api, "archive.open", json!({ "path": copy })).is_none() {
            continue;
        }
        opened += 1;
        for method in ["archive.status", "start.data", "archive.firstOpen", "surnames.list", "places.list", "stories.list", "media.list", "media.missing", "sources.list", "archive.check"] {
            check(&mut api, method, json!({}));
        }
        check(&mut api, "history.feed", json!({ "limit": 50 }));
        let people: Vec<String> = check(&mut api, "people.list", json!({}))
            .and_then(|l| l["people"].as_array().map(|a| a.iter().filter_map(|p| p["id"].as_str().map(str::to_string)).collect()))
            .unwrap_or_default();
        checked += people.len();
        for id in &people {
            for method in ["person.get", "person.panel", "person.editData", "person.relations", "person.hover"] {
                check(&mut api, method, json!({ "id": id }));
            }
            for (up, down) in [(1, 1), (3, 0), (0, 2)] {
                check(&mut api, "tree.graph", json!({ "id": id, "up": up, "down": down }));
            }
        }
        check(&mut api, "tree.overview", json!({ "focus": people.first() }));
        // Saving the edit form unchanged must change nothing, whatever the file holds.
        for id in &people {
            let Some(mut form) = check(&mut api, "person.editData", json!({ "id": id })) else { continue };
            form["id"] = json!(id);
            check(&mut api, "person.update", form);
        }
        if let Some(status) = check(&mut api, "archive.status", json!({})) {
            if status["unsavedChanges"] != 0 {
                // Save the copy and show what differs from the original.
                check(&mut api, "archive.save", json!({ "author": "test", "allowForeign": true }));
                let (before, after) = (std::fs::read_to_string(&path).unwrap_or_default(), std::fs::read_to_string(&copy).unwrap_or_default());
                let removed: Vec<&str> = before.lines().filter(|l| !after.lines().any(|a| a == *l)).take(4).collect();
                let added: Vec<&str> = after.lines().filter(|l| !before.lines().any(|b| b == *l) && !l.contains("TAG _HLM_") && *l != "1 SCHMA").take(8).collect();
                failures.push(format!("{name}: saving the edit form unchanged left {} changes; removed {removed:?}, added {added:?}", status["unsavedChanges"]));
            }
        }
    }
    eprintln!("{opened} files opened, {checked} people read on every screen");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(opened > 0 && checked > 0, "nothing was checked");
}
