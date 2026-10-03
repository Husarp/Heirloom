//! GEDCOM dates (5.5.1 and 7.0): qualifiers such as ABT, BEF, AFT, BET…AND, FROM…TO, calendars, dual years
//! (1750/51) and BCE. Used for sorting and ages; the original text is always kept for display.
//! Polish input such as "ok. 1850" belongs to the editor (Phase 2), not to the file format.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualifier {
    Exact,
    About,
    Calculated,
    Estimated,
    Before,
    After,
    Between,
    From,
    To,
    FromTo,
    Interpreted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Calendar {
    Gregorian,
    Julian,
    Hebrew,
    French,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ymd {
    pub year: i32,
    pub month: Option<u8>,
    pub day: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateValue {
    pub qualifier: Qualifier,
    pub start: Ymd,
    /// The second date of BET…AND and FROM…TO.
    pub end: Option<Ymd>,
    pub calendar: Calendar,
}

impl DateValue {
    /// A number that sorts dates in time order: yyyymmdd × 10, plus 1 for "before", 9 for "after", else 5.
    /// Missing months and days count as 0, so "1878" sorts before "12 MAR 1878".
    pub fn sort_key(&self) -> i64 {
        let d = self.start;
        let base = i64::from(d.year) * 10_000 + i64::from(d.month.unwrap_or(0)) * 100 + i64::from(d.day.unwrap_or(0));
        let adjust = match self.qualifier {
            Qualifier::Before | Qualifier::To => 1,
            Qualifier::After => 9,
            _ => 5,
        };
        base * 10 + adjust
    }
}

/// Parses a GEDCOM date value. Returns `None` for text that isn't a date (e.g. a free-text phrase).
pub fn parse(text: &str) -> Option<DateValue> {
    let mut s = text.trim().to_ascii_uppercase();
    let mut calendar = Calendar::Gregorian;
    // 5.5.1 calendar escapes: @#DJULIAN@, @#DFRENCH R@ …
    while let Some(start) = s.find("@#D") {
        let Some(len) = s[start + 3..].find('@') else { break };
        calendar = calendar_named(&s[start + 3..start + 3 + len]).unwrap_or(calendar);
        s.replace_range(start..start + 3 + len + 1, " ");
    }
    // A phrase in brackets (INT 1850 (około 1850), or a 5.5.1 date phrase) is not part of the date.
    if let Some(p) = s.find('(') {
        s.truncate(p);
    }
    let tokens: Vec<&str> = s.split_whitespace().collect();
    let (qualifier, rest) = match tokens.first().copied()? {
        "ABT" | "ABOUT" => (Qualifier::About, &tokens[1..]),
        "CAL" => (Qualifier::Calculated, &tokens[1..]),
        "EST" => (Qualifier::Estimated, &tokens[1..]),
        "BEF" => (Qualifier::Before, &tokens[1..]),
        "AFT" => (Qualifier::After, &tokens[1..]),
        "BET" => (Qualifier::Between, &tokens[1..]),
        "FROM" => (Qualifier::From, &tokens[1..]),
        "TO" => (Qualifier::To, &tokens[1..]),
        "INT" => (Qualifier::Interpreted, &tokens[1..]),
        _ => (Qualifier::Exact, &tokens[..]),
    };
    let split_at = match qualifier {
        Qualifier::Between => Some(rest.iter().position(|t| *t == "AND")?),
        Qualifier::From => rest.iter().position(|t| *t == "TO"),
        _ => None,
    };
    match split_at {
        Some(i) => {
            let start = simple(&rest[..i], &mut calendar)?;
            let end = simple(&rest[i + 1..], &mut calendar);
            let qualifier = if qualifier == Qualifier::From { Qualifier::FromTo } else { qualifier };
            Some(DateValue { qualifier, start, end, calendar })
        }
        None => Some(DateValue { qualifier, start: simple(rest, &mut calendar)?, end: None, calendar }),
    }
}

/// `[calendar] [[day] month] year [epoch]`
fn simple(tokens: &[&str], calendar: &mut Calendar) -> Option<Ymd> {
    let mut t = tokens;
    if let Some(c) = t.first().and_then(|w| calendar_named(w)) {
        *calendar = c;
        t = &t[1..];
    }
    let mut bce = false;
    if let Some(last) = t.last()
        && matches!(*last, "BCE" | "BC" | "B.C.")
    {
        bce = true;
        t = &t[..t.len() - 1];
    }
    let (year_token, head) = t.split_last()?;
    let mut year = parse_year(year_token)?;
    if bce {
        year = 1 - year;
    }
    let (month, day) = match head {
        [] => (None, None),
        [m] => (Some(month_number(m)?), None),
        [d, m] => (Some(month_number(m)?), Some(d.parse::<u8>().ok().filter(|d| (1..=31).contains(d))?)),
        _ => return None,
    };
    Some(Ymd { year, month, day })
}

/// "1878", or a dual year "1750/51" (the later year, which is how it is counted today). At most five digits: ages
/// and gaps between absurd years would overflow.
fn parse_year(token: &str) -> Option<i32> {
    const MAX: i32 = 99_999;
    match token.split_once('/') {
        Some((a, b)) => {
            let year: i32 = a.parse().ok().filter(|y: &i32| (1..=MAX).contains(y))?;
            let suffix: i32 = b.parse().ok()?;
            let unit = 10i32.checked_pow(u32::try_from(b.len()).ok()?)?;
            // Checked: a nonsense year such as "2147483647/99" must not overflow (a panic in debug builds).
            let mut later = (year - year % unit).checked_add(suffix)?;
            if later < year {
                later = later.checked_add(unit)?;
            }
            Some(later).filter(|y| *y <= MAX)
        }
        None => token.parse().ok().filter(|y: &i32| (1..=MAX).contains(y)),
    }
}

const GREGORIAN: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const FRENCH: [&str; 13] = ["VEND", "BRUM", "FRIM", "NIVO", "PLUV", "VENT", "GERM", "FLOR", "PRAI", "MESS", "THER", "FRUC", "COMP"];
const HEBREW: [&str; 13] = ["TSH", "CSH", "KSL", "TVT", "SHV", "ADR", "ADS", "NSN", "IYR", "SVN", "TMZ", "AAV", "ELL"];

fn month_number(token: &str) -> Option<u8> {
    [&GREGORIAN[..], &FRENCH[..], &HEBREW[..]]
        .iter()
        .find_map(|list| list.iter().position(|m| *m == token))
        .and_then(|i| u8::try_from(i + 1).ok())
}

/// A month as GEDCOM writes it in its calendar (`MAR`, `VEND`, `NSN`); None when the calendar has no such month.
pub fn month_name(calendar: Calendar, month: u8) -> Option<&'static str> {
    let names: &[&'static str] = match calendar {
        Calendar::Gregorian | Calendar::Julian => &GREGORIAN,
        Calendar::French => &FRENCH,
        Calendar::Hebrew => &HEBREW,
    };
    names.get(usize::from(month).checked_sub(1)?).copied()
}

fn calendar_named(name: &str) -> Option<Calendar> {
    match name {
        "GREGORIAN" => Some(Calendar::Gregorian),
        "JULIAN" => Some(Calendar::Julian),
        "HEBREW" => Some(Calendar::Hebrew),
        "FRENCH_R" | "FRENCH R" => Some(Calendar::French),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> i64 {
        parse(text).unwrap_or_else(|| panic!("{text} should parse")).sort_key()
    }

    #[test]
    fn exact_and_partial_dates() {
        let d = parse("12 MAR 1878").unwrap();
        assert_eq!((d.qualifier, d.start), (Qualifier::Exact, Ymd { year: 1878, month: Some(3), day: Some(12) }));
        assert_eq!(key("12 MAR 1878"), 187_803_125);
        assert!(key("1878") < key("MAR 1878") && key("MAR 1878") < key("12 MAR 1878"));
    }

    #[test]
    fn qualifiers_and_ranges() {
        assert_eq!(parse("ABT 1850").unwrap().qualifier, Qualifier::About);
        let bet = parse("BET 1850 AND 1855").unwrap();
        assert_eq!((bet.qualifier, bet.end.map(|e| e.year)), (Qualifier::Between, Some(1855)));
        let span = parse("FROM 1905 TO 1912").unwrap();
        assert_eq!((span.qualifier, span.start.year, span.end.map(|e| e.year)), (Qualifier::FromTo, 1905, Some(1912)));
        assert_eq!(parse("FROM 1905").unwrap().qualifier, Qualifier::From);
        assert!(key("BEF 1900") < key("1900") && key("1900") < key("AFT 1900"));
        assert_eq!(parse("INT 1850 (około 1850)").unwrap().qualifier, Qualifier::Interpreted);
    }

    #[test]
    fn calendars_dual_years_and_epochs() {
        assert_eq!(parse("JULIAN 28 FEB 1878").unwrap().calendar, Calendar::Julian);
        assert_eq!(parse("@#DJULIAN@ 28 FEB 1878").unwrap().calendar, Calendar::Julian);
        assert_eq!(parse("1750/51").unwrap().start.year, 1751);
        assert_eq!(parse("1699/00").unwrap().start.year, 1700);
        assert_eq!(parse("44 BCE").unwrap().start.year, -43);
        assert_eq!(parse("abt mar 1850").unwrap().start.month, Some(3), "lower case is accepted");
    }

    #[test]
    fn non_dates() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("(zimą 1915)"), None);
        assert_eq!(parse("12 marca 1878"), None, "localized months are not GEDCOM");
        assert_eq!(parse("BET 1850"), None);
    }

    #[test]
    fn odd_years_are_rejected_without_panicking() {
        assert_eq!(parse("2147483647/99"), None, "used to overflow");
        assert_eq!(parse("2000000000/999999999"), None);
        assert_eq!(parse("-1750/51"), None, "negative years are not dual years");
        assert_eq!(parse("0/1"), None, "no year 0, as for plain years");
        assert_eq!(parse("32 JAN 1900"), None);
        assert_eq!(parse("99999 BCE").unwrap().start.year, -99_998);
        // Ages and gaps between such years overflowed (a panic in debug builds).
        assert_eq!(parse("2147483647"), None);
        assert_eq!(parse("2000000000 BCE"), None);
    }
}
