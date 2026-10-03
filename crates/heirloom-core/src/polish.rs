//! Polish forms: dates as people read and type them, and surnames (gender and marital forms, family plurals).
//! The file format stays GEDCOM; these only turn it into Polish and back.

use crate::fold::fold;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::gedcom::date::{self, Calendar, DateValue, Qualifier, Ymd};

const MONTHS_GENITIVE: [&str; 12] =
    ["stycznia", "lutego", "marca", "kwietnia", "maja", "czerwca", "lipca", "sierpnia", "września", "października", "listopada", "grudnia"];
const MONTHS_NOMINATIVE: [&str; 12] =
    ["styczeń", "luty", "marzec", "kwiecień", "maj", "czerwiec", "lipiec", "sierpień", "wrzesień", "październik", "listopad", "grudzień"];
const ROMAN: [&str; 12] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII"];

fn year(y: i32) -> String {
    if y <= 0 { format!("{} p.n.e.", 1 - y) } else { y.to_string() }
}

/// "12 marca 1878", "marzec 1878", "1878".
fn ymd_long(d: Ymd) -> String {
    match (d.day, d.month) {
        (Some(day), Some(m)) if (1..=12).contains(&m) => format!("{day} {} {}", MONTHS_GENITIVE[m as usize - 1], year(d.year)),
        (_, Some(m)) if (1..=12).contains(&m) => format!("{} {}", MONTHS_NOMINATIVE[m as usize - 1], year(d.year)),
        _ => year(d.year),
    }
}

/// "12.03.1878", "03.1878", "1878".
fn ymd_short(d: Ymd) -> String {
    match (d.day, d.month) {
        (Some(day), Some(m)) => format!("{day:02}.{m:02}.{}", year(d.year)),
        (_, Some(m)) => format!("{m:02}.{}", year(d.year)),
        _ => year(d.year),
    }
}

fn with_qualifier(d: &DateValue, part: fn(Ymd) -> String) -> String {
    let start = part(d.start);
    let end = d.end.map(part).unwrap_or_default();
    let text = match d.qualifier {
        Qualifier::Exact | Qualifier::Interpreted => start,
        Qualifier::About | Qualifier::Estimated => format!("ok. {start}"),
        Qualifier::Calculated => format!("wyliczone: {start}"),
        Qualifier::Before => format!("przed {start}"),
        Qualifier::After => format!("po {start}"),
        Qualifier::Between => format!("między {start} a {end}"),
        Qualifier::From => format!("od {start}"),
        Qualifier::To => format!("do {start}"),
        Qualifier::FromTo => format!("od {start} do {end}"),
    };
    match d.calendar {
        Calendar::Gregorian => text,
        Calendar::Julian => format!("{text} (kal. juliański)"),
        Calendar::Hebrew => format!("{text} (kal. żydowski)"),
        Calendar::French => format!("{text} (kal. republikański)"),
    }
}

/// The long form for reading: "12 marca 1878", "ok. 1850", "między 1850 a 1855".
pub fn format_date(d: &DateValue) -> String {
    if NUMERIC_DATES.load(Ordering::Relaxed) {
        return format_date_short(d);
    }
    with_qualifier(d, ymd_long)
}

/// Ustawienia › Osoby i daty › Format daty: „12.03.1878” everywhere instead of „12 marca 1878”. One archive is open
/// at a time, so this is set for the whole program when its settings are read.
static NUMERIC_DATES: AtomicBool = AtomicBool::new(false);

pub fn set_numeric_dates(on: bool) {
    NUMERIC_DATES.store(on, Ordering::Relaxed);
}

/// The short form for tables: "12.03.1878", "03.1878", "przed 1910".
pub fn format_date_short(d: &DateValue) -> String {
    with_qualifier(d, ymd_short)
}

/// The year with its qualifier, for tree cards and lists: "1878", "ok. 1850", "przed 1910", "1850/1855".
pub fn year_text(d: &DateValue) -> String {
    let start = year(d.start.year);
    match d.qualifier {
        Qualifier::Exact | Qualifier::Interpreted | Qualifier::From => start,
        Qualifier::About | Qualifier::Estimated | Qualifier::Calculated => format!("ok. {start}"),
        Qualifier::Before | Qualifier::To => format!("przed {start}"),
        Qualifier::After => format!("po {start}"),
        Qualifier::Between | Qualifier::FromTo => match d.end {
            Some(end) if end.year != d.start.year => format!("{start}/{}", year(end.year)),
            _ => start,
        },
    }
}

/// Dates that are only approximate get a dotted underline in the interface.
pub fn is_uncertain(d: &DateValue) -> bool {
    matches!(
        d.qualifier,
        Qualifier::About | Qualifier::Estimated | Qualifier::Calculated | Qualifier::Before | Qualifier::After | Qualifier::Between
    )
}

/// Reads a date typed in Polish (or already in GEDCOM form) and returns it in GEDCOM 7 syntax:
/// "12.03.1878" → "12 MAR 1878", "03.1878" → "MAR 1878", "ok. 1850" → "ABT 1850", "12 III 1878",
/// "przed 1900", "po 1900", "między 1850 a 1855", "od 1905 do 1912", "12 marca 1878".
pub fn parse_date_input(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    // Already GEDCOM ("ABT 1850", "12 MAR 1878", "INT 1850 (około 1850)"), or a bare year: kept as written (upper
    // case, a 5.5.1 calendar escape turned into its 7.0 name). Rewritten, it would lose what a Polish form can't say
    // (EST, INT with its phrase, "1750/51", the months of the Hebrew and French calendars).
    let (head, phrase) = text.split_at(text.find('(').unwrap_or(text.len()));
    let gedcom_like = head.chars().all(|c| c.is_ascii_alphanumeric() || " @#_/".contains(c));
    if gedcom_like && date::parse(text).is_some() {
        return Some(format!("{} {phrase}", crate::gedcom::calendar_names(&head.to_ascii_uppercase())).trim_end().to_string());
    }
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();
    let join = |w: &[&str]| w.join(" ");
    let (qualifier, rest): (Qualifier, String) = match words.first().copied()? {
        "ok." | "ok" | "około" | "okolo" | "ca" | "ca." | "~" => (Qualifier::About, join(&words[1..])),
        "szac." | "szacunkowo" => (Qualifier::Estimated, join(&words[1..])),
        "wyl." | "wyliczone" | "wyliczone:" => (Qualifier::Calculated, join(&words[1..])),
        "przed" => (Qualifier::Before, join(&words[1..])),
        "po" => (Qualifier::After, join(&words[1..])),
        "między" | "miedzy" => {
            let rest = &words[1..];
            let split = rest.iter().position(|w| *w == "a" || *w == "i")?;
            let (a, b) = (ymd_input(&join(&rest[..split]))?, ymd_input(&join(&rest[split + 1..]))?);
            return Some(format!("BET {} AND {}", ymd_gedcom(a, Calendar::Gregorian)?, ymd_gedcom(b, Calendar::Gregorian)?));
        }
        "od" => {
            let rest = &words[1..];
            if let Some(split) = rest.iter().position(|w| *w == "do") {
                let (a, b) = (ymd_input(&join(&rest[..split]))?, ymd_input(&join(&rest[split + 1..]))?);
                return Some(format!("FROM {} TO {}", ymd_gedcom(a, Calendar::Gregorian)?, ymd_gedcom(b, Calendar::Gregorian)?));
            }
            (Qualifier::From, join(rest))
        }
        "do" => (Qualifier::To, join(&words[1..])),
        _ => (Qualifier::Exact, lower.clone()),
    };
    let ymd = ymd_input(&rest)?;
    let value = DateValue { qualifier, start: ymd, end: None, calendar: Calendar::Gregorian };
    gedcom_value(&value)
}

/// One date without a qualifier: "12.03.1878", "1878-03-12", "12 marca 1878", "12 III 1878", "03.1878", "1878".
fn ymd_input(text: &str) -> Option<Ymd> {
    let text = text.trim().trim_end_matches(['r', '.']).trim_end_matches(" r").trim();
    let parts: Vec<&str> = text.split(['.', '-', '/', ' ']).filter(|p| !p.is_empty()).collect();
    let number = |p: &str| p.parse::<i32>().ok();
    let month = |p: &str| -> Option<u8> {
        if let Ok(m) = p.parse::<u8>() {
            return (1..=12).contains(&m).then_some(m);
        }
        let upper = p.to_uppercase();
        if let Some(i) = ROMAN.iter().position(|r| *r == upper) {
            return Some(i as u8 + 1);
        }
        let folded = fold(p);
        let known = |list: &[&str; 12]| list.iter().position(|m| fold(m) == folded);
        known(&MONTHS_GENITIVE).or_else(|| known(&MONTHS_NOMINATIVE)).map(|i| i as u8 + 1)
    };
    let ymd = match parts.as_slice() {
        [y] => Ymd { year: number(y)?, month: None, day: None },
        [y, m, d] if y.len() == 4 => Ymd { year: number(y)?, month: Some(month(m)?), day: Some(d.parse().ok()?) },
        [d, m, y] => Ymd { year: number(y)?, month: Some(month(m)?), day: Some(d.parse().ok()?) },
        [m, y] => Ymd { year: number(y)?, month: Some(month(m)?), day: None },
        _ => return None,
    };
    let valid_day = match (ymd.day, ymd.month) {
        (Some(d), Some(m)) => d >= 1 && d <= days_in_month(ymd.year, m),
        _ => true,
    };
    (valid_day && (1..=2200).contains(&ymd.year)).then_some(ymd)
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        _ => 31,
    }
}

/// "12 MAR 1878", or "3 COMP 8" in the French calendar; None when the calendar has no such month.
fn ymd_gedcom(d: Ymd, calendar: Calendar) -> Option<String> {
    let year = if d.year <= 0 { format!("{} BCE", 1 - i64::from(d.year)) } else { d.year.to_string() };
    Some(match (d.day, d.month) {
        (Some(day), Some(m)) => format!("{day} {} {year}", date::month_name(calendar, m)?),
        (_, Some(m)) => format!("{} {year}", date::month_name(calendar, m)?),
        _ => year,
    })
}

/// A date value written back in GEDCOM 7 syntax; None when a month doesn't exist in the date's calendar.
pub fn gedcom_value(d: &DateValue) -> Option<String> {
    let start = ymd_gedcom(d.start, d.calendar)?;
    let end = match d.end {
        Some(end) => ymd_gedcom(end, d.calendar)?,
        None => String::new(),
    };
    let calendar = match d.calendar {
        Calendar::Gregorian => "",
        Calendar::Julian => "JULIAN ",
        Calendar::Hebrew => "HEBREW ",
        Calendar::French => "FRENCH_R ",
    };
    Some(match d.qualifier {
        Qualifier::Exact | Qualifier::Interpreted => format!("{calendar}{start}"),
        Qualifier::About => format!("ABT {calendar}{start}"),
        Qualifier::Calculated => format!("CAL {calendar}{start}"),
        Qualifier::Estimated => format!("EST {calendar}{start}"),
        Qualifier::Before => format!("BEF {calendar}{start}"),
        Qualifier::After => format!("AFT {calendar}{start}"),
        Qualifier::Between => format!("BET {calendar}{start} AND {calendar}{end}"),
        Qualifier::From => format!("FROM {calendar}{start}"),
        Qualifier::To => format!("TO {calendar}{start}"),
        Qualifier::FromTo => format!("FROM {calendar}{start} TO {calendar}{end}"),
    })
}

/// The key that joins a surname's forms into one group: "Kowalska" and "Kowalski", "Nowakowa", "Nowakówna" and
/// "Nowak", "Kaczmarkowa" and "Kaczmarek" all get the same key. Letters are folded (ł → l).
pub fn surname_key(surname: &str) -> String {
    let mut s = fold(surname.trim());
    for (female, male) in [("dzka", "dzki"), ("cka", "cki"), ("ska", "ski")] {
        if let Some(stem) = s.strip_suffix(female) {
            return format!("{stem}{male}");
        }
    }
    for suffix in ["owna", "owa"] {
        if let Some(stem) = s.strip_suffix(suffix) {
            if stem.chars().count() >= 3 {
                s = stem.to_string();
                break;
            }
        }
    }
    // The mobile e of Kaczmarek → Kaczmarkowa, Stelmaszec → Stelmaszcowa.
    for (full, short) in [("ek", "k"), ("ec", "c")] {
        if let Some(stem) = s.strip_suffix(full) {
            if stem.chars().count() >= 3 && !stem.ends_with(['a', 'e', 'i', 'o', 'u', 'y']) {
                return format!("{stem}{short}");
            }
        }
    }
    s
}

/// The masculine (dictionary) form of a surname when it is recognisably a female form: "Kowalska" →
/// "Kowalski". Old marital forms (Nowakowa) are left alone: their base can't always be restored.
pub fn masculine_form(surname: &str) -> String {
    for (female, male) in [("dzka", "dzki"), ("cka", "cki"), ("ska", "ski")] {
        if let Some(stem) = surname.strip_suffix(female) {
            return format!("{stem}{male}");
        }
    }
    surname.to_string()
}

/// Names whose e (or ie) drops in a way the rule below can't see: Wróbel → Wróbl-, Wawrzyniec → Wawrzyńc-.
const MOBILE_E: [(&str, &str); 5] = [("Wróbel", "Wróbl"), ("Wawrzyniec", "Wawrzyńc"), ("Stępień", "Stępni"), ("Kozieł", "Kozł"), ("Orzeł", "Orł")];

fn drop_mobile_e(word: &str) -> Option<String> {
    if let Some((_, stem)) = MOBILE_E.iter().find(|(w, _)| *w == word) {
        return Some((*stem).to_string());
    }
    // Franciszek → Franciszk-, Paweł → Pawł-, Kaczmarek → Kaczmark-.
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n > 4 && chars[n - 2] == 'e' && matches!(chars[n - 1], 'k' | 'ł' | 'c') && !"aeiouyąęó".contains(chars[n - 3]) {
        let mut s: String = chars[..n - 2].iter().collect();
        s.push(chars[n - 1]);
        return Some(s);
    }
    None
}

/// "gałąź Kowalskich", "rodzina Nowaków": the family name in the genitive plural.
pub fn family_genitive(surname: &str) -> String {
    let s = masculine_form(surname.trim());
    for (single, genitive) in [("dzki", "dzkich"), ("cki", "ckich"), ("ski", "skich")] {
        if let Some(stem) = s.strip_suffix(single) {
            return format!("{stem}{genitive}");
        }
    }
    if let Some(stem) = drop_mobile_e(&s) {
        return format!("{stem}ów");
    }
    match s.chars().last() {
        Some('a') | Some('o') => format!("{}ów", &s[..s.len() - 1]),
        Some('y') | Some('i') | None => s,
        _ => format!("{s}ów"),
    }
}

/// A given name in the genitive: "syn Antoniego i Agnieszki", "Powiedzonko Józefa". Only the first name is
/// declined; unusual names are left as they are.
pub fn given_genitive(given: &str, female: bool) -> String {
    let name = given.split_whitespace().next().unwrap_or("");
    let lower = name.to_lowercase();
    if lower.ends_with('a') {
        let stem = &name[..name.len() - 1];
        // After a vowel the j goes: Maja → Mai.
        let vowel_j = lower.ends_with("ja") && lower[..lower.len() - 2].ends_with(|c: char| "aeiouyąęó".contains(c));
        return if vowel_j {
            format!("{}i", &name[..name.len() - 2])
        } else if ["ia", "ja", "ka", "ga", "la", "ea"].iter().any(|end| lower.ends_with(end)) {
            // Marii, Łucji, Agnieszki, Jadwigi, Anieli, Salomei.
            format!("{stem}i")
        } else {
            format!("{stem}y")
        };
    }
    if female {
        return name.to_string();
    }
    if lower.ends_with('i') {
        return format!("{name}ego");
    }
    if lower.ends_with('y') {
        return format!("{}ego", &name[..name.len() - 1]);
    }
    if let Some(stem) = drop_mobile_e(name) {
        return format!("{stem}a");
    }
    if lower.ends_with(|c: char| c.is_alphabetic() && !"aeiouyąęó".contains(c)) {
        return format!("{name}a");
    }
    name.to_string()
}

/// A given name in the instrumental: "Ślub z Marianną", "z Józefem".
pub fn given_instrumental(given: &str, female: bool) -> String {
    let name = given.split_whitespace().next().unwrap_or("");
    let lower = name.to_lowercase();
    if lower.ends_with('a') {
        return format!("{}ą", &name[..name.len() - 1]);
    }
    if female {
        return name.to_string();
    }
    if lower.ends_with('i') || lower.ends_with('y') {
        return format!("{name}m");
    }
    let base = drop_mobile_e(name).unwrap_or_else(|| name.to_string());
    if base.ends_with('k') || base.ends_with('g') {
        return format!("{base}iem");
    }
    if lower.ends_with(|c: char| c.is_alphabetic() && !"aeiouyąęó".contains(c)) {
        return format!("{base}em");
    }
    name.to_string()
}

/// A surname in the instrumental: "z Marianną Nowak", "z Józefem Kowalskim", "z Heleną Kowalską".
pub fn surname_instrumental(surname: &str, female: bool) -> String {
    let s = surname.trim();
    for (nominative, instrumental) in [("ska", "ską"), ("cka", "cką"), ("dzka", "dzką"), ("ski", "skim"), ("cki", "ckim"), ("dzki", "dzkim")] {
        if let Some(stem) = s.strip_suffix(nominative) {
            return format!("{stem}{instrumental}");
        }
    }
    if female || s.is_empty() {
        return s.to_string();
    }
    let base = drop_mobile_e(s).unwrap_or_else(|| s.to_string());
    if base.ends_with('k') || base.ends_with('g') {
        return format!("{base}iem");
    }
    if s.ends_with(|c: char| c.is_alphabetic() && !"aeiouyąęó".contains(c)) {
        return format!("{base}em");
    }
    s.to_string()
}

/// „z” or „ze” before a word: „ze Stanisławem”, „ze Szczepanem”, „ze Zbigniewem”, but „z Sabiną”, „z Józefem”.
pub fn z_or_ze(word: &str) -> &'static str {
    let mut letters = word.chars().flat_map(char::to_lowercase);
    match (letters.next(), letters.next()) {
        (Some(first), Some(second)) if "szśźż".contains(first) && !"aeiouyąęó".contains(second) => "ze",
        _ => "z",
    }
}

/// The family's name in the plural, for labels over the tree: "Kowalscy", "Zawadzcy", "Nowakowie",
/// "Kaczmarkowie", "Zarębowie".
pub fn family_plural(surname: &str) -> String {
    let s = masculine_form(surname.trim());
    if s.is_empty() {
        return s;
    }
    for (single, plural) in [("dzki", "dzcy"), ("cki", "ccy"), ("ski", "scy")] {
        if let Some(stem) = s.strip_suffix(single) {
            return format!("{stem}{plural}");
        }
    }
    if let Some((_, stem)) = MOBILE_E.iter().find(|(w, _)| *w == s) {
        return format!("{stem}owie");
    }
    let lower = s.to_lowercase();
    let vowel = |c: char| "aeiouyąęó".contains(c);
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 4 && (lower.ends_with("ek") || lower.ends_with("ec")) && !vowel(chars[chars.len() - 3]) {
        let stem: String = chars[..chars.len() - 2].iter().collect();
        return format!("{stem}{}owie", chars[chars.len() - 1]);
    }
    match lower.chars().last() {
        Some('a') | Some('o') => format!("{}owie", &s[..s.len() - 1]),
        Some('y') | Some('i') => format!("rodzina {s}"),
        _ => format!("{s}owie"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long(text: &str) -> String {
        format_date(&date::parse(text).unwrap())
    }

    #[test]
    fn dates_in_polish() {
        assert_eq!(long("12 MAR 1878"), "12 marca 1878");
        assert_eq!(long("MAR 1878"), "marzec 1878");
        assert_eq!(long("ABT 1850"), "ok. 1850");
        assert_eq!(long("BEF 1910"), "przed 1910");
        assert_eq!(long("BET 1850 AND 1855"), "między 1850 a 1855");
        assert_eq!(long("FROM 1905 TO 1912"), "od 1905 do 1912");
        assert_eq!(long("@#DJULIAN@ 28 FEB 1878"), "28 lutego 1878 (kal. juliański)");
        assert_eq!(format_date_short(&date::parse("12 MAR 1878").unwrap()), "12.03.1878");
        assert_eq!(format_date_short(&date::parse("3 MAR 1878").unwrap()), "03.03.1878");
        let y = |t: &str| year_text(&date::parse(t).unwrap());
        assert_eq!((y("12 MAR 1878"), y("ABT 1850"), y("BEF 1910"), y("BET 1850 AND 1855")), ("1878".into(), "ok. 1850".into(), "przed 1910".into(), "1850/1855".into()));
        assert!(is_uncertain(&date::parse("ABT 1850").unwrap()) && !is_uncertain(&date::parse("1878").unwrap()));
    }

    #[test]
    fn typed_dates_become_gedcom() {
        let p = |t: &str| parse_date_input(t);
        assert_eq!(p("12.03.1878").as_deref(), Some("12 MAR 1878"));
        assert_eq!(p("12 III 1878").as_deref(), Some("12 MAR 1878"));
        assert_eq!(p("12 marca 1878 r.").as_deref(), Some("12 MAR 1878"));
        assert_eq!(p("1878-03-12").as_deref(), Some("12 MAR 1878"));
        assert_eq!(p("03.1878").as_deref(), Some("MAR 1878"));
        assert_eq!(p("marzec 1878").as_deref(), Some("MAR 1878"));
        assert_eq!(p("1878").as_deref(), Some("1878"));
        assert_eq!(p("ok. 1850").as_deref(), Some("ABT 1850"));
        assert_eq!(p("przed 1900").as_deref(), Some("BEF 1900"));
        assert_eq!(p("po 05.1920").as_deref(), Some("AFT MAY 1920"));
        assert_eq!(p("między 1850 a 1855").as_deref(), Some("BET 1850 AND 1855"));
        assert_eq!(p("od 1905 do 1912").as_deref(), Some("FROM 1905 TO 1912"));
        assert_eq!(p("od 1905").as_deref(), Some("FROM 1905"));
        assert_eq!(p("ABT 1850").as_deref(), Some("ABT 1850"));
        assert_eq!(p("31.02.1878"), None, "no 31 February");
        assert_eq!(p("jutro"), None);
    }

    #[test]
    fn gedcom_dates_are_kept_as_written() {
        let p = |t: &str| parse_date_input(t);
        assert_eq!(p("HEBREW 5 NSN 5650").as_deref(), Some("HEBREW 5 NSN 5650"), "not turned into AUG");
        assert_eq!(p("FRENCH_R 3 COMP 2").as_deref(), Some("FRENCH_R 3 COMP 2"), "the 13th month used to panic");
        assert_eq!(p("@#DHEBREW@ 1 ELL 5650").as_deref(), Some("HEBREW 1 ELL 5650"), "5.5.1 escapes become 7.0 names");
        // What the reader understands but a Polish form can't say: the edit form hands these back unchanged.
        assert_eq!(p("EST 1850").as_deref(), Some("EST 1850"));
        assert_eq!(p("1750/51").as_deref(), Some("1750/51"));
        assert_eq!(p("44 BCE").as_deref(), Some("44 BCE"));
        assert_eq!(p("INT 1850 (około 1850)").as_deref(), Some("INT 1850 (około 1850)"));
        assert_eq!(p("abt 1850").as_deref(), Some("ABT 1850"), "other programs write lower case");
        assert_eq!(p("12 mar 1878").as_deref(), Some("12 MAR 1878"));
        assert_eq!(p("po 1900").as_deref(), Some("AFT 1900"), "Polish words are still read as Polish");
        let value = DateValue { qualifier: Qualifier::Exact, start: Ymd { year: 8, month: Some(13), day: Some(3) }, end: None, calendar: Calendar::French };
        assert_eq!(gedcom_value(&value).as_deref(), Some("FRENCH_R 3 COMP 8"));
        assert_eq!(gedcom_value(&DateValue { calendar: Calendar::Gregorian, ..value }), None, "no 13th month to write");
    }

    #[test]
    fn surname_forms_join_into_groups() {
        let k = surname_key;
        assert_eq!(k("Kowalska"), k("Kowalski"));
        assert_eq!(k("Zawadzka"), k("Zawadzki"));
        assert_eq!(k("Nowakowa"), k("Nowak"));
        assert_eq!(k("Nowakówna"), k("Nowak"));
        assert_eq!(k("Kaczmarkowa"), k("Kaczmarek"));
        assert_eq!(k("Wiśniewska"), k("wisniewski"));
        assert_ne!(k("Nowak"), k("Nowicki"));
        assert_eq!(k("Mazur"), "mazur");
    }

    #[test]
    fn names_in_other_cases() {
        assert_eq!(family_genitive("Kowalski"), "Kowalskich");
        assert_eq!(family_genitive("Kowalska"), "Kowalskich");
        assert_eq!(family_genitive("Nowak"), "Nowaków");
        assert_eq!(family_genitive("Kaczmarek"), "Kaczmarków");
        assert_eq!(family_genitive("Mazur"), "Mazurów");
        let g = |n: &str, f: bool| given_genitive(n, f);
        assert_eq!(
            [g("Józef", false), g("Antoni", false), g("Franciszek", false), g("Paweł", false), g("Jerzy", false)],
            ["Józefa", "Antoniego", "Franciszka", "Pawła", "Jerzego"]
        );
        assert_eq!([g("Agnieszka", true), g("Maria", true), g("Helena", true), g("Jadwiga", true)], ["Agnieszki", "Marii", "Heleny", "Jadwigi"]);
        assert_eq!(
            [g("Aniela", true), g("Urszula", true), g("Tekla", true), g("Salomea", true), g("Maja", true), g("Łucja", true), g("Józefa", true)],
            ["Anieli", "Urszuli", "Tekli", "Salomei", "Mai", "Łucji", "Józefy"]
        );
        let i = |n: &str, f: bool| given_instrumental(n, f);
        assert_eq!([i("Marianna", true), i("Józef", false), i("Antoni", false), i("Franciszek", false)], ["Marianną", "Józefem", "Antonim", "Franciszkiem"]);
        assert_eq!(surname_instrumental("Kowalska", true), "Kowalską");
        assert_eq!(surname_instrumental("Nowak", true), "Nowak");
        assert_eq!(surname_instrumental("Nowak", false), "Nowakiem");
        assert_eq!(surname_instrumental("Kowalski", false), "Kowalskim");
    }

    #[test]
    fn family_plurals() {
        let cases = [
            ("Kowalski", "Kowalscy"),
            ("Kowalska", "Kowalscy"),
            ("Zawadzki", "Zawadzcy"),
            ("Szczepański", "Szczepańscy"),
            ("Nowicki", "Nowiccy"),
            ("Nowak", "Nowakowie"),
            ("Mazur", "Mazurowie"),
            ("Wójcik", "Wójcikowie"),
            ("Kaczmarek", "Kaczmarkowie"),
            ("Zaręba", "Zarębowie"),
            ("Król", "Królowie"),
            ("Wróbel", "Wróblowie"),
            ("Stępień", "Stępniowie"),
            ("Kozieł", "Kozłowie"),
        ];
        for (single, plural) in cases {
            assert_eq!(family_plural(single), plural);
        }
        assert_eq!((family_genitive("Wróbel"), family_genitive("Stępień")), ("Wróblów".into(), "Stępniów".into()));
        assert_eq!(surname_instrumental("Stępień", false), "Stępniem");
        assert_eq!((given_genitive("Wawrzyniec", false), given_instrumental("Wawrzyniec", false)), ("Wawrzyńca".into(), "Wawrzyńcem".into()));
    }

    #[test]
    fn z_or_ze_before_a_name() {
        let with = |name: &str| format!("{} {name}", z_or_ze(name));
        assert_eq!([with("Stanisławem"), with("Szczepanem"), with("Zbigniewem"), with("Świętosławem")], ["ze Stanisławem", "ze Szczepanem", "ze Zbigniewem", "ze Świętosławem"]);
        assert_eq!([with("Sabiną"), with("Zofią"), with("Józefem"), with("Marianną")], ["z Sabiną", "z Zofią", "z Józefem", "z Marianną"]);
    }
}
