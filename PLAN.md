# Heirloom — Project Plan

> A Windows desktop app that keeps a family's history in one place: one big interactive family tree, plus a rich
> profile for every person with photos, a structured life story, sayings, stories, trivia, documents and sources.

---

> **IMPORTANT — Development rule:** every feature or change is presented to the user and explicitly approved before
> it is implemented. This plan collects ideas, and some of them may change or be dropped. Nothing gets built without
> a green light.

---

## 0. Status and decisions

- **2026-09-27: planning done.** The research is in [docs/RESEARCH.md](docs/RESEARCH.md), and the step-by-step
  roadmap with checkboxes is in [TODO.md](TODO.md).
- **2026-09-28: Phase 0 started.** The resize test passed ([spikes/resize-test](spikes/resize-test/README.md)).
- **Decisions so far:**

  | Decision | Choice | Why |
  |---|---|---|
  | **Direction** | Our own app (option C) | Existing apps compared in [RESEARCH §1](docs/RESEARCH.md#1-existing-apps-could-we-use-one-instead-of-building) |
  | **Name** | Heirloom | App ID `com.husarp.heirloom` |
  | **Licence** | MIT, like the other apps (confirmed 2026-09-28) | "Others can use it as they want". Consequence: we don't copy code from Gramps, which is GPL; we write our own Polish date and kinship logic from the rules in [RESEARCH §3](docs/RESEARCH.md#3-genealogy-domain-and-polish-specifics) |
  | **AI** | Not built into the app | The app imports files that an external AI chat makes by following [docs/AI_INSTRUCTIONS.md](docs/AI_INSTRUCTIONS.md) |
  | **Platform** | PC (Windows) only | A phone version isn't planned |
  | **UI language** | Polish | Built with translations in mind, so English can be added later |
  | **Users** | The whole family, on one computer, with no login | From the designer brief (2026-09-28) |
  | **No "Ty" (no home person)** | Relationships are always shown relative to the person being viewed. The tree opens on the last person viewed, else on a suggested one (the start person setting was removed 2026-10-03) | From the designer brief (2026-09-28) |
  | **Modes** | Browsing by default. Editing is switched on separately and asks "Kto edytuje?" (who is editing); that name goes into the change history | From the designer brief (2026-09-28) |
  | **The researcher's AI** | Free ChatGPT, maybe free Gemini | Small batches, parts of at most 10 persons, results copied from the chat (no downloads) |
  | **Fully offline** | Nothing is downloaded while the app runs: fonts, icons and every other asset ship with the app | Web links (e.g. Wikipedia) open in the browser only when clicked |
  | **Approved 2026-09-28** | Open on Start and load the tree on demand (§11.1). The family archive is an open folder with GEDCOM, editable with a clear indication, and the Import is the built-in converter (§11.2) | The sections below are updated to match |
  | **Resize** | Staying with Tauri; no Electron comparison | Your decision, 2026-09-28 |

## 1. What we know about the data

- **There is no existing family tree** anywhere: no MyHeritage account, no Gramps database, no GEDCOM file.
- **We only have raw material.**
  - It is mostly documents and pictures: JPG screenshots, scans, photos and some PDFs.
  - An external AI agent will first turn them into text.
  - It will then turn that text into import files (see [docs/IMPORT_FORMAT.md](docs/IMPORT_FORMAT.md)).
- **So the import is the main way people get into the app.** Typing people in by hand is the second way; GEDCOM
  import from other programs is a later extra.
- **Scale:** hundreds of people, and the app is built for up to 10,000. Possibly several GB of media, although many
  of the images are only kilobytes each.

## 2. Requirements (from the brief)

### 2.1 The tree: the main engine
- One big family tree with all the connections, arranged automatically.
- The user can adjust it: drag, reorder, sort (how, see §4.2).
- Different ways to view it, for example by family or by surname, plus searching for people.
- It must stay smooth with a lot of people and data, and **must not lag when the window is resized**. Lockdown lags
  because CustomTkinter redraws every widget in Python on each step of a resize.

### 2.2 Person profile
- Every person has a profile page you can open.
- **Photos:**
  - a main profile photo;
  - more photos, each with a description (caption).
- **Biography (życiorys), laid out like a good article rather than plain text:**
  - headings and clear indentation;
  - the person's sayings (powiedzonka), important stories and trivia (ciekawostki);
  - divided into sections;
  - a table of contents at the top for jumping to a section;
  - a short summary at the very top.
- **Relatives** shown with labels, e.g. "matka: X", "syn: Y".
- **Mentions:** a person mentioned in any text can be a link, and clicking it opens that person.
- **Web links:** for some people, links to pages such as Wikipedia.
- **Facts:**
  - places, such as the town of birth, linked to a place page;
  - dates: birth and other events, plus the date the person was added to the app;
  - maiden names, which must be searchable.

### 2.3 Catalog, search and sorting
- A catalog of all the people.
- Sort and group by family, surname, dates (e.g. birth) and date added.
- Search, including by maiden name.

### 2.4 Adding data: import of files made by an external AI
- **The AI is not built into the app.**
  - We write an **instruction document**.
  - The researcher gives it, together with their raw material, to another AI chat.
  - That chat produces a file in our format.
- **Uploading the file:**
  - the app creates new people or updates existing ones, with their connections;
  - the import opens as a **preview**, like creating a profile, where the user:
    - connects names mentioned in the file to people already in the app;
    - sets connections by hand where needed;
    - arranges the photos and picks the profile photo;
    - then clicks **Save**.
- People can of course be edited in the app afterwards.

### 2.5 History and changelog
- A record of what was added or changed, and when, including the date each person was added.

### 2.6 Media and storage
- Media types: text, JPG/PNG screenshots, other photos, some PDFs; possibly several GB in total.
- **Wish: a very good built-in compressor**, where files are unpacked when opened and the unpacked copy is removed
  when closed. What the research suggests instead is in §4.7.

### 2.7 Settings
- Appearance, the kinds of display (views), and similar options.

### 2.8 Design
- A prompt for a designer. The UI must be pleasant and convenient for:
  - editing people and adding new things (trivia, places);
  - sorting people;
  - browsing the whole catalog.
- The prompt: [docs/DESIGNER_PROMPT.md](docs/DESIGNER_PROMPT.md).

### 2.9 Platforms
- **The PC (Windows) is what matters.** No phone version is planned.

## 3. Principles

1. **Always fast.** Window resize, pan, zoom, search and opening a profile never stall. There are measurable targets
   in §6.
2. **Nothing gets lost.** Every change is recorded and can be undone; there are automatic backups; import and export
   are lossless, media included.
3. **Review before save.** Imported data never lands without a preview, and nothing is ever merged automatically.
4. **Evidence stays visible but simple.** Every fact can show its source and certainty, without bureaucracy.
5. **Polish first.**
   - Search works with and without Polish letters (ł is included).
   - Name forms: Kowalski/Kowalska, Nowakowa/Nowakówna, "z domu".
   - Polish date wording ("ok.", "przed", "między … a").
   - Julian/Gregorian double dates, historical place names, Polish kinship terms.
6. **Edit where you read.** Editing happens in place on the profile, and everything can be done from the keyboard.
7. **Offline and private.** No account, no cloud, no internet needed: fonts and icons ship with the app. The family
   data lives outside the code repository.

## 4. What the app does

✅ = first usable version (v1.0) · ⏭ = later, each item approved separately.

Screen-by-screen details (the fields, actions, states and messages the designer asked about) are in
[docs/DESIGNER_ANSWERS.md](docs/DESIGNER_ANSWERS.md), in Polish.

### 4.1 Main window
- ✅ **Left sidebar:** Start, Drzewo, Osoby, Nazwiska, Miejsca, Historie, Media, Źródła, Import, Ustawienia.
- ✅ **Global search / command palette (Ctrl+K):**
  - finds people (every name form, maiden names, nicknames), places and actions;
  - works with or without Polish letters.
- ✅ **Navigation:** back/forward like a browser, and a hover card on every mention of a person.
- ⏭ **Oś czasu** (timeline) in the sidebar.

### 4.2 Tree (Drzewo)
The tree is loaded only when opened, with a short "Układam drzewo…" state. It does no work while another screen is
shown (§11.1).

- **Views:**
  - ✅ **Rodzina:** focused on one person, with parents, partners, children and siblings; click someone to refocus,
    with animation.
  - ✅ **Przodkowie** (pedigree) and **Potomkowie** (descendants).
  - ✅ **Całe drzewo:** everyone, laid out by generation.
  - ⏭ **Klepsydra** (hourglass).
  - ⏭ **Wachlarz:** a fan chart, colourable by missing photos or sources.
  - ⏭ **Ścieżka pokrewieństwa:** the path between two people.
  - ⏭ **Oś czasu:** lifespans on a timeline.
- **Interaction:**
  - ✅ zoom at the cursor, pan, fit to screen, minimap;
  - ✅ keyboard: ↑ parents, ↓ children, ←/→ siblings and partners;
  - ✅ search-to-jump;
  - ✅ hover highlights a person's direct line;
  - ✅ colour by branch, surname, father's/mother's side, or generation;
  - ✅ collapse/expand branches;
  - ✅ a side panel with a mini profile and "add relative" actions.
- **Manual arrangement.** The layout stays automatic, and your changes are stored as instructions, so adding people
  never scrambles the tree:
  - ✅ drag to reorder siblings or partners (the default order is by birth);
  - ✅ drag a whole branch to move it (the offset survives relayout; "reset branch");
  - ⏭ pins, and generation override;
  - ⏭ saved views ("Strona mamy");
  - ⏭ a frozen "poster" mode for free-form print layouts.
- **Cards:**
  - photo, given names, SURNAME, "z d." maiden name, years with †, and a branch-colour strip;
  - 4 levels of detail depending on zoom: dot → box → name and years → full card.

### 4.3 Person profile
✅ **Sections, top to bottom:**
1. **Hero:** photo, all names, life dates and places, age, tags, and the actions (edit, show in tree, add photo,
   history).
2. **W skrócie:** the summary plus a key-facts grid.
3. **Spis treści:** a sticky table of contents built from the biography's section titles.
4. **Rodzina:** relatives grouped (parents, siblings, spouses and children, grandparents, grandchildren) with labels.
5. **Życiorys:** Markdown sections with mentions and source markers [1].
6. **Historie, Powiedzonka, Ciekawostki:** stories, sayings and trivia.
7. **Galeria:** captions; a lightbox with zoom.
8. **Dokumenty:** PDFs and scans, with the transcription and translation side by side.
9. **Źródła:** sources.
10. **Linki:** web links.
11. **Oś życia:** the person's timeline.
12. **Wspomniany w…:** backlinks from other people's texts.
13. **Historia zmian:** change history.

**Later:**
- ⏭ "Ścieżka pokrewieństwa": how any two people are related, e.g. "Antoni jest prapradziadkiem Heleny", using
  Polish kinship terms (table in [RESEARCH §3](docs/RESEARCH.md#3-genealogy-domain-and-polish-specifics)).
- ⏭ A per-person research to-do list (Do zbadania).
- ⏭ Face tags in photos.

### 4.4 Editing
✅ **In place, section by section, with Zapisz / Anuluj and undo/redo:**
- **Names:** birth, married and other names; nickname; original forms (Latin/Cyrillic); sex; living/deceased.
- **Smart date field:** understands "12.03.1878", "03.1878", "1878", "ok. 1850", "przed 1900", "między 1850 a
  1855" and Roman-numeral months; shows how it read the input; optional Julian date.
- **Place picker:** a hierarchy such as village › parish › county, historical names, and "create a new place".
- **Relatives:**
  - add a parent, partner, child or sibling by searching (photo and years shown, to tell apart the many "Jan
    Kowalski"s) or by creating a new person;
  - relation type: birth, adopted, foster or step;
  - marriage date and place.
- **Photos:** drag and drop, reorder, profile photo with a round crop, caption, date.
- **Biography editor (TipTap):**
  - headings, lists, quotes, links;
  - "@" to mention a person, "/" for the command menu;
  - insert a photo from the gallery;
  - cite a source.
- **Stories, sayings and trivia** as separate items, each with a date and a source.
- **Web links** and **sources/citations** attached to facts.

### 4.5 Catalog, surnames, places
- ✅ **Osoby:**
  - a virtualized table with configurable columns, sorting, filter chips and grouping (surname group, branch,
    generation, birthplace, decade), an A–Z rail, and a card-grid toggle;
  - dates added and edited can be sorted.
- ✅ **Nazwiska:** surname groups that join the variants (-ski/-ska, -owa/-ówna, spelling variants), with counts.
- ✅ **Miejsca:** the place hierarchy, historical names, and a place page listing the people and events there.
  - ⏭ a map.
- ✅ **Branches ("rodziny"):** user-defined, from a founding couple to all their descendants. Used for "sort by
  family" and for tree colours.
- ⏭ **Statistics.**

### 4.6 Import
The Import is the **built-in converter** (§11.2 rule 8). It turns the researcher's files into the archive. For a family
with no archive yet, "Utwórz archiwum z plików" creates the folder and then runs the Import.

✅ **Pipeline** (details in [docs/IMPORT_FORMAT.md](docs/IMPORT_FORMAT.md) §6):
1. Paste the AI's answer ("Wklej odpowiedź AI"), or drop the batch folder: the JSON parts plus the M-files (photos,
   PDFs, text notes).
2. Check that every part is complete (end marker) and validate it.
3. Normalize names, dates and places.
4. Match incoming people against existing ones, with a score and the reasons shown.
5. Review.
6. Save:
   - the people are written into the archive's `rodzina.ged` (after a backup copy);
   - the files are copied into `media/` with readable names;
   - the batch is recorded in the history.
7. Undo the import ("Cofnij import") if needed.

✅ **The review screen:**
- buckets: new / matched / needs review;
- a side-by-side comparison with per-field keep / replace / add as alternate;
- an issues panel: unresolved mentions, missing files, the AI's questions, uncertain facts;
- a mini tree preview, with new people drawn dashed;
- a media grid where a ★ marks the profile photo;
- the evidence shown next to each fact.

**Also:**
- ✅ **"Kopiuj instrukcję AI"** (copy the AI instructions): the current prompt, always matching the format version.
- ✅ **Opening a GEDCOM file or folder directly** (§11.2): our own archives, plus other programs' files (GEDCOM 5.5.1
  or 7.0). Handling of other programs' quirks will keep improving over time.
- ⏭ **Roster export**, so the AI can reference people already in the app.

### 4.7 Media and documents
- ✅ **Storage:**
  - files live in the archive's `media/` folder with readable names, e.g. `1878 Akt urodzenia Józef Kowalski.jpg`;
  - duplicates are detected by hash, in the cache;
  - thumbnails and previews are generated in the background, into the cache.
- ✅ **"Zmniejsz zdjęcia bez utraty jakości"**, an explicit tool that never runs silently. It replaces zip-style
  compression, which saves only 1–4%:
  - PNG/BMP/TIFF → lossless WebP, 26–41% smaller, after a pixel check;
  - JPEGs and PDFs are kept as they are.
- ✅ **Missing files** (e.g. moved outside the app): find them by name in a folder, or point to them by hand.
- ✅ **Viewing:**
  - images are decoded in memory, so there is no "unpack" step;
  - PDFs open in the built-in pdf.js viewer;
  - "Open in external app" makes a temporary copy, and those copies are cleaned up at startup.
- ✅ **Media library:** filter by person, type and date; document viewer with transcription and translation.
- ⏭ **"Compact archive":** JPEG → JPEG XL, about 20% smaller, bit-exact reversible.
- ⏭ **Face tagging.**

### 4.8 History, safety, export
- ✅ **Change log** (in `.heirloom/historia.jsonl`): every change records who, when, and what (before → after).
  There is a history tab per person, a global activity feed, undo of any change, and undo of a whole import batch.
- ✅ **Created and edited dates** on every record, sortable.
- ✅ **Safe saving:**
  - the edit bar shows who is editing and which file (§11.2 rule 4);
  - a backup copy of `rodzina.ged` is made before every save (kept in `.heirloom/`, rotated), and the new version is
    written safely;
  - if another program changed the file meanwhile: "Wczytaj nową wersję" (reload) or "Zapisz moje zmiany jako
    kopię" (save my changes as a copy).
- ✅ **Backups:** "Utwórz kopię archiwum (.zip)" of the whole folder, and restore. The folder itself can also simply be
  copied.
- ✅ **GEDZIP export** (GEDCOM 7 + photos in one file), and an archive check.
- ⏭ **GEDCOM 5.5.1 compatibility export**, for MyHeritage and Ancestry.
- ⏭ **A privacy flag for living people**, used in exports.

### 4.9 Settings (Ustawienia)
- ✅ **Wygląd:** light/dark/system theme, accent colour, text size (100–150%), density, animations on/off.
- ✅ **Drzewo:** default view, start person, card style, colour mode, line style, sibling order.
- ✅ **Osoby i daty:** name order, date format, Julian dates, how maiden names are written.
- ✅ **Archiwum:**
  - the folder and the data file; the read-only switch;
  - where the `.heirloom` files are kept (next to the data or in the app), and export/import of the display settings;
  - backups and GEDZIP export;
  - tools: missing files, "Zmniejsz zdjęcia", archive check, rebuild the cache.
- ✅ **Import:** the AI instructions, the format version, import history with undo.
- ✅ **Osoby edytujące:** the names offered by "Kto edytuje?".
- ✅ **O programie:** version, "what's new" (from the CHANGELOG), licence, a keyboard-shortcut cheat sheet.
- ⏭ **Prywatność** (with GEDCOM export), **Język** (English), a PIN for edit mode.

### 4.10 Start (home screen)
- ✅ **The app opens here** (§11.1): archive name and switcher, stats, recently added and edited people, quick actions.
- ✅ **Wybór archiwum and Pierwsze otwarcie** (choosing an archive, first opening): recent archives; "Utwórz
  archiwum z plików", "Otwórz folder…", "Otwórz plik GEDCOM…"; what was found and what's missing.
- ⏭ "W tym dniu" (anniversaries), a random family story, data-quality nudges ("12 osób bez daty urodzenia").

### 4.11 Later ideas (each needs approval)
- **Timeline** with Polish historical context: partitions, uprisings, wars.
- **Map** of events and migrations.
- **Consistency checks,** e.g. a mother too old at a birth, or a date off by 12–13 days between the Julian and
  Gregorian calendars.
- **Duplicate finder** across the whole tree.
- **Printing:** posters and a PDF family book.
- **Coats of arms (herb)** as their own records.

## 5. Architecture

### 5.1 Stack
- **Tauri 2** with **React 19 + TypeScript + Vite + Tailwind 4 + Zustand + i18next**, the same base as Handy Tool.
- **UI libraries:**
  - **PixiJS v8** for the tree;
  - **TanStack Virtual/Table** for long lists;
  - **TipTap** for the editor;
  - **pdf.js** for PDFs.
- **Rust core:**
  - **rusqlite** (bundled SQLite, FTS5);
  - **image** + **fast_image_resize** + **webp** for media;
  - **PDFium** for PDF thumbnails and text;
  - **blake3** for file hashes;
  - **ged_io** for GEDCOM (later).

### 5.2 Code layout (when the code starts)
```
Heirloom/
  src/                  React UI: app shell, features (tree, person, catalog, import, settings…), api.ts
  src/tree-engine/      PixiJS renderer (culling, level of detail, photo atlases)
  src-tauri/            thin Tauri app: commands, the media protocol, window
  crates/heirloom-core/ data model, SQLite + migrations, change log, search, dates, names, kinship, import, media
  crates/heirloom-layout/  tree layout algorithms (focus views + the whole-family view)
  crates/heirloom-gen/  synthetic family generator for tests (dev only)
  docs/
```

### 5.3 How the parts talk
- **UI ↔ Rust:** typed Tauri commands wrapped in `api.ts`. Layouts go back as raw binary arrays (not JSON), so even
  10,000 people load instantly.
- **Media:** photos are served through a custom `heirloom://` protocol (thumbnails, previews and originals), never as
  base64.
- **Background jobs:** thumbnails, PDF text and import processing run on Rust worker threads and report progress to
  the UI.
- **Archive and cache:**
  - The Rust core reads and writes the archive folder. The GEDCOM parser keeps a lossless line tree, so data it
    doesn't understand survives a save.
  - A SQLite cache is built from the archive for fast search, lists and layout (one writer thread plus readers, WAL).
    It can be deleted and rebuilt at any time.
- **The UI also runs in a normal browser with a mocked `api.ts`,** so it can be tested quickly with Playwright.

### 5.4 Where the data lives (never in this repository)
- **The family archive:** a normal folder the user chooses (§11.2). It holds:
  - `rodzina.ged` (GEDCOM 7), `media/` and `zrodla-ai/`;
  - `.heirloom/`: settings, tree arrangement, change history, and backup copies of the `.ged`.

  This folder is the source of truth: copy it and everything goes with it.
- **The cache:** `%LOCALAPPDATA%\Heirloom\cache\<archive>\`, holding SQLite (search, lists, layout) and thumbnails.
  It's rebuilt automatically, never synced, and safe to delete.
- **OneDrive or a USB stick are fine for the archive folder.** The data is plain files written with a safe replace.
  The SQLite cache stays outside the folder, because sync can corrupt open SQLite files.

### 5.5 Data model (first draft)

This is the model the app works with, in memory and in the SQLite cache. Where each part is stored:
- **Loaded from and saved to `rodzina.ged`:** standard GEDCOM 7 structures where they exist; documented `_HLM_…`
  extension tags for the rest (§11.2).
- **Every record has a stable GEDCOM 7 `UID`,** so links, history and layout survive saves.
- **Display-only tables** (`branch`, `layout_intent`, `setting`) are saved to `.heirloom/` files.
- **`change_log`** goes to `.heirloom/historia.jsonl`.

| Table | Holds |
|---|---|
| `person` | id (ULID), sex, living, summary, profile photo, created/updated at and by, import batch |
| `person_name` | person, type (birth/married/other), given, surname, nickname, original form + language, folded search key, surname group |
| `union` | partner A, partner B (either may be empty), type (marriage/partnership/unknown), order for each partner |
| `union_child` | union, child, relation to each parent (birth/adopted/foster/step/unknown), order |
| `event` | type, date (qualifier, calendar, values, original text, sort key, earliest/latest day), Julian date, place, value, cause, certainty, basis |
| `event_role` | event, person, role (principal/father/mother/spouse/godparent/witness/…), age as written |
| `place` + `place_name` | hierarchy (parent), type, coordinates; names with language and years of use |
| `source`, `citation` | source (kind, title, parish, year, akt, archive, call number, URL); citation (what it supports, locator, media, crop, transcription, certainty) |
| `media` + `media_link` | hash, type, size, dimensions, stored format, caption, date, transcription, translation; links to people/events/sources with a role (profile/gallery/document) and order |
| `text_item` + `text_person` | person, kind (bio/story/saying/trivia/note), title, Markdown, date, place, certainty, order; plus everyone who appears in it (the text shows on each of their profiles) |
| `mention` | derived: which text mentions which person (for "Wspomniany w…") |
| `link` | person, URL, title, kind |
| `branch` | user-defined families: name, colour, founding union |
| `layout_intent` | order constraints, branch offsets, collapsed branches, per view |
| `change_log` | time, author, batch, entity, action, before/after (JSON) |
| `import_batch` | time, author, name, raw pasted text, status, stats |
| `setting` | key/value |
| FTS5 tables | folded text of names, places, texts, captions and transcriptions |

### 5.6 Key algorithms
- **Search:** FTS5 plus our own letter folding (ł→l, ó→o, …) applied both to the indexed text and to each query,
  plus prefix search. "Similar surnames" use Beider–Morse / Daitch–Mokotoff (`rphonetic`).
- **Dates:** we store the verbatim text, the structured value, the earliest/latest day and a sort key (which can be
  overridden). Polish parser and display.
- **Kinship:** relationships are found through common ancestors, and Polish terms come from the table in RESEARCH §3.
  In-laws go through unions, and distant relatives get a descriptive label.
- **Tree layout:**
  - **Focus views** use tidy trees built from the focus person, with repeated people marked ↻.
  - **The whole-family view** uses a layered, generation-based layout. It keeps couples as blocks and siblings in
    order, respects your stored orders and offsets, and is cached until the data changes.
  - Details: [RESEARCH §7](docs/RESEARCH.md#7-tree-engine).
- **Rendering:** PixiJS draws only what is on screen, with 4 levels of detail and thumbnails packed into atlases.
  - The canvas buffer is screen-sized and only grows, so a window resize never re-creates it and **never triggers a
    relayout**.
  - The Phase 0 test showed this matters: resize hitches dropped from 4–9 to 0–1 per 60 resizes.
- **Import matching:** points for names, dates, places and relatives, with vetoes (e.g. a sex conflict). The scoring
  table is in RESEARCH §8.

### 5.7 As built (2026-09-28): where the code differs from §5.1–§5.3
- **Styling:** plain CSS with the design's tokens (`src/styles/tokens.css`, light and dark) instead of Tailwind. The
  design came as exact CSS values, so copying them was simpler and closer to it.
- **Text:** the Polish strings are written in the code; no i18next. An English UI is in "Later" in TODO.
- **One command channel:** every UI call goes through one JSON dispatcher, `Api::call(method, args)`
  (`crates/heirloom-api`). The Tauri window uses it through one command (`call`); layouts go back as compact JSON
  arrays, not binary. That was fast enough for 10,000 people.
- **Browser testing:** instead of a mocked `api.ts`, a small dev server (`crates/heirloom-bridge`) serves the real
  API and the media to a normal browser (`/api/<method>`, `/media/…`, proxied by Vite). The screens are tested
  against real data.
- **Tree:** the three focus views (Rodzina, Przodkowie, Potomkowie) are DOM cards over SVG lines with a CSS-transform
  camera and 4 levels of detail; their layout is in TypeScript (`src/screens/tree/layout.ts`). Only „Całe drzewo” uses
  PixiJS, with the layout computed in Rust (`crates/heirloom-api/src/tree.rs`). There is no separate
  `heirloom-layout` crate yet.
- **Search:** folded in-memory search over the derived view (Polish letters, ł included, maiden names). It answers
  in a few milliseconds at 10,000 people, so FTS5 isn't used for it yet.
- **PDFs:** shown with the WebView's built-in viewer; no pdf.js or PDFium yet (no PDF thumbnails).
- **Display settings** (name order, date format, Julian dates, surname joins) are stored with the archive and applied
  when the derived view is built; changing them rebuilds it.
- **Editing (2026-09-29, design v2):** in place on the profile, one section at a time (§4.4); the separate edit form
  is gone. A section's draft becomes an unsaved change with „Gotowe”; „Anuluj” undoes back to the undo depth the
  section had when it was opened; „Cofnij zapis” takes back one save's history entries.
- **Installer (2026-09-29):** not Tauri's NSIS bundle but the owner's standard (`../APP-STANDARDS.md` §1): a Python +
  PyInstaller `HeirloomSetup-<version>.exe` that installs, updates and uninstalls per user, built by
  `scripts/build.ps1` after a self-test of the packaged app. Update checks (§2–§3) wait for a decision: they would
  contact GitHub, and Heirloom works offline.

## 6. Performance targets (acceptance criteria)

Measured on this PC with a synthetic archive of **5,000 people, 20,000 media files and about 5 GB**, plus a
**10,000-person** stress graph:

| What | Target |
|---|---|
| Window resize on the tree screen | no visible stutter; frames under 16 ms |
| Pan and zoom | 60 fps at 5,000 people; at least 30 fps at 10,000 |
| Cold start until usable | under 2 s |
| Opening a profile | under 150 ms |
| Search while typing | results under 100 ms |
| Laying out the whole-family view | under 1 s at 5,000 people |
| Importing 100 people + 300 images | the review screen under 5 s; thumbnails finish in the background |
| 10 minutes of pan and zoom | memory stays flat (no leaks) |

## 7. Testing

- **Rust unit tests:**
  - the Polish date parser, name folding and gender forms;
  - the kinship calculator, checked against the RESEARCH §3 table;
  - import validation, with fixtures including cut-off and broken files;
  - match scoring;
  - media round-trips (the pixel hash must match);
  - layout rules: no overlaps, generations respected, stored orders kept.
- **Synthetic generator** (`heirloom-gen`), for realistic families with remarriages, adoptions and cousin
  marriages, used for the performance targets in §6.
- **UI tests:** Vitest for UI logic, and Playwright against the browser build with a mocked API, plus a few smoke
  tests of the real app.
- **The performance targets are re-checked** at the end of every tree-engine phase.

## 8. Roadmap (detailed tasks in [TODO.md](TODO.md))

| Phase | Goal |
|---|---|
| **0. Checks before building** | Resize/pan/zoom test with 3,000 dummy cards (Tauri vs Electron); trial run of the AI instructions on 5–10 real documents; media test on real files; design round 1 |
| **1. Foundation** | Project setup, Rust core, the archive folder (GEDCOM 7 reader/writer, `.heirloom/` files, safe saving), SQLite cache, change log, archive picker and first-open screen, browse/edit modes, settings, synthetic generator |
| **2. People and profiles** | People, names, dates, places, families; the profile page; in-place editing; photos and documents; the catalog; search; history |
| **3. Import (the converter)** | Format 1.0 importer (AI answers + photos + PDFs + notes), review screen, matching, writing into the archive (GEDCOM + `media/`), undo; "Utwórz archiwum z plików"; "Kopiuj instrukcję AI" |
| **4. Tree** | Layout crate, PixiJS renderer, focus views, the whole-family view, arranging the tree |
| **5. Finishing v1.0** | Start screen, surnames, places, branches, settings, a design pass with the final mockups, installer |
| **6. Later** | Items marked ⏭, one at a time, each approved first |

## 9. Risks

| Risk | What we do |
|---|---|
| **Resize lag in WebView2**, the engine Tauri uses | The Phase 0 test **passed on this PC** (2026-09-28): panning and zooming at a locked 75 fps with 10,000 people; resizing at 72 fps with a screen-sized canvas. Still to do: a check by eye. Fallback: Electron, since the UI code stays the same |
| **The whole-family layout looks messy for complex families** | Focus views come first; the big view is tuned on real data and on public test trees (up to 34k people) |
| **The AI misreads documents or invents facts** | Transcription first and checked by a person; `basis`/`certainty` fields; the review screen; nothing is merged automatically |
| **Large answers get cut off** (the researcher uses free ChatGPT or Gemini) | Parts of at most 10 persons, small batches, a manifest, and the end marker |
| **Data loss** | Change log; a backup copy of the `.ged` before every save; safe writes (write a new file, then swap); imports never overwrite without asking; the archive folder is easy to copy |
| **Another program changes `rodzina.ged` while Heirloom is open** | Heirloom notices and offers: reload, or save my changes as a copy |
| **Other programs' GEDCOM quirks** (encodings, vendor tags) | A lossless line tree keeps unknown data; the "Inne dane z pliku" section shows it; tested with the gedcom.io test files and real exports (RESEARCH §2) |
| **Scope creep** | The approval rule, the phases, and the "later" list |
| **Privacy** of living relatives, and photos uploaded to AI chats | A local-only app; a reminder to switch off model training; roster export only on opt-in |

## 10. Open questions

- None. (§11 approved 2026-09-28. The home-person question is settled: there is no "Ty"; see §0.)

## 11. Approved proposals (2026-09-28)

### 11.1 Open on Start; load the tree only when it's opened
- The app opens on **Start** (fast: stats, recent changes, quick actions), not on the tree.
- The first time **Drzewo** is opened, it shows a short "Układam drzewo…" state while the layout is computed. That
  takes well under a second for thousands of people, and the result is cached.
- Leaving the tree stops its drawing completely (no work in the background). The layout stays in memory, so coming
  back is instant.
- This helps startup and background CPU use. It does not change how resizing feels while the tree is open; that is
  handled separately (the eye check, then possibly an Electron comparison).

### 11.2 The family archive as an open folder (the "overlay" idea)
**Status:** approved 2026-09-28, with your changes (rule 4: editing allowed with clear indication; rule 8: the
built-in converter). The sections above are updated to match.

**The idea:** Heirloom is a front-end over data the family owns, in universal formats. It doesn't lock the data
inside itself, and its own display information lives in small separate files.

**How others do it:**
- **Ancestris:** "Your data is only in a Gedcom file, so never lost or unreadable by other software."
- **Family Historian:** it "can save all of its data to the GEDCOM format".
- **Obsidian:** the "vault" is a normal folder of files; the app's own settings live in a hidden `.obsidian` folder
  inside it.
- **Lightroom and darktable:** the original photos are never changed; edits live in small "sidecar" files (XMP) next to
  them.
- **Kodi and Jellyfin:** they read music and film folders as they are. Extra information lives in small `.nfo` files
  next to the media, and their internal database is only a cache.

**Proposed layout:**
```
Rodzina Kowalskich/            ← any folder you choose; copy it anywhere, open it on any PC
  rodzina.ged                  ← all people, families, events, places, sources and stories (GEDCOM 7)
  media/                       ← photos, scans and PDFs as normal files with readable names
  zrodla-ai/                   ← the researcher's AI files, kept as delivered (where the data came from)
  .heirloom/                   ← Heirloom's own small files (like .obsidian); the data never depends on them
    ustawienia.json            ← how to display: colours, start person, card style…
    uklad.json                 ← tree arrangement: moved branches, collapsed branches, saved views
    historia.jsonl             ← who changed what, and when
```
The search index, thumbnails and tree-layout cache go to `%LOCALAPPDATA%\Heirloom\cache\`. They are rebuilt
automatically, never synced, and safe to delete.

**Rules:**
1. **Open formats only.** The family data lives only in GEDCOM 7 plus normal media files, so any genealogy program can
   open it.
2. **Display information stays out of the data.** It lives in `.heirloom/`. Delete that folder and the data is intact;
   Heirloom just starts with defaults.
3. **Writes only on Save.** Heirloom changes the data only when someone edits or imports and clicks Save. It makes a
   backup copy of the `.ged` first, writes the new version safely, and records the change in the history.
4. **Any archive can be edited, and it's always clear which file is being edited.** (Changed 2026-09-28 at your
   request; before, other people's files opened read-only.)
   - In edit mode, a bar at the top says who is editing and which file the changes are saved to.
   - The first save into a file that came from another program (e.g. a cousin's GEDCOM from MyHeritage) asks first.
     It makes a backup copy and also offers "Zapisz jako nowe archiwum" (save as a new archive).
   - "Tylko do odczytu" (read-only) remains as an optional switch for each archive, off by default.
5. **No setup needed to open.** Open a folder or a `.ged` and the tree appears.
   - Heirloom detects the text encoding, finds photos by file name, picks a start person and groups surnames.
   - A short, optional setup screen shows what it found and what's missing.
6. **Where the `.heirloom` files live.** Next to the data by default, or inside the app (for read-only or shared
   folders). They can be exported and imported as one settings file.
7. **Big and small archives both work.** The first time a 10,000-person file is opened, the cache is built (a second
   or two). After that, opening is instant as long as the file hasn't changed.
8. **The built-in converter.** The family has no GEDCOM yet, only text and pictures, so the app creates it.
   - The **Import** is the converter. It takes the researcher's files: the AI's answers (text), photos, PDFs and text
     notes. It shows the review screen, and on Save writes the people into the archive's `rodzina.ged` and copies the
     files into `media/`.
   - On first launch the main action is "Utwórz archiwum z plików" (create an archive from files): choose a folder,
     then go through the Import.
   - Only the AI's answers turn into people automatically. The app has no AI inside, so plain notes and photos without
     an AI answer are added as documents or photos to people chosen by hand.

**What GEDCOM can't hold on its own:** story kinds, certainty, mentions, historical place names, Julian dates, photo
crops.
- GEDCOM 7's official tools cover part of it: QUAY (certainty of a source), PHRASE (the original date text), CROP
  (photo regions), TRAN (other name versions) and HTML notes.
- The rest goes into documented extension tags (`_HLM_…`) declared in the file header. That is the official GEDCOM 7
  way, and other programs keep or ignore them.

**Trade-offs:**
- **More work** than a private database: a careful GEDCOM writer, and detecting when another program changed the
  file while Heirloom is open (then: reload, or save my changes as a copy).
- **Media files keep readable names** in `media/`. So compression becomes an explicit tool ("Zmniejsz zdjęcia bez
  utraty jakości"), never a silent step. Duplicates are still detected, by hash in the cache.
- **MyHeritage and Ancestry read only GEDCOM 5.5.1,** so they get a separate compatibility export, which loses some
  extras.

**What changed in this plan:**
- §5.3–§5.5: the SQLite database becomes a rebuildable cache, and the archive folder becomes the source of truth.
- §4.7 media: readable files and explicit compression.
- §4.8 history and backups: backups are copies of the folder or the `.ged`, and the history lives in
  `.heirloom/historia.jsonl`.
- §4.6 import: it writes into the archive.
- The designer notes: an archive picker, a setup screen and a read-only state.
- TODO Phases 1–3.
