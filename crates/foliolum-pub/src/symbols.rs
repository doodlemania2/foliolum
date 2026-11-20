// Copyright (c) 2026 The Foliolum authors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Symbol fonts draw pictures at ordinary character codes: in Wingdings the letter "l" is a round
//! bullet. A Publisher file names the font and stores the code, so without that font (a browser has
//! none of them, and they can't be redistributed) the bullet comes out as an "l" or a box.
//!
//! This maps the codes Office offers as bullets to the Unicode characters that look the same, so any
//! font with symbol coverage draws them. Each entry was checked against the Unicode character's name
//! (Unicode's own Wingdings and Webdings mapping is in its document L2/11-052; the Symbol font's in
//! Unicode's MAPPINGS/VENDORS/ADOBE/symbol.txt). A code not listed here falls back to "•" for a
//! bullet, so a bullet always stays a bullet.

/// Is `family` a symbol font whose codes need mapping?
pub(crate) fn is_symbol_font(family: &str) -> bool {
    let f = family.trim().to_ascii_lowercase();
    f.starts_with("wingdings") || f == "webdings" || f == "symbol"
}

/// The Unicode character drawn by `c` in symbol font `family`, when it is one of the listed codes.
/// Windows also stores symbol-font text in the private use area (U+F020–U+F0FF), which is read the
/// same way.
pub(crate) fn to_unicode(family: &str, c: char) -> Option<char> {
    let mut code = c as u32;
    if (0xF020..=0xF0FF).contains(&code) {
        code -= 0xF000;
    }
    let f = family.trim().to_ascii_lowercase();
    let table: &[(u32, char)] = if f == "wingdings" {
        WINGDINGS
    } else if f == "symbol" {
        SYMBOL
    } else {
        &[]
    };
    table.iter().find(|(k, _)| *k == code).map(|(_, u)| *u)
}

/// A bullet character for paragraph text: `c` in `family`, mapped when `family` is a symbol font.
pub(crate) fn bullet(family: Option<&str>, c: char) -> (char, bool) {
    match family {
        Some(f) if is_symbol_font(f) => (to_unicode(f, c).unwrap_or('\u{2022}'), true),
        _ => (c, false),
    }
}

/// Wingdings codes Office's bullet picker offers, and a few common pictographs.
const WINGDINGS: &[(u32, char)] = &[
    (0x46, '\u{261E}'), // WHITE RIGHT POINTING INDEX
    (0x4A, '\u{263A}'), // WHITE SMILING FACE
    (0x4C, '\u{2639}'), // WHITE FROWNING FACE
    (0x6C, '\u{25CF}'), // BLACK CIRCLE
    (0x6E, '\u{25A0}'), // BLACK SQUARE
    (0x6F, '\u{25A1}'), // WHITE SQUARE
    (0x71, '\u{2751}'), // LOWER RIGHT SHADOWED WHITE SQUARE
    (0x75, '\u{25C6}'), // BLACK DIAMOND
    (0x76, '\u{2756}'), // BLACK DIAMOND MINUS WHITE X
    (0x77, '\u{2B25}'), // BLACK MEDIUM DIAMOND
    (0xA7, '\u{25AA}'), // BLACK SMALL SQUARE
    (0xA8, '\u{25FB}'), // WHITE MEDIUM SQUARE
    (0xD8, '\u{27A2}'), // THREE-D TOP-LIGHTED RIGHTWARDS ARROWHEAD
    (0xFC, '\u{2714}'), // HEAVY CHECK MARK
];

/// Symbol font codes used as bullets.
const SYMBOL: &[(u32, char)] = &[
    (0xB7, '\u{2022}'), // BULLET
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wingdings_bullets_become_their_unicode_characters() {
        assert_eq!(bullet(Some("Wingdings"), 'l'), ('●', true));
        assert_eq!(bullet(Some("Wingdings"), '\u{F06C}'), ('●', true));
        assert_eq!(bullet(Some("Wingdings"), 'Ø'), ('➢', true));
        assert_eq!(bullet(Some("Symbol"), '·'), ('•', true));
    }

    #[test]
    fn unknown_symbol_codes_stay_bullets_and_text_fonts_are_untouched() {
        assert_eq!(bullet(Some("Webdings"), 'x'), ('•', true));
        assert_eq!(bullet(Some("Wingdings 2"), 'x'), ('•', true));
        assert_eq!(bullet(Some("Arial"), '–'), ('–', false));
        assert_eq!(bullet(None, '•'), ('•', false));
    }
}
