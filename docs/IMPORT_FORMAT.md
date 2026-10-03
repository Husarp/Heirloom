# Heirloom import format: `heirloom-import` 1.0 (draft)

**Status:** a draft for the Phase 0 trial run. It will change once it has been tested with real material.

This file defines what a researcher's AI chat produces and what the app's importer reads. The instructions the
researcher gives to the chat are in [AI_INSTRUCTIONS.md](AI_INSTRUCTIONS.md) (in Polish), and the research behind the
choices is in [RESEARCH.md §8](RESEARCH.md#8-ai-assisted-import-external-ai-chat--file--app).

## 1. Overview

- **Batch:** one import is one **batch**, i.e. one chat about one branch of the family (3–8 records on free ChatGPT
  or Gemini). The chat delivers it in one or more **parts**, each holding at most 10 persons (free plans give short
  answers).
- **Part:** one JSON object, which appears in the chat as one fenced ` ```json ` block.
- **The researcher's files** (photos, scans, PDFs, text notes) are named with an **M-number prefix**, for example
  `M001 akt urodzenia Józef 1878.jpg`. The AI refers to a file only by that number, because chats don't see file
  names reliably. Later, the app will assign and stamp the M-numbers itself.
- **IDs are local to the batch:**

  | Prefix | Object |
  |---|---|
  | `P` | persons |
  | `E` | events |
  | `T` | texts |
  | `S` | sources |
  | `Q` | questions |
  | `N` | notes pasted into the chat (not uploaded as files) |
  | `M` | files |

  Numbering continues across the parts of one batch.
- **Language:** keys and enum values are English; everything a person reads (names, captions, texts) is Polish.
- **Unknown values are omitted.** Never write "nieznany" or "unknown" as a value.
- **Structure:** the file is kept flat on purpose (arrays that reference each other by ID), because AI output is more
  accurate that way than with deep nesting.

## 2. Top level

| Key | Required | Meaning |
|---|---|---|
| `format` | yes | Always `"heirloom-import"`. |
| `version` | yes | `"1.0"`. A new major version means breaking changes. |
| `prompt_rev` | yes | The revision of AI_INSTRUCTIONS.md used, e.g. `"2026-09-27"`. |
| `batch` | yes | A unique name for the batch: a short label plus a date, e.g. `"Kowalscy-Leczna-2026-10-01"`. |
| `part` | yes | 1, 2, 3, … |
| `created` | no | The date the file was made, `YYYY-MM-DD`. |
| `author` | no | The researcher's name. |
| `lang` | no | The language of the texts; `"pl"`. |
| `manifest` | part 1 | The IDs of **all** persons planned in the batch, across all parts. |
| `persons`, `events`, `relationships`, `texts`, `media`, `sources`, `links`, `coverage`, `questions` | no | Arrays, described in §3. Leave out any that are empty. |
| `end` | yes, **last key** | The end-of-part marker, described in §3.10. |

## 3. Objects

### 3.1 `persons`

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | `P1`, `P2`, … |
| `names` | yes, at least 1 | Name objects (below). |
| `sex` | no | `M`, `F` or `U` (unknown). |
| `living` | no | `true` or `false`. Leave it out if unknown. |
| `summary` | no | 2–4 factual sentences in Polish. |
| `tags` | no | Short Polish labels, e.g. `["kolejarz", "emigrant"]`. |
| `match` | no | The handle of an existing person from a roster (planned, see §8), e.g. `"@K3F9X"`. It is only a suggestion; the importer never merges on it automatically. |

**Name object:**

| Key | Required | Meaning |
|---|---|---|
| `type` | yes | `birth` (the birth surname, so for a woman her maiden name), `married` (surname after marriage), or `other` (alias, religious name, …). |
| `given` | one of `given` / `surname` | Given names in modern Polish form (Jan for Joannes). |
| `surname` | one of `given` / `surname` | The surname in the form used for that person: Kowalska for a woman. |
| `nickname` | no | E.g. "Dziadek Józek". |
| `orig` | no | Exactly as written in the source (Latin, Cyrillic, old spelling). |
| `lang` | no | The language of `orig`: `pl`, `la`, `ru`, `de`, … |
| `src` | no | Source references (§4.4). |

### 3.2 `events`

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | `E1`, `E2`, … |
| `type` | yes | `birth`, `baptism`, `banns`, `marriage`, `divorce`, `death`, `burial`, `residence`, `occupation`, `education`, `military`, `emigration`, `religion`, or `other`. |
| `type_other` | when `type` is `other` | A short Polish name for the event. |
| `people` | yes, at least 1 | `[{ "p": "P1", "role": "…", "age_orig": "…" }]`. Roles: `principal` (the child at a baptism, the deceased, the person whose occupation it is), `father`, `mother`, `spouse`, `godparent`, `witness`, `informant`, `officiant`, `other`. `age_orig` is that person's age **as written**, e.g. `"lat 30"`. |
| `date` | no | The date in the syntax of §4.1. |
| `date_orig` | no | The date as written in the source. |
| `julian` | no | The Julian date, for double-dated records (§4.1). |
| `place` | no | The locality in Polish spelling (transliterated from Cyrillic if necessary). |
| `place_orig` | no | The place as written. |
| `place_note` | no | Anything else the material says about the place: parish, county, gubernia, country. |
| `value` | no | For `occupation`, `religion`, `education`, `residence` and `military`: the content, e.g. "kolejarz na stacji Lublin". |
| `cause` | no | For `death`: the cause, as written. |
| `note` | no | A short Polish note. |
| `src`, `basis`, `certainty` | recommended | See §4.4. |

### 3.3 `relationships`

The importer builds families from these. Always state them explicitly, even when an event already implies them.

| Form | Meaning |
|---|---|
| `{ "type": "parent", "parent": "P2", "child": "P1", "kind": "birth" }` | `kind`: `birth`, `adopted`, `foster`, `step` or `unknown`. |
| `{ "type": "partners", "a": "P2", "b": "P3", "kind": "marriage" }` | `kind`: `marriage`, `partnership` or `unknown`. |
| `{ "type": "sibling", "a": "P5", "b": "P6", "kind": "full" }` | Only when the parents are not in the material. `kind`: `full`, `half` or `unknown`. |

Every relationship may also carry `src`, `basis` and `certainty` (§4.4).

A relation the material states without the people in between (e.g. "Jan był wujem Marii") is **not** turned into
invented intermediate persons. It goes into a `note` text and a question.

### 3.4 `texts`

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | `T1`, `T2`, … |
| `person` | yes | The person the text belongs to. Other people are linked with mentions (§4.3). |
| `kind` | yes | `bio` (one section of the biography), `story`, `saying` (powiedzonko), `trivia` (ciekawostka) or `note` (a research note). |
| `title` | for `bio` and `story` | The section or story title. The app builds the biography's table of contents from the `bio` titles, in array order. |
| `md` | yes | The text in Polish Markdown: paragraphs, **bold**, *italic*, lists, links and mentions. No headings; split the text into separate `bio` items instead. |
| `date` | no | When the story happened (§4.1). |
| `place` | no | For `story`: where it happened, in Polish spelling. |
| `src`, `basis`, `certainty` | recommended | See §4.4. |

### 3.5 `media`

One entry per file the AI saw, describing that file.

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | The file's M-number exactly as in its name, e.g. `M001`. |
| `kind` | no | `photo`, `document` or `other`. |
| `document_type` | no | E.g. "akt urodzenia", "akt małżeństwa", "zrzut ekranu indeksu", "notatki". |
| `caption` | no | A short Polish caption. |
| `date` / `place` | no | When and where the photo was taken or the document was made. |
| `depicts` | no | The people visible in a photo. |
| `about` | no | The people a document concerns. |
| `profile_for` | no | The people for whom this photo is a good profile photo. This is a suggestion; the user decides. |
| `transcription` | no | For documents: a diplomatic transcription, as checked by the researcher in step 1. |
| `translation` | no | A Polish translation, when the document is in another language. |
| `note` | no | Anything else. |

### 3.6 `sources`

| Key | Required | Meaning |
|---|---|---|
| `id` | yes | `S1`, `S2`, … |
| `kind` | yes | `parish_record`, `civil_record`, `index`, `photo`, `letter`, `oral`, `note`, `book`, `website` or `other`. |
| `title` | yes | E.g. "Akt urodzenia nr 45/1878, parafia rzymskokatolicka Łęczna". |
| `parish`, `year`, `akt` | no | For registers: the parish, the year and the record (akt) number. |
| `archive`, `call_number` | no | The archive and its call number (sygnatura), when the material gives them. |
| `url` | no | Only if the URL appears **literally** in the material. |
| `media` | no | The M-numbers of the files that show this source. |
| `note` | no | Anything else. |

### 3.7 `links`

`{ "person": "P1", "url": "https://pl.wikipedia.org/wiki/…", "title": "Wikipedia", "kind": "wikipedia" }`

`kind` is `wikipedia`, `geneteka`, `familysearch`, `grave` or `other`. As with sources, a URL is included only if it
appears literally in the material.

### 3.8 `coverage`

There is one entry for **every** file and pasted note, so nothing is skipped silently:
`{ "m": "M003", "status": "partial", "note": "Data przyjęcia do pracy nieczytelna." }`

- `m` is an M-number or an N-number (a pasted note).
- `status` is `extracted`, `partial`, `unreadable` or `not_relevant`.

### 3.9 `questions`

These are questions for the reviewer, in Polish:
`{ "id": "Q1", "about": ["P4"], "text": "Czy …?" }`

Use them for doubts, possible duplicates, conflicting sources and relations that couldn't be recorded.

### 3.10 `end` (always the last key)

`{ "part": 1, "final": true, "counts": { "persons": 4, "events": 3 }, "marker": "END-HEIRLOOM-IMPORT" }`

- `final` is `true` only in the batch's last part.
- `counts` holds the number of items in each array of this part.
- `marker` must be exactly `END-HEIRLOOM-IMPORT`. A part without the marker is treated as **cut off** (truncated).

## 4. Value formats

### 4.1 Dates
- **Exact or partial:** `YYYY-MM-DD`, `YYYY-MM` or `YYYY`.
- **With a qualifier:**

  | Syntax | Polish wording in the source |
  |---|---|
  | `ABT 1850` | około, ok. |
  | `BEF 1900-03` | przed |
  | `AFT 1920` | po |
  | `BET 1850 AND 1855` | między 1850 a 1855 |
  | `FROM 1905 TO 1912` | od … do … |
  | `FROM 1905` | od 1905 |
  | `TO 1912` | do 1912 |

  `EST` and `CAL` exist too, but only the app uses them, for estimates it calculates.
- **Keep the original wording:** always add `date_orig` when the source phrases the date in words or unusually.
- **Double dating**, as in Congress Poland records, e.g. "28 lutego / 12 marca 1878": `date` holds the **Gregorian**
  date (the second one) and `julian` the Julian one (the first).
- **Ages:** the AI never computes a birth date from an age. It copies the age into `people[].age_orig`, and the app
  derives an estimated date, marked as inferred.

### 4.2 IDs
- **Local IDs** use the prefixes P, E, T, S, Q and N plus a number (`P12`). They are unique within the batch and
  continue across its parts.
- **File IDs** are the M-number exactly as in the file name (`M001`).

### 4.3 Mentions inside texts
- **Form:** `[visible text](person:P12)`. The visible text can be inflected: "syn [Antoniego](person:P2)".
- **Roster people:** `[Jana](person:@K3F9X)` for someone from a roster (§8).
- **Unknown people:** someone who is neither in the batch nor in the roster stays plain text.

### 4.4 Evidence fields
- **`src`:** a list of S-, M- or N-IDs, e.g. `["S1"]` or `["M004"]`.
- **`basis`:** `stated` (written in the source) or `inferred` (a conclusion drawn from it).
- **`certainty`:** `high`, `medium` or `low`.

### 4.5 Text rules
- Use Polish quotation marks „…” inside text values, so no escaping is needed.
- Texts are factual. Family stories are told as reported ("Według relacji rodzinnej…").

## 5. Example: a complete, valid part

```json
{
  "format": "heirloom-import",
  "version": "1.0",
  "prompt_rev": "2026-09-27",
  "batch": "Kowalscy-Leczna-2026-10-01",
  "part": 1,
  "created": "2026-10-01",
  "author": "Badacz",
  "lang": "pl",
  "manifest": ["P1", "P2", "P3", "P4"],
  "persons": [
    {
      "id": "P1",
      "sex": "M",
      "living": false,
      "names": [
        { "type": "birth", "given": "Józef", "surname": "Kowalski", "nickname": "Dziadek Józek",
          "orig": "Іосифъ Ковальскій", "lang": "ru", "src": ["S1"] }
      ],
      "summary": "Józef Kowalski urodził się w 1878 roku w Wólce w parafii Łęczna. Od 1905 roku pracował jako kolejarz na stacji Lublin.",
      "tags": ["kolejarz"]
    },
    {
      "id": "P2",
      "sex": "M",
      "names": [
        { "type": "birth", "given": "Antoni", "surname": "Kowalski", "orig": "Антоній Ковальскій", "lang": "ru", "src": ["S1"] }
      ]
    },
    {
      "id": "P3",
      "sex": "F",
      "names": [
        { "type": "birth", "given": "Agnieszka", "surname": "Mazur", "orig": "Агнешки урожденной Мазуръ", "lang": "ru", "src": ["S1"] },
        { "type": "married", "given": "Agnieszka", "surname": "Kowalska", "src": ["S1"] }
      ]
    },
    {
      "id": "P4",
      "sex": "F",
      "names": [
        { "type": "birth", "given": "Marianna", "surname": "Nowak", "src": ["S2"] },
        { "type": "married", "given": "Marianna", "surname": "Kowalska", "src": ["S2"] }
      ]
    }
  ],
  "events": [
    {
      "id": "E1",
      "type": "birth",
      "date": "1878-03-12",
      "julian": "1878-02-28",
      "date_orig": "28 февраля / 12 марта 1878",
      "place": "Wólka",
      "place_note": "parafia Łęczna",
      "people": [
        { "p": "P1", "role": "principal" },
        { "p": "P2", "role": "father", "age_orig": "30 лѣтъ" },
        { "p": "P3", "role": "mother" }
      ],
      "src": ["S1"],
      "basis": "stated",
      "certainty": "high"
    },
    {
      "id": "E2",
      "type": "marriage",
      "date": "1904-02-14",
      "place": "Łęczna",
      "people": [
        { "p": "P1", "role": "spouse" },
        { "p": "P4", "role": "spouse" }
      ],
      "src": ["S2"],
      "basis": "stated",
      "certainty": "high"
    },
    {
      "id": "E3",
      "type": "occupation",
      "value": "kolejarz na stacji Lublin",
      "date": "FROM 1905",
      "people": [{ "p": "P1", "role": "principal" }],
      "src": ["M003"],
      "basis": "stated",
      "certainty": "medium"
    }
  ],
  "relationships": [
    { "type": "parent", "parent": "P2", "child": "P1", "kind": "birth", "src": ["S1"], "basis": "stated", "certainty": "high" },
    { "type": "parent", "parent": "P3", "child": "P1", "kind": "birth", "src": ["S1"], "basis": "stated", "certainty": "high" },
    { "type": "partners", "a": "P2", "b": "P3", "kind": "marriage", "src": ["S1"], "basis": "stated", "certainty": "high" },
    { "type": "partners", "a": "P1", "b": "P4", "kind": "marriage", "src": ["S2"], "basis": "stated", "certainty": "high" }
  ],
  "texts": [
    {
      "id": "T1",
      "person": "P1",
      "kind": "bio",
      "title": "Dzieciństwo",
      "md": "Józef urodził się 12 marca 1878 roku w Wólce w parafii Łęczna jako syn [Antoniego](person:P2) i [Agnieszki z domu Mazur](person:P3).",
      "src": ["S1"],
      "basis": "stated",
      "certainty": "high"
    },
    {
      "id": "T2",
      "person": "P1",
      "kind": "bio",
      "title": "Praca na kolei",
      "md": "Od 1905 roku pracował jako kolejarz na stacji Lublin.",
      "src": ["M003"],
      "basis": "stated",
      "certainty": "medium"
    },
    {
      "id": "T3",
      "person": "P1",
      "kind": "story",
      "title": "Zima 1915: ucieczka przed frontem",
      "md": "Według relacji cioci Heleny w 1915 roku Józef z [Marianną](person:P4) i dziećmi wyjechali na wschód dwoma wozami. Wrócili w 1918 roku.",
      "date": "1915",
      "src": ["M004"],
      "basis": "stated",
      "certainty": "medium"
    },
    {
      "id": "T4",
      "person": "P1",
      "kind": "saying",
      "md": "„Pociąg nie czeka, a robota nie ucieknie.”",
      "src": ["M004"],
      "basis": "stated",
      "certainty": "medium"
    },
    {
      "id": "T5",
      "person": "P1",
      "kind": "trivia",
      "md": "Jako pierwszy we wsi miał rower (1909).",
      "src": ["M004"],
      "basis": "stated",
      "certainty": "low"
    }
  ],
  "media": [
    {
      "id": "M001",
      "kind": "document",
      "document_type": "akt urodzenia",
      "caption": "Akt urodzenia Józefa Kowalskiego, Łęczna 1878, nr 45",
      "about": ["P1", "P2", "P3"],
      "transcription": "Состоялось въ посадѣ Ленчна двадцать восьмаго Февраля / двѣнадцатаго Марта тысяча восемьсотъ семьдесятъ восьмаго года […]",
      "translation": "Działo się w osadzie Łęczna 28 lutego / 12 marca 1878 roku […]"
    },
    {
      "id": "M002",
      "kind": "photo",
      "caption": "Józef i Marianna w dniu ślubu",
      "date": "1904",
      "depicts": ["P1", "P4"],
      "profile_for": ["P1"]
    },
    { "id": "M003", "kind": "document", "document_type": "legitymacja kolejowa", "caption": "Legitymacja kolejowa Józefa Kowalskiego", "about": ["P1"] },
    { "id": "M004", "kind": "document", "document_type": "notatki", "caption": "Notatki z rozmowy z ciocią Heleną", "about": ["P1", "P4"] },
    { "id": "M005", "kind": "document", "document_type": "zrzut ekranu indeksu", "caption": "Geneteka: ślub Józefa Kowalskiego i Marianny Nowak, Łęczna 1904", "about": ["P1", "P4"] }
  ],
  "sources": [
    { "id": "S1", "kind": "parish_record", "title": "Akt urodzenia nr 45/1878, parafia rzymskokatolicka Łęczna", "parish": "Łęczna", "year": 1878, "akt": "45", "media": ["M001"] },
    { "id": "S2", "kind": "index", "title": "Geneteka: indeks małżeństw, parafia Łęczna, 1904", "parish": "Łęczna", "year": 1904, "akt": "12", "media": ["M005"] }
  ],
  "coverage": [
    { "m": "M001", "status": "extracted" },
    { "m": "M002", "status": "extracted" },
    { "m": "M003", "status": "partial", "note": "Data przyjęcia do pracy nieczytelna." },
    { "m": "M004", "status": "extracted" },
    { "m": "M005", "status": "extracted" }
  ],
  "questions": [
    { "id": "Q1", "about": ["P4"], "text": "Czy Marianna Nowak pochodziła z Ciechanek? Notatki (M004) tak mówią, ale indeks (M005) nie podaje miejscowości." }
  ],
  "end": {
    "part": 1,
    "final": true,
    "counts": { "persons": 4, "events": 3, "relationships": 4, "texts": 5, "media": 5, "sources": 2, "coverage": 5, "questions": 1 },
    "marker": "END-HEIRLOOM-IMPORT"
  }
}
```

## 6. How the app will read it (planned)

1. **Collect.** The user pastes the AI's whole answer into "Wklej odpowiedź AI", or drops `.json` files, together with
   the M-files. The app extracts every ` ```json ` block.
2. **Check that each part is complete.** The exact `END-HEIRLOOM-IMPORT` marker must be present. Only then are
   cosmetic fixes applied (curly quotes, trailing commas).
3. **Validate.**
   - **Blocking errors:**
     - the JSON can't be parsed;
     - the marker is missing;
     - `format` is wrong or the major `version` is unknown;
     - IDs are duplicated;
     - a reference points to a P/E/T/S/Q ID that doesn't exist;
     - a part listed in the manifest is missing once the final part has arrived.
   - **Warnings:**
     - counts that don't match;
     - unknown keys (kept and shown) and unknown enum values (mapped to `other`);
     - dates that don't fit §4.1 (kept as `date_orig`);
     - M-files that are missing;
     - mentions of unknown IDs (turned into plain text);
     - low-certainty and inferred facts.
4. **Normalize.**
   - Names: Polish letters folded for matching; gender and marital forms reduced to a base surname; Latin, Russian and
     German given names mapped to Polish.
   - Dates, including Julian ones.
   - Ages turned into estimated dates.
   - Places suggested from the existing place list.
5. **Match.** Each incoming person is scored against the existing people (the scoring is in
   [RESEARCH §8](RESEARCH.md#8-ai-assisted-import-external-ai-chat--file--app)).
6. **Review.** The user works through buckets (new / matched / needs review), per-field decisions, an issues panel
   with the AI's questions, a mini tree preview and a media grid with the profile-photo star.
7. **Save.**
   - Everything is saved in one transaction, after a snapshot.
   - Every saved row carries the batch ID.
   - "Cofnij import" (undo import) reverts the whole batch.

The raw pasted text is stored unchanged with the batch.

## 7. Versioning

- `format` + `version` identify the schema; a new major version is a breaking change.
- `prompt_rev` records which revision of AI_INSTRUCTIONS.md produced the file.
- The importer keeps unknown keys and shows them, so newer files never silently lose data.

## 8. Roster (planned)

Later, the app will export a short **roster** of existing people for the chat to use. Each line holds a handle (`@`
+ 4 Crockford-base32 characters + a check character), the name, the years and the closest relatives. For example:

`@K3F9X — Jan Kowalski (1873–1941), syn Wojciecha i Marianny z d. Nowak; mąż Anny z d. Wiśniewskiej`

The AI can then fill in `match` and write mentions like `(person:@K3F9X)`, which cuts down duplicates. Exporting data
about living people is opt-in.
