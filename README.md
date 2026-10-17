# Heirloom

A Windows desktop app for keeping a family's history in one place:
- one big interactive family tree;
- a rich profile page for every person: photos, a structured life story, family sayings, stories, trivia, documents
  and sources.

People are added from files that an external AI chat prepares from a researcher's notes and scans. The app's Import
checks them, matches them with the people already in the archive, and saves only what you approve. The interface is
in Polish, and the app works fully offline. It goes online only when you ask it to: „Sprawdź teraz” in Ustawienia ›
O programie, or „Sprawdzaj aktualizacje” there switched on (off by default). Even then it asks GitHub only for the
newest Heirloom version's number (nothing from the archive goes with it).

## Status

- **Version 1 is built** (2026-09-28), from the finished design (`design/`), and follows **design v2** since
  2026-09-29 (version 0.3.0; the review fixes in 0.3.1–0.3.3):
  - Start, the tree (Rodzina, Przodkowie, Potomkowie, Całe drzewo), Osoby, person profiles edited in place section by
    section, Nazwiska, Miejsca, Historie, Media with a lightbox, Brakujące pliki, Źródła, Historia zmian, the
    five-step Import (drop everything at once: AI answers, photos, PDFs, notes), and Ustawienia;
  - browse and edit modes („Kto edytuje?”, one edit bar), saving with a backup copy, undo/redo, „Cofnij zapis”,
    conflict checks, an undo for a whole import, „Zapisz jako nowe archiwum” for a file from another program, and
    a question before the window closes with unsaved changes;
  - the archive is a normal folder: `rodzina.ged` (GEDCOM 7) + `media/` + `zrodla-ai/` + `.heirloom/`.
- **Still to do** (details in [TODO.md](TODO.md)):
  - a trial run of the AI instructions on a few real documents (a simulated trial with fictional packages passes);
  - design round 2 (`design/FEEDBACK_DRAFT.md`: status table at the top, new questions in §L);
  - a performance run on the 5,000- and 10,000-person test families.
- Everything is approved step by step: see [PLAN.md](PLAN.md) and [TODO.md](TODO.md).

## Technology

- **App:** Tauri 2 (a Rust core) with a React 19 + TypeScript interface; plain CSS with the design's tokens.
- **Tree:** DOM cards over SVG lines for the family views; PixiJS on the GPU for „Całe drzewo”.
- **Data:** GEDCOM 7 in the archive folder, read and written without losing anything; lists and search (Polish
  letters included) are worked out in memory from it.
- **Media:** stored once per file (duplicates found by hash), thumbnails cached outside the archive.

Details and reasons: [PLAN.md §5](PLAN.md#5-architecture), and what changed while building: PLAN §5.7.

## Development

You need Rust (`%USERPROFILE%\.cargo\bin` on the PATH) and Node.js. Run `npm install` once.

| What | Command |
|---|---|
| Run the app in a window, with live reload | `npx tauri dev` |
| Build the app (`target\release\heirloom.exe`) | `npx tauri build --no-bundle` |
| Build the installer `build\HeirloomSetup-<version>.exe` (checks the versions, self-tests the app) | `scripts\build.ps1` (Python with PyInstaller in `.venv`, or `-Python <python.exe>`) |
| The same build on GitHub (Windows runner; a `v*` tag also attaches the exe to that tag's release) | Actions › „Windows build” › Run workflow, or `gh workflow run windows-build.yml` |
| The same screens in a normal browser, on a test family | `cargo run -p heirloom-bridge -- --open test-archives/demo`, then `npm run dev` and open http://localhost:1420 |
| Run all Rust tests | `cargo test --workspace` |
| Type-check and test the interface | `npx tsc --noEmit -p tsconfig.json` and `npx vitest run` |
| Test the installer's registry code (runs on any system, touches nothing) | `python -m unittest installer/test_setup.py` |
| Make a test family (fictional, never real data) | `cargo run --release -p heirloom-gen -- test-archives/5000 --people 5000 --media 200` |
| Download the official GEDCOM 7 test files (then `cargo test` runs over them) | `powershell -File scripts\fetch-gedcom-test-files.ps1` |
| Time the core on it | `cargo run --release -p heirloom-core --example open_bench -- test-archives/5000` |

- `crates/heirloom-core`: GEDCOM reading/writing, the archive folder, history, Polish dates and names (and a SQLite
  search cache, not used by the app yet).
- `crates/heirloom-api`: every command the screens use (JSON in, JSON out), the file server and the Import.
- `crates/heirloom-bridge`: a small dev server that gives a browser the same commands (for testing the screens).
- `crates/heirloom-gen`: the test-family generator (`--seed N` gives the same family every time).
- `src-tauri/`: the app window and the `heirloom://` file protocol. `src/`: the screens.
- `installer/setup.py`: `HeirloomSetup-<version>.exe`, one exe that installs, updates and uninstalls, per user, without
  administrator rights (APP-STANDARDS.md §1), in a window with the standard's steps and Heirloom's look (§5). It never
  closes a running Heirloom by force (after you say OK it asks it to close the way its ✕ does, so Heirloom asks about
  unsaved changes), puts the old files back if a step fails, and never touches the family archives; on uninstall the
  program's own settings and thumbnails go to the Recycle Bin only if you tick the box. It registers the set file
  (`.heirloom-zestaw`, „Otwórz razem…”) as Heirloom's file type for the user, removed again on uninstall.
- `src-tauri/icons/`: the app's icon. `heirloom-5a.ico` is the source (copied unchanged to `icon.ico`);
  pictures up to 256 px come from its frames, bigger ones from `source.svg`; `heirloom-zestaw.ico`/`.svg` is the set
  file's document icon.
- Updates (APP-STANDARDS.md §2–§3, `crates/heirloom-api/src/update.rs`): only with „Sprawdzaj aktualizacje” switched
  on (off by default), at start and on coming back to the window (at most every 5 minutes), the app asks
  `api.github.com/repos/Husarp/Heirloom/releases/latest`, and a newer version shows a banner on Start; „Sprawdź teraz”
  and „GitHub” in Ustawienia › O programie always work. „Aktualizuj” downloads that release's `.exe` to
  `%TEMP%\Heirloom-aktualizacja` with the progress shown, asks about unsaved changes, starts it as
  `HeirloomSetup-X.Y.Z.exe --update` (progress only, no questions, no administrator rights; it starts Heirloom again)
  and closes; the installer is deleted at the next start.
- `.github/workflows/windows-build.yml`: runs `scripts/build.ps1` on a Windows runner and keeps the installer as the
  run's artifact.
- `test-archives/` (test families) and `test-files/` (the gedcom.io files) are ignored by Git.
- `crates/heirloom-api/tests/fixtures/ai-trial/`: fictional AI answers for the Import tests (a careful package, a
  sloppy one and a later one; `make-images.ps1` draws their scans). Kept on the developer's computer only, not in
  Git; without them `tests/ai_trial.rs` is skipped.

## Documents

- [PLAN.md](PLAN.md): requirements, decisions, features, architecture, performance targets, risks
- [TODO.md](TODO.md): the roadmap, with a checkbox per task
- [CHANGELOG.md](CHANGELOG.md): history of changes
- [docs/RESEARCH.md](docs/RESEARCH.md): research notes with sources (existing apps, GEDCOM, technology, compression,
  tree engine, AI import)
- [docs/IMPORT_FORMAT.md](docs/IMPORT_FORMAT.md): the import file format (`heirloom-import` 1.0)
- [docs/AI_INSTRUCTIONS.md](docs/AI_INSTRUCTIONS.md): instructions for the researcher and their AI chat (in Polish);
  the app copies them with „Kopiuj instrukcję AI”
- [docs/GEDCOM_EXTENSIONS.md](docs/GEDCOM_EXTENSIONS.md): how the archive is stored in GEDCOM 7, and the `_HLM_…` tags
- The design files (`design/`, with `IMPLEMENTATION_SPEC.md` and `FEEDBACK_DRAFT.md`) and the designer's prompt and
  answers (`docs/DESIGNER_PROMPT.md`, `docs/DESIGNER_ANSWERS.md`) are kept on the developer's computer only, not in
  Git

## Licence

**All rights reserved** — © 2026 Husarp. The code is public to read, and the releases are free to
download and use, but the app may not be copied, changed, shared, sold or built upon without
written permission. Full terms in [LICENSE](LICENSE).

## Your family data

The family archive is a normal folder that you choose ([PLAN.md §11.2](PLAN.md#11-approved-proposals-2026-09-28)):
- `rodzina.ged`: the family (GEDCOM 7, readable by other genealogy programs);
- `media/`: photos and documents, either all together or in a folder per person (see below);
- `zrodla-ai/`: the researcher's AI answers, kept as they were delivered;
- `.heirloom/`: the app's settings for this archive, the change history and backup copies.

Only rebuildable data (thumbnails) lives in `%LOCALAPPDATA%\Heirloom`, and the app's own
preferences (recent archives, theme) in `%APPDATA%\Heirloom`. The archive is never stored in this project folder, so
it can't end up in Git or on GitHub.

### Two ways of working (since 0.7.0)
The first time an archive is opened on a computer, Heirloom asks „Jak chcesz pracować z tym archiwum?”. The choice can be
changed later in Ustawienia › Archiwum › „Sposób pracy”.
- **„Pracuj na archiwum”**: for your own archive. Changes are saved in the archive itself (`rodzina.ged`, `media/`).
  Before every save Heirloom keeps a copy of the data file in `.heirloom/kopie`.
- **„Nie ruszaj oryginału”**: for an archive that is someone else's, shared, on a USB stick, or made by another program.
  The archive stays exactly as it is. Your changes, new photos and imports are saved in a folder of their own,
  `<Documents>\Heirloom\Moje zmiany\<archive name>` (`%LOCALAPPDATA%\Heirloom\Moje zmiany` if Documents can't be
  written). It is a normal archive folder too, and a `CO TO JEST.txt` in it explains what it is. The Start screen shows
  how many changes are not in the archive yet. When you decide, „Przenieś zmiany do archiwum…” (Ustawienia › Archiwum ›
  „Twoje zmiany”) puts them into the original: first it offers a backup, then it copies the new files, merges the data
  (if someone changed the archive meanwhile, you choose per person which version to keep) and writes the file. The move
  can be undone with „Cofnij” while nobody else has changed the archive. „Porzuć moje zmiany…” starts again from the
  current archive; the old folder is renamed, never deleted.
- An older Heirloom (before 0.7.0) doesn't know about „Moje zmiany”: opened on the same computer, it would edit the
  original directly.

### Backups
Before an import, before moving your changes into the archive and before moving files, Heirloom offers a backup of the
whole archive („Zrób kopię zapasową”, or „Dalej bez kopii”). „Utwórz kopię zapasową” on Start and
Ustawienia › Archiwum › Kopie zapasowe › „Utwórz kopię” make the same .zip any time. It is saved without asking in
`<Documents>\Heirloom\Kopie zapasowe\<archive name>` (the folder can be changed there with „Zmień…”; a USB stick or an
external disk is a better place than the same disk), as `Heirloom-<name>-RRRR-MM-DD-GGMM.zip`. Heirloom keeps the 5
latest of its own copies and deletes older ones; zips saved elsewhere are never touched. A long backup can be stopped
with „Przerwij”. „Przywróć z kopii…” unpacks a .zip into a new folder next to the archive („<name> (z kopii RRRR-MM-DD)”)
and opens it; the current archive stays as it was.

### Folders per person (since 0.7.0)
Ustawienia › Archiwum › „Zdjęcia i pliki” › „Układ plików w folderze media” is „Wszystkie razem” (everything in `media/`)
or „W folderach osób” (new photos go to e.g. `media/Kowalski Józef/`). The choice is stored in the archive and is the same
for everyone using it. What is shown never depends on it: a photo is found by the path in `rodzina.ged`. Files move only
when asked: „Przenieś pliki” in the switching dialog, or „Przejrzyj…” (also offered by a banner on Start) opens
„Porządek w zdjęciach”, which lists files that are in another place than recorded („Popraw ścieżki”), photos of people
lying loose, files nothing refers to (assign them to a person), folders put into `media/` and files outside the archive.
The paths in `rodzina.ged` change together with the files, nothing is overwritten or deleted, and „Cofnij porządkowanie”
undoes a run. In „Nie ruszaj oryginału” the files of the archive stay where they are.
