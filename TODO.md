# Heirloom — TODO

This is the roadmap, with a checkbox per task.
- **Approval:** every item is approved before it is built ([PLAN.md](PLAN.md)).
- **Where things are explained:** the phases follow PLAN §8, and "→ check:" gives the success test for a task.
- **New ideas** go into this list.

## Planning
- [x] Project folder, name (Heirloom), README, CHANGELOG
- [x] Research: existing apps, GEDCOM, technology, compression, tree engine, AI import ([docs/RESEARCH.md](docs/RESEARCH.md))
- [x] Direction chosen: our own app (option C)
- [x] Full plan ([PLAN.md](PLAN.md))
- [x] Import format, draft ([docs/IMPORT_FORMAT.md](docs/IMPORT_FORMAT.md))
- [x] AI instructions for the researcher, draft ([docs/AI_INSTRUCTIONS.md](docs/AI_INSTRUCTIONS.md))
- [x] Designer prompt, draft ([docs/DESIGNER_PROMPT.md](docs/DESIGNER_PROMPT.md))
- [x] Licence confirmed: MIT (2026-09-28)

## Phase 0 — Checks before building
- [x] **Resize test:** a minimal Tauri 2 + React + PixiJS app with 3,000 dummy cards; measure window resize, pan and
      zoom on this PC → check: no visible stutter, frames under 16 ms
      ([spikes/resize-test](spikes/resize-test/README.md), 2026-09-28)
  - **Panning and zooming:** a locked 75 fps (the screen's maximum), with 3,000 and with 10,000 people.
  - **Window resize, with a screen-sized canvas:** 72 fps, 95% of frames within about 17 ms.
- [x] **Your eye check:** in `spikes/resize-test`, run `run.cmd` and drag the window edges → result (2026-09-28): "not
      smooth, but not lagging much"
- [x] **Resize follow-up:** dropped. You decided to stay with Tauri, with no Electron comparison (2026-09-28).
- [x] **AI trial:** the researcher makes one batch with AI_INSTRUCTIONS.md → check: the JSON passes validation and the
      transcriptions are correct; then improve the prompt and format
  - The researcher uses free ChatGPT, maybe free Gemini, so the instructions were adapted on 2026-09-28: 3–8
    documents per batch, parts of at most 10 persons, copy instead of download.
  - Simulated instead (the partner can't do a trial, 2026-09-29): fictional packages written the way a chat AI
    answers — careful, sloppy and a later one (`crates/heirloom-api/tests/fixtures/ai-trial`, `tests/ai_trial.rs`).
    They led to three import fixes and family-based matching. A real researcher batch is still welcome.
- [ ] **Media test** on a sample of real files: sizes before and after (PNG → lossless WebP, thumbnails), and speed
- [x] **Design round 1** in Claude Design (DESIGNER_PROMPT.md) → pick a direction, save it into `design/`
  - [x] The designer's questions answered: details per tab, no "Ty", browse and edit modes, v1 priorities
        ([docs/DESIGNER_ANSWERS.md](docs/DESIGNER_ANSWERS.md), 2026-09-28)
  - [x] Update 2 for the designer: open on Start, the archive folder screens, read-only state, saving and
        conflicts, missing files, the Archiwum settings, fully offline (same file, "Uzupełnienie 2")
  - [x] Update 3 for the designer: editing allowed but always clearly shown (no forced read-only); "Utwórz archiwum z
        plików" as the main first-launch action; the Import as the built-in converter (same file, "Uzupełnienie 3")
  - [x] Review the finished design and propose changes (the list is in `design/FEEDBACK_DRAFT.md`, 2026-09-28)
  - [x] Design v2 (the designer's fixes for the P1 items, page 17) saved into `design/` and built (2026-09-29)
- [ ] **Design round 2:** hand the updated `design/FEEDBACK_DRAFT.md` to the designer (its status table says what v2
      settled and what is still open, with the new questions from building v2)
- [x] **Decided (2026-09-28):** PLAN §11.1 (open on Start, tree loaded on demand) and §11.2 (the archive as an open
      folder with GEDCOM + `.heirloom` files, editable with a clear indication, the Import as the built-in converter)

## Phase 1 — Foundation
- [x] **Project setup:** Tauri 2 + React 19 + TypeScript + Vite + Zustand; app ID `com.husarp.heirloom`
      (2026-09-28). Plain CSS with the design tokens instead of Tailwind, and Polish text in the code instead of
      i18next (PLAN §5.7).
- [x] Cargo workspace: `heirloom-core`, `heirloom-gen` (2026-09-28); `heirloom-layout` comes with the tree (Phase 4)
- [x] **GEDCOM reader/writer** (PLAN §11.2), in `crates/heirloom-core` (2026-09-28):
  - a lossless line tree underneath, so unknown data survives a save;
  - GEDCOM 7 in and out;
  - GEDCOM 5.5.1 in (UTF-8/16, ANSEL, CP1250, vendor tags);
  - `_HLM_…` extension tags declared in the header.

  → check: our own archive and the gedcom.io test files round-trip unchanged. Our own files and hand-made 5.5.1
  samples pass; all 22 official gedcom.io test files come back byte for byte, every screen reads them, and saving
  the edit form unchanged changes nothing in them (2026-09-29; `scripts/fetch-gedcom-test-files.ps1`).
- [x] **The archive folder** (2026-09-28):
  - create and open;
  - the `.heirloom/` files (`ustawienia.json`, `historia.jsonl`, `kopie/`; `uklad.json` comes with the tree);
  - safe saving: a backup copy of the `.ged`, then write a new file and swap;
  - noticing when another program changed the file (reload, or save as a copy).
- [x] **SQLite cache** in `%LOCALAPPDATA%\Heirloom\cache\`, built from the archive and rebuilt when the `.ged` changes
      (PLAN §5.4–§5.5) (2026-09-28)
- [x] **Change log and undo/redo** (`historia.jsonl`) (2026-09-28)
- [x] **Browse and edit modes** (2026-09-28):
  - "Kto edytuje?" (editor names);
  - the edit bar showing who is editing and which file;
  - the first save into another program's file asks first;
  - an optional read-only switch.
- [x] **Wybór archiwum and Pierwsze otwarcie** (2026-09-28): recent archives; "Utwórz archiwum z plików", "Otwórz folder…",
      "Otwórz plik GEDCOM…"; what was found and what's missing; the start person
- [x] **The app opens on Start;** the tree screen loads on demand and does no work in the background (PLAN §11.1)
- [x] `heirloom://` media protocol (thumbnails, previews, originals; thumbnails cached per archive) (2026-09-28)
- [x] **Synthetic generator:** 5k and 10k people with media, written as an archive folder, for tests
      (`crates/heirloom-gen`, 2026-09-28)
  - Timing on this PC (release): 5,000 people open + parse in 64 ms, the cache builds in 101 ms, a search takes
    1.4 ms; 10,000 people: 130 ms, 262 ms and 2 ms.
- [x] Settings basics: light/dark theme, text size (2026-09-28)
- [x] Browser testing: instead of a mocked `api.ts`, the dev bridge (`crates/heirloom-bridge`) serves the real API
      to a normal browser (2026-09-28). Playwright is not set up yet.
- [x] Local Git repository, initialized with the noreply e-mail (2026-09-28); GitHub only when you ask

## Phase 2 — People and profiles
- [x] **Names:** typed names, Polish gender and marital forms, folded search keys, surname groups (2026-09-28)
- [x] **Dates:** Polish parser and display, qualifiers, Julian dates, sort keys → check: unit tests for every form
      in PLAN §4.4 (2026-09-28)
- [ ] **Places:** hierarchy and place picker done (2026-09-28); historical names, place types and coordinates not yet
- [x] **Families:** unions, children with relation types, siblings and partners by date (2026-09-28); manual order is
      in Phase 4 "Arranging"
- [x] **Profile page** (2026-09-28):
  - [x] Hero, W skrócie, sticky table of contents
  - [x] Rodzina with labels
  - [x] Życiorys; Historie, Powiedzonka, Ciekawostki
  - [x] Linki, Źródła, Wspomniany w…, Historia zmian
- [x] **In-place editing** + TipTap editor with @mentions (`[text](person:ID)`) (2026-09-28); the "/" menu not yet
- [ ] **Media v1:**
  - [x] files copied into `media/` with readable names, duplicates detected by hash;
  - [x] thumbnails and previews in the cache;
  - [x] gallery, the Media library and the lightbox; profile photo crop (GEDCOM `CROP`) in the backend;
  - [x] the "Brakujące pliki" (missing files) tool;
  - [ ] "Zmniejsz zdjęcia bez utraty jakości" as an explicit tool (PNG → lossless WebP, with a pixel check). The
        Settings row is there, switched off. **Not for now** (your decision, 2026-09-29).
- [ ] **Documents:** PDFs open in the WebView's own viewer and the transcription/translation view is on Źródła
      (2026-09-28); PDF thumbnails and text (PDFium) not yet
- [x] **Catalog (Osoby):** virtualized table, sort/filter/group, A–Z rail, card grid (2026-09-28)
- [x] **Search:** Polish folding (including ł), Ctrl+K palette → check: "Lukasz" finds "Łukasz" and maiden names are
      found (2026-09-28; an in-memory search, fast enough without FTS5, PLAN §5.7)
- [x] **Performance check:** profile opens in under 150 ms, search answers in under 100 ms (PLAN §6) → 2026-09-28,
      10,000 people, release build: profile 15 ms, search 15 ms (including the HTTP bridge)

## Phase 3 — Import (the built-in converter)
- [x] **"Utwórz archiwum z plików":** on first launch, choose a folder, then go through the Import (2026-09-28)
- [x] **Importer for heirloom-import 1.0** (2026-09-28):
  - input: the paste box, or a folder or files dropped in (AI answers, photos, PDFs, text notes);
  - extraction of the JSON blocks, end-marker check, cosmetic repair;
  - validation (errors vs warnings).
- [x] **Normalization:** names, dates (ages turned into estimates), places (2026-09-28)
- [x] **Matching**, with a score and the reasons shown (RESEARCH §8) (2026-09-28)
- [x] **Review screen** (the 5-step wizard, 2026-09-28):
  - [x] buckets: new / matched / needs review
  - [x] side-by-side comparison with per-field decisions
  - [x] issues panel, including the AI's questions
  - [x] mini tree preview
  - [x] media grid with the ★ profile-photo mark
- [x] **Save into the archive** (2026-09-28):
  - the people go into `rodzina.ged` (backup copy first);
  - the files go into `media/` with readable names;
  - the batch is recorded in the history;
  - "Cofnij import" undoes it.
- [x] **"Kopiuj instrukcję AI"**, always matching the format version (2026-09-28)
- [x] **Test fixtures:** cut-off parts and broken JSON are unit-tested, the documented example imports end to end
      (2026-09-28), and the simulated AI trial packages import in full (2026-09-29)

## Phase 4 — Tree
- [x] **Focus layouts** (tidy tree) in TypeScript, `src/screens/tree/layout.ts` (2026-09-28; PLAN §5.7); duplicates
      marked ↻ not yet
- [ ] **Renderers** (2026-09-28): DOM + SVG for the focus views, PixiJS for „Całe drzewo”:
  - [x] world coordinates and culling (a screen-sized canvas that only grows)
  - [x] 4 levels of detail (75 / 40 / 15 %, with hysteresis)
  - [ ] photo atlases (photos in „Całe drzewo”)
  - [x] the side panel for the selected person
- [x] **Rodzina view:** refocus animation, keyboard navigation, side panel with "add relative" (2026-09-28)
- [x] **Przodkowie and Potomkowie views** (2026-09-28)
- [ ] **Całe drzewo** (first version 2026-09-28):
  - [x] generation rows, surname clusters, couples kept together, sibling order
  - [ ] x-coordinates (Brandes–Köpf), family "bus" lines
  - [ ] caching
- [ ] **Arranging:** reorder siblings and partners, move a branch (stored as an offset), reset
- [ ] **Extras:** colour modes, highlight the direct line, collapse/expand (siblings), search-to-jump and rounded
      lines are done (2026-09-28); the minimap is not
- [ ] **Performance check:** resize, 60 fps at 5k and 30 fps at 10k, layout in under 1 s (PLAN §6) → the whole-family
      layout takes 74 ms at 10,000 people (2026-09-28); frame rates in the app window not measured yet

## Phase 5 — Finishing v1.0
- [x] Start screen: stats, recently added and edited, quick actions, „W tym dniu” (2026-09-28)
- [ ] Nazwiska and Miejsca (place pages) done (2026-09-28); branches (rodziny) not yet
- [x] Settings complete (PLAN §4.9) (2026-09-28), except the rows listed under "Found while building"
- [x] Design pass with the final Claude Design mockups (light + dark) (2026-09-28)
- [x] Archive tools: "Utwórz kopię archiwum (.zip)", GEDZIP export, archive check, export/import of the display
      settings (2026-09-28)
- [x] Windows installer (2026-09-29): `HeirloomSetup-<version>.exe` from `scripts/build.ps1` + `installer/setup.py`,
      the APP-STANDARDS §1 pattern (install/update/uninstall in one exe, per user, version check, self-test of the
      packaged app); "what's new" (short Polish notes in Ustawienia › O programie, 2026-09-28)
- [x] **Update checks and in-app updates** (APP-STANDARDS §2–§3) (2026-10-03, your decision: the full standard, the
      repo is public): GitHub `/releases/latest` (only with checks on) at start and on coming back to the window, at
      most every 5 minutes, versions compared as numbers (Rust, `update.rs`); a banner on Start and on the archive
      picker (✕ until the next start; no banner while checks are off); Ustawienia › O programie: „Sprawdzaj
      aktualizacje” (off by default, your 2026-10-03 decision, `aplikacja.json`), „Sprawdź teraz”, „GitHub”, „Pobierz
      aktualizację”; the installer downloaded with the progress shown, unsaved changes asked about, then the installer
      starts as `--update` and Heirloom closes; „Spróbuj ponownie” + „GitHub” on failure; the old installer goes at
      the next start. The „offline” texts say it goes online only on „Sprawdź teraz” or with checks on. Not yet tried
      on Windows
- [x] **Windows build on GitHub** (2026-10-03): `.github/workflows/windows-build.yml` runs `scripts/build.ps1` on
      `windows-latest` (by hand or on a `v*` tag), keeps the installer and `BUILT.json` as the run's artifact, and on a
      tag attaches the installer to that tag's release. The self-test now gives up after 2 minutes instead of waiting
      for ever. Not run yet
- [x] **Installer window** (2026-10-03, APP-STANDARDS §5): `installer/setup.py` rebuilt in Heirloom's look with the
      standard's steps (welcome, „Heirloom jest uruchomiony”, progress with „Pokaż szczegóły”, finish, `--update`,
      failure with „Spróbuj ponownie” and the old version put back, uninstall). Checked in a Linux sandbox with
      Windows stubbed out (every page screenshotted at 100/125/150 %); not run on Windows yet
- [ ] Final performance run on the 5k and 10k test archives → first run 2026-09-28 (10,000 people, release build):
      profile 15 ms, search 15 ms, whole-family layout 74 ms; cold start 2.2–2.7 s to the first screen (target 2 s,
      much of it the WebView starting); frame rates not measured yet

## Found while building (2026-09-28)
- [ ] **Import:** link an unrecognised mention to a person or create them (the „Nierozpoznana wzmianka” card), crop the
      profile photo during the import („Kadruj”); ~~show a file's transcription in step 4~~ done 2026-10-03 (the
      „transkrypcja” chip opens the file with its transcription, translation and note, read-only)
- [x] **Import:** after a person is joined with someone in the archive, suggest their relatives from the batch for
      that person's relatives (matching by family, not only by name) (2026-09-29: +3 points per relative of a clear
      match)
- [ ] **Media:** „Zapisz kopię jako…”; image size and „Dodano” (when, by whom) in the list; a 2048 px preview for big
      scans; removing a profile photo; folders dropped onto Media
- [ ] **Miejsca:** list the people with no place; place types, coordinates, historical names, parishes
- [ ] **Nazwiska:** variant forms in `surnames.list`, so the search doesn't load everyone
- [ ] **Historie:** the story's exact branch (not only its colour), the story's own photos, an „uncertain” flag on
      story dates, the rich editor in the story panel
- [ ] **Historia zmian:** `current` for import rows; links for text, source and media rows
- [x] **Backups:** file names in local time, same format (2026-10-03); old copies are pruned by their file time, and
      „Ostatnia kopia” counts only the data file's own copies
- [ ] **Tree:** manual sibling order (drag), minimap, ↻ duplicates, photos in „Całe drzewo”
- [ ] **Settings:** moving the settings files („Obok danych / W programie”), accent colours, „Zmniejsz zdjęcia”,
      restoring a backup („Przywróć”), keeping an editor's change count when they are renamed
- [ ] **App window:** a Content-Security-Policy (it's off now)
- [x] **Windows installer:** built the APP-STANDARDS way (Python + PyInstaller) instead of NSIS, so nothing had to
      be downloaded; the NSIS bundle is switched off in `tauri.conf.json` (2026-09-29)
- [ ] **Interface polish (from the code review):** focus inside dialogs (first field, focus trap), names for checkboxes
      and switches, error toasts for the theme and text-size switches, the surname options, „Zapomnij” and Ctrl Z/Y,
      a fallback for missing photos in the side panel and the editor, a confirmation before deleting a profile link,
      one `places.list` call instead of four in the editor's place fields, virtual lists for Źródła, Historie and
      Brakujące pliki when they grow into thousands
- [ ] **Left from the design check against the spec (2026-09-28):**
  - tree: the highlight rules of §3.9 (lines to the selected person's children, only the chosen child's branch),
    surname labels at the smaller zoom levels, „+N potomków” pills in Potomkowie, pills and „+5 rodzeństwa” keeping
    their size at LOD 2, Potomkowie row labels at the left edge, both spouses in the Potomkowie sub-line, the loading
    skeleton, the breadcrumb on the first open, year lines on the Całe drzewo bands and a „Widok” rectangle;
  - side panel: hide the empty photo box in browse mode, a label for „żyje”;
  - Osoby: the filter row on one line, the full texts of the surname popover;
  - profile: „Gałąź … · pokolenie” order, short places in the hero line, uncut family tiles, age for a „przed …”
    death, „między … a …” instead of „1956/1958”, „wg daty” and „Pokaż wszystkie” on the gallery;
  - edit form: ellipsis on long places and placeholders, „Uzupełnij” links in the completeness card;
  - Miejsca: all sub-places under „Podrzędne”, a wider date column; lightbox: the zoom group after the counter,
    „Pokaż oryginał (W×H)”; Źródła: the spec's type chips, a first source selected on arrival;
  - the theme button's tooltip should say what the next click does
- [ ] **Left from the backend review (2026-09-28):**
  - undoing an import (or a „create”) doesn't check links made later to those records, and deleting a text or a
    photo can leave pointers from families, events or citations (Sprawdź archiwum reports them);
  - re-importing into joined people adds their texts again, and every batch adds new source records even for
    sources already in the archive;
  - ~~lenient reading of a few slips (`"events": null`, `"id": 5`, `"part": "1"`)~~ done 2026-09-29;
  - ~~joining two people of one batch with the same archive person should be refused in step 3, not at the end~~
    done 2026-10-03: refused in step 3 when the two are relatives in the batch (what the save can't write), with a
    message saying who; unrelated people may still be one archive person and are marked so;
  - ~~files copied into `media/` before a failed import stay there (nothing is lost, it's clutter)~~ done 2026-10-03:
    a failed import removes its copies, and `zrodla-ai/` is written only after the save;
  - the GEDZIP export packs any local file a FILE line points to (absolute paths, `../`): a GEDCOM from someone
    else could pull private files into an export;
  - the `heirloom://` handler decodes whole originals for thumbnails, one thread per request: a screen of big scans
    on a cold cache can take a lot of memory;
  - the date-format switch is set for the whole program; a person with only AKA/OTHER names can't change the given
    name in the form
- [ ] **Speed:** `people.list` (Osoby, and the Nazwiska search) takes 0.3–0.7 s at 10,000 people, mostly allocating and
      freeing 200,000 small JSON values; try a faster allocator (mimalloc) or a leaner list
- [ ] **Tests:** more smoke tests of the real app window (two run over the DevTools port, start-up and the close
      question, as throwaway scripts so far; worth keeping in `scripts/`), Playwright against the bridge
- [ ] **Test families:** `heirloom-gen` gives some people partners and children after their death (Sprawdź archiwum
      finds 37 such dates in `test-archives/demo`)

## Done on 2026-09-29
- [x] **Links that lead nowhere:** listed on the profile („ten link prowadzi donikąd”, „Usuń link” in edit mode), and
      mentions of someone who is gone shown dimmed
- [x] **The official GEDCOM 7 test files** round-trip and open on every screen (`scripts/fetch-gedcom-test-files.ps1`)
- [x] **Design v2:** editing in place, the edit bar, the first-save dialog with „Zapisz jako nowe archiwum”, first
      launch, the Import's file list, search, toasts, keyboard focus and the tree keys, the new contrast tokens, cards
      „Imię NAZWISKO · 1878 † 1951” (details in CHANGELOG 0.3.0)
- [x] **Review round (0.3.1):** three reviews of design v2 and the installer (backend, interface, installer); every
      confirmed finding fixed — „Cofnij zapis” by time and author, no undo over later changes, the close question,
      drafts kept in sections, the uninstaller removing its folder (details in CHANGELOG 0.3.1); the last open one,
      an import kept across „Zapisz jako nowe archiwum”, in 0.3.3

## Done on 2026-10-03 (small fixes found while planning 0.4.0, docs/CHANGES_PLAN.md §6)
- [x] **Import step 1:** a dropped file that is gone since is listed as gone („Pliku nie ma już w tym miejscu”), never
      read as an empty file that then fails the import; Thumbs.db, desktop.ini, .DS_Store and similar files are skipped
- [x] **Import step 1:** the paste view shows the first error even when something was recognised; „To nie jest
      odpowiedź w formacie Heirloom” and the unsupported version name their part and file (no more „Część 0”) and
      show at their row; a second, different part with a number already seen is a warning, not dropped silently; one
      warning for all files without an M-number; the „Kopiuj instrukcję dla AI” toast says to pass it to the person
      who searches the records (it holds Part A for them and Part B for the chat)
- [x] **Import counts:** each file counted once in step 1; the summary counts only the files that will be saved;
      „Zatwierdź” no longer counts files by a status they never have
- [x] **Undone imports:** the Done screen's „Cofnij import” turns off after an undo from the toast too (and after
      coming back); undoing an import already undone says so; Ustawienia › Import counts them („3 importy, 1
      cofnięty”) and step 1's „Poprzednie importy” marks them
- [x] **GEDZIP export:** a FILE path written with `\` (GEDCOM 5.5.1 from Windows programs) is found on every system

## From building design v2 (2026-09-29; questions for the designer are in FEEDBACK_DRAFT.md §L)
- [ ] **Opening a .gdz file** („Otwórz plik GEDCOM… .ged, .gdz” in the design): only the export exists now
- [ ] **Notes in the Import:** the design's „Notatka · Do historii „Zima 1915”” means a .txt note becomes a story;
      now a note is kept as a document of the person it is pointed at
- [ ] **Uzupełnienie (completeness):** the old form's card („14 z 16 pól”, „Brak danych”) has no place in v2 yet
- [ ] **„Zapisz jako nowe archiwum”:** a progress bar for big copies (it runs as one step now), and the same rule
      as the GEDZIP export for FILE lines that point at files elsewhere on the disk
- [ ] **Search:** the old married and maiden forms (Kowalowa, Kowalówna) and hyphenated surnames by their parts
- [ ] **Arrow keys in the tree:** ↑ on a card in the top row re-centres the tree on that person's parent; a smoother
      way to walk further up may be wanted

## From your first test of the app (2026-09-29; planned only, not started)
- [x] **Całe drzewo — surname labels come and go while zooming** (done 2026-10-03): a family's label disappears although there is
      room for it (test family: at 15 % „Jabłońscy” show; at 23 % „Ostrowscy” appear and „Jabłońscy” vanish; at
      36 % both show). Now a label that would overlap another is dropped, bigger families first; it should move
      aside instead (a second row, or elsewhere over its own family), and a label already shown should stay while
      it still fits. Now `labels.ts`: measured widths, left/right/lower spots, shown labels placed first and kept
      with a little less room; on the test family both show at 15, 23 and 36 %. A one- or two-person family at the
      screen's edge can still blink while zooming in a crowded corner
- [x] **Całe drzewo — a label stays when its family is gone** (done 2026-10-03): „Dąbrowscy” stays at the edge of the screen although
      none of the family is visible; a label should show only while some of its family is on screen (now only
      left–right is checked, not up–down)
- [x] **Clicking a person works the same in every tree view** (your rule: one click = one result everywhere, so
      it's easy to remember): a click opens the side panel with that person, and „Otwórz profil” there goes to the
      full profile. Rodzina, Przodkowie and Potomkowie do this already; in Całe drzewo the click selects the person
      but the side panel is switched off for that view (`Tree.tsx:304`, `view !== "overview"`), so nothing seems to
      happen — and the click only counts when it lands on the small card or dot. Back from the profile — the
      mouse's back button, Alt ←, or the ← button in the top bar — returns to the same view at the same zoom and
      place (the tree doesn't remember its zoom and position yet). The click part done 2026-10-03: Całe drzewo opens the side
      panel (its relatives move the camera there), rings the selected person (also after „Skocz do osoby…”), finds
      the nearest person in the grid cell, Esc closes the panel, and only the left button selects in every view.
      Back to the same place done 2026-10-03: each Back/Forward entry of the tree keeps its view, person, selection,
      zoom and position; a new centre (double-click, a pill, a relative in the panel, „Skocz do osoby…”) is a step
      in history, the arrow keys and Home only update the current one; coming back fetches nothing and Całe drzewo
      isn't rebuilt (a new centre there no longer fetches everyone again either)
- [x] **Settings: no row that only looks like a setting** (checked 2026-09-29; every other row does change the app;
      the shared „wkrótce” look done 2026-10-03: greyed row, a „wkrótce” badge, the control visible but inert).
      Rows you can see but not change now:
  - Drzewo › „Kolejność rodzeństwa”: only the text „Według daty” (manual order needs dragging siblings in the tree,
    see „Tree” under „Found while building”);
  - Drzewo › „Kolory gałęzi”: swatches only — each branch's colour should be choosable;
  - Wygląd › „Kolor akcentu”: switched off, green only;
  - Pliki ustawień › „Gdzie trzymać”: switched off („zmiana miejsca w kolejnej wersji”);
  - Eksport › „GEDCOM 5.5.1”: switched off („wersja 2”);
  - Narzędzia › „Zmniejsz zdjęcia”: switched off (not for now, as you decided).
  Build the two you named (sibling order, branch colours). The others stay visible but greyed out, all marked
  the same way: „wkrótce” (agreed 2026-09-29); the two to build look the same until they're built
- [x] **„Osoba startowa” → a start setting with two options** (agreed 2026-09-29; done 2026-10-03): open the archive on **Start**,
      or in **the last place** — whatever screen you were on (a profile, a tree view at its zoom and position, a
      list…), remembered per archive, also after closing the program. No chosen start person: „Osoba startowa”
      (Ustawienia › Drzewo) goes, and so does the first-open question „Od kogo zacząć drzewo?” that sets it; the
      tree centres on the last viewed person, or on a suggested one when nobody was viewed yet. Done: Ustawienia ›
      Archiwum › „Po otwarciu archiwum: Start · Ostatnie miejsce” (Start by default), kept on this computer in
      aplikacja.json for each archive: the screen, the tree as last seen (view, person, zoom, position) and
      „Ostatnio oglądane”. The Import, a new person and Ustawienia aren't restored (Start instead). The first-open
      screen keeps its summary; Home in the tree goes to the suggested person (it did nothing before)
- [x] **Top bar › „Rozmiar tekstu”: „150%” sticks out of the box** (your screenshot, 2026-09-29; done 2026-10-03): the popover is a
      fixed 230 px wide (`TextSizePopover.tsx:28`) and the four buttons need more; let it grow to fit them. Done: it
      grows to fit, a second click on the icon closes it, a failure shows a toast, and a size the buttons don't
      offer (105, 115…) shows as „Teraz: 115%”
- [x] **Import › „Upuść wszystko naraz”: a click anywhere on the box opens the file picker** (now only the small
      „albo wybierz pliki” / „lub folder…” links do). One Windows dialog can't pick files and folders at once, so:
      the box opens the file picker (several files at once), and the „lub folder…” link stays for a whole folder
      (dropping a folder works already) — agreed 2026-09-29, done 2026-10-03
- [ ] **Every picture opens bigger on a click** (2026-09-29): wherever the program shows an image — profile photos
      and gallery, the side panel, the Import (file list, step 4), Źródła, Brakujące pliki — a click shows it large
      (the lightbox Media already has), and a click outside it or Esc makes it small again; check each place
- [ ] **Import step 1: „N błędów w paczce” should say what is wrong** (2026-09-29): now it can't be clicked and
      only a tooltip says the details are in the „Sprawdź” step. A click should show the errors in plain words,
      and each error should sit at the file it belongs to (errors from the later checks, e.g. a wrong format or
      mixed batches, aren't attached to any file row yet — from the backend review). Your second screenshot: all 8
      rows say „Rozpoznano”, yet the foot says „1 błąd w paczce” — with no hint where it is
- [ ] **Import: an error never leaves you stuck** (your screenshot, 2026-09-29: „1 błąd — import zablokowany”, „Dalej”
      off, no way to fix it). Every error offers what to do right away: fix it here (where it can be fixed), leave
      out just the part with the error and import the rest, or set it aside — „Odłożone”: a place where the entries
      with problems wait („6 wpisów czeka na poprawę”) to be fixed, deleted or imported later. Every message also
      says in plain words what to do. Errors that concern the whole package (a wrong format or version) can't be left
      out — for those the message must say what to do instead
- [ ] **Import: several packages in one import** (agreed 2026-09-29; now „Wklejone części pochodzą z różnych
      paczek” blocks it): packages dropped together (e.g. Sadowscy-Chodel + Sadowscy-Francja) go in as one import,
      and each person and file shows which package it came from
- [ ] **An unfinished import survives closing the program** (2026-09-29): the files, decisions, answers to the AI's
      questions and the step reached are remembered, and the Import continues where it stopped
- [ ] **Import step 1: removing a file is slow** (1–2 s for each „×”, 2026-09-29): each removal reads and checks
      the whole package again; the row should go at once
- [x] **The toast after an import is squeezed** (your screenshot, 2026-09-29): „Zaimportowano 13 osób ·
      Sadowscy-Chodel-2026-09-29 · 5 plików” breaks into many short lines because the two buttons („Cofnij import”,
      „Pokaż w drzewie”) take the width; lay it out so the text fits (e.g. the buttons under the text) — done
      2026-10-03: with two buttons, or a detail line and a button, the buttons go under the text
- [ ] **Całe drzewo: lines between parents and children** (2026-09-29), as in Rodzina, Przodkowie and Potomkowie;
      now it shows only the cards in generation bands, plus one line for the selected person's direct line
- [x] **Tree toolbar: remove „Żyjący / Zmarli / Wszyscy”** (agreed 2026-09-29; done 2026-10-03): most people in an archive of
      ancestors are dead, so the filter isn't needed; it also never worked in Całe drzewo (it only dims people in the
      three family views). The Osoby list keeps its own filter
- [x] **Tree toolbar: the card style out of „Filtry”** (agreed 2026-09-29; done 2026-10-03 as the dropdown „Karty: ze
      zdjęciem / inicjały”, hidden in Całe drzewo; Ustawienia says „Ze zdjęciem / Inicjały”): „Ze zdjęciem / Bez zdjęcia
      (inicjały)” isn't a filter (it looked like „show people with / without a photo”) but how the cards look, so it
      stays, under its own name (e.g. „Karty: ze zdjęciem / inicjały”), and the „Filtry” button, then empty, goes.
      It also stays in Ustawienia › Drzewo › „Styl karty”. (It does nothing in Całe drzewo, whose cards have no
      photos yet)
- [ ] **Branch colours: a few ready palettes** (2026-09-29): the present twelve colours are rather dark and muted;
      add e.g. a lighter and a brighter set to choose from, next to choosing each branch's colour („Kolory gałęzi”,
      above). Each palette checked in the light and the dark theme, so names on the coloured stripes stay readable
- [x] **Tree: „Koloruj wg” doesn't work** (2026-09-29; done 2026-10-03): in Całe drzewo „Nazwisko” and „Strona ojca–matki” aren't
      handled at all (they show the branch colours); check every view and every mode, and that a change shows at once.
      Now: „nazwisko” uses the surname borne now (`surnameBranch` from the backend, joins followed), „strona” is
      relative to the person in the centre (in Całe drzewo: the clicked one) with a key under the tree, and Całe
      drzewo re-tints at once instead of waiting for a rebuild. „strona” there uses a small [father, mother] array
      until the overview gets real family links
- [ ] **Rethink the tree views — a design task** (2026-09-29; your notes, to design and agree before building):
  - **„Ród” — a whole surname line, as a new view of its own** (agreed 2026-09-29): from the oldest known ancestor with the surname down through all
    generations, everyone who bore it — to see how the line branched and where it nearly died out (e.g. mostly
    daughters). A daughter who took her husband's name is shown, with her husband and children folded until clicked;
  - **partners always as their own cards**, even of another surname; now Potomkowie only writes them under the
    card („∞ Marianna z d. Nowak”), and Przodkowie shows only the couples of parents;
  - **the person in the centre** stays (Rodzina: parents, siblings, partners, children); you get there e.g. by
    clicking a person in Całe drzewo (see „Clicking a person…” above);
  - **Przodkowie and Potomkowie are unclear** — what they are and how they differ from the rest (now: 3 generations
    up as a pedigree to the right; 2 generations down); say it on screen, go deeper, or fold them into „Ród”;
  - now in Rodzina, walking down hides the older generations, so a whole family can't be seen at once;
  - first **an agent's review from a family member's point of view** (what's confusing, what's missing), then a
    proposal for you to approve; the designer may need to draw the new view. Not yet: remind you when coding of
    these plans starts (2026-09-29)
- [ ] **Check that the AI instructions still fit** (2026-09-29): `docs/AI_INSTRUCTIONS.md` (the text „Kopiuj
      instrukcję dla AI” copies) and `docs/IMPORT_FORMAT.md` were written before the planned Import changes —
      Heirloom numbering the files itself, several packages in one import, `.zip` packages, photos without an AI
      answer. Update them together with those changes (and the format version if it changes), then run the AI-trial
      packages again
- [ ] **Rodzina: siblings always shown** (2026-09-29): now they're folded into „+N rodzeństwa” and need a click.
      The person in the centre always shows the closest family: parents, siblings (without the siblings'
      children), partner(s) and children — so when a branch ends with no children, you go on through a sibling
      straight away. Check how it looks with 10+ siblings
- [x] **Your idea: how the archive stores its data** — decided 2026-09-29: it stays one file. Now: one `rodzina.ged` (GEDCOM 7, all
      people, families, sources, texts) + `media/` (photos, PDFs) + `zrodla-ai/` (the AI answers as they came) +
      `.heirloom/` (settings, change history, backup copies); „Zapisz import” writes the batch into `rodzina.ged` at
      once, after a backup copy, as one change „Cofnij import” can take back; what GEDCOM has no field for is written
      into the same file with Heirloom's own `_HLM_…` tags, so it travels with it. Your proposal: `osoby/` with one
      GEDCOM file per person + `media/`, the small program files next to them or in the program's own database. My
      suggestion: keep one file — GEDCOM is made as one file per tree (relatives point at each other inside it), no
      other program opens a folder of per-person files as one tree, and 10,000 files are slower and more fragile to
      copy and back up than one
- [ ] **„Przenieś archiwum…”** (agreed 2026-09-29): one click in Ustawienia › Archiwum moves the whole archive
      folder (data, photos, the program's files) somewhere else — e.g. onto the shared server — and Heirloom opens it
      from there; now: close it, move the folder in Explorer, open it again with „Otwórz folder…”
- [ ] **One archive shared on a server** (2026-09-29; how you'll work: either only your partner edits, or you share
      one archive on a server or a synced folder, so what anyone does is the same for both). Everything is in the
      archive folder already (the family data with Heirloom's extras in `rodzina.ged`, `media/`, and `.heirloom/`
      „Obok danych”), so sharing the folder shares all of it. To add: a notice „Teraz edytuje: Ewa, od 14:05” so two
      people don't edit at once (a save already warns when the file changed on disk meanwhile); test it on a network
      drive and in OneDrive / Google Drive / Dropbox folders
- [ ] **„Gdzie trzymać” the program's files: build it** (wanted 2026-09-29; it's the „Obok danych / W programie” row
      greyed out in Ustawienia › Pliki ustawień): „Obok danych” keeps `.heirloom/` (settings, history, backups) in
      the archive folder — shared with everyone who has the folder; „W programie” keeps it on this computer only.
      Switching moves the files with one click
- [ ] **Pictures inside a biography** (2026-09-29): a photo from the archive placed in a biography chapter's text,
      shown with its caption — or, if that is too much, a link in the text that opens the photo big. Biography
      chapters themselves already exist (see the answer of 2026-09-29)
- [ ] **Test families with long biographies** (2026-09-29): `heirloom-gen` and the AI-trial packages have no
      biography chapters, so this part of the profile can't be seen while testing; give some people a long life
      story in chapters
- [x] **Import step 1: „Wyczyść listę”** (agreed 2026-09-29): one click removes everything loaded into the import
      in progress (e.g. the wrong folder was dropped), instead of „×” on every row; the archive isn't touched — done
      2026-10-03: it asks first only when answers and decisions from steps 2–5 would be lost
- [ ] **Ustawienia: „Cofnij wszystkie importy”** (agreed 2026-09-29): the archive goes back to how it was before
      the first import. Something to do only on purpose: several confirmations, each explaining exactly what will
      be removed (which imports, how many people and files); my suggestion: a backup copy (.zip) first. Each import
      can already be undone on its own; this does all of them at once
- [ ] **„Przygotuj pliki dla AI”: Heirloom numbers the files itself** (agreed 2026-09-29): now the
      researcher renames every file to `M001 …` by hand (200 photos = 200 renames). Instead: pick a folder, Heirloom
      makes numbered copies (`M001 IMG_2034.jpg`, …) in a new folder, originals untouched, and gives the line to
      paste into the chat („Wgrywam M001–M200”). Chats don't see file names reliably, so the AI can't do the
      numbering (IMPORT_FORMAT.md already foresees the app doing it)
- [ ] **Import accepts a .zip** (agreed 2026-09-29): a package sent as one file (by e-mail, or from a chat that
      can make ZIP files) is dropped as it is and read inside
- [ ] **Import: many photos → one person in one go** (agreed 2026-09-29): drop a lot of photos (also without any AI
      answer), select some or all of them in the list, „Przypisz do osoby…” with a search, and they all go to that
      person; now each file without a description asks for its person on its own row („Nie ma go w paczce — wskaż
      osobę”)
- [ ] **File names in `media/`: keep them or name them after the person** (asked for 2026-09-29): when photos are
      added, a choice — keep their own names, or name them after the person they go to („Sadowski Stanisław
      1907 01.jpg”). Heirloom itself doesn't need nice names (it keeps a link to each file, and who is on a photo is
      in the data), but someone opening `media/` without Heirloom does. Renaming must go through Heirloom (a file
      renamed in Explorer loses its link). How (agreed 2026-09-29): the question comes when photos are added, with a
      „Zapamiętaj mój wybór” checkbox; Ustawienia holds the choice („Pytaj” / „Zachowaj nazwy” / „Według osoby”) and
      can bring the question back. „Według osoby” is the default, and the old name is kept in the photo's details,
      so nothing is lost (agreed 2026-09-29). (A later tool to rename files already in the archive: dropped.)
- [ ] **The photo's title and description written into exported copies** (metadata, e.g. XMP — what Windows shows
      as „Tytuł”; agreed 2026-09-29): only in copies made by an export, so each photo sent to someone carries its
      description; the archive's own files are never changed (the data stays in `rodzina.ged`)
- [x] ~~**Your idea: the AI writes GEDCOM instead of the JSON answer**~~ — decided 2026-09-29: the JSON answer stays
      (Heirloom checks it, explains errors in Polish and matches people without doubling them; chat AIs often break
      GEDCOM's strict rules). The archive itself is GEDCOM anyway (`rodzina.ged`)
- [ ] **Import a GEDCOM file into an existing archive** (2026-09-29; to decide): a `.ged` from someone else goes
      through the Import's checking and matching, so the two trees join without doubled people (now a `.ged` can only
      be opened as its own archive)
- [ ] **Where edits are saved** (your question, 2026-09-29): already works like this — Heirloom's own archive: into
      `rodzina.ged`, after a backup copy of the old version; a file from another program: the first save asks
      „Zapisać w tym pliku?” or „Zapisz jako nowe archiwum” (the original stays as it was). Anything more wanted here
      is still open

## Later (each approved separately)
- [ ] "Ścieżka pokrewieństwa": how any two people are related (Polish kinship terms)
- [ ] PIN for edit mode
- [ ] Audio and video (e.g. recorded family stories)
- [ ] A random story, data-quality nudges on the Start screen („W tym dniu” is done)
- [ ] Wachlarz (fan chart), Klepsydra, Ścieżka pokrewieństwa, Oś czasu
- [ ] Timeline with Polish history; map
- [ ] Face tagging in photos
- [ ] Consistency checks; duplicate finder
- [ ] Research to-do list per person
- [ ] Roster export for the AI
- [ ] GEDCOM 5.5.1 compatibility export (MyHeritage, Ancestry); Gramps XML import
- [ ] "Compact archive" (JPEG → JPEG XL)
- [ ] Privacy flag for living people
- [ ] Statistics; posters; PDF family book
- [ ] English UI; shortcut settings
- [ ] Coats of arms (herb)
