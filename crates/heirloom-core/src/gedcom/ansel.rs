//! ANSEL, the character set of many older GEDCOM files (RESEARCH §2). Accents are separate bytes placed
//! *before* their letter: `E2 'o'` is "ó", `F1 'a'` is "ą"; Ł and ł have their own bytes.

use unicode_normalization::UnicodeNormalization;

/// Decodes ANSEL bytes. Returns the text (NFC) and how many bytes could not be read (shown as U+FFFD).
pub fn decode(bytes: &[u8]) -> (String, usize) {
    let mut out = String::with_capacity(bytes.len());
    let mut marks: Vec<char> = Vec::new();
    let mut unknown = 0;
    for &b in bytes {
        if b < 0x80 {
            if b < 0x20 && !marks.is_empty() {
                // An accent with no letter after it on its line (e.g. a line split between the two). Put after
                // the line break, it would start the next line and turn that GEDCOM line into plain text.
                unknown += marks.len();
                out.extend(marks.drain(..).map(|_| '\u{FFFD}'));
            }
            out.push(b as char);
            out.extend(marks.drain(..));
        } else if let Some(mark) = combining(b) {
            marks.push(mark);
        } else if let Some(c) = special(b) {
            out.push(c);
            out.extend(marks.drain(..));
        } else {
            unknown += 1;
            marks.clear();
            out.push('\u{FFFD}');
        }
    }
    out.extend(marks.drain(..));
    (out.nfc().collect(), unknown)
}

/// ANSEL combining marks → Unicode combining characters.
fn combining(b: u8) -> Option<char> {
    Some(match b {
        0xE0 => '\u{0309}', // hook above
        0xE1 => '\u{0300}', // grave
        0xE2 => '\u{0301}', // acute: ć ń ó ś ź
        0xE3 => '\u{0302}', // circumflex
        0xE4 => '\u{0303}', // tilde
        0xE5 => '\u{0304}', // macron
        0xE6 => '\u{0306}', // breve
        0xE7 => '\u{0307}', // dot above: ż
        0xE8 => '\u{0308}', // diaeresis
        0xE9 => '\u{030C}', // caron
        0xEA => '\u{030A}', // ring above
        0xEB => '\u{FE20}', // ligature, left half
        0xEC => '\u{FE21}', // ligature, right half
        0xED => '\u{0315}', // comma above right
        0xEE => '\u{030B}', // double acute
        0xEF => '\u{0310}', // candrabindu
        0xF0 => '\u{0327}', // cedilla
        0xF1 => '\u{0328}', // ogonek (right hook): ą ę
        0xF2 => '\u{0323}', // dot below
        0xF3 => '\u{0324}', // diaeresis below
        0xF4 => '\u{0325}', // ring below
        0xF5 => '\u{0333}', // double low line
        0xF6 => '\u{0332}', // low line
        0xF7 => '\u{0326}', // comma below (left hook)
        0xF8 => '\u{031C}', // right cedilla
        0xF9 => '\u{032E}', // breve below
        0xFA => '\u{FE22}', // double tilde, left half
        0xFB => '\u{FE23}', // double tilde, right half
        0xFE => '\u{0313}', // comma above
        _ => return None,
    })
}

/// ANSEL letters and symbols that are single bytes.
fn special(b: u8) -> Option<char> {
    Some(match b {
        0xA1 => 'Ł',
        0xA2 => 'Ø',
        0xA3 => 'Đ',
        0xA4 => 'Þ',
        0xA5 => 'Æ',
        0xA6 => 'Œ',
        0xA7 => 'ʹ',
        0xA8 => '·',
        0xA9 => '♭',
        0xAA => '®',
        0xAB => '±',
        0xAC => 'Ơ',
        0xAD => 'Ư',
        0xAE => 'ʼ',
        0xB0 => 'ʻ',
        0xB1 => 'ł',
        0xB2 => 'ø',
        0xB3 => 'đ',
        0xB4 => 'þ',
        0xB5 => 'æ',
        0xB6 => 'œ',
        0xB7 => 'ʺ',
        0xB8 => 'ı',
        0xB9 => '£',
        0xBA => 'ð',
        0xBC => 'ơ',
        0xBD => 'ư',
        0xBE => '□',
        0xBF => '■',
        0xC0 => '°',
        0xC1 => 'ℓ',
        0xC2 => '℗',
        0xC3 => '©',
        0xC4 => '♯',
        0xC5 => '¿',
        0xC6 => '¡',
        0xC7 | 0xCF => 'ß',
        0xC8 => '€',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_polish_letters() {
        // J(acute)ozef (L-stroke)(ogonek)aczy(acute)nski (dot)Z(acute)o(l-stroke)(acute)c
        let bytes = b"J\xE2ozef \xA1\xF1aczy\xE2nski \xE7Z\xE2o\xB1\xE2c";
        assert_eq!(decode(bytes), ("Józef Łączyński Żółć".to_string(), 0));
    }

    #[test]
    fn marks_unknown_bytes() {
        let (text, unknown) = decode(b"a\x81b");
        assert_eq!(text, "a\u{FFFD}b");
        assert_eq!(unknown, 1);
    }

    #[test]
    fn an_accent_before_a_line_break_stays_on_its_line() {
        // Before the fix the acute went after the "\n" and "0 @I2@ INDI" was no longer a record line.
        let (text, unknown) = decode(b"1 NOTE Wi\xE2\n0 @I2@ INDI\r\n1 NAME B\xE2\r\n");
        assert_eq!(text, "1 NOTE Wi\u{FFFD}\n0 @I2@ INDI\r\n1 NAME B\u{FFFD}\r\n");
        assert_eq!(unknown, 2);
    }
}
