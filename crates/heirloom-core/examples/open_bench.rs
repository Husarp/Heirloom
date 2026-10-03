//! Times the core on an archive (PLAN.md §6): open + parse, the people view, building the cache, searching,
//! writing. Run in release mode:
//! `cargo run --release -p heirloom-core --example open_bench -- test-archives/5000`

use heirloom_core::gedcom::model;
use heirloom_core::{Archive, Cache};
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: open_bench <archive folder or .ged file>")?;
    let size = std::fs::metadata(Path::new(&path).join("rodzina.ged")).map(|m| m.len()).unwrap_or(0);

    let t = Instant::now();
    let archive = Archive::open(Path::new(&path))?;
    let open = t.elapsed();

    let t = Instant::now();
    let model = model::extract(&archive.doc);
    let view = t.elapsed();

    let cache_root = std::env::temp_dir().join("heirloom-bench-cache");
    let mut cache = Cache::open(&cache_root, &archive.settings().archive_id)?;
    let t = Instant::now();
    cache.rebuild(&model, &archive.fingerprint_hex())?;
    let rebuild = t.elapsed();

    let t = Instant::now();
    let fresh = cache.is_fresh(&archive.fingerprint_hex())?;
    let fresh_check = t.elapsed();

    let t = Instant::now();
    let hits = cache.search("kowal", 50)?;
    let search = t.elapsed();

    let t = Instant::now();
    let bytes = archive.doc.to_bytes();
    let write = t.elapsed();

    println!("archive:        {path} ({:.1} MB)", size as f64 / 1e6);
    println!("people:         {} (families: {})", model.persons.len(), model.families.len());
    println!("open + parse:   {open:?} ({} warnings)", archive.warnings.len());
    println!("people view:    {view:?}");
    println!("cache rebuild:  {rebuild:?}");
    println!("cache fresh?:   {fresh} ({fresh_check:?})");
    println!("search 'kowal': {search:?} ({} hits)", hits.len());
    println!("write to bytes: {write:?} ({:.1} MB)", bytes.len() as f64 / 1e6);
    Ok(())
}
