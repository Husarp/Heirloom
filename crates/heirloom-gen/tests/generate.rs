use heirloom_core::Archive;
use heirloom_core::gedcom::model;
use std::collections::HashSet;

#[test]
fn generated_archive_is_consistent_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("test");
    let options = heirloom_gen::Options { people: 600, media: 5, seed: 7, name: "Test".into() };
    heirloom_gen::write_archive(&root, &options).unwrap();

    let archive = Archive::open(&root).unwrap();
    assert!(archive.warnings.is_empty(), "{:?}", archive.warnings);
    assert!(!archive.is_foreign());
    let bytes = std::fs::read(archive.data_path()).unwrap();
    assert_eq!(archive.doc.to_bytes(), bytes, "reading and writing changes nothing");

    let m = model::extract(&archive.doc);
    assert_eq!(m.persons.len(), 600);
    let people: HashSet<&str> = m.persons.iter().map(|p| p.xref.as_str()).collect();
    let families: HashSet<&str> = m.families.iter().map(|f| f.xref.as_str()).collect();
    for p in &m.persons {
        assert!(p.famc.iter().chain(&p.fams).all(|f| families.contains(f.as_str())), "{} points to a missing family", p.xref);
        assert!(p.uid.is_some() && !p.display_name().is_empty());
    }
    for f in &m.families {
        assert!(f.partners.iter().chain(&f.children).all(|x| people.contains(x.as_str())));
    }
    assert!(m.persons.iter().any(|p| p.maiden_name().is_some()), "married women keep maiden names");
    assert!(m.persons.iter().any(|p| p.death.is_none()), "some people are still living");
    let years: Vec<i32> = m.persons.iter().filter_map(|p| p.birth.as_ref()?.date.map(|d| d.start.year)).collect();
    let (first, last) = (years.iter().min().unwrap(), years.iter().max().unwrap());
    assert!(*first < 1750 && *last > 1950, "the family spans the centuries: {first}–{last}");
    for i in 1..=5 {
        assert!(root.join("media").join(format!("M{i:04}.png")).is_file());
    }
}

#[test]
fn same_seed_gives_same_family() {
    let options = heirloom_gen::Options { people: 300, media: 0, seed: 42, name: "A".into() };
    let a = heirloom_gen::generate(&options).0;
    let b = heirloom_gen::generate(&options).0;
    let names = |d: &heirloom_core::gedcom::Document| {
        model::extract(d).persons.iter().map(|p| p.display_name()).collect::<Vec<_>>()
    };
    assert_eq!(names(&a), names(&b));
}
