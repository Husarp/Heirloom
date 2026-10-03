# Heirloom

A Windows desktop app for keeping a family's history in one place:
- one big interactive family tree;
- a rich profile page for every person: photos, a structured life story, family sayings, stories, trivia, documents
  and sources.

People are added from files that an external AI chat prepares from a researcher's notes and scans. The app's Import
checks them, matches them with the people already in the archive, and saves only what you approve. The interface is
in Polish, and the app works fully offline.

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
| The same screens in a normal browser, on a test family | `cargo run -p heirloom-bridge -- --open test-archives/demo`, then `npm run dev` and open http://localhost:1420 |
| Run all Rust tests | `cargo test --workspace` |
| Type-check and test the interface | `npx tsc --noEmit -p tsconfig.json` and `npx vitest run` |
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
  administrator rights (APP-STANDARDS.md §1). It never closes a running Heirloom (it asks you to save and close it)
  and never touches the family archives; on uninstall the program's own
  settings and thumbnails go to the Recycle Bin only if you tick the box.
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

MIT — © 2026 Husarp. See [LICENSE](LICENSE).

## Your family data

The family archive is a normal folder that you choose ([PLAN.md §11.2](PLAN.md#11-approved-proposals-2026-09-28)):
- `rodzina.ged`: the family (GEDCOM 7, readable by other genealogy programs);
- `media/`: photos and documents;
- `zrodla-ai/`: the researcher's AI answers, kept as they were delivered;
- `.heirloom/`: the app's settings for this archive, the change history and backup copies.

Only rebuildable data (thumbnails) lives in `%LOCALAPPDATA%\Heirloom`, and the app's own
preferences (recent archives, theme) in `%APPDATA%\Heirloom`. The archive is never stored in this project folder, so
it can't end up in Git or on GitHub.
