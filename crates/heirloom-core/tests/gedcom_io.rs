//! The official GEDCOM 7 test files from gedcom.io (FamilySearch), when they have been downloaded into
//! `test-files/gedcom.io/` with `scripts/fetch-gedcom-test-files.ps1` (they are kept out of Git). Every file must
//! come back byte for byte after reading and writing, and the view built from it must not fail.

use heirloom_core::gedcom::{Document, view};
use std::path::PathBuf;

fn test_files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-files/gedcom.io");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "ged")).collect();
    files.sort();
    files
}

#[test]
fn official_test_files_come_back_unchanged() {
    let files = test_files();
    if files.is_empty() {
        eprintln!("No files in test-files/gedcom.io: run scripts/fetch-gedcom-test-files.ps1 to include this test.");
        return;
    }
    let mut failures = Vec::new();
    for path in &files {
        let original = std::fs::read(path).unwrap();
        let (doc, _) = Document::from_bytes(&original);
        let written = doc.to_bytes();
        if written != original {
            let (a, b) = (String::from_utf8_lossy(&original), String::from_utf8_lossy(&written));
            let line = a.lines().zip(b.lines()).position(|(x, y)| x != y).map_or(a.lines().count().min(b.lines().count()), |n| n);
            failures.push(format!(
                "{}: differs at line {}: {:?} → {:?}",
                path.file_name().unwrap().to_string_lossy(),
                line + 1,
                a.lines().nth(line).unwrap_or(""),
                b.lines().nth(line).unwrap_or("")
            ));
        }
        view::build(&doc);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
