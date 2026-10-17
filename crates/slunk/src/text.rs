//! Text as it's saved: in Unicode NFC, then trimmed as JavaScript trims
//! (slunk's 3_alternate_names.md §4.2, design §10). The same characters can
//! come in two forms, like "é" as one code point or as "e" and an accent,
//! which don't compare equal. NFC makes them one. NFKC isn't used: it would
//! rewrite what's printed, turning "E=MC²" into "E=MC2". Paths aren't text:
//! they have to match the disk.

use unicode_normalization::UnicodeNormalization;

/// Text in NFC, then trimmed.
pub fn clean_text(text: &str) -> String {
    let text: String = text.nfc().collect();
    trim(&text).to_string()
}

/// Cleaned text, or `None` when it's blank.
pub fn clean_or_none(text: Option<&str>) -> Option<String> {
    text.map(clean_text).filter(|text| !text.is_empty())
}

/// Trims what JavaScript's `trim()` trims: its white space and line
/// terminators. That's Unicode's White_Space without U+0085, plus U+FEFF.
pub fn trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// Whether JavaScript's `trim()` trims the character.
pub(crate) fn is_js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_nfc_then_trimmed_as_javascript_trims() {
        assert_eq!(clean_text(" Rock\n"), "Rock");
        // "é" as "e" and a combining accent becomes one code point
        assert_eq!(clean_text("Cafe\u{301}"), "Caf\u{e9}");
        // JavaScript trims U+FEFF and U+3000, but not U+0085
        assert_eq!(clean_text("\u{feff}\u{3000}Jazz\u{a0}"), "Jazz");
        assert_eq!(clean_text("\u{85}Jazz"), "\u{85}Jazz");
        // NFC, not NFKC
        assert_eq!(clean_text("E=MC²"), "E=MC²");
        assert_eq!(clean_or_none(Some("  ")), None);
        assert_eq!(clean_or_none(None), None);
        assert_eq!(clean_or_none(Some(" a ")).as_deref(), Some("a"));
    }

    #[test]
    fn trim_removes_exactly_what_javascript_does() {
        // Every code point for which `String.fromCodePoint(c).trim() === ""`
        // in Node 26
        let javascript = [
            0x9, 0xa, 0xb, 0xc, 0xd, 0x20, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004,
            0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
            0xfeff,
        ];
        let trimmed: Vec<u32> = (0..=0x10ffff)
            .filter_map(char::from_u32)
            .filter(|&c| is_js_space(c))
            .map(u32::from)
            .collect();
        assert_eq!(trimmed, javascript);
    }
}
