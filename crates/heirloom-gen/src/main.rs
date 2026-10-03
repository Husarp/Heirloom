//! `heirloom-gen <folder> [--people 5000] [--media 0] [--seed 1878] [--name "Rodzina testowa"]`
//! Writes a fictional test archive (never real family data) — by convention into `test-archives/`.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut folder: Option<PathBuf> = None;
    let mut options = heirloom_gen::Options { people: 5000, media: 0, seed: 1878, name: "Rodzina testowa".into() };
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| fail(&format!("missing value after {arg}")));
        match arg.as_str() {
            "--people" => options.people = value().parse().unwrap_or_else(|_| fail("--people needs a number")),
            "--media" => options.media = value().parse().unwrap_or_else(|_| fail("--media needs a number")),
            "--seed" => options.seed = value().parse().unwrap_or_else(|_| fail("--seed needs a number")),
            "--name" => options.name = value(),
            _ if folder.is_none() && !arg.starts_with("--") => folder = Some(PathBuf::from(&arg)),
            _ => fail(&format!("unknown argument {arg}")),
        }
    }
    let folder = folder.unwrap_or_else(|| fail("usage: heirloom-gen <folder> [--people N] [--media N] [--seed N] [--name TEXT]"));
    let start = Instant::now();
    match heirloom_gen::write_archive(&folder, &options) {
        Ok(archive) => {
            let size = std::fs::metadata(archive.data_path()).map(|m| m.len()).unwrap_or(0);
            println!(
                "Wrote {} ({} people, {} photos, {:.1} MB) in {:?}",
                archive.data_path().display(),
                options.people,
                options.media,
                size as f64 / 1e6,
                start.elapsed()
            );
        }
        Err(e) => fail(&e.to_string()),
    }
}

fn fail(message: &str) -> ! {
    eprintln!("heirloom-gen: {message}");
    std::process::exit(1)
}
