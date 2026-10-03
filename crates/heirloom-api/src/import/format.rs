//! The `heirloom-import` 1.0 file (docs/IMPORT_FORMAT.md), read leniently: every field is optional here and the
//! checks report what is missing, so a small AI slip never loses the rest of the answer.

use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

/// Accepts `"S1"` as well as `["S1"]` (AI answers mix them up).
fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(match value {
        Value::String(s) => vec![s],
        Value::Array(items) => items.into_iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    })
}

/// Numbers where text was expected ("year": 1878) are read as text.
fn text_or_number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    let value = Value::deserialize(d)?;
    Ok(match value {
        Value::String(s) if !s.trim().is_empty() => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// Slips of a chat AI fixed before reading, so one of them never loses a whole part: `null` where a list or a text
/// should be, numbers written as text (`"part": "2"`), identifiers written as numbers (`"id": 5`), „tak” for true,
/// and the sex in Polish (`"K"`, „kobieta”).
pub fn normalize(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            for (key, v) in map.iter_mut() {
                match (key.as_str(), &*v) {
                    ("part", Value::String(s)) => {
                        if let Ok(n) = s.trim().parse::<u32>() {
                            *v = Value::from(n);
                        }
                    }
                    ("living" | "final", Value::String(s)) => match s.trim().to_lowercase().as_str() {
                        "true" | "tak" | "yes" => *v = Value::Bool(true),
                        "false" | "nie" | "no" => *v = Value::Bool(false),
                        _ => {}
                    },
                    ("id" | "p" | "parent" | "child" | "a" | "b" | "person" | "m", Value::Number(n)) => *v = Value::String(n.to_string()),
                    ("sex", Value::String(s)) => {
                        let code = match s.trim().to_lowercase().as_str() {
                            "k" | "kobieta" | "f" | "female" => Some("F"),
                            "m" | "mężczyzna" | "male" => Some("M"),
                            "u" | "nieznana" | "nieznany" | "unknown" => Some("U"),
                            _ => None,
                        };
                        if let Some(code) = code {
                            *v = Value::String(code.into());
                        }
                    }
                    _ => normalize(v),
                }
            }
        }
        Value::Array(items) => {
            items.retain(|v| !v.is_null());
            items.iter_mut().for_each(normalize);
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Part {
    pub format: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub version: Option<String>,
    pub prompt_rev: Option<String>,
    pub batch: Option<String>,
    pub part: Option<u32>,
    pub created: Option<String>,
    pub author: Option<String>,
    pub lang: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub manifest: Vec<String>,
    pub persons: Vec<Person>,
    pub events: Vec<Event>,
    pub relationships: Vec<Relationship>,
    pub texts: Vec<Text>,
    pub media: Vec<Media>,
    pub sources: Vec<Source>,
    pub links: Vec<Link>,
    pub coverage: Vec<Coverage>,
    pub questions: Vec<Question>,
    pub end: Option<End>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Person {
    pub id: String,
    pub names: Vec<Name>,
    pub sex: Option<String>,
    pub living: Option<bool>,
    pub summary: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub tags: Vec<String>,
    #[serde(rename = "match")]
    pub matching: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Name {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub given: Option<String>,
    pub surname: Option<String>,
    pub nickname: Option<String>,
    pub orig: Option<String>,
    pub lang: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub src: Vec<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Role {
    pub p: String,
    pub role: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub age_orig: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Event {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub type_other: Option<String>,
    pub people: Vec<Role>,
    #[serde(deserialize_with = "text_or_number")]
    pub date: Option<String>,
    pub date_orig: Option<String>,
    pub julian: Option<String>,
    pub place: Option<String>,
    pub place_orig: Option<String>,
    pub place_note: Option<String>,
    pub value: Option<String>,
    pub cause: Option<String>,
    pub note: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub src: Vec<String>,
    pub basis: Option<String>,
    pub certainty: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Relationship {
    #[serde(rename = "type")]
    pub relation: Option<String>,
    pub parent: Option<String>,
    pub child: Option<String>,
    pub a: Option<String>,
    pub b: Option<String>,
    pub kind: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub src: Vec<String>,
    pub basis: Option<String>,
    pub certainty: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Text {
    pub id: String,
    pub person: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub md: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub date: Option<String>,
    pub place: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub src: Vec<String>,
    pub basis: Option<String>,
    pub certainty: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Media {
    pub id: String,
    pub kind: Option<String>,
    pub document_type: Option<String>,
    pub caption: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub date: Option<String>,
    pub place: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub depicts: Vec<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub about: Vec<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub profile_for: Vec<String>,
    pub transcription: Option<String>,
    pub translation: Option<String>,
    pub note: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Source {
    pub id: String,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub parish: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub year: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub akt: Option<String>,
    pub archive: Option<String>,
    #[serde(deserialize_with = "text_or_number")]
    pub call_number: Option<String>,
    pub url: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    pub media: Vec<String>,
    pub note: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Link {
    pub person: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Coverage {
    pub m: Option<String>,
    pub status: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Question {
    pub id: String,
    #[serde(deserialize_with = "one_or_many")]
    pub about: Vec<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct End {
    pub part: Option<u32>,
    #[serde(rename = "final")]
    pub is_final: Option<bool>,
    pub counts: Map<String, Value>,
    pub marker: Option<String>,
}

pub const MARKER: &str = "END-HEIRLOOM-IMPORT";

pub const EVENT_TYPES: &[&str] = &[
    "birth", "baptism", "banns", "marriage", "divorce", "death", "burial", "residence", "occupation", "education", "military",
    "emigration", "religion", "other",
];

/// The dates of the format (`1878-03-12`, `ABT 1850`, `BET 1850 AND 1855`, `FROM 1905 TO 1912` …) in GEDCOM 7
/// syntax; None when the text doesn't follow the format.
pub fn gedcom_date(text: &str) -> Option<String> {
    const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
    let simple = |token: &str| -> Option<String> {
        let parts: Vec<&str> = token.split('-').collect();
        let year: i32 = parts.first()?.parse().ok().filter(|y| (1..3000).contains(y))?;
        match parts.as_slice() {
            [_] => Some(year.to_string()),
            [_, m] => {
                let m: usize = m.parse().ok().filter(|m| (1..=12).contains(m))?;
                Some(format!("{} {year}", MONTHS[m - 1]))
            }
            [_, m, d] => {
                let m: usize = m.parse().ok().filter(|m| (1..=12).contains(m))?;
                let d: u32 = d.parse().ok().filter(|d| (1..=31).contains(d))?;
                Some(format!("{d} {} {year}", MONTHS[m - 1]))
            }
            _ => None,
        }
    };
    let text = text.trim();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let out = match tokens.as_slice() {
        [d] => simple(d)?,
        [q @ ("ABT" | "EST" | "CAL" | "BEF" | "AFT" | "FROM" | "TO"), d] => format!("{q} {}", simple(d)?),
        ["BET", a, "AND", b] => format!("BET {} AND {}", simple(a)?, simple(b)?),
        ["FROM", a, "TO", b] => format!("FROM {} TO {}", simple(a)?, simple(b)?),
        _ => return None,
    };
    Some(out)
}

/// The other way round: a GEDCOM date (`12 MAR 1878`, `ABT 1850`, `BET 1850 AND 1855`) in this format's syntax, for a
/// date typed by hand while checking a batch.
pub fn from_gedcom(gedcom: &str) -> Option<String> {
    const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
    let month = |t: &str| MONTHS.iter().position(|m| *m == t).map(|k| k + 1);
    let tokens: Vec<&str> = gedcom.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        if ["ABT", "EST", "CAL", "BEF", "AFT", "FROM", "TO", "BET", "AND"].contains(&t) {
            out.push(t.to_string());
            i += 1;
            continue;
        }
        let (text, used) = match (t.parse::<u32>().ok(), tokens.get(i + 1).and_then(|m| month(m)), tokens.get(i + 2)) {
            (Some(day), Some(m), Some(year)) => (format!("{}-{m:02}-{day:02}", year.parse::<i32>().ok()?), 3),
            _ => match (month(t), tokens.get(i + 1)) {
                (Some(m), Some(year)) => (format!("{}-{m:02}", year.parse::<i32>().ok()?), 2),
                _ => (t.parse::<i32>().ok()?.to_string(), 1),
            },
        };
        out.push(text);
        i += used;
    }
    let text = out.join(" ");
    gedcom_date(&text).is_some().then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gedcom_dates_come_back_in_the_format() {
        assert_eq!(from_gedcom("12 MAR 1878").as_deref(), Some("1878-03-12"));
        assert_eq!(from_gedcom("ABT 1850").as_deref(), Some("ABT 1850"));
        assert_eq!(from_gedcom("BET MAR 1850 AND 1855").as_deref(), Some("BET 1850-03 AND 1855"));
        assert_eq!(from_gedcom("@#DJULIAN@ 1850"), None);
    }

    #[test]
    fn format_dates_become_gedcom() {
        assert_eq!(gedcom_date("1878-03-12").as_deref(), Some("12 MAR 1878"));
        assert_eq!(gedcom_date("1878-03").as_deref(), Some("MAR 1878"));
        assert_eq!(gedcom_date("1878").as_deref(), Some("1878"));
        assert_eq!(gedcom_date("ABT 1850").as_deref(), Some("ABT 1850"));
        assert_eq!(gedcom_date("BEF 1900-03").as_deref(), Some("BEF MAR 1900"));
        assert_eq!(gedcom_date("BET 1850 AND 1855").as_deref(), Some("BET 1850 AND 1855"));
        assert_eq!(gedcom_date("FROM 1905 TO 1912").as_deref(), Some("FROM 1905 TO 1912"));
        assert_eq!(gedcom_date("FROM 1905").as_deref(), Some("FROM 1905"));
        assert_eq!(gedcom_date("zimą 1915"), None);
        assert_eq!(gedcom_date("1878-13-01"), None);
    }

    #[test]
    fn lenient_fields() {
        let part: Part = serde_json::from_str(r#"{ "format": "heirloom-import", "version": 1.0, "persons": [ { "id": "P1", "names": [ { "type": "birth", "given": "Jan", "src": "S1" } ], "imie_ojca": "Wojciech" } ], "sources": [ { "id": "S1", "title": "Akt", "year": 1878 } ] }"#).unwrap();
        assert_eq!(part.version.as_deref(), Some("1.0"));
        assert_eq!(part.persons[0].names[0].src, ["S1"]);
        assert!(part.persons[0].extra.contains_key("imie_ojca"));
        assert_eq!(part.sources[0].year.as_deref(), Some("1878"));
    }
}
