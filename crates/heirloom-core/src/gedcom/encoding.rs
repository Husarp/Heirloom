//! Turning file bytes into text: UTF-8 (with or without BOM), UTF-16, ANSEL, and the Windows code pages of
//! older Polish (1250) and Western (1252) programs. Heirloom always writes UTF-8.

use super::ansel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Ansel,
    Windows1250,
    Windows1252,
}

impl Encoding {
    pub fn label(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf16Le | Encoding::Utf16Be => "UTF-16",
            Encoding::Ansel => "ANSEL",
            Encoding::Windows1250 => "Windows-1250",
            Encoding::Windows1252 => "Windows-1252",
        }
    }
}

pub struct Decoded {
    pub text: String,
    pub encoding: Encoding,
    /// The file started with a UTF-8 byte-order mark.
    pub bom: bool,
    pub warnings: Vec<String>,
}

/// Detects the encoding (byte-order mark, then valid UTF-8, then the header's CHAR line, then a guess
/// between the Polish and Western code pages) and decodes.
pub fn decode(bytes: &[u8]) -> Decoded {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return utf8(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return utf16(rest, Encoding::Utf16Le);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return utf16(rest, Encoding::Utf16Be);
    }
    if bytes.len() >= 2 && bytes[0] != 0 && bytes[1] == 0 {
        return utf16(bytes, Encoding::Utf16Le);
    }
    if bytes.len() >= 2 && bytes[0] == 0 && bytes[1] != 0 {
        return utf16(bytes, Encoding::Utf16Be);
    }
    // A file that is valid UTF-8 is UTF-8, whatever its header says (mislabelled files are common).
    if std::str::from_utf8(bytes).is_ok() {
        return utf8(bytes, false);
    }
    let declared = declared_charset(bytes).unwrap_or_default();
    if declared.contains("ANSEL") {
        let (text, unknown) = ansel::decode(bytes);
        let mut warnings = Vec::new();
        if unknown > 0 {
            warnings.push(format!("{unknown} znaków w kodowaniu ANSEL nie dało się odczytać (zastąpiono je znakiem �)."));
        }
        return Decoded { text, encoding: Encoding::Ansel, bom: false, warnings };
    }
    let encoding = if declared.contains("1250") { Encoding::Windows1250 } else { guess_code_page(bytes) };
    let codec = if encoding == Encoding::Windows1250 { encoding_rs::WINDOWS_1250 } else { encoding_rs::WINDOWS_1252 };
    let (text, _) = codec.decode_without_bom_handling(bytes);
    Decoded { text: text.into_owned(), encoding, bom: false, warnings: Vec::new() }
}

/// UTF-8 bytes, with a byte-order mark if asked.
pub fn encode_utf8(text: &str, bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    out.extend_from_slice(text.as_bytes());
    out
}

fn utf8(bytes: &[u8], bom: bool) -> Decoded {
    let text = String::from_utf8_lossy(bytes);
    let mut warnings = Vec::new();
    if matches!(text, std::borrow::Cow::Owned(_)) {
        warnings.push("Niektóre bajty nie są poprawnym UTF-8 — zastąpiono je znakiem �.".to_string());
    }
    Decoded { text: text.into_owned(), encoding: Encoding::Utf8, bom, warnings }
}

fn utf16(bytes: &[u8], encoding: Encoding) -> Decoded {
    let codec = if encoding == Encoding::Utf16Le { encoding_rs::UTF_16LE } else { encoding_rs::UTF_16BE };
    let (text, had_errors) = codec.decode_without_bom_handling(bytes);
    let mut warnings = Vec::new();
    if had_errors {
        warnings.push("Niektóre znaki UTF-16 są uszkodzone — zastąpiono je znakiem �.".to_string());
    }
    Decoded { text: text.into_owned(), encoding, bom: false, warnings }
}

/// The value of `1 CHAR …` in the header, upper-cased.
fn declared_charset(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]);
    for line in head.split(['\r', '\n']).map(str::trim) {
        if let Some(rest) = line.strip_prefix("1 CHAR ") {
            return Some(rest.trim().to_ascii_uppercase());
        }
        if line.starts_with("0 ") && !line.contains("HEAD") {
            break;
        }
    }
    None
}

/// Windows-1250 (Polish) or Windows-1252 (Western)? Picks the one in which the letters make sense.
fn guess_code_page(bytes: &[u8]) -> Encoding {
    let (as_1250, _) = encoding_rs::WINDOWS_1250.decode_without_bom_handling(bytes);
    let (as_1252, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
    let polish = as_1250.chars().filter(|c| "ąćęłńśźżĄĆĘŁŃŚŹŻ".contains(*c)).count();
    let western = as_1252.chars().filter(|c| "àâäçèéêëîïôöùûüÿñæœßÀÂÄÇÈÉÊËÎÏÔÖÙÛÜŸÑÆŒ".contains(*c)).count();
    if polish > 0 && polish >= western { Encoding::Windows1250 } else { Encoding::Windows1252 }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLISH: &str = "0 HEAD\r\n1 CHAR ANSI\r\n0 @I1@ INDI\r\n1 NAME Józef Łączyński /Wiśniewski/\r\n1 PLAC Źródła, Żółkiewka\r\n";

    #[test]
    fn utf8_with_and_without_bom() {
        let with = decode(&encode_utf8("0 HEAD ż", true));
        assert_eq!((with.text.as_str(), with.encoding, with.bom), ("0 HEAD ż", Encoding::Utf8, true));
        let without = decode("0 HEAD ż".as_bytes());
        assert!(!without.bom);
    }

    #[test]
    fn utf16_little_endian_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in POLISH.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let d = decode(&bytes);
        assert_eq!(d.encoding, Encoding::Utf16Le);
        assert_eq!(d.text, POLISH);
    }

    #[test]
    fn windows_1250_polish_file_labelled_ansi() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1250.encode(POLISH);
        let d = decode(&bytes);
        assert_eq!(d.encoding, Encoding::Windows1250);
        assert_eq!(d.text, POLISH);
    }

    #[test]
    fn windows_1252_french_file_stays_western() {
        let french = "0 HEAD\r\n1 CHAR ANSI\r\n0 @I1@ INDI\r\n1 NAME Hélène /Lefèvre/\r\n1 PLAC Besançon, Côte-d'Or, Française\r\n";
        let (bytes, _, _) = encoding_rs::WINDOWS_1252.encode(french);
        let d = decode(&bytes);
        assert_eq!(d.encoding, Encoding::Windows1252);
        assert_eq!(d.text, french);
    }

    #[test]
    fn ansel_file() {
        let mut bytes = b"0 HEAD\r\n1 CHAR ANSEL\r\n0 @I1@ INDI\r\n1 NAME J".to_vec();
        bytes.extend_from_slice(b"\xE2ozef /\xA1\xF1aczy\xE2nski/\r\n");
        let d = decode(&bytes);
        assert_eq!(d.encoding, Encoding::Ansel);
        assert!(d.text.contains("1 NAME Józef /Łączyński/"), "{}", d.text);
    }

    #[test]
    fn valid_utf8_wins_over_a_wrong_label() {
        let d = decode("0 HEAD\n1 CHAR ANSEL\n1 NAME Żółć\n".as_bytes());
        assert_eq!(d.encoding, Encoding::Utf8);
    }
}
