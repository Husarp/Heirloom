//! Folding text for search, so "lukasz" finds "Łukasz" and "zolc" finds "żółć".
//!
//! Unicode decomposition strips most accents, but some letters have no decomposition (Polish ł, also đ, ø, ß…),
//! so they are mapped by hand. SQLite's own FTS5 folding misses them too (RESEARCH §5).

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Lower-cases `text` and removes diacritics, including the letters that don't decompose.
pub fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.nfd() {
        match c {
            'ł' | 'Ł' => out.push('l'),
            'đ' | 'Đ' => out.push('d'),
            'ø' | 'Ø' => out.push('o'),
            'ß' => out.push_str("ss"),
            'æ' | 'Æ' => out.push_str("ae"),
            'œ' | 'Œ' => out.push_str("oe"),
            c if is_combining_mark(c) => {}
            c => out.extend(c.to_lowercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn folds_polish_letters_including_l_with_stroke() {
        assert_eq!(fold("Łukasz Wiśniewski"), "lukasz wisniewski");
        assert_eq!(fold("ŻÓŁĆ gęślą jaźń"), "zolc gesla jazn");
    }

    #[test]
    fn folds_other_letters_without_decomposition() {
        assert_eq!(fold("Straße Øster Đorđe Æsop"), "strasse oster dorde aesop");
    }
}
