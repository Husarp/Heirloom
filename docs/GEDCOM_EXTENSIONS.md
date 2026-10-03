# How Heirloom stores its data in GEDCOM 7

The family archive's `rodzina.ged` is a normal **GEDCOM 7.0** file (PLAN §11.2). Heirloom uses the standard structures
wherever they exist. For the rest it uses a few **extension tags** starting with `_HLM_`, declared in the header as
GEDCOM 7 requires:

```
0 HEAD
1 SCHMA
2 TAG _HLM_KIND https://github.com/Husarp/Heirloom/blob/main/docs/GEDCOM_EXTENSIONS.md#_hlm_kind
```

Other programs keep or ignore these tags. Nothing important is **only** in an extension tag: a story is still a normal
note, a photo is still a normal `OBJE`.

## Standard structures Heirloom writes

| What | GEDCOM 7 |
|---|---|
| Person | `INDI` with `UID` (a UUID, never changes) |
| Names | `NAME Jan /Kowalski/` with `TYPE BIRTH` / `MARRIED` / `AKA` / `OTHER`, `GIVN`, `SURN`, `NICK`; the form written in a source goes to `TRAN` + `LANG` |
| Sex | `SEX M` / `F` / `X` / `U` |
| Died, date unknown | `DEAT Y` |
| Events | `BIRT`, `BAPM`, `DEAT`, `BURI`, `EMIG`, `EVEN` + `TYPE`; on the family `MARR`, `MARB` (banns), `DIV` |
| Facts with a value | `OCCU`, `EDUC`, `RELI`, `RESI`; military service is `EVEN` + `TYPE Służba wojskowa` |
| Dates | `DATE` in GEDCOM 7 syntax; the wording in the source goes to `PHRASE` |
| Places | `PLAC` as written, from the smallest to the largest (`Wólka, Łęczna, lubelskie`); a note about the place goes to `PLAC.NOTE` |
| Other people in an event | `ASSO @I2@` + `ROLE` (`FATH`, `MOTH`, `GODP`, `WITN`, `OFFICIATOR`, `SPOU`, `OTHER`) |
| Families | `FAM` with `HUSB`, `WIFE`, `CHIL`; a child's link type is `FAMC.PEDI` (`BIRTH`, `ADOPTED`, `FOSTER`, `OTHER`) |
| Texts (biography, stories, sayings, trivia, notes) | shared notes: `0 @N1@ SNOTE <text>`, linked from each person it belongs to with `1 SNOTE @N1@` |
| Photos and documents | `0 @O1@ OBJE` with `FILE media/<name>`, `FORM <media type>`, `MEDI PHOTO` (photos) or `MEDI MANUSCRIPT` + `PHRASE <document type>` (documents), `TITL <caption>`; linked from people with `1 OBJE @O1@` |
| Profile photo crop | `OBJE.CROP` (`TOP`, `LEFT`, `HEIGHT`, `WIDTH`) |
| Sources | `0 @S1@ SOUR` with `TITL`, `REPO @R1@` + `CALN` (archive and call number), `OBJE` (the scans); cited with `SOUR @S1@` + `PAGE` + `QUAY` |
| Created / changed | `CREA` and `CHAN` with `DATE` + `TIME` |

## Extension tags

| Tag | Where | Meaning |
|---|---|---|
| `_HLM_KIND` | `SNOTE` | The kind of text: `summary` (W skrócie), `bio` (one section of the biography), `story`, `saying`, `trivia`, `note` (research note). A note without it is an ordinary note. |
| `_HLM_TITLE` | `SNOTE` | The title of a biography section or story. The table of contents is built from the `bio` titles, in the order the person links them. |
| `_HLM_DATE` | `SNOTE` | When a story happened (GEDCOM 7 date syntax). |
| `_HLM_PLAC` | `SNOTE` | Where a story happened. |
| `_HLM_CERT` | events, `SNOTE`, `FAMC`, `FAM` | Certainty: `high` (pewne), `medium` (prawdopodobne), `low` (niepewne). |
| `_HLM_BASIS` | events, `SNOTE`, `FAMC`, `FAM` | `stated` (written in the source) or `inferred` (a conclusion drawn from it). |
| `_HLM_JULIAN` | `DATE` | The Julian date of a double-dated record, e.g. `28 FEB 1878` next to `DATE 12 MAR 1878`. |
| `_HLM_AGE` | `ASSO`, events | A person's age exactly as written in the source (`lat 30`). |
| `_HLM_ORIG` | `PLAC` | The place exactly as written in the source. |
| `_HLM_TAG` | `INDI` | A short label, e.g. `kolejarz` (one tag per line). |
| `_HLM_LINK` | `INDI` | A web link. Substructures: `TITL` (the title) and `TYPE` (`wikipedia`, `geneteka`, `familysearch`, `grave`, `other`). |
| `_HLM_PROFILE` | `INDI.OBJE` | `Y` on the person's profile photo. Without it, the first photo is used. |
| `_HLM_DATE` | `OBJE` | When a photo was taken or a document was made. |
| `_HLM_PLAC` | `OBJE` | Where a photo was taken or a document was made. |
| `_HLM_TRANSCRIPTION` | `OBJE` | A transcription of a document, as written. |
| `_HLM_TRANSLATION` | `OBJE` | A Polish translation of a document. |
| `_HLM_KIND` | `SOUR` | The kind of source: `parish_record`, `civil_record`, `index`, `photo`, `letter`, `oral`, `note`, `book`, `website`, `other`. |
| `_HLM_PARISH` | `SOUR` | The parish of a register. |
| `_HLM_YEAR` | `SOUR` | The year of a register. |
| `_HLM_AKT` | `SOUR` | The record (akt) number. |
| `_HLM_URL` | `SOUR` | A web address of the source. |
| `_HLM_BATCH` | records | The import batch that created the record (for "Cofnij import"). |

## Mentions inside texts

A text mentions a person as `[visible text](person:<UID>)`, e.g. `syn [Antoniego](person:5f0c…)`. The link uses the
person's `UID`, so it survives when another program renumbers the records. Other programs show the text with the
link as it is.
