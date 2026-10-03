# Heirloom — Research notes

Collected on **2026-09-27**, before choosing the project's direction. Sources: six research passes over web pages,
the GitHub API, package registries and a few local measurements on this PC. All versions and numbers are as of that
date.

**Caveats:**
- Reddit could not be read (it blocks automated access).
- The web-search quota ran out during the later passes, so some checks used project sites and GitHub directly.
- **[unverified]** marks claims that rest on a single weak source or on memory.

Sections: 1 Existing apps · 2 File formats (GEDCOM) · 3 Genealogy domain & Polish specifics · 4 Technology ·
5 Storage & search · 6 Media compression · 7 Tree engine · 8 AI-assisted import · 9 Reusable parts & licences

---

## 1. Existing apps: could we use one instead of building?

**Verdict:** no free app covers the whole brief. The strongest free, open-source option is Gramps 6, together with
Gramps Web. None of the free apps has any of these:
- a smooth, editable "everyone" tree for thousands of people;
- article-like biographies (summary, table of contents, sayings, stories, trivia);
- import of AI-made files with a review screen;
- media compression.

| App | Licence / cost | Polish UI | Whole-family tree | Life stories | Modern look | Notes |
|---|---|---|---|---|---|---|
| **Gramps 6.0.8** (desktop) | GPL-2.0+, free | ✅ 99% | ◐ add-ons; "everyone" view is slow | ◐ formatted notes with links to people; no headings | ❌ GTK3, dated | Python; since 2001; 6.1 beta2 on 2026-09-13 |
| **Gramps Web** | AGPL-3.0, free | ◐ 59% | ◐ one person at a time; "all connected" capped at about 1,000 | ◐ blog-style stories, `[[links]]` | ✅ | needs Docker on Windows; revision history (who, when, diff) |
| **webtrees 2.2.6** | GPL-3.0, free | ✅ 100% | ◐ via modules | ✅ Stories module | ◐ | PHP 8.3+ web server (SQLite works; current XAMPP is too old) |
| **Ancestris 13** | GPL-3.0, free | ✅ | ◐ | ◐ | ◐ | Java; reads and writes GEDCOM 7; AI transcription |
| **GEDKeeper 3.15** | GPL-3.0, free | ✅ | ◐ | ◐ | ◐ | C#; has an MCP connector |
| **My Family Tree 16.1** (Chronoplex) | free, closed source | ✅ | ◐ chart with minimap | ◐ formatted notes | ✅ | GEDCOM 7 + GEDZIP; Windows only |
| **Legacy 10** | free since 2024, closed source | ❌ | ◐ | ◐ Stories tool | ❌ | Windows only |
| **Family Tree Builder 8** (MyHeritage) | freemium | ✅ | ❌ the all-in-one chart is paid | ◐ | ◐ | smart-match merging is paid |
| **RootsMagic Essentials** | free, cut-down | ❌ | ❌ | ❌ | ◐ | no web links, no note formatting |
| **Obsidian + Charted Roots** | Obsidian free (closed); plugin MIT | ◐ | ◐ | ✅ Markdown pages with headings and wikilinks | ✅ | plugin v0.22; GEDCOM/Gramps import with a review step |
| **Topola Viewer** | Apache-2.0 | ✅ | ✅ charts (viewer only) | ❌ | ✅ | opens .ged/.gdz in the browser; can run offline |
| **Family Gem** (Android) | GPL-3.0 | ✅ 97% | ◐ | ◐ | ✅ | GEDCOM 5.5.1 only |

**Gramps** — https://github.com/gramps-project/gramps (about 3,100★, about 14 MB of Python)
- **Data model:** event-centric, so one event is shared by several people, each with a role. Places form a
  hierarchy with dates.
- **Tools:** rule-based filters, "Verify the Data", a duplicate finder with merge, and the "Import and Merge" add-on.
- **Polish support:** a Polish relationship calculator (`gramps/plugins/rel/rel_pl.py`) and a Polish date handler
  (`gramps/gen/datehandler/_date_pl.py`).
- **Media** stay where they are on disk; nothing is copied or compressed.
- **History:** a last-changed time per record, plus undo within a session.
- **Tree add-ons:**
  - Graph View (uses Graphviz);
  - FamilyTreeView (zoomable, with a minimap; described as "not yet stable");
  - Topola add-on (loads the viewer from the web).
- **Narrated Web Site** report builds a static website.
- **Performance:** a user reports that importing a 161k-person GEDCOM took 17 minutes.
- **Usability complaints from newcomers:** the marriage date is hidden in the family's Events tab, and the tree is
  "just a browser".

**Gramps Web**
- **Code:** frontend https://github.com/gramps-project/gramps-web (JavaScript/Lit, about 1,700★, v26.9.1); API
  https://github.com/gramps-project/gramps-web-api (Python/Flask, v3.22.3).
- **Features:**
  - stories (blog) and rich notes with a `[[` link dialog;
  - full-text and semantic search, OCR, face detection;
  - revision history with diffs;
  - AI chat (OpenAI, Mistral or a local Ollama model);
  - installable as a PWA.
- **On Windows** it needs Docker Desktop (3 containers). The alternative is `gramps-web-desktop`, a pip package
  described as "alpha … for experimental use only" (4★).
- **Performance:** a core developer's 100k-person test found slow list views.

**webtrees**
- Stories, an interactive tree, find duplicates, merging of records and whole trees.
- Approval of pending edits, and a change log.
- Add-on modules: D3 charts, "Potts Biography", themes, a Topola tree.
- Exports GEDCOM 5.5.1 only.

**Paid apps, for reference**
- **Family Historian 7:** the only app with "all relatives" and "everyone" diagrams you can browse and edit. Also rich
  notes with links, and linking faces in photos to people.
- **Heredis 2027:** an AI reading assistant that transcribes and translates records.
- **MacFamilyTree 11:** Mac only; considered the best-looking.
- **RootsMagic 11:** a Life Summary panel and an AI Prompt Builder.
- **Ahnenblatt 4:** now paid, and refuses GEDCOM 7 files.
- **GenoPro:** a free drag-anywhere canvas.

**Newer GitHub projects (2023–26)**
- Bonsai: C#, MIT; a family wiki; no GEDCOM.
- Gramps Connect: AGPL; alpha.
- Tauri/Electron family-tree apps (vata-app, Tree-Monk): under 35★ and only months old.

**What Polish users say** (forum.genealodzy.pl)
- MyHeritage dominates, with complaints about billing and bugs.
- For desktop use, members recommend Gramps ("free, fully translated, frequently updated").
- There is also a paid Polish program, "Drzewo Genealogiczne II".

**Common complaints about genealogy apps**
- too many windows, and fields buried deep;
- redesigns that break keyboard workflows (RootsMagic 8);
- fragile sync;
- data lost in GEDCOM transfers (media, notes, citations);
- subscriptions.

**Rules this suggests for our app:**
- edit directly on the person page, keyboard first;
- undo and history everywhere;
- lossless import and export, media included;
- no account needed;
- charts that print on one page or one poster.

## 2. File formats (GEDCOM)

**Basics**
- **GEDCOM** (GEnealogical Data COMmunication, FamilySearch, 1984) is the standard exchange format, stored in `.ged`
  files. Every serious app keeps its own database and uses GEDCOM only to move data in and out.
- **Current version:** FamilySearch GEDCOM **7.0.18** (2026-02-17). Whether the next one is 7.1 or 8.0 is still
  being discussed: place records, citation records and a revised NAME structure are on the table, with no release
  date. https://gedcom.io/changelog/
- **7.0 compared with 5.5.1** (https://gedcom.io/migrate/):
  - UTF-8 only; no CONC continuation lines; no line-length limit.
  - Shared notes (SNOTE); HTML or Markdown notes (MIME).
  - TRAN + LANG for translations and transliterations (NAME-TRAN for names).
  - EXID/UID identifiers; SUBN removed; RELA becomes ROLE.
  - New: NO (negative assertion), SEX X, PHRASE, SDATE (sort date), CROP (image region).
  - FILE paths are URIs.
  - Dual years like 1750/51 are removed; store one DATE plus a PHRASE with the original text.
- **GEDZIP (.gdz):** a zip with `gedcom.ged` plus the media files at relative paths.
  - Put media under `media/`; no leading `/`, no `..`, no `\`.
  - Store images uncompressed.
- **Gramps XML (.gramps):** gzipped XML (schema 1.7.2).
  - Objects link by handles.
  - Dates are stored as dateval, daterange, datespan or datestr, each with a quality flag.
  - Media carry an MD5 checksum.
- **Gramps package (.gpkg):** a tar.gz with `data.gramps` plus the media. This is the lossless way out of Gramps.
  Gramps Web imports .gramps files but not .gpkg.
- **GEDCOM X:** the JSON/XML model behind FamilySearch's API. Only relevant if we ever sync with FamilySearch.

**Who supports GEDCOM 7 (2025–26)**
- **Reads and writes 7.0:** Family Historian 7 (including GEDZIP), Ancestris 12+, My Family Tree 16. FamilySearch
  exports 7.0 (you plus 8 generations).
- **Reads 7.0 but writes 5.5.1:** RootsMagic 9–11.
- **7.0 only through extras:** Gramps 6.0 through a separate library (gramps-gedcom7); webtrees through a
  third-party module.
- **5.5.1 only:** Ancestry/FTM, MyHeritage, Findmypast [one source]. Ahnenblatt refuses 7.0.
- **Viewers:** Topola reads .gdz; Family Gem reads 5.5.1 only.

**Pitfalls when importing other apps' files**
- **Vendor-specific tags:**
  - Ancestry: `1 FILE` is empty, so there are no media paths at all; citations use `_APID`, plus `_META`.
  - MyHeritage: media links are signed URLs that **expire**, so download the files at import time. Also `_PRIM`,
    `_POSITION` (a crop box), `_UID`.
  - FTM: `_FREL`/`_MREL`, `_MILT`.
  - RootsMagic: `_TMPLT` (source templates).
  - Legacy: `_TAG…_TAG9`, `_TODO`.
  - GEDCOM-L (a German convention): `_GODP` (godparents), `_GOV` (gazetteer IDs).
  - Reference lists: webtrees `app/CustomTags` (GPL: read it, don't copy it) and Gramps' `libgedcom.py`.
- **Encodings:**
  - 5.5.1 allows ANSEL, UTF-8, UNICODE (meaning UTF-16) and ASCII. Files in the wild also claim ANSI (cp1252),
    IBMPC and others, or carry the wrong label.
  - Sniff the BOM and check UTF-8 validity before trusting the header.
  - **ANSEL puts combining marks before the base letter:** Ł = A1, ł = B1, acute (ć ń ó ś ź) = E2, dot above (ż) =
    E7, ogonek (ą ę) = F1. Libraries that treat ANSEL as cp1252 mangle Polish names.
- **Lines:**
  - CONC must split at a non-space, but writers often split at spaces, so words get glued together or broken apart.
  - Also expect `@` characters that weren't doubled, CR-only line endings, and HTML tags inside 5.5.1 notes (sanitize
    them).
- **Dates:**
  - `AFT 1850` means ≥1851 under the 5.5.1 wording but ≥1850 in 7.0.
  - In 7.0, `BET x AND y` requires x ≤ y.
  - Dual years (1750/51) appear.
  - Numeric or localized dates appear too: Ancestry writes DD/MM/YYYY.
- **General rule:** keep every unknown tag losslessly (a raw line tree underneath a typed model), and show an import
  report instead of dropping anything silently.

**Test data and tools**
- **Official GEDCOM 7 test files:** https://gedcom.io/testfiles/gedcom70/ (maximal70 .ged/.gdz, minimal70, notes,
  remarriage, same-sex marriage, …).
- **Torture tests:** TGC551/TGC55C (non-commercial use only).
- **Encoding variants:** https://github.com/frizbog/gedcom4j/tree/master/sample (UTF-16, ANSEL, BOM).
- **Real-world corpus:** https://github.com/D-Jeffrey/gedcom-samples: royal92 (ANSEL, 3k people), Habsburg (34k),
  Longsword (203k).
- **Validators:** https://github.com/ArmidaleSoftware/gedcom7 (C#, MIT; GedValidate/GedCompare), domorium (MIT),
  js-gedcom.
- **Converters:** ged5to7 (5.5.1→7.0), @kleiobase/gedcom-converter (7.0→5.5.1).

**Libraries**
- **Rust:** **ged_io 0.16.4** (MIT). It reads and writes 5.5.1 and 7.0 and handles UTF-8/16, ANSEL, Latin-1/9,
  GEDZIP and a lenient mode. It's the best permissive candidate; first check that vendor tags survive a round trip.
  gedcomkit is excluded because it is AGPL.
- **TypeScript:**
  - @treeviz/gedcom-parser 2.1.23 (MIT): 5.5.1 and 7.0, GEDZIP.
  - read-gedcom (MIT): read-only, last release 2022.
  - parse-gedcom (used by Topola): last release 2021.
- **Python:** gedcom7 (MIT; strict 7.0.18, by Gramps Web's lead developer); ged4py (MIT; read-only 5.5.1).

**What this means for Heirloom**
- **Storage:** our own SQLite schema; GEDCOM is for exchange only.
- **Export:**
  - 7.0 + GEDZIP as the full archive, plus a 5.5.1 UTF-8 compatibility export.
  - Biographies go into HTML notes, face tags into CROP, Cyrillic/Latin name forms into NAME-TRAN, and the Julian or
    original date text into PHRASE.
  - Declare our own `_TAGS` in HEAD.SCHMA.
- **Import:** expect mostly 5.5.1 from MyHeritage, Ancestry or Legacy. Handle UTF-8/16, ANSEL and CP1250, and common
  vendor tags such as `_MARNM`, `_UID`, `_PRIM` and `_FREL`/`_MREL`. We have no existing tree, so import is a nice
  extra rather than the main entry point.
- **The AI should not write GEDCOM.** In tests, models invented dates and places and guessed sex from names. GEDCOM
  also has no room for bio sections, quotes, confidence or original-vs-normalized values. So we use our own JSON (§8)
  and let the app produce GEDCOM.

## 3. Genealogy domain and Polish specifics

**Data model to adopt** (a simplified GEDCOM/Gramps core)
- **Person:** several typed names (birth, married, Latin, Cyrillic, alias), sex M/F/X/U, a privacy flag, external IDs.
- **Union:** two partners, a type, events (banns/zapowiedzi, marriage, divorce) and an ordered list of children.
- **Child link, typed separately for each parent:** birth / adopted / foster / step / other. This is what step-parents
  and adoption need.
- **Event with participants and roles:** principal, father, mother, godparent, witness, officiant, … A baptism record
  is naturally one event covering the child, parents, godparents, priest and midwife.
- **Associations:** godparent, witness, neighbour, guardian.
- **Place:** a hierarchy with dates, multilingual names (Poznań/Posen, Lwów/Lviv/Lemberg), coordinates and external IDs
  (Wikidata, GOV). Plus a parallel church hierarchy: parish → deanery → diocese.
- **Source → citation → repository:**
  - the citation holds the record (akt) number, a confidence level, a transcription and an image crop;
  - the repository holds the archive, call number (sygnatura) and URL.
- **Everything else:** media with captions and face regions, stories/quotes/notes, tasks, tags, an audit log.
- **Overkill for a family archive:**
  - persona/assertion evidence models;
  - citation-template libraries;
  - LDS ordinances and DNA tools;
  - multi-level certainty — a 3-state flag (proven / proposed / disproven) is enough.

**Dates**
- **Store four things:** the verbatim text; the structured value (qualifier ABT/EST/CAL/BEF/AFT/BET…AND/FROM…TO, plus
  the calendar); the earliest and latest day numbers; and a sort key the user can override.
- **Sorting:** undated events sort by type (birth, baptism, marriage, death, burial). Keep a manual child order.
- **Parse Polish input:** ok., przed, po, między…a, Roman-numeral months, and 7bris/8bris/9bris/Xbris (September to
  December).
- **Double dating in Congress Poland** (common in the Russian-era records, 1868–1917), e.g. "12/24 января 1883": Julian
  first, then Gregorian. The gap is 12 days in the 1800s and 13 days from 1900. Store the Gregorian date as the main
  date and keep the Julian one in the phrase.

**Record languages by partition**
- **Latin:** older parish books everywhere; tabular Latin books in Galicia until about WWI.
- **Polish:** narrative records ("Działo się…") in the Duchy of Warsaw and Congress Poland, 1808 to 1867/68.
- **Russian:** Congress Poland from about 1868 to 1917.
- **German:** the Prussian partition, including civil registry offices (Standesamt) from 1874, often in Kurrent
  handwriting.

**Administrative units**
- **Congress Poland:** 1816 voivodeships → 1837 renamed gubernie → 1844 merged into 5 → 1867 ten gubernie, plus rural
  communes (gminy) → 1912 the Chełm gubernia split off.
- **Prussian partition:** Provinz → Regierungsbezirk → Kreis → Standesamt.
- **Galicia:** cyrkuł → powiat.

**Names**
- **Surname endings by gender:** -ski/-ska, -cki/-cka, -dzki/-dzka.
- **Older forms:** -owa for a wife, -ówna for a daughter (Nowakowa, Nowakówna). For surnames ending in -a: -ina/-yna
  for a wife, -anka for a daughter.
- **Maiden names:** "z domu", "z d.", "de domo", or the genitive plural "z Kowalskich".
- **Earlier and alternate names:** "primo / secundo voto" for earlier marriages; "vel / alias / vulgo" for alternate
  surnames.
- **Latin given names:** Joannes = Jan, Adalbertus = Wojciech, Laurentius = Wawrzyniec, Hyacinthus = Jacek,
  Aegidius = Idzi, Casimirus = Kazimierz, Ladislaus = Władysław, Venceslaus = Wacław, Jacobus = Jakub,
  Matthias = Maciej, Hedvigis = Jadwiga, Catharina = Katarzyna, Margaretha = Małgorzata, Agnes = Agnieszka.
- **Coats of arms (herb):** one herb is shared by many unrelated surnames ("herbu Jelita"), so it should be its own
  record, with an image and a link.

**Polish resources**
- Geneteka: 68M indexed records from 6,273 parishes.
- metryki.genealodzy.pl (register scans).
- parafie.genealodzy.pl: which registers survive and where.
- szukajwarchiwach.gov.pl: over 55M scans.
- Poznań Project: about 1.83M marriages.
- BaSIA: over 6M records.
- JRI-Poland: over 6.1M Jewish records.
- GenBaza.
- Gazetteers: Słownik geograficzny Królestwa Polskiego; GOV (about 1.2M places).

**Polish kinship terms** (for "who is this person to me")

| Relation | Term (M / F) | Note |
|---|---|---|
| parent / grandparent | ojciec / matka; dziadek / babcia (babka) | each generation up adds "pra-": pradziadek, prapradziadek |
| child / grandchild | syn / córka; wnuk / wnuczka | prawnuk, praprawnuk |
| half-sibling | brat przyrodni / siostra przyrodnia | exactly one shared parent |
| step-sibling | rodzeństwo przybrane | no shared parent; often wrongly called "przyrodni" |
| step-parent / step-child | ojczym / macocha; pasierb / pasierbica | |
| adoptive or foster | ojciec / syn przybrany (adopcyjny) | |
| father's brother / his wife | stryj / stryjenka (stryjna) | old-fashioned; today "wujek / ciocia" |
| mother's brother / his wife | wuj (wujek) / wujenka (wujna) | wujenka and wujna old-fashioned |
| parent's sister / her husband | ciotka (ciocia) / wujek | |
| brother's children | bratanek / bratanica | |
| sister's children | siostrzeniec / siostrzenica | |
| first cousins | brat / siostra stryjeczny, wujeczny, cioteczny | modern: kuzyn / kuzynka |
| grandparent's brother | dziadek stryjeczny (grandfather's side), dziadek wujeczny (grandmother's side) | |
| grandparent's sister | babcia cioteczna | |
| spouse's parents | teść / teściowa | |
| child's spouse | zięć / synowa | |
| spouse's brother; sister's husband | szwagier | |
| spouse's sister | szwagierka | |
| brother's wife | bratowa | |
| godparent / godchild | ojciec / matka chrzestna; chrześniak / chrześniaczka | |

For distant relatives, use descriptive labels ("wnuk brata", "kuzyn drugiego stopnia"). Gramps' `rel_pl.py`
implements this.

**Consistency checks** (starting from Gramps' defaults)
- mother older than 48 or younger than 17 at a birth;
- father older than 65 or younger than 18;
- spouses more than 30 years apart;
- marriage before 17 or after 50;
- a lifespan over 90;
- more than 8 years between siblings.

Add these:
- a child born after a parent's death;
- inconsistent surname spellings;
- dates that look off by 12 or 13 days (Julian vs Gregorian).

**Privacy:** treat a person as living if there is no death record and they were born less than 100–120 years ago.
Offer a private flag per person and a "hide living people" filter for exports.

**Good ideas seen elsewhere**
- RootsMagic 11: a Life Summary panel.
- Ancestry, FamilySearch, Heredis: timelines that include relatives' events and historical context.
- FamilySearch Memories: stories with photos, audio and tagged people.
- FTM "Turn Back Time": undo any of the last 5,000 changes.
- webtrees: "On this day" anniversaries.
- MacFamilyTree: a to-do assistant with 15 detectors.
- FamilySearch: a fan chart coloured by missing photos or sources, used as a research dashboard.
- Geni: the relationship path from you to anyone.
- Poster-size wall charts; face tagging; OCR; research tasks; statistics.

## 4. Technology

**Ranking**
1. **Tauri 2 (2.12.0) + React/TypeScript + PixiJS v8 (WebGL) + a Rust core.** Smallest footprint, the web's
   rich-text/PDF/table ecosystem, and a core that could be reused later.
2. **Electron + React.** Resize issues were fixed upstream (Electron 39.2.6 and 40); about 100× bigger.
3. **Flutter 3.47.** Impeller regressions on Windows, the long-open resize-jank issue #44136, and a freeze bug
   (#192537, Sep 2026). Its rich-text, @mention and PDF ecosystem is weaker.

**Other options**
- Avalonia 12, Compose Multiplatform 1.12.1 (smooth live resize is opt-in) and Qt 6.11 (C++, licensing) are credible
  but second-tier here.
- Slint, egui and iced lack rich text and PDF.

**Main Tauri risk: WebView2 resize lag**
- **Known issues:**
  - tauri#13270: content lags behind the mouse and bars appear at the edges; `transparent:true` is reported to fix it.
  - tauri#6322.
  - WebView2Feedback #2815 and #2715: avoid frameless custom title bars.
  - A Chromium fix for stale frames during resize landed around Feb 2026; whether it cures Tauri's lag is
    [unverified].
- **Mitigations:**
  - Set a background colour on both the window and the webview, and keep native window decorations.
  - **Make resize cheap:**
    - compute the layout once, in world coordinates;
    - on resize, change only the canvas size and the camera, inside requestAnimationFrame;
    - don't update React state on every resize event;
    - debounce expensive redraws until about 150 ms after the last resize;
    - draw only what is visible, with level of detail, and pack thumbnails into texture atlases.
- **Decision gate:** a 1–2 day test with 3,000 dummy cards on this PC.

**Why Lockdown lags:** CustomTkinter draws every widget itself in Python and redraws all of them on each step of a
resize. Any toolkit would have that problem without the rules above.

**UI framework:** React 19 + TypeScript + Vite, rather than Svelte 5 or Solid.
- Official TipTap bindings with a Mention extension, TanStack Table/Virtual, pdf.js wrappers, and the most AI training
  data.
- Don't use React Flow for thousands of nodes; it is DOM-based.
- Structure: UI → a thin `api.ts` → a Rust crate.

## 5. Storage and search

**SQLite**
- Use rusqlite 0.40.2 with the bundled SQLite, on a dedicated database thread. Settings: WAL, `synchronous=NORMAL`,
  one writer.
- Avoid tauri-plugin-sql: it puts raw SQL in the JavaScript side.

**FTS5 does not fold "ł" (tested on this PC)**
- With `unicode61 remove_diacritics 2`, the letters ó ź ż ć ę ś fold to plain letters but ł does not. "lodz",
  "glowna" and "wroclaw" found nothing, and SQLite's developers have no fix planned.
- **Fix:** an app-level fold function (NFD, strip combining marks, ł→l, Ł→L, plus ø, đ, ß), applied to both the indexed
  text and every query.
- Use `prefix='2 3'` for search-as-you-type. There is no Polish stemmer.

**Media in the database or as files**
- Reads are faster from the database below about 100 KB and from files above that (sqlite.org).
- For about 10 KB items on Windows, SQLite was about 5× faster than separate files.
- So: thumbnails go in a separate, rebuildable `thumbs.db`; originals and previews are files.

**Content-addressed media store**
- Files are named `media/ab/cd/<blake3>.<ext>` and never changed after writing.
- Duplicates collapse automatically, and a file can be checked against its name.
- The database maps each hash to its type, size, dimensions, codec and pixel hash.
- Reference counts, with delayed cleanup of unused files.

**Cloud sync**
- Don't keep the live database in OneDrive or Dropbox. Copying it mid-transaction corrupts it, the `-wal` file
  travels separately, and sync clients create conflicted copies. On Windows 11, Documents may itself be redirected
  into OneDrive.
- Keep the live database in `%LOCALAPPDATA%`. Put snapshots and the (never-changing) media store in the synced or
  backup folder.

**Backups**
- `VACUUM INTO` a temporary name, then rename; or use the online backup API.
- Never copy a live database file.

## 6. Media compression (measured on this PC)

**Generic compressors**

| Test set | gzip -9 | zstd -19 | xz -9e | brotli -11 |
|---|---|---|---|---|
| 26 JPEGs, 24.8 MB | 2.8% | 3.5% | 3.3% | 4.1% |
| 20 PNG screenshots, 7.6 MB | ~1% | ~1% | ~1% | ~1% |

**Format-aware options**
- **JPEG → JPEG XL, lossless transcoding:** about 20% smaller, and the original JPEG can be restored bit-for-bit.
  - Tools: libjxl 0.12.0 (cjxl/djxl, BSD); `jpegxl-rs` (needs CMake and a C++ toolchain); `jxl-oxide`, a pure-Rust
    decoder that can reconstruct the JPEG.
  - Browser support: in Chrome only behind a flag. An "Intent to Ship" (Aug 2026) names no release, and WebView2's
    status is unknown. So decode JXL in Rust for now.
- **PNG → lossless WebP:** 26–41% smaller. In a 2021 test: oxipng 12%, WebP 41%, AVIF 20%, JXL 48% (but slow). WebP
  can't exceed 16,383 px on a side.
- **Lossy AVIF/WebP:** only for previews, never for stored originals. Screenshots with text need full colour
  resolution (4:4:4).
- **PDF:** qpdf's lossless mode gains little and invalidates signatures. Ghostscript is lossy and AGPL, so don't bundle
  it.
- **Thumbnails:** the `image` crate plus `fast_image_resize` (13 ms vs 211 ms for a 4928×3279 Lanczos3 resize).

**Verdict on "compress everything, unpack to a temp file on open"**
- It gains only 1–4%.
- It adds a delay to every open, temp files leak if the app crashes, antivirus rescans every temp copy, and "delete on
  close" is unreliable for external programs.
- **Instead:** format-aware lossless conversion, and in-memory decoding served to the UI through a custom URL protocol.
  Temp copies are made only for "Open in external app", in a per-session folder cleared at startup.

**Pipeline**
1. **On import:** hash the original (BLAKE3), deduplicate, detect the type and read EXIF.
2. **Stored originals:**
   - JPEG: kept as-is in v1. Offer an optional "Compact archive" that converts to JXL, checking that the reconstructed
     JPEG matches the original before deleting it.
   - PNG, BMP, TIFF: lossless WebP when both sides are ≤16,383 px, otherwise JXL, after checking the pixel hash.
   - PDF: kept as-is.
3. **Derived files, rebuildable at any time:**
   - a 256 px WebP thumbnail (in `thumbs.db`);
   - a 2048 px WebP preview;
   - for PDFs, a first-page thumbnail and the extracted text for search (PDFium in Rust).
4. **Viewing PDFs:** pdf.js in the UI, plus an "Open in default app" fallback.

## 7. Tree engine

**Two layout families**
- **"Everyone" view:** a Sugiyama layered layout adapted to genealogy.
  1. Model the graph as person → union → child.
  2. Assign generations with two DFS passes (Mařík 2016), keeping spouses on the same row.
  3. Treat couple chains as blocks.
  4. Run constrained barycenter sweeps: siblings stay together in order, user orders are hard constraints, and the
     previous order is the starting point.
  5. Place x-coordinates with Brandes–Köpf, then centre each union over its children.
  6. Apply branch offsets and pins, then remove overlaps within each row.
  7. Route "bus" lines.
  - Pedigree collapse is drawn as one node with a long edge.
  - Relayout only when the data or the layout intent changes; cache the result.
  - Target: under 200 ms at 10k people (to be benchmarked).
- **Focus views** (family, pedigree, descendants, hourglass, bowtie):
  - Unfold the graph into a tree from the focus person and use a tidy-tree layout (van der Ploeg / d3-flextree).
  - Mark people who appear twice with ↻, and draw stubs for relatives not shown.
  - Fan chart = polar geometry; timeline = interval packing; relationship path = bidirectional BFS.
- **Build our own layout code.** No open-source library combines generation rows, couples, sibling order, pins and
  stable relayout at 10k people. yFiles is commercial and OGDF is GPL. Use elkjs or Graphviz-WASM only as prototypes
  and baselines.

**Library facts**
- **family-chart:** focus views only, O(n²); slows above about 1k people and crashes at 4k (#62).
- **relatives-tree:** a bug with remarried parents (#24).
- **Topola:** Apache-2.0, SVG, active (3.10.4).
- **dTree:** dead.
- **ELK:** EPL; 3k nodes take 11 s.
- **dagre v3:** stable relayout (`useDynamic`).
- **d3-dag:** its optimal layouts blow up (900 nodes took about 5 minutes).
- **Graphviz-WASM:** practical up to about 1k people, which is Gramps Web's limit.

**Rendering**
- SVG and Canvas top out at about 10k graphical elements, i.e. about 1k full cards. WebGL (PixiJS) held 50 FPS at 400k
  nodes without text (Horak 2018).
- **PixiJS v8 rules:**
  - at most 16 textures per batch;
  - culling is off by default, so do our own;
  - no per-card masks or filters;
  - DynamicBitmapFont handles Polish diacritics;
  - HTMLText only for a handful of labels.
- **Spatial index:** flatbush, for culling and hit-testing; rebuild it after each layout.
- **Levels of detail**, by the card's width on screen, with 15% hysteresis:

  | Level | Width | What is drawn |
  |---|---|---|
  | L0 | under 10 px | dots |
  | L1 | 10–40 px | box + colour strip (surname from 28 px) |
  | L2 | 40–120 px | + given name, years |
  | L3 | 120 px and up | + photo, maiden name, badges |

- **Photos:** 96 px and 192 px thumbnails, decoded in a worker into 2048² atlases with least-recently-used eviction.
  Only L3 cards request photos.
- **DOM overlay** for the selected card, inline editors and menus.

**Interaction**
- Zoom at the cursor, pan, fit to screen, minimap.
- Keyboard: ↑ parents, ↓ children, ←/→ siblings and spouses; back/forward history.
- Refocus animation of 300–500 ms that keeps the clicked card fixed on screen.
- Diacritic-insensitive search that flies to the person.
- Hover highlights direct lines; a path to the home person.
- Colour modes: grandparent lines, side, surname, generation, completeness, birthplace.

**Manual arrangement, stored as intent rather than coordinates**
- Dragging within a row reorders people (an order constraint).
- Alt-dragging moves a whole branch; the offset survives relayout and has a "reset branch" action.
- Pins.
- Generation override through a menu.
- A separate frozen "Poster" mode for free-form print layouts.
- Precedents: Family Historian (moves push branches outward), GenoPro (everything manual), RootsMagic 7's chart editor
  (removed in 8, and users complained).

**Card and line design**
- **Cards:** about 200×72 px with a 56 px photo; given names in semibold, then SURNAME, then the maiden name muted,
  then the years with †.
  - a 4 px branch-colour strip; gender shown as a subtle accent rather than a fill;
  - badges: ↻ for duplicates, +N for hidden relatives.
- **Lines:**
  - orthogonal "family bus" connectors with rounded corners;
  - dashed for adoptive/step, dotted for uncertain;
  - a union dot showing the marriage year;
  - soft curves for cross-links.
- **Inspiration:** MacFamilyTree, MyHeritage (vertical cards, branch colours) and FamilySearch (fan-chart colour
  modes).

**Papers:**
- McGuffin & Balakrishnan 2005;
- GeneaQuilts 2010;
- TimeNets 2010;
- Mařík 2016 (https://ceur-ws.org/Vol-1649/218.pdf);
- Racine 2025 (TU Wien, large genealogy trees).

## 8. AI-assisted import (external AI chat → file → app)

**Format: strict JSON**
- A truncated JSON file fails loudly instead of losing data silently.
- Not YAML: it truncates silently, and implicit typing turns "NO", dates and "0123" into non-strings.
- Not Markdown: it produces subtle, hard-to-detect errors.
- Not GEDCOM: models invent data, and there's no room for our fields.

**Schema**
- **Structure:** flat top-level arrays `persons, events, relationships, texts, links, media, sources, coverage,
  questions`.
- **IDs:** local, with a type prefix (P/E/T/S/Q). M-IDs are assigned by the app.
- **Language:** keys and enum values in English, narrative text in Polish.
- **Required fields:** only `id` plus one name. Unknown fields are omitted, never written as "unknown".
- **Original vs normalized:** `orig` + `lang` sit next to each normalized value.
- **Per fact:** `src`, `basis: stated|inferred` and `certainty`.
- **For the reviewer:** `questions[]` for the person reviewing, and `coverage[]` recording each input item as
  extracted, partial or unreadable.
- **Quotes:** use Polish „…” quotes inside text, so no escaping is needed.

**Rules for the AI**
- Use only this chat's material. Never invent names, dates, places, URLs or record numbers.
- Don't compute birth years from ages: copy the age, and the app derives the estimated date.
- Transcribe as written (in `orig`) and never modernize silently. Mark doubtful letters with [?] and illegible parts
  with [...].
- Don't merge people unless the source says they're the same.
- Keep both dates for double-dated records.
- Never abbreviate. Stop at a record boundary and ask the user to type "dalej".
- Message 1 is the transcription, which a person checks. Message 2 is the file.

**Big inputs**
- One chat per family branch, about 5–15 records.
- The output comes in parts of at most 15 persons. Part 1 carries a manifest of all planned IDs.
- Optional: a roster of existing people, with handles `@` + 4 base32 characters + a check character.
- Keep the instructions in a ChatGPT Project, Claude Project or Gemini Gem; all three are available on free tiers.

**Reading old records: the reality**
- **English (best case):** Gemini 3 Pro reaches about 1.7% character error rate on 18th–19th-century manuscripts.
- **Other languages are much worse.** In the Transkribus benchmark, the best LLM had 34.7% CER on German, against 8.6%
  for a specialist model.
- **Polish forum reports:** standard formulas are read fine, but surnames, occupations and places go wrong, and one
  model invented a spouse.
- **Cyrillic:** a marriage record was read as a birth, and a baptism as a police protocol.
- **Printed index screenshots (Geneteka)** are fine. Pasting the copied table text is even better.
- **Tips:** crop to one entry and keep it upright. State the parish, years, language and record type. Ask for a
  diplomatic transcription, then a translation, then extraction. For Kurrent script, run Transkribus first.

**Delivery**
- The model writes one fenced ```json block per part. The user presses Copy and pastes into the app's "Paste AI
  answer" box, which extracts every fenced block.
- Downloads are optional: ChatGPT links expire; Claude has file creation on all plans since Feb 2026; Gemini's file
  export doesn't list JSON.

**Truncation**
- The exact end marker must be present. If it's missing, the part is truncated, and it must never be auto-closed.
- Only then allow cosmetic repair (jsonrepair / llm_json).
- Check the manifest across parts. A count mismatch is only a warning; a missing part blocks the import.

**Media**
- Chats don't see filenames. So the app assigns M1…Mn to the inputs and makes stamped upload copies with the ID printed
  in the margin.
- The prompt lists the manifest and the AI references M-IDs. Originals stay in the app.
- Remind users about training opt-outs before they upload family photos.

**Matching incoming people to existing ones**
- **Matching keys:**
  - NFC, casefold, then strip diacritics with an explicit ł→l.
  - Map surname gender and marital forms to base candidates.
  - Recognise maiden-name markers: z domu, z d., de domo, geb., урожд., z Kowalskich.
  - Map Latin, German and Russian given names to Polish canonical forms, including Latin case endings.
  - Transliterate Russian with a Polish table: Щ→szcz, Ж→ż, Ш→sz.
- **Phonetic matching:**
  - Beider–Morse, with Polish, Russian and German rules; fewer false positives.
  - Daitch–Mokotoff, for recall.
  - Rust: `rphonetic` (Apache-2.0); load its Polish, Russian and German rule files.
- **Scoring:**

  | Signal | Points |
  |---|---|
  | Given name: equivalent / variant / different | +3 / +1.5 / −3 |
  | Surname: exact / phonetic | +2 / +1 |
  | Birth year difference: 0 / ≤2 / ≤5 / >5 / >10 | +2 / +1 / 0 / −2 / veto |
  | Same village / same parish | +1.5 / +1 |
  | Father's given name: match / mismatch | +2 / −2 |
  | Mother's given + maiden name: match / mismatch | +2.5 / −2 |
  | Spouse: match / mismatch | +2.5 / −1 |

  - Vetoes: a sex conflict, an ancestor/descendant pair, impossible lifespans.
  - Downweight agreement on common names like "Jan Kowalski".
  - Thresholds: ≥8 suggested match (still confirmed by a click), 4–8 possible, <4 new. Always show the point
    breakdown.

**Review screen**
- Buckets: matched / new / needs review. Withhold "Add" until every suggested match is confirmed or rejected.
- Side-by-side comparison, with rows for relatives.
- Per field: keep / replace / add as alternate. Default to alternate and never overwrite.
- Bulk actions.
- An issues panel: unresolved references, missing media, the AI's questions, low-certainty facts.
- The evidence next to each fact.
- A mini tree preview with new people drawn dashed.
- A media grid where a star marks the profile photo.
- **Undo:** a batch ID on every row, a snapshot before saving, and "Revert batch".

**Mentions inside text**
- Written as `[inflected text](person:ID)`: a plain Markdown link that survives AI round trips and allows Polish
  inflection ("z [Janem](person:P12)").
- TipTap Mention with custom Markdown rendering and parsing.
- Canonical storage is a deterministic Markdown subset, plus a derived mentions table for "mentioned in" backlinks.

**Recommended file skeleton** (the name and details are to be finalized in the import-format spec)
```json
{"format":"heirloom-import","version":"1.0","prompt_rev":"2026-09a","batch":"B7Q2","part":1,"lang":"pl",
 "manifest":["P1","P2"],
 "persons":[{"id":"P1","sex":"M","names":[{"type":"birth","given":"Jan","surname":"Kowalski","orig":"Янъ Ковальскій","lang":"ru","src":["S1"]}]},
            {"id":"P2","sex":"M","names":[{"type":"birth","given":"Wojciech","surname":"Kowalski","src":["S1"]}]}],
 "events":[{"id":"E1","type":"baptism","date":"1873-03-27","date_orig":"15/27 марта 1873","julian":"1873-03-15",
            "place":"Wiskitki","people":[{"p":"P1","role":"principal"},{"p":"P2","role":"father"}],
            "src":["S1"],"basis":"stated","certainty":"high"}],
 "relationships":[{"type":"parent","parent":"P2","child":"P1","src":["S1"],"basis":"stated"}],
 "texts":[{"id":"T1","person":"P1","kind":"bio","title":"Dzieciństwo","md":"Syn [Wojciecha](person:P2), ochrzczony w Wiskitkach.","src":["M4"]}],
 "media":[{"id":"M2","depicts":["P1"],"profile_for":["P1"],"caption":"Portret, ok. 1900"}],
 "sources":[{"id":"S1","kind":"parish_record","media":["M1"],"parish":"Wiskitki","year":1873,"akt":"45"}],
 "coverage":[{"m":"M1","status":"extracted"},{"m":"M3","status":"unreadable"}],
 "questions":[{"id":"Q1","about":["P2"],"text":"Czy Wojciech z aktu 45/1873 to ten sam Wojciech co w notatce?"}],
 "end":{"part":1,"final":true,"counts":{"persons":2,"events":1},"marker":"END-HEIRLOOM-IMPORT"}}
```

## 9. Reusable parts and licences

**Licences**
- **Permissive,** usable in any app: ged_io (MIT), @treeviz/gedcom-parser (MIT), Topola (Apache-2.0), family-chart
  (MIT), d3-dag (MIT), flatbush, PixiJS (MIT), TipTap (MIT), pdf.js (Apache-2.0), PDFium, rphonetic (Apache-2.0).
- **Copyleft:**
  - Gramps and its add-ons: GPL-2.0-or-later.
  - webtrees, Ancestris, Family Gem: GPL-3.0.
  - Gramps Web and gedcomkit: AGPL-3.0.
  - Copying or porting their code puts our app under the same licence. File formats and behaviour can be
    reimplemented freely.
- **If Heirloom is licensed GPL-3.0,** we may port Gramps' logic: the Polish date handling (`_date_pl.py`), Polish
  relationship names (`rel_pl.py`), GEDCOM quirk handling and the duplicate finder.

**AI-genealogy projects worth learning from**
- **Gramps Web's assistant:** tool calling instead of dumping the whole tree into the prompt.
- **MCP servers:** gramps-mcp, GedcomMCP, wikitree-mcp.
- **strom-research** (MPL-2.0): every fact cites the folio, entry and quoted words; unproven facts stay "leads";
  every change is a git commit.
- **ArchiveVision-OCR:** produces both a diplomatic and a normalized transcription.
- **Handwriting tools:** Transkribus (credits; API since June 2026), Kraken/eScriptorium, PyLaia, Loghi.

**Phone (later, not a priority):** Family Gem (GPL-3.0, v1.3, Polish UI, GEDCOM 5.5.1 only) could open a GEDCOM
5.5.1 export with relative `media/` paths.
