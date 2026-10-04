# Changelog

Format: `X.Y.Z — YYYY-MM-DD HH:MM: description`. Newest on top.
X = major change, Y = feature / bigger change, Z = small change.

## 0.5.0 — 2026-10-04 16:41: „Otwórz razem…” — several archives as one tree, „To ta sama osoba?”, new windows
- **„Otwórz razem…”** (in the archive picker and in the sidebar's archive menu): pick two or more archives and a
  name; Heirloom saves a **set file** next to them (`Rodzina razem.heirloom-zestaw`) and shows them as one: one tree
  (all views, Całe drzewo too), one Osoby list, Nazwiska, Miejsca, Ctrl K, Start's numbers and the profiles. The
  archives' files are only read, never changed. The set file holds the list of archives
  (paths relative to it, so moving the whole family folder keeps working) and who is the same person; opening it
  opens the view again, and the recent list shows sets with „zestaw · 2 archiwa”.
- **Where each person comes from:** every archive has its colour; a small dot on every avatar (Osoby, the photo
  grid, Ctrl K, Start, Miejsca, the profile's Rodzina, the tree's panel) and in the corner of the tree cards, a badge
  with the archive's name on the profile and in the panel. A person in two archives gets both colours. The tree has
  „Koloruj wg: archiwum” with its key (people in several archives in green).
- **„To ta sama osoba?” („Do sprawdzenia” in the sidebar, with the count):** Heirloom looks for people of different
  archives who are probably one person (the Import's matching: names, dates, places, parents, partners) and shows
  one pair at a time, side by side with parents, partners and children (names on both sides marked), the percent
  and the reasons. „Tak, to ta sama osoba” (T) makes them one person in this view and the two trees join there
  (often the next pairs, e.g. the spouse, become near-certain); „Nie” (N) is remembered and not asked again;
  „Pomiń” (→) leaves it for later. „Decyduje:” names who answered, saved with the link. „Połącz wszystkie…” links
  every pair from 90% at once (two copies of the same family: 413 pairs in one click). Links whose people are gone
  from an archive are listed under „Nie znaleziono”.
- **On a profile in the combined view:** „Ta osoba jest w 2 archiwach” with each archive's record and „Różnice”
  (birth or death given differently), „Rozłącz…”, and „Połącz z osobą z innego archiwum…” (search limited to the
  other archives, then the same side-by-side check).
- **Browse-only (decision 1a):** the combined view can't be edited — the Edycja switch is locked with a short
  explanation, a bar under the top bar says so, Import, „Historia zmian” and the archives' settings are not offered.
  **„Edytuj w jego archiwum”** (profile and tree panel; a menu for a person in several archives) opens that person
  in their own archive in a new window; coming back to the combined window reads the saved archive again
  („Odświeżono: Kowalscy (zmiany z innego okna).”).
- **Start of a set:** „W tym zestawie” lists the archives with their colours, people and folders; an archive that
  isn't found shows „Nie znaleziono…” here and in a bar, with „Wskaż folder…”; „Dodaj archiwum…”, „Usuń z zestawu”
  (the archive stays on disk) and the way to „Do sprawdzenia”.
- **„Otwórz w nowym oknie”:** every row of the recent list (archives and sets) and every archive of a set can open
  in a window of its own (another Heirloom process). Windows no longer erase each other's recent list, last places
  or settings: `aplikacja.json` is read again before every change.
- **Fixes on the way:** a child listed in two of a person's families is drawn once in the tree; the hint over the
  locked Edycja switch is no longer one cut-off line.

## 0.4.2 — 2026-10-04 15:29: Maiden names everywhere, long names fit on the tree cards, relatives changed and unlinked in place
- **Maiden names:** a woman who took her husband's surname now shows „z d. …” wherever her name stands on its own:
  under the name in the tree's side panel and on her profile (moved up from the end of the dates line), after the
  name of every relative in the side panel (before only the mother and the wife, and without the married surname),
  in the Nazwiska lists and in the Osoby photo grid. Tree cards keep their „z d.” line as before; Ctrl K, the
  Osoby list, the mention cards and the profile's Rodzina tiles already had it.
- **Tree:**
  - a name too long for its card gets a smaller font, down to 13 px on one line, then onto two lines (given names,
    then the surname) down to 11.5 px; only a name still too long is cut with „…”, and then the whole name shows
    on hover. The cards keep their size, so nothing moves. The same in the narrow sibling cards; in Całe drzewo a
    long name shrinks to 12 px at most and is then cut;
  - the capitals' accents (Ó, Ń, Ś) at the top of a card name are no longer cut off („KROL” → „KRÓL”);
  - „Osoba w centrum” over the centre person is gone: the highlighted card says it.
- **Relatives: change, unlink or delete them where you see them.** Before, only adding a relative was obvious;
  removing one was in the profile's Rodzina only after „Edytuj sekcję”, and making a child a sibling instead wasn't
  possible at all. Now, in edit mode:
  - in Rodzina every relative's card has a small strip hanging under it (in Przodkowie and Potomkowie the chosen
    card's): „Edytuj osobę”, „Zmień pokrewieństwo” (what they are to the person: ojciec / matka, brat / siostra,
    mąż / żona, syn / córka; for a parent and child also biologiczne, adopcja, przybrane, nieznane), „Odłącz” and
    „Usuń osobę z archiwum…”. It hangs from the card's bottom-left corner, so the lines stay free, fits the narrow
    sibling cards, and in the last column of Przodkowie it sits beside the card; the menu stays on screen;
  - the profile's Rodzina has the same tools on every tile, with the kind as a chip („córka · adoptowana”); using
    one opens the section, so its „Anuluj” takes the change back;
  - „Odłącz” asks first, with a sentence that says exactly what changes („Weronika Michalak przestanie być
    dzieckiem: Elżbieta Michalak i Michał Michalak.”, „… nie będą już parą. Ich dzieci (…) zostaną dziećmi tylko:
    …”); the person stays in the archive. Deleting the person asks separately and says it is not an unlink;
  - every change is one step of Ctrl Z (and „Cofnij” in the message); someone made their own ancestor (a grandson
    as his grandfather's father) is refused with a plain message, and no family is left with fewer than two people.
- **Review fixes (before release):**
  - a parent added to a family that already has children (the usual „Dodaj krewnego › Rodzic” or „Zmień
    pokrewieństwo › ojciec”) skipped the check for a circle: a brother's daughter could become the mother of her
    own father. Now refused like the rest; a new test tries every change and unlink between any two people of a
    small family and checks the families stay tidy and Ctrl Z brings back exactly what was there;
  - „Cofnij” in an older message (after another change, or after Ctrl Z) took back the newest change instead of
    its own; now it says the message is out of date and to use „Cofnij” in the bar;
  - a change from the tree's strip first finishes a profile section still open, so that section's „Anuluj” can't
    take the tree's change back later.
- **Fix: removing a parent took the person away from both parents.** Unlinking a parent now takes only that parent
  out of the family (and so from its other children too, as the question says); the other parent stays.

## 0.4.1 — 2026-10-03 19:57: The installer with a running Heirloom, siblings always shown
- **Tree:**
  - Rodzina always shows the brothers and sisters as cards, in birth order: older ones left of the person, younger
    ones right of the partners; half-siblings at the outer end on the shared parent's side, with their line from
    that parent and „brat przyrodni” / „siostra przyrodnia”; from 9 siblings on the cards are narrower. „+N
    rodzeństwa” and „Przywróć automatyczny układ” are gone. A second partner's parents' line no longer runs over
    the siblings, with three or more partners their parents' lines no longer share a stretch, and a partner's
    „Dodaj rodziców” moves aside instead of covering the person's own parents;
  - the zoom buttons in Rodzina, Przodkowie and Potomkowie are a row at the bottom right, as high as the line legend
    and the same as in Całe drzewo (which gets „Dopasuj do ekranu” too); the legend drops whole items when the window
    is narrow.
- **Fix: Heirloom froze when the installer was run by hand while it was open (0.4.0).** The installer closed it with
  `taskkill` without /F, which sends „close” to every window of the program, also to the invisible one its event
  loop runs through; that window was destroyed, so the close question could be answered but the window never closed
  (nor with ✕), and the installer gave up after 15 s. Now:
  - Heirloom keeps a small file for the installer (`%LOCALAPPDATA%\Heirloom\running\<pid>.json`) saying whether
    it has unsaved changes or an open section's draft, removed when it exits (one left by a crash is recognised);
  - the installer checks it: with nothing unsaved, OK closes Heirloom the way its ✕ does; with unsaved work it says
    so („W archiwum „…” są 3 niezapisane zmiany”) and offers „Czekaj” (Heirloom comes to the front; once the changes
    are saved or Heirloom is closed, the installer closes it and goes on by itself), „Zamknij mimo to” (asked once
    more, then closed by force) and „Anuluj instalację”; an older Heirloom that says nothing is asked to close and
    asks about its changes itself;
  - the installer never sends „close” to that invisible window, and Heirloom now ignores it there anyway;
  - no program file is moved or replaced while heirloom.exe runs (checked once more right before copying); an update
    from inside the app that finds Heirloom still open after 15 s shows the same page instead of failing.

## 0.4.0 — 2026-10-03 15:52: Fixes from the first test, update checks, a public repository
- **Tree:**
  - „Koloruj wg” works in every view and every mode, and a change shows at once (also in Całe drzewo, which
    re-tints instead of rebuilding); „nazwisko” follows the surname a person bears, „strona” counts from the person
    in the centre, with its own colour key;
  - one click opens the side panel in every view, Całe drzewo included (the person gets a ring there, the click
    finds the nearest person, only the left button selects, Esc closes the panel);
  - Back, Forward and Alt ← return to the same view, person, zoom and position; coming back to the tree no
    longer loads it again;
  - Całe drzewo's surname labels stay in place while zooming, move aside instead of disappearing, and hide when
    their family is off screen;
  - the toolbar loses „Żyjący / Zmarli / Wszyscy”, and „Filtry” becomes „Karty: ze zdjęciem / inicjały”.
- **„Po otwarciu archiwum: Start · Ostatnie miejsce”** (Ustawienia › Archiwum) replaces „Osoba startowa” and the
  first-open question „Od kogo zacząć drzewo?”; Home in the tree goes to the suggested person.
- **Ustawienia:** the rows that can't be changed yet are all marked „wkrótce”; the „Rozmiar tekstu” box fits 150%.
- **Import:**
  - „Wyczyść listę”, a click anywhere on the drop box opens the file picker, the toast after an import is no
    longer squeezed, the „transkrypcja” chip in step 4 shows the file's transcription;
  - two related people of a package can't be joined with one archive person (refused already in step 3);
  - a failed import removes the files it copied; `zrodla-ai/` is written only after a successful save;
  - fixes: a dropped file that is gone, the paste view hiding an error, file counts, undone imports told apart,
    system files (Thumbs.db, desktop.ini…) skipped, clearer format/version errors, a repeated part reported, one
    warning for all files without a number.
- **Updates (APP-STANDARDS §2–§3):** Heirloom stays fully offline unless you ask: „Sprawdź teraz” in Ustawienia ›
  O programie asks GitHub for the newest version, and „Sprawdzaj aktualizacje” (off by default) does it by itself at
  start and when the window comes back, at most every 5 minutes, with a banner. The new installer is downloaded and
  run by the app itself after asking to save (`--update`: progress only, no administrator rights), and it starts
  Heirloom again. It sends nothing else.
- **The installer's window (APP-STANDARDS §5)** in Heirloom's look (cream, deep green, its logo, serif headings):
  „Zainstaluj X” / „Aktualizuj A → X”, „Heirloom jest uruchomiony” (OK closes it the way its own ✕ does, so it asks
  about unsaved changes), a bar driven by the real steps with „Pokaż szczegóły”, a ticked „Uruchom Heirloom”.
  `--update` shows only the progress (waiting up to 15 s for Heirloom to close), starts Heirloom again and closes by
  itself. A failure puts the replaced files back and offers „Spróbuj ponownie” / „Zamknij” (the old version starts
  again on close); uninstalling uses the same window. The work runs off the window's thread; sharp at 125/150 %.
- **Backups** are named in local time.
- **Development:** the repository is public; the design files, the designer's documents and the AI-trial test
  packages stay on the developer's computer (the AI-trial tests skip without them). The Windows installer is built
  on GitHub (`.github/workflows/windows-build.yml`). New Rust dependency: `ureq` (for the update check).

## 0.3.3 — 2026-09-29 04:09: An import in progress survives „Zapisz jako nowe archiwum”
- **Fix (from the backend review, the one finding still open):** „Zapisz import” saves earlier edits first, and
  in a file from another program that first save can become a new archive — which threw the import away (steps
  2–5 lost, and the Import screen started again from step 1). The new archive holds the same family with the same
  record ids, so the import now carries over, in the backend and on the Import screen.
- **Checked:** every finding of the three reviews (backend 8, interface 13, installer 8) against the code; all are
  fixed, except two left on purpose: copying files a GEDCOM points to anywhere on the disk (needs a rule, see
  TODO) and stale files in the program folder on update (harmless while the program is one file).
- **Tests:** 160 Rust tests (new: the import carried over, a file that can't be copied keeps its link, a save
  whose history couldn't be written offers no „Cofnij zapis”), 19 interface tests.

## 0.3.2 — 2026-09-29 03:45: The first person of a new archive counts as one change
- **Fix:** the first edit of an archive also declares Heirloom's extension tags in the file's header, and that was
  counted as a change of its own: one new person showed „2 niezapisane zmiany” (in the edit bar and in the question
  when closing), and the editor got 2 changes in Ustawienia › Osoby edytujące. The header is bookkeeping, as in the
  history list, which already left it out. Found by testing the close question on the release build.
- **Development:** the Vite dev server no longer watches `build\` (the build's self-test locked `heirloom.exe`
  there and the dev server crashed); `scripts\build.ps1` tries packing the program again for a few seconds when
  `heirloom.exe` is still held just after the self-test (a build failed on that once).
- **Tests:** 157 Rust tests; the close question checked on the release exe (a typed draft, unsaved changes,
  „Anuluj”, „Zamknij bez zapisu”, „Zapisz i zamknij”, closing with nothing changed).

## 0.3.1 — 2026-09-29 03:30: Review round — fixes from three reviews (backend, interface, installer)
- **Family data safety:**
  - „Cofnij zapis” finds the save by the time and author its history entries share, never by their position (an
    unreadable history or another computer could shift that and undo the wrong records); and every undo from the
    history (a save, one change, a whole import) now leaves alone a record changed since — a later edit or import is
    never overwritten; the toast says how many were left;
  - closing the window with unsaved changes asks: „Zapisz i zamknij”, „Zamknij bez zapisu”, „Anuluj” (before, they
    were lost without a word);
  - a text or link being typed in a profile section is no longer dropped by „Gotowe”, „Anuluj”, leaving the
    profile or switching the archive without asking; an edit outside the open section (a photo from the hero,
    „Cofnij” in the history, „Usuń link”, „Cofnij zapis”) finishes the section first, so its „Anuluj” can't undo
    it; after „Cofnij” the section's „Anuluj” never reaches further back; after an import or a reload „Cofnij zapis”
    no longer offers an older save;
  - „Zapisz jako nowe archiwum” copies the files first and changes a link only for a file that arrived (a failed
    copy keeps pointing at the file where it is), checks the free space, never lets a file named like the data file
    or `.heirloom` overwrite them, allows a folder next to the downloaded file, and names the archive after the
    chosen folder;
  - an emptied event also loses the import's `_HLM_BASIS` mark (it was left as an empty „died” event).
- **Fixes:** a new person with only a given name couldn't be created (and every new person asked „Odrzucić
  zmiany?” when left untouched); Esc in a menu or a search inside an open section closed the whole section; the
  „edit” link from the tree came back with Back; Alt ← → changed a switch as well as the screen; Ctrl Enter in a
  place field picked a place instead of „Gotowe”; the edit bar pushed „Zapisz” out of a 1100 px window; search
  labels for hyphenated surnames and maiden names; a switch no longer navigates from under a dialog.
- **Import step 1:** a file dropped on its own and inside its folder is one file; the first dropped file stays and
  the second copy is left out; two answers with the same file name keep their own rows; answer files stay files
  when the missing part is added; a kind chosen by hand survives a reload; re-dropping a folder brings back files
  taken off inside it; the list says when the package has errors; duplicates are found by hashing each file once
  (it re-read every earlier file of the same size).
- **Installer:** uninstalling now really removes the program folder (the command's quoting was broken — the
  Reckless Driving installer has the same bug, flagged separately), and only the program's two files and an empty
  folder; a running Heirloom is never killed (it may hold unsaved changes): the installer asks to save and close
  it; the uninstall box no longer touches `%LOCALAPPDATA%\Heirloom\archives` (an archive in a read-only folder keeps
  its history and backups there); the Recycle Bin is used through Windows itself, which asks before deleting
  anything for good; folders come from Windows (a desktop on OneDrive, an apostrophe in a user name); shortcuts
  that fail are reported; an older version over a newer one is confirmed; the build stops on any failed file step
  and checks it packages the exe it just built; the self-test can't pass without the bundled fonts, and a normal
  start never depends on it.
- **Tests:** 156 Rust tests and 19 interface tests pass; the installer steps pass in a sandbox.

## 0.3.0 — 2026-09-29 02:45: Design v2 (round 1 of the designer's fixes), broken links, the AI trial
- **Design v2** (`design/`, page 17 „Poprawki P1”; the previous pages are in the Recycle Bin and in the first zip):
  - **editing in place (A7, A2):** the separate edit form is gone. In edit mode each profile section has „Edytuj
    sekcję”; one section is open at a time, framed, with „Anuluj” (Esc) and „Gotowe” (Ctrl Enter). „Dane osobowe”
    is the hero itself: names and other forms, nickname, sex, whether the person lives, occupation, birth, baptism,
    death and burial with the place and a certainty (pewne / prawdopodobne / niepewne), religion with „Brak danych
    w źródłach”, tags, the photo and „Usuń osobę…”. The family section edits the kind of each relation, removes one
    and adds relatives (someone from the archive or a new person); a new person is the same section, open and empty;
  - **one edit bar on every screen (17a):** who edits, the file and folder, the unsaved changes with „Cofnij”,
    „Anuluj” (drops them) and „Zapisz”; after saving „Zapisano o …”, „Cofnij zapis” (the whole save comes back as
    unsaved changes) and „Zakończ edycję”. The save chips in the top bar are gone;
  - **no edit buttons while browsing (A6):** the tree's side panel, „Dodaj rodziców”, the profile; the tree: a
    click chooses a person and opens the panel, a double click puts them in the middle (H1);
  - **cards (A9):** „Józef KOWALSKI”, „1878 † 1951”, „1931 · żyje” on the tree cards (all views, Całe drzewo too),
    the profile's family tiles, the search results and the person pickers;
  - **the first save into another program's file (A3, 17b):** the new dialog, with „Zapisz jako nowe archiwum”: the
    current state becomes a GEDCOM 7 archive in an empty folder, with its files copied in; the original stays as it
    was;
  - **first launch (A1, A4, 17c):** „Utwórz archiwum z plików” is the filled main action; „Utwórz puste archiwum” is
    gone (an archive filled by hand starts the same way); the empty archive puts „Wczytaj pliki rodziny” first; the
    read-only bar and its tooltip with the new text and a „Wyłącz” button;
  - **Import step 1 (A5, 17d):** after a drop the list of recognised files replaces the paste box: the file, its
    kind (changeable: photo, document, note), the part of the answer with its counts or whom the file shows, its
    state (Rozpoznano / Do wskazania / Pominięty) and „usuń”; a second copy of the same file is skipped; the answer's
    text can still be pasted with Ctrl V;
  - **search (B1, 17e):** grouped results (Osoby, Miejsca, Polecenia), the matched part marked, a label saying why a
    result matches (nazwisko panieńskie, forma męska/żeńska, przydomek), the number of results and the time, Ctrl
    Enter shows the person in the tree; no results: similar surnames and a search in the stories. A surname now also
    finds its other gender's form (Wiśniewska ↔ Wiśniewski). „Skocz do osoby…” in the tree is the same window for
    people;
  - **toasts (B2, 17f):** bottom right, at most 3, 6 s (10 s with an action), paused while pointed at or focused,
    inverted in the dark theme, read out by screen readers; the import's toast has „Cofnij import” and „Pokaż w
    drzewie”;
  - **keyboard focus (B3, 17g):** one 2 px accent ring, inside rows and menus; switches change with ← →; the tree:
    ↑ a parent (↑ again: the other parent), ↓ the first child, ← → the row, Enter the panel, Shift Enter the profile,
    Home the start person, Esc closes the panel, and each step is read out;
  - **contrast (D1–D3, D5, D6, D8, G1):** new `--line`, `--text3`, `--warn`, `--card-border`, link colours, a
    stronger popover border in dark, and the letters on the smallest cards in a readable ink per theme; fields and
    outline buttons use `--line` (D7);
  - the tree toolbar never breaks a control in two lines (the screenshot you sent the designer).
- **Links that lead nowhere** (your answer to Q5): a profile lists links to records that are gone (after undoing an
  import, or in a file from another program) with „ten link prowadzi donikąd”, and in edit mode „Usuń link”; a
  mention of someone who is gone is shown dimmed with the same explanation.
- **The AI trial, simulated** (your answer to Q2): fictional packages written the way a chat AI answers the
  instructions (`crates/heirloom-api/tests/fixtures/ai-trial`), a careful one, a sloppy one and a later one. They
  found and fixed: a part sent again didn't clear the „cut off” error, `"part": "2"` and other slips gave a
  misleading message (now read leniently), and a later package's parents were only „to review” (relatives of a
  clear match now get points from the family).
- **The official GEDCOM 7 test files** (`scripts/fetch-gedcom-test-files.ps1`): all 22 open on every screen and come
  back byte for byte; saving an unchanged profile changes nothing (two fixes: an unknown sex was written as `SEX U`,
  and the header was rewritten without edits).
- **The Windows installer** (APP-STANDARDS §1): `scripts/build.ps1` checks that Cargo.toml, package.json and
  tauri.conf.json agree, builds the app, self-tests the packaged exe (`heirloom.exe --selftest`: a hidden window checks
  the interface, the bundled fonts and an archive round trip, with throwaway settings) and makes
  `build/HeirloomSetup-<version>.exe`: one exe that installs, updates („Aktualizuj”) and uninstalls, per user, without
  administrator rights. It never touches the family archives; on uninstall the program's own settings and thumbnails
  go to the Recycle Bin only if the box is ticked. The window now starts hidden and shows itself once ready. Update
  checks (§2–§3) are not built: they wait for your decision.
- **Housekeeping:** `design/_ds/` (another company's design system) is in the Recycle Bin (J1); the designer's copy
  of DESIGNER_ANSWERS is the current one (J2).
- **Tests:** 154 Rust tests and 19 interface tests pass.

## 0.2.1 — 2026-09-28 23:23: Review round — fixes from three reviews (Rust, interface, design)
- **Family data safety:**
  - the edit form sends only what changed, and the backend's edit round trip is lossless: saving an unchanged form
    no longer drops name suffixes, doubles other names, loses TYPE phrases, deletes `BIRT Y`-style events, turns
    sex X into U or rewrites EST as ABT (checked on all 400 demo people: zero changes);
  - Julian, Hebrew and French dates survive the upgrade of a GEDCOM 5.5.1 file (they were broken on the first
    edit); Hebrew and French months are written correctly; a line break inside a value no longer breaks the file;
  - the Import: asks before saving into another program's file; a failed save no longer leaves the import in
    memory (a retry imported everything twice); two imports with the same name are kept apart; no duplicate facts
    when importing into joined people; candidates stay right after the archive changes; ages that aren't ages are
    ignored;
  - Ctrl S can't create a person twice; leaving the editor, „Anuluj”, unlinking a photo or a person, and „Cofnij
    import” ask first; step 1 of the Import no longer silently drops decisions when the text changes; errors that
    were silent now show a message.
- **Security:** the file server refuses alternate data streams and Windows device names; the dev bridge only
  accepts local pages.
- **New:** „Usuń osobę…” in the edit form (with a confirmation; Ctrl Z brings the person back); places in the Ctrl K
  search.
- **Screens:** Start's columns no longer overflow at 1280 px; the tree draws the children of two marriages on
  separate lines, keeps the side panel's photo size, shows married daughters by their married name, keeps the
  Przodkowie labels below the toolbars, gives Przodkowie and Potomkowie the full width, and waits for a real window
  size before placing the camera; surname groups in Osoby follow the A–Z order; the tree follows changes made in
  Ustawienia; „Pierwsze otwarcie” no longer returns on every launch after „Pomiń”; a backup offers to save first;
  times of changes show in local time (they were shown in UTC).
- **Polish:** „ze Stanisławem”, „Śmierć żony, Tekli”, „Anieli”, „Tekli”, „Wróblowie”, „Stępniowie”, „Wawrzyńca”,
  „82 lata temu”, and plural forms in about 15 places.
- **Speed (10,000 people):** the Osoby sort 1 s → 65 ms, `places.list` 766 → 158 ms, import matching 70 → 3 ms per
  person, `people.list` 250 → 100 ms.
- **Keyboard:** the rows that were mouse-only (recent archives, people, lists on Start and the profile) can be used
  with Tab and Enter.
- **Tests:** 142 Rust tests (26 new; the reviewer's were each seen failing before their fix) and 16 interface tests
  pass.

## 0.2.0 — 2026-09-28 22:20: The app — every v1 screen, built from the design
- **The app window (Tauri 2 + React 19 + TypeScript):**
  - one command channel: every screen talks to the Rust core through `Api::call(method, args)`
    (`crates/heirloom-api`), in the window and in a normal browser;
  - `crates/heirloom-bridge`: a small dev server that serves the same commands and files to a browser, so the screens
    are tested on real data;
  - the `heirloom://` file protocol: originals, previews and thumbnails (cached per archive, EXIF rotation applied);
  - the design's tokens in plain CSS (light and dark), the bundled fonts (Newsreader, IBM Plex Sans), text size,
    density and animation settings.
- **Screens:**
  - Wybór archiwum (with „Utwórz archiwum z plików” first), opening a big archive, Pierwsze otwarcie, Start (with „W
    tym dniu”), Historia zmian;
  - Drzewo: Rodzina, Przodkowie and Potomkowie (DOM cards over SVG lines, 4 levels of detail, keyboard, side panel,
    colour modes, direct line), and Całe drzewo (PixiJS, generation bands, surname clusters, labels that don't
    overlap);
  - Osoby (a virtualized table with filters, grouping and A–Z), the person profile (all sections, texts edited in
    place with @mentions), the edit form (Polish dates with feedback, places, relations);
  - Nazwiska, Miejsca, Historie, Media with a lightbox, Brakujące pliki, Źródła with a scan and transcription
    viewer;
  - Ustawienia: appearance, tree, people and dates, the archive (storage, read-only), backups, export, tools,
    editors, „O programie”.
- **Browse and edit modes:** „Kto edytuje?”, the edit bar, saving with a backup copy, undo/redo, the conflict and
  „another program's file” dialogs, the read-only switch.
- **Import (5 steps):** paste the AI answer or drop a folder; the check with fixes (paste a missing part, fix a date,
  skip a file, answer the AI's questions); matching with a percentage and field-by-field choices; files with
  previews before they are copied, and profile photos; a summary where every person and field can be unticked. The
  commit saves with a backup, keeps the raw answers in `zrodla-ai/`, and can be undone as a whole.
- **Archive tools:** a .zip backup, GEDZIP export, an archive check, storage use with free space, rebuilding the
  thumbnails, exporting and loading the display settings.
- **Display settings stored with the archive:** name order, date format, Julian dates next to Gregorian ones, surname
  joins („Dołącz”), rounded tree lines.
- **Checked:** 116 Rust tests and 16 interface tests pass. The release app (`heirloom.exe`) was built and driven
  through the WebView's DevTools port: it opens an archive and shows Start, the tree, Media with `heirloom://`
  thumbnails, Całe drzewo and the Import, with no console errors. On 10,000 people: a profile in 15 ms, search in
  15 ms, the whole-family layout in 74 ms.
- **Docs:** PLAN §5.7 (how the build differs from the plan), README (how to run it), TODO (what's done and what was
  found), `design/FEEDBACK_DRAFT.md` §K (design points found while building).

## 0.1.1 — 2026-09-28 19:31: Core review — fixes for saving, backups, history, GEDCOM reading and the cache
- **Saving and backups (`archive.rs`):**
  - backup rotation deleted the wrong files: another data file's copies (`rodzina-stara-…` also starts with
    `rodzina-`), files put into `kopie/` by hand, and even the copy it had just made. It now deletes only
    `<name>-<timestamp>.ged` copies, never the new one;
  - an old backup open in another program no longer blocks every save;
  - a same-size change made by another program that kept the old file time (FAT32 USB sticks, copy tools) was
    overwritten without a conflict. Saving now compares the file's contents;
  - "Utwórz archiwum" in a folder that is already an archive (with a differently named `.ged`) overwrote its
    settings. It is now refused;
  - confirming a save into one program's file no longer covers another `.ged` opened in the same folder;
  - a record removed and added back unchanged left "unsaved changes" on forever. It is now written.
- **History:** records that share a key (many `0 _PLAC_DEFN` records from Legacy, repeated xrefs) were logged as
  changed on every save. They are now compared in file order.
- **GEDCOM reading:**
  - an ANSEL accent right before a line break swallowed the next line, and could merge a whole person into the
    record before it;
  - a file labelled ANSEL/ASCII/ANSI that is really UTF-8 kept its wrong label, so Polish letters added in
    Heirloom were misread by other programs. The header now says UTF-8;
  - CR CR LF line endings were written back as CR alone. They are now written as CRLF;
  - pointers followed by a space (`1 FAMS @F1@ `) were ignored, which dropped family links;
  - names written surname first (`/Kovács/ János`) lost the given name, and an empty `_MARNM` hid the surname;
  - a nonsense double year such as `2147483647/99` crashed the date parser (overflow).
- **Cache:** a repeated xref made the whole cache rebuild fail. Schema version 2.
- 57 tests, all passing.

## 0.1.0 — 2026-09-28 19:10: Phase 1 started — the Rust core
- **Cargo workspace** with two crates.
- **`crates/heirloom-core`:**
  - GEDCOM reading and writing on a lossless line tree, so unknown data and other programs' tags survive a save:
    GEDCOM 7 in and out; GEDCOM 5.5.1 in (CONC lines, UTF-16, ANSEL, Windows-1250/1252 with a Polish-letter guess);
  - GEDCOM dates: qualifiers (ABT, BEF, BET…AND, FROM…TO…), Julian and other calendars, double years, sort keys;
  - a people-and-families view (birth and married names, maiden names, `_MARNM`);
  - the archive folder (PLAN §11.2): `rodzina.ged` + `media/` + `.heirloom/`. Opening writes nothing. Saving makes
    a backup copy in `.heirloom/kopie/` first, then writes a new file and swaps it in. It notices when another program
    changed the file, asks before the first save into another program's file, and has an optional read-only switch;
  - undo/redo of edits, and the change history (`historia.jsonl`, one line per changed record);
  - the search cache (SQLite + FTS5) in `%LOCALAPPDATA%\Heirloom\cache\`. Polish letters are folded, so "Lukasz"
    finds "Łukasz".
- **`crates/heirloom-gen`:** a generator of fictional test families as archive folders — 300 years, maiden names,
  remarriages, adoptions, Julian dates, sources, notes and placeholder photos; the same seed gives the same family.
- **Timing on this PC** (release build):

  | Archive | Open + parse | Cache build | Search | Write |
  |---|---|---|---|---|
  | 5,000 people (2.0 MB) | 64 ms | 101 ms | 1.4 ms | 13 ms |
  | 10,000 people (3.9 MB) | 130 ms | 262 ms | 2.0 ms | 31 ms |

- 38 tests, all passing.
- `spikes/resize-test`: its build outputs were moved to the Recycle Bin (rebuilt on the next run).
- Local Git repository initialized (noreply e-mail); nothing is committed or pushed yet.
- README: a Development section; the family-data section now describes the archive folder.

## 0.0.8 — 2026-09-28 18:36: Archive-folder design approved; plan and roadmap updated to match
- **PLAN §11.1 and §11.2 approved.** §0 records it, along with "staying with Tauri, no Electron comparison".
- **PLAN body updated** to the approved design:
  - §4.6 import: it's the built-in converter and writes into `rodzina.ged` + `media/`; opening GEDCOM files directly
    is now part of v1;
  - §4.7 media: readable files in `media/`, explicit "Zmniejsz zdjęcia", a missing-files tool;
  - §4.8: safe saving with the edit bar, backup copies, conflict handling; GEDZIP export;
  - §4.9: the Archiwum settings; §4.10: the app opens on Start, with archive choosing and first opening; §4.2: the
    tree loads on demand;
  - §5.3–§5.5: the archive folder is the source of truth, SQLite is a rebuildable cache, UIDs, where each part is
    saved;
  - §8 roadmap and §9 risks.
- **TODO.md:**
  - Phase 0: the resize follow-up dropped, the decision recorded;
  - Phase 1 rewritten: GEDCOM reader/writer, the archive folder, cache, modes, archive picker, opening on Start;
  - Phase 2: media v1; Phase 3: "Utwórz archiwum z plików" and saving into the archive.
- **`docs/AI_INSTRUCTIONS.md`:** text documents about people (.txt, .docx, PDF) are numbered and uploaded too, and the
  AI sorts the information to the right person.
- **README:** status updated.

## 0.0.7 — 2026-09-28 18:31: Your changes to the archive-folder proposal; designer update 3
- `PLAN.md` §11.2:
  - rule 4: any archive can be edited, and it's always clear which file is being edited. The edit bar shows who is
    editing and the file. The first save into another program's file asks first and makes a backup. Read-only is an
    optional switch (was: other people's files read-only by default).
  - rule 8, new: the built-in converter. The Import turns the researcher's files (the AI's answers, photos, PDFs,
    notes) into the archive's `rodzina.ged` + `media/`, and "Utwórz archiwum z plików" is the first-launch action.
- `docs/DESIGNER_ANSWERS.md`: "Uzupełnienie 3" for the designer, correcting the read-only state from update 2 and
  adding the first-launch "create archive from files" path.
- `TODO.md`: update 3 recorded; §11.2 marked as changed, waiting for the final OK.

## 0.0.6 — 2026-09-28 18:24: Free ChatGPT/Gemini, fully offline, archive-folder proposal, designer update 2
- The researcher uses free ChatGPT (maybe free Gemini). Changes for that:
  - `docs/AI_INSTRUCTIONS.md`: smaller batches (3–8 documents), parts of at most 10 persons (was 15), notes on
    free-plan limits, and copying results instead of downloading;
  - `docs/IMPORT_FORMAT.md`: the same limits;
  - `PLAN.md`: the risk table updated.
- **Fully offline** recorded in PLAN §0 and §3: all fonts and icons ship with the app, and nothing is downloaded while
  it runs.
- **PLAN §11, new, waiting for approval:**
  - §11.1: open on Start and load the tree on demand, with no tree work in the background;
  - §11.2: the family archive as an open folder — GEDCOM 7 + `media/` + small `.heirloom/` settings files, with the
    cache kept in `%LOCALAPPDATA%` — plus read-only opening of other people's files, zero-setup opening, how other
    apps do it (Ancestris, Obsidian, Lightroom/darktable, Kodi/Jellyfin), the trade-offs, and what would change in
    the plan.
- `docs/DESIGNER_ANSWERS.md`: "Uzupełnienie 2" for the designer:
  - opening on Start;
  - new screens for choosing and first opening an archive;
  - the read-only state; saving and conflicts;
  - missing files; "Inne dane z pliku";
  - the Archiwum settings; fully offline.
- `TODO.md`: the eye-check result ("not smooth, but not lagging much") and a resize follow-up (the transparent
  variant, then possibly an Electron comparison); the design review and the §11 decision added.

## 0.0.5 — 2026-09-28 16:51: Answers for the designer; no "Ty"; browse and edit modes
- `docs/DESIGNER_ANSWERS.md` (new, in Polish): concrete answers to the designer's 10 questions — fields, actions,
  states and what matters most for each area — plus the v1 / later priorities:
  - Historie, Oś czasu, Miejsca, Media, Źródła;
  - Nazwiska, Import (with example messages), Ustawienia;
  - profile sections, tree modes.
- Decisions from the designer brief, recorded in `PLAN.md` §0:
  - the whole family uses one computer, with no login;
  - **there is no "Ty"**: relationships are shown relative to the person being viewed, and the tree opens on a
    start person;
  - browsing is the default; edit mode asks "Kto edytuje?".
- `PLAN.md` changes:
  - "Kim jest dla mnie" became "Ścieżka pokrewieństwa" (how any two people are related);
  - the Settings list is updated (Import instead of Import/AI, new "Osoby edytujące", Privacy/Language/Shortcuts
    moved later);
  - stories get a place and a list of people who appear in them.
- `docs/DESIGNER_PROMPT.md`: the "Ty" references were removed, the Settings list updated, and a link to the answers
  added.
- `docs/IMPORT_FORMAT.md` and `docs/AI_INSTRUCTIONS.md`: an optional `place` for stories.
- `TODO.md`: the designer's questions are marked as answered; later items added for a PIN for edit mode and for audio
  and video.

## 0.0.4 — 2026-09-28 00:45: MIT licence; Phase 0 resize test passed
- **Licence: MIT**, confirmed. Added `LICENSE` (© 2026 Husarp) and recorded it in PLAN.md and TODO.md.
- **`spikes/resize-test`:** a throwaway Tauri 2 + React + PixiJS test app.
  - It has 3,000 or 10,000 dummy people with Polish names, the planned window layout, 3 levels of detail and
    on-screen-only drawing.
  - It shows live frame-time stats and runs a built-in benchmark (`benchmark.cmd`) that pans, zooms and resizes its
    own window, then writes a report.
- **Results on this PC** (Iris Xe, 75 Hz):
  - panning and zooming at a locked 75 fps, with 3,000 and with 10,000 people;
  - window resize at 72 fps and 0–1 frames over 33 ms, after switching to a screen-sized canvas buffer (before: 66–68
    fps and 4–9 frames over 33 ms).
  - Tauri passes, so the Electron comparison isn't needed.
- **PLAN.md:** a new rendering rule (the canvas buffer is screen-sized and only grows), and the resize-risk row
  updated with the results.

## 0.0.3 — 2026-09-27 23:41: Direction chosen (our own app); full plan, roadmap, import format, AI instructions
- Decisions recorded:
  - our own app (option C);
  - licence MIT (to be confirmed);
  - PC only;
  - Polish UI;
  - AI not built into the app.
- `PLAN.md`, now the full plan:
  - principles, and what each part of the app does (marked v1.0 or later);
  - architecture, code layout and where the data lives;
  - a first data model;
  - performance targets, testing, roadmap and risks.
- `TODO.md`: new. The roadmap as checkboxes, from Phase 0 (checks before building) to v1.0, plus a "later" list.
- `docs/IMPORT_FORMAT.md`: new. The `heirloom-import` 1.0 draft format, with a complete example; the example was
  checked with a validator.
- `docs/AI_INSTRUCTIONS.md`: new, in Polish.
  - Part A: how the researcher prepares batches (M001… files) and checks transcriptions.
  - Part B: the prompt to paste into ChatGPT, Claude or Gemini (transcription first, then the file in parts, with no
    invented data).
- `docs/DESIGNER_PROMPT.md`: the Import screen is marked as the main way in, with the "Wklej odpowiedź AI" box and
  evidence shown next to each fact.
- `README.md`: status, planned technology and the list of documents.
- The old `Kindred` folder was moved to the Recycle Bin, after checking it was identical to `Heirloom`.

## 0.0.2 — 2026-09-27 19:42: Renamed to Heirloom; plan and research notes
- The project was renamed from Kindred to **Heirloom** (folder and documents).
- `PLAN.md`:
  - the requirements from the brief;
  - what we know about the data: no existing tree, only raw documents and pictures;
  - ideas from the research, marked as proposals;
  - the direction options (A: Gramps + add-ons, B: fork Gramps Web, C: our own app), with the decision still
    pending.
- `docs/RESEARCH.md`: condensed research notes with sources on existing apps, GEDCOM, technology, storage,
  compression, the tree engine, AI import and licences.
- `docs/DESIGNER_PROMPT.md`: renamed, and the phone screens removed (the PC comes first).

## 0.0.1 — 2026-09-27 17:07: Project created
- Created the project folder (working name Kindred).
- First draft of `docs/DESIGNER_PROMPT.md`, a prompt for Claude Design.
