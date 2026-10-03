//! The cache: a SQLite copy of the archive's people and families for fast lists and search (PLAN.md §5.4).
//! It lives outside the archive (`%LOCALAPPDATA%\Heirloom\cache\<archive id>\`), is never synced, and is
//! rebuilt whenever the data file changes — deleting it loses nothing.

use crate::Result;
use crate::fold::fold;
use crate::gedcom::model::{Event, Model, Person};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "2";

const SCHEMA: &str = "
DROP TABLE IF EXISTS person_fts;
DROP TABLE IF EXISTS family_child;
DROP TABLE IF EXISTS family;
DROP TABLE IF EXISTS name;
DROP TABLE IF EXISTS person;
-- xref is not UNIQUE: a broken file can repeat one, and that must not stop the whole cache from building.
CREATE TABLE person(
  id INTEGER PRIMARY KEY, xref TEXT NOT NULL, uid TEXT, sex TEXT NOT NULL,
  given TEXT NOT NULL, surname TEXT NOT NULL, maiden TEXT, display TEXT NOT NULL,
  birth_date TEXT, birth_sort INTEGER, birth_place TEXT,
  death_date TEXT, death_sort INTEGER, death_place TEXT);
CREATE TABLE name(person_id INTEGER NOT NULL, kind TEXT NOT NULL, given TEXT NOT NULL, surname TEXT NOT NULL);
CREATE TABLE family(
  id INTEGER PRIMARY KEY, xref TEXT NOT NULL, partner1 INTEGER, partner2 INTEGER,
  marriage_date TEXT, marriage_sort INTEGER, marriage_place TEXT);
CREATE TABLE family_child(family_id INTEGER NOT NULL, child_id INTEGER NOT NULL, ord INTEGER NOT NULL);
CREATE INDEX person_xref ON person(xref);
CREATE INDEX family_xref ON family(xref);
CREATE INDEX name_person ON name(person_id);
CREATE INDEX family_child_child ON family_child(child_id);
CREATE INDEX person_surname ON person(surname);
CREATE INDEX person_birth ON person(birth_sort);
-- Contentless: the text is already folded (ł → l …), only person ids come back.
CREATE VIRTUAL TABLE person_fts USING fts5(text, content='', tokenize='unicode61 remove_diacritics 2', prefix='2 3');
";

pub struct Cache {
    conn: Connection,
}

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonHit {
    pub xref: String,
    pub display: String,
    pub maiden: Option<String>,
    pub birth: Option<String>,
    pub death: Option<String>,
}

impl Cache {
    /// `%LOCALAPPDATA%\Heirloom\cache`.
    pub fn default_root() -> Option<PathBuf> {
        Some(dirs::data_local_dir()?.join("Heirloom").join("cache"))
    }

    /// Opens (or creates) the cache of one archive under `root`.
    pub fn open(root: &Path, archive_id: &str) -> Result<Cache> {
        let dir = root.join(archive_id);
        std::fs::create_dir_all(&dir)?;
        let conn = Connection::open(dir.join("cache.db"))?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        Ok(Cache { conn })
    }

    /// The cache was built from exactly this version of the data file.
    pub fn is_fresh(&self, fingerprint: &str) -> Result<bool> {
        Ok(self.meta("fingerprint")?.as_deref() == Some(fingerprint)
            && self.meta("schema")?.as_deref() == Some(SCHEMA_VERSION))
    }

    /// Replaces the cache contents with `model`, in one transaction.
    pub fn rebuild(&mut self, model: &Model, fingerprint: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(SCHEMA)?;
        let mut ids: HashMap<&str, i64> = HashMap::with_capacity(model.persons.len());
        {
            let mut person = tx.prepare(
                "INSERT INTO person(id, xref, uid, sex, given, surname, maiden, display, birth_date, birth_sort,
                 birth_place, death_date, death_sort, death_place) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            )?;
            let mut name = tx.prepare("INSERT INTO name(person_id, kind, given, surname) VALUES (?1, ?2, ?3, ?4)")?;
            let mut fts = tx.prepare("INSERT INTO person_fts(rowid, text) VALUES (?1, ?2)")?;
            for (i, p) in model.persons.iter().enumerate() {
                let id = i64::try_from(i + 1).unwrap_or(i64::MAX);
                ids.insert(p.xref.as_str(), id);
                let (bd, bs, bp) = event_columns(p.birth.as_ref());
                let (dd, ds, dp) = event_columns(p.death.as_ref());
                person.execute(params![
                    id, p.xref, p.uid, p.sex.code(), p.given(), p.surname(), p.maiden_name(), p.display_name(),
                    bd, bs, bp, dd, ds, dp
                ])?;
                for n in &p.names {
                    name.execute(params![id, n.kind.code(), n.given, n.surname])?;
                }
                fts.execute(params![id, search_text(p)])?;
            }
        }
        {
            let mut family = tx.prepare(
                "INSERT INTO family(id, xref, partner1, partner2, marriage_date, marriage_sort, marriage_place)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            let mut child = tx.prepare("INSERT INTO family_child(family_id, child_id, ord) VALUES (?1, ?2, ?3)")?;
            for (i, f) in model.families.iter().enumerate() {
                let id = i64::try_from(i + 1).unwrap_or(i64::MAX);
                let partner = |k: usize| f.partners.get(k).and_then(|x| ids.get(x.as_str()).copied());
                let (md, ms, mp) = event_columns(f.marriage.as_ref());
                family.execute(params![id, f.xref, partner(0), partner(1), md, ms, mp])?;
                for (ord, c) in f.children.iter().enumerate() {
                    if let Some(child_id) = ids.get(c.as_str()) {
                        child.execute(params![id, child_id, i64::try_from(ord).unwrap_or(0)])?;
                    }
                }
            }
        }
        tx.execute("INSERT OR REPLACE INTO meta(key, value) VALUES ('fingerprint', ?1)", [fingerprint])?;
        tx.execute("INSERT OR REPLACE INTO meta(key, value) VALUES ('schema', ?1)", [SCHEMA_VERSION])?;
        tx.commit()?;
        Ok(())
    }

    /// People matching every word of `query` (word starts, any letters: "lukasz kow" finds "Łukasz Kowalski").
    /// Maiden names, married names, nicknames and birth and death places are searched too.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<PersonHit>> {
        let folded = fold(query);
        let words: Vec<String> =
            folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| format!("\"{w}\"*")).collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT p.xref, p.display, p.maiden, p.birth_date, p.death_date FROM person_fts
             JOIN person p ON p.id = person_fts.rowid WHERE person_fts MATCH ?1 ORDER BY rank LIMIT ?2",
        )?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let hits = stmt
            .query_map(params![words.join(" "), limit], |r| {
                Ok(PersonHit { xref: r.get(0)?, display: r.get(1)?, maiden: r.get(2)?, birth: r.get(3)?, death: r.get(4)? })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }

    pub fn person_count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT count(*) FROM person", [], |r| r.get(0))?)
    }

    fn meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }
}

fn event_columns(event: Option<&Event>) -> (Option<&str>, Option<i64>, Option<&str>) {
    match event {
        Some(e) => (e.date_text.as_deref(), e.date.map(|d| d.sort_key()), e.place.as_deref()),
        None => (None, None, None),
    }
}

fn search_text(p: &Person) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for n in &p.names {
        parts.push(&n.given);
        parts.push(&n.surname);
        if let Some(nick) = &n.nickname {
            parts.push(nick);
        }
    }
    for e in [&p.birth, &p.death].into_iter().flatten() {
        if let Some(place) = &e.place {
            parts.push(place);
        }
    }
    fold(&parts.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gedcom::{Document, model};

    const FILE: &str = "0 HEAD\n1 GEDC\n2 VERS 7.0\n\
0 @I1@ INDI\n1 NAME Łukasz /Wiśniewski/\n1 SEX M\n1 BIRT\n2 DATE 1901\n2 PLAC Żółkiewka\n1 FAMS @F1@\n\
0 @I2@ INDI\n1 NAME Marianna /Nowak/\n2 TYPE BIRTH\n1 NAME Marianna /Wiśniewska/\n2 TYPE MARRIED\n2 NICK Mania\n1 SEX F\n1 FAMS @F1@\n\
0 @I3@ INDI\n1 NAME Józef /Wiśniewski/\n1 SEX M\n1 FAMC @F1@\n\
0 @F1@ FAM\n1 HUSB @I1@\n1 WIFE @I2@\n1 CHIL @I3@\n0 TRLR\n";

    fn cache(dir: &Path) -> Cache {
        let (doc, _) = Document::from_bytes(FILE.as_bytes());
        let mut cache = Cache::open(dir, "test-archive").unwrap();
        assert!(!cache.is_fresh("abc").unwrap());
        cache.rebuild(&model::extract(&doc), "abc").unwrap();
        cache
    }

    fn found(cache: &Cache, query: &str) -> Vec<String> {
        cache.search(query, 10).unwrap().into_iter().map(|h| h.xref).collect()
    }

    #[test]
    fn search_ignores_polish_letters_and_finds_maiden_names() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path());
        assert!(cache.is_fresh("abc").unwrap());
        assert_eq!(cache.person_count().unwrap(), 3);
        assert_eq!(found(&cache, "lukasz"), ["@I1@"], "ł folds to l");
        assert_eq!(found(&cache, "ŁUKASZ wiśn"), ["@I1@"]);
        assert_eq!(found(&cache, "nowak"), ["@I2@"], "maiden name");
        assert_eq!(found(&cache, "mania"), ["@I2@"], "nickname");
        assert_eq!(found(&cache, "zolkiew"), ["@I1@"], "birth place");
        assert_eq!(found(&cache, "wis").len(), 3, "prefix search");
        assert!(found(&cache, "kowalski").is_empty());
        assert!(found(&cache, "  ").is_empty());
    }

    #[test]
    fn search_text_never_breaks_the_match_syntax() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path());
        for query in ["\"", "\"lukasz", "(", "NOT lukasz", "NEAR(a b)", "text:lukasz", "^luk", "-luk", "*", "a\0b", "𞤀", "lukasz 𞤀"] {
            assert!(cache.search(query, 10).is_ok(), "{query:?}");
        }
        assert_eq!(found(&cache, "\"lukasz"), ["@I1@"]);
        assert_eq!(found(&cache, "OR"), Vec::<String>::new(), "operators are searched as words");
    }

    #[test]
    fn a_repeated_xref_does_not_stop_the_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let text = "0 HEAD\n0 @I1@ INDI\n1 NAME Jan /Nowak/\n0 @I1@ INDI\n1 NAME Anna /Nowak/\n\
0 @F1@ FAM\n1 CHIL @I1@\n0 @F1@ FAM\n0 TRLR\n";
        let (doc, _) = Document::from_bytes(text.as_bytes());
        let mut cache = Cache::open(dir.path(), "test-archive").unwrap();
        cache.rebuild(&model::extract(&doc), "abc").unwrap();
        assert_eq!(found(&cache, "nowak").len(), 2, "both people are still searchable");
    }

    #[test]
    fn stores_families_and_display_fields() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(dir.path());
        let hit = &cache.search("marianna", 1).unwrap()[0];
        assert_eq!((hit.display.as_str(), hit.maiden.as_deref()), ("Marianna Wiśniewska", Some("Nowak")));
        let (p1, p2, children): (i64, i64, i64) = cache
            .conn
            .query_row(
                "SELECT partner1, partner2, (SELECT count(*) FROM family_child WHERE family_id = f.id) FROM family f",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((p1, p2, children), (1, 2, 1));
    }
}
