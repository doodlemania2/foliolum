// Copyright (c) 2026 The Foliolum authors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Symbol fonts draw pictures at ordinary character codes: in Wingdings the letter "l" is a round
//! bullet, and in Symbol the letter "p" is a Greek pi. A Publisher file names the font and stores the
//! code, so without that font (a browser has none of them, and they can't be redistributed) the text
//! comes out as the letters themselves, or as boxes.
//!
//! This maps those codes to the Unicode characters that look the same, so any font with that
//! character draws them (DesignCraft falls back, character by character, to a loaded font that has
//! it; Foliolum bundles Noto Sans Symbols 2 for these). It is used for bullets and for text set in a
//! symbol font.
//!
//! - **Symbol:** Unicode's own table, `MAPPINGS/VENDORS/ADOBE/symbol.txt`, without the codes it maps
//!   to the private use area, except the serif and sans serif ©, ® and ™, which become those characters
//!   (the bracket and arrow pieces there have no standard character). Where it lists two
//!   characters for a code, the first. Codes it maps to themselves are listed too, because Windows
//!   may store them in the private use area (U+F021 for "!").
//! - **Wingdings:** the Unicode characters named in Unicode's study of the Wingdings repertoire
//!   (L2/11-052R, 2011), which Unicode 7 encoded under those names, looked up by name; plus the
//!   shapes Office's bullet picker offers. Codes with no Unicode character are left out.
//!
//! Every entry's comment is the Unicode name of the character it maps to. Webdings and Wingdings 2
//! and 3 have no table yet: a bullet in them becomes "•", and text in them is left as it is and
//! counted in a warning.

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

/// A character of text set in symbol font `family`: the Unicode character it draws, or `None` when
/// there is no mapping (the caller keeps the character and counts it).
pub(crate) fn text_char(family: &str, c: char) -> Option<char> {
    // Spaces, line breaks and tabs are the same in every font.
    if c.is_whitespace() {
        return Some(c);
    }
    if c == '\u{F020}' {
        return Some(' ');
    }
    to_unicode(family, c)
}

/// A bullet character for paragraph text: `c` in `family`, mapped when `family` is a symbol font.
pub(crate) fn bullet(family: Option<&str>, c: char) -> (char, bool) {
    match family {
        Some(f) if is_symbol_font(f) => (to_unicode(f, c).unwrap_or('\u{2022}'), true),
        _ => (c, false),
    }
}

/// Wingdings codes and the Unicode characters they draw.
const WINGDINGS: &[(u32, char)] = &[
    (0x21, '\u{1F589}'), // LOWER LEFT PENCIL
    (0x22, '\u{2702}'),  // BLACK SCISSORS
    (0x23, '\u{2701}'),  // UPPER BLADE SCISSORS
    (0x24, '\u{1F453}'), // EYEGLASSES
    (0x25, '\u{1F514}'), // BELL
    (0x26, '\u{1F4D6}'), // OPEN BOOK
    (0x27, '\u{1F56F}'), // CANDLE
    (0x28, '\u{1F57F}'), // BLACK TOUCHTONE TELEPHONE
    (0x29, '\u{2706}'),  // TELEPHONE LOCATION SIGN
    (0x2A, '\u{2709}'),  // ENVELOPE
    (0x2C, '\u{1F4EA}'), // CLOSED MAILBOX WITH LOWERED FLAG
    (0x2D, '\u{1F4EB}'), // CLOSED MAILBOX WITH RAISED FLAG
    (0x2E, '\u{1F4EC}'), // OPEN MAILBOX WITH RAISED FLAG
    (0x2F, '\u{1F4ED}'), // OPEN MAILBOX WITH LOWERED FLAG
    (0x30, '\u{1F5C0}'), // FOLDER
    (0x31, '\u{1F5C1}'), // OPEN FOLDER
    (0x32, '\u{1F5CE}'), // DOCUMENT
    (0x34, '\u{1F5D0}'), // PAGES
    (0x35, '\u{1F5C4}'), // FILE CABINET
    (0x36, '\u{231B}'),  // HOURGLASS
    (0x38, '\u{1F5B0}'), // TWO BUTTON MOUSE
    (0x39, '\u{1F5B2}'), // TRACKBALL
    (0x3B, '\u{1F5B4}'), // HARD DISK
    (0x3C, '\u{1F5AB}'), // WHITE HARD SHELL FLOPPY DISK
    (0x3D, '\u{1F5AC}'), // SOFT SHELL FLOPPY DISK
    (0x3F, '\u{270D}'),  // WRITING HAND
    (0x40, '\u{1F58E}'), // LEFT WRITING HAND
    (0x41, '\u{270C}'),  // VICTORY HAND
    (0x42, '\u{1F58F}'), // TURNED OK HAND SIGN
    (0x43, '\u{1F44D}'), // THUMBS UP SIGN
    (0x44, '\u{1F44E}'), // THUMBS DOWN SIGN
    (0x45, '\u{261C}'),  // WHITE LEFT POINTING INDEX
    (0x46, '\u{261E}'),  // WHITE RIGHT POINTING INDEX
    (0x47, '\u{261D}'),  // WHITE UP POINTING INDEX
    (0x49, '\u{1F590}'), // RAISED HAND WITH FINGERS SPLAYED
    (0x4A, '\u{263A}'),  // WHITE SMILING FACE
    (0x4B, '\u{1F610}'), // NEUTRAL FACE
    (0x4C, '\u{2639}'),  // WHITE FROWNING FACE
    (0x4D, '\u{1F4A3}'), // BOMB
    (0x4E, '\u{2620}'),  // SKULL AND CROSSBONES
    (0x4F, '\u{1F3F3}'), // WAVING WHITE FLAG
    (0x51, '\u{2708}'),  // AIRPLANE
    (0x52, '\u{263C}'),  // WHITE SUN WITH RAYS
    (0x53, '\u{1F322}'), // BLACK DROPLET
    (0x54, '\u{2744}'),  // SNOWFLAKE
    (0x55, '\u{1F546}'), // WHITE LATIN CROSS
    (0x56, '\u{271E}'),  // SHADOWED WHITE LATIN CROSS
    (0x57, '\u{1F548}'), // CELTIC CROSS
    (0x58, '\u{2720}'),  // MALTESE CROSS
    (0x59, '\u{2721}'),  // STAR OF DAVID
    (0x5A, '\u{262A}'),  // STAR AND CRESCENT
    (0x5B, '\u{262F}'),  // YIN YANG
    (0x5C, '\u{0950}'),  // DEVANAGARI OM
    (0x5D, '\u{2638}'),  // WHEEL OF DHARMA
    (0x5E, '\u{2648}'),  // ARIES
    (0x5F, '\u{2649}'),  // TAURUS
    (0x60, '\u{264A}'),  // GEMINI
    (0x61, '\u{264B}'),  // CANCER
    (0x62, '\u{264C}'),  // LEO
    (0x63, '\u{264D}'),  // VIRGO
    (0x64, '\u{264E}'),  // LIBRA
    (0x65, '\u{264F}'),  // SCORPIUS
    (0x66, '\u{2650}'),  // SAGITTARIUS
    (0x67, '\u{2651}'),  // CAPRICORN
    (0x68, '\u{2652}'),  // AQUARIUS
    (0x69, '\u{2653}'),  // PISCES
    (0x6C, '\u{25CF}'),  // BLACK CIRCLE
    (0x6D, '\u{1F53E}'), // LOWER RIGHT SHADOWED WHITE CIRCLE
    (0x6E, '\u{25A0}'),  // BLACK SQUARE
    (0x6F, '\u{25A1}'),  // WHITE SQUARE
    (0x71, '\u{2751}'),  // LOWER RIGHT SHADOWED WHITE SQUARE
    (0x72, '\u{2752}'),  // UPPER RIGHT SHADOWED WHITE SQUARE
    (0x75, '\u{25C6}'),  // BLACK DIAMOND
    (0x76, '\u{2756}'),  // BLACK DIAMOND MINUS WHITE X
    (0x77, '\u{2B25}'),  // BLACK MEDIUM DIAMOND
    (0x78, '\u{2612}'),  // BALLOT BOX WITH X
    (0x7A, '\u{2318}'),  // PLACE OF INTEREST SIGN
    (0x7D, '\u{1F676}'), // SANS-SERIF HEAVY DOUBLE TURNED COMMA QUOTATION MARK ORNAMENT
    (0x80, '\u{24EA}'),  // CIRCLED DIGIT ZERO
    (0x81, '\u{2460}'),  // CIRCLED DIGIT ONE
    (0x82, '\u{2461}'),  // CIRCLED DIGIT TWO
    (0x83, '\u{2462}'),  // CIRCLED DIGIT THREE
    (0x84, '\u{2463}'),  // CIRCLED DIGIT FOUR
    (0x85, '\u{2464}'),  // CIRCLED DIGIT FIVE
    (0x86, '\u{2465}'),  // CIRCLED DIGIT SIX
    (0x87, '\u{2466}'),  // CIRCLED DIGIT SEVEN
    (0x88, '\u{2467}'),  // CIRCLED DIGIT EIGHT
    (0x89, '\u{2468}'),  // CIRCLED DIGIT NINE
    (0x8A, '\u{2469}'),  // CIRCLED NUMBER TEN
    (0x8B, '\u{24FF}'),  // NEGATIVE CIRCLED DIGIT ZERO
    (0x8C, '\u{2776}'),  // DINGBAT NEGATIVE CIRCLED DIGIT ONE
    (0x8D, '\u{2777}'),  // DINGBAT NEGATIVE CIRCLED DIGIT TWO
    (0x8E, '\u{2778}'),  // DINGBAT NEGATIVE CIRCLED DIGIT THREE
    (0x8F, '\u{2779}'),  // DINGBAT NEGATIVE CIRCLED DIGIT FOUR
    (0x90, '\u{277A}'),  // DINGBAT NEGATIVE CIRCLED DIGIT FIVE
    (0x91, '\u{277B}'),  // DINGBAT NEGATIVE CIRCLED DIGIT SIX
    (0x92, '\u{277C}'),  // DINGBAT NEGATIVE CIRCLED DIGIT SEVEN
    (0x93, '\u{277D}'),  // DINGBAT NEGATIVE CIRCLED DIGIT EIGHT
    (0x94, '\u{277E}'),  // DINGBAT NEGATIVE CIRCLED DIGIT NINE
    (0x95, '\u{277F}'),  // DINGBAT NEGATIVE CIRCLED NUMBER TEN
    (0xA6, '\u{1F53F}'), // UPPER RIGHT SHADOWED WHITE CIRCLE
    (0xA7, '\u{25AA}'),  // BLACK SMALL SQUARE
    (0xA8, '\u{25FB}'),  // WHITE MEDIUM SQUARE
    (0xAB, '\u{2605}'),  // BLACK STAR
    (0xAF, '\u{2735}'),  // EIGHT POINTED PINWHEEL STAR
    (0xB4, '\u{2BD1}'),  // UNCERTAINTY SIGN
    (0xB5, '\u{272A}'),  // CIRCLED WHITE STAR
    (0xB7, '\u{1F550}'), // CLOCK FACE ONE OCLOCK
    (0xB8, '\u{1F551}'), // CLOCK FACE TWO OCLOCK
    (0xB9, '\u{1F552}'), // CLOCK FACE THREE OCLOCK
    (0xBA, '\u{1F553}'), // CLOCK FACE FOUR OCLOCK
    (0xBB, '\u{1F554}'), // CLOCK FACE FIVE OCLOCK
    (0xBC, '\u{1F555}'), // CLOCK FACE SIX OCLOCK
    (0xBD, '\u{1F556}'), // CLOCK FACE SEVEN OCLOCK
    (0xBE, '\u{1F557}'), // CLOCK FACE EIGHT OCLOCK
    (0xBF, '\u{1F558}'), // CLOCK FACE NINE OCLOCK
    (0xC0, '\u{1F559}'), // CLOCK FACE TEN OCLOCK
    (0xC1, '\u{1F55A}'), // CLOCK FACE ELEVEN OCLOCK
    (0xC2, '\u{1F55B}'), // CLOCK FACE TWELVE OCLOCK
    (0xD5, '\u{232B}'),  // ERASE TO THE LEFT
    (0xD6, '\u{2326}'),  // ERASE TO THE RIGHT
    (0xD8, '\u{27A2}'),  // THREE-D TOP-LIGHTED RIGHTWARDS ARROWHEAD
    (0xDC, '\u{27B2}'),  // CIRCLED HEAVY WHITE RIGHTWARDS ARROW
    (0xEF, '\u{21E6}'),  // LEFTWARDS WHITE ARROW
    (0xF0, '\u{21E8}'),  // RIGHTWARDS WHITE ARROW
    (0xF1, '\u{21E7}'),  // UPWARDS WHITE ARROW
    (0xF2, '\u{21E9}'),  // DOWNWARDS WHITE ARROW
    (0xF3, '\u{2B04}'),  // LEFT RIGHT WHITE ARROW
    (0xF4, '\u{21F3}'),  // UP DOWN WHITE ARROW
    (0xF5, '\u{2B01}'),  // NORTH WEST WHITE ARROW
    (0xF6, '\u{2B00}'),  // NORTH EAST WHITE ARROW
    (0xF7, '\u{2B03}'),  // SOUTH WEST WHITE ARROW
    (0xF8, '\u{2B02}'),  // SOUTH EAST WHITE ARROW
    (0xFB, '\u{2717}'),  // BALLOT X
    (0xFC, '\u{2714}'),  // HEAVY CHECK MARK
    (0xFD, '\u{2612}'),  // BALLOT BOX WITH X
    (0xFE, '\u{2611}'),  // BALLOT BOX WITH CHECK
];

/// Symbol font codes and the Unicode characters they draw.
const SYMBOL: &[(u32, char)] = &[
    (0x21, '\u{0021}'), // EXCLAMATION MARK
    (0x22, '\u{2200}'), // FOR ALL
    (0x23, '\u{0023}'), // NUMBER SIGN
    (0x24, '\u{2203}'), // THERE EXISTS
    (0x25, '\u{0025}'), // PERCENT SIGN
    (0x26, '\u{0026}'), // AMPERSAND
    (0x27, '\u{220B}'), // CONTAINS AS MEMBER
    (0x28, '\u{0028}'), // LEFT PARENTHESIS
    (0x29, '\u{0029}'), // RIGHT PARENTHESIS
    (0x2A, '\u{2217}'), // ASTERISK OPERATOR
    (0x2B, '\u{002B}'), // PLUS SIGN
    (0x2C, '\u{002C}'), // COMMA
    (0x2D, '\u{2212}'), // MINUS SIGN
    (0x2E, '\u{002E}'), // FULL STOP
    (0x2F, '\u{002F}'), // SOLIDUS
    (0x30, '\u{0030}'), // DIGIT ZERO
    (0x31, '\u{0031}'), // DIGIT ONE
    (0x32, '\u{0032}'), // DIGIT TWO
    (0x33, '\u{0033}'), // DIGIT THREE
    (0x34, '\u{0034}'), // DIGIT FOUR
    (0x35, '\u{0035}'), // DIGIT FIVE
    (0x36, '\u{0036}'), // DIGIT SIX
    (0x37, '\u{0037}'), // DIGIT SEVEN
    (0x38, '\u{0038}'), // DIGIT EIGHT
    (0x39, '\u{0039}'), // DIGIT NINE
    (0x3A, '\u{003A}'), // COLON
    (0x3B, '\u{003B}'), // SEMICOLON
    (0x3C, '\u{003C}'), // LESS-THAN SIGN
    (0x3D, '\u{003D}'), // EQUALS SIGN
    (0x3E, '\u{003E}'), // GREATER-THAN SIGN
    (0x3F, '\u{003F}'), // QUESTION MARK
    (0x40, '\u{2245}'), // APPROXIMATELY EQUAL TO
    (0x41, '\u{0391}'), // GREEK CAPITAL LETTER ALPHA
    (0x42, '\u{0392}'), // GREEK CAPITAL LETTER BETA
    (0x43, '\u{03A7}'), // GREEK CAPITAL LETTER CHI
    (0x44, '\u{0394}'), // GREEK CAPITAL LETTER DELTA
    (0x45, '\u{0395}'), // GREEK CAPITAL LETTER EPSILON
    (0x46, '\u{03A6}'), // GREEK CAPITAL LETTER PHI
    (0x47, '\u{0393}'), // GREEK CAPITAL LETTER GAMMA
    (0x48, '\u{0397}'), // GREEK CAPITAL LETTER ETA
    (0x49, '\u{0399}'), // GREEK CAPITAL LETTER IOTA
    (0x4A, '\u{03D1}'), // GREEK THETA SYMBOL
    (0x4B, '\u{039A}'), // GREEK CAPITAL LETTER KAPPA
    (0x4C, '\u{039B}'), // GREEK CAPITAL LETTER LAMDA
    (0x4D, '\u{039C}'), // GREEK CAPITAL LETTER MU
    (0x4E, '\u{039D}'), // GREEK CAPITAL LETTER NU
    (0x4F, '\u{039F}'), // GREEK CAPITAL LETTER OMICRON
    (0x50, '\u{03A0}'), // GREEK CAPITAL LETTER PI
    (0x51, '\u{0398}'), // GREEK CAPITAL LETTER THETA
    (0x52, '\u{03A1}'), // GREEK CAPITAL LETTER RHO
    (0x53, '\u{03A3}'), // GREEK CAPITAL LETTER SIGMA
    (0x54, '\u{03A4}'), // GREEK CAPITAL LETTER TAU
    (0x55, '\u{03A5}'), // GREEK CAPITAL LETTER UPSILON
    (0x56, '\u{03C2}'), // GREEK SMALL LETTER FINAL SIGMA
    (0x57, '\u{03A9}'), // GREEK CAPITAL LETTER OMEGA
    (0x58, '\u{039E}'), // GREEK CAPITAL LETTER XI
    (0x59, '\u{03A8}'), // GREEK CAPITAL LETTER PSI
    (0x5A, '\u{0396}'), // GREEK CAPITAL LETTER ZETA
    (0x5B, '\u{005B}'), // LEFT SQUARE BRACKET
    (0x5C, '\u{2234}'), // THEREFORE
    (0x5D, '\u{005D}'), // RIGHT SQUARE BRACKET
    (0x5E, '\u{22A5}'), // UP TACK
    (0x5F, '\u{005F}'), // LOW LINE
    (0x61, '\u{03B1}'), // GREEK SMALL LETTER ALPHA
    (0x62, '\u{03B2}'), // GREEK SMALL LETTER BETA
    (0x63, '\u{03C7}'), // GREEK SMALL LETTER CHI
    (0x64, '\u{03B4}'), // GREEK SMALL LETTER DELTA
    (0x65, '\u{03B5}'), // GREEK SMALL LETTER EPSILON
    (0x66, '\u{03C6}'), // GREEK SMALL LETTER PHI
    (0x67, '\u{03B3}'), // GREEK SMALL LETTER GAMMA
    (0x68, '\u{03B7}'), // GREEK SMALL LETTER ETA
    (0x69, '\u{03B9}'), // GREEK SMALL LETTER IOTA
    (0x6A, '\u{03D5}'), // GREEK PHI SYMBOL
    (0x6B, '\u{03BA}'), // GREEK SMALL LETTER KAPPA
    (0x6C, '\u{03BB}'), // GREEK SMALL LETTER LAMDA
    (0x6D, '\u{00B5}'), // MICRO SIGN
    (0x6E, '\u{03BD}'), // GREEK SMALL LETTER NU
    (0x6F, '\u{03BF}'), // GREEK SMALL LETTER OMICRON
    (0x70, '\u{03C0}'), // GREEK SMALL LETTER PI
    (0x71, '\u{03B8}'), // GREEK SMALL LETTER THETA
    (0x72, '\u{03C1}'), // GREEK SMALL LETTER RHO
    (0x73, '\u{03C3}'), // GREEK SMALL LETTER SIGMA
    (0x74, '\u{03C4}'), // GREEK SMALL LETTER TAU
    (0x75, '\u{03C5}'), // GREEK SMALL LETTER UPSILON
    (0x76, '\u{03D6}'), // GREEK PI SYMBOL
    (0x77, '\u{03C9}'), // GREEK SMALL LETTER OMEGA
    (0x78, '\u{03BE}'), // GREEK SMALL LETTER XI
    (0x79, '\u{03C8}'), // GREEK SMALL LETTER PSI
    (0x7A, '\u{03B6}'), // GREEK SMALL LETTER ZETA
    (0x7B, '\u{007B}'), // LEFT CURLY BRACKET
    (0x7C, '\u{007C}'), // VERTICAL LINE
    (0x7D, '\u{007D}'), // RIGHT CURLY BRACKET
    (0x7E, '\u{223C}'), // TILDE OPERATOR
    (0xA0, '\u{20AC}'), // EURO SIGN
    (0xA1, '\u{03D2}'), // GREEK UPSILON WITH HOOK SYMBOL
    (0xA2, '\u{2032}'), // PRIME
    (0xA3, '\u{2264}'), // LESS-THAN OR EQUAL TO
    (0xA4, '\u{2044}'), // FRACTION SLASH
    (0xA5, '\u{221E}'), // INFINITY
    (0xA6, '\u{0192}'), // LATIN SMALL LETTER F WITH HOOK
    (0xA7, '\u{2663}'), // BLACK CLUB SUIT
    (0xA8, '\u{2666}'), // BLACK DIAMOND SUIT
    (0xA9, '\u{2665}'), // BLACK HEART SUIT
    (0xAA, '\u{2660}'), // BLACK SPADE SUIT
    (0xAB, '\u{2194}'), // LEFT RIGHT ARROW
    (0xAC, '\u{2190}'), // LEFTWARDS ARROW
    (0xAD, '\u{2191}'), // UPWARDS ARROW
    (0xAE, '\u{2192}'), // RIGHTWARDS ARROW
    (0xAF, '\u{2193}'), // DOWNWARDS ARROW
    (0xB0, '\u{00B0}'), // DEGREE SIGN
    (0xB1, '\u{00B1}'), // PLUS-MINUS SIGN
    (0xB2, '\u{2033}'), // DOUBLE PRIME
    (0xB3, '\u{2265}'), // GREATER-THAN OR EQUAL TO
    (0xB4, '\u{00D7}'), // MULTIPLICATION SIGN
    (0xB5, '\u{221D}'), // PROPORTIONAL TO
    (0xB6, '\u{2202}'), // PARTIAL DIFFERENTIAL
    (0xB7, '\u{2022}'), // BULLET
    (0xB8, '\u{00F7}'), // DIVISION SIGN
    (0xB9, '\u{2260}'), // NOT EQUAL TO
    (0xBA, '\u{2261}'), // IDENTICAL TO
    (0xBB, '\u{2248}'), // ALMOST EQUAL TO
    (0xBC, '\u{2026}'), // HORIZONTAL ELLIPSIS
    (0xBF, '\u{21B5}'), // DOWNWARDS ARROW WITH CORNER LEFTWARDS
    (0xC0, '\u{2135}'), // ALEF SYMBOL
    (0xC1, '\u{2111}'), // BLACK-LETTER CAPITAL I
    (0xC2, '\u{211C}'), // BLACK-LETTER CAPITAL R
    (0xC3, '\u{2118}'), // SCRIPT CAPITAL P
    (0xC4, '\u{2297}'), // CIRCLED TIMES
    (0xC5, '\u{2295}'), // CIRCLED PLUS
    (0xC6, '\u{2205}'), // EMPTY SET
    (0xC7, '\u{2229}'), // INTERSECTION
    (0xC8, '\u{222A}'), // UNION
    (0xC9, '\u{2283}'), // SUPERSET OF
    (0xCA, '\u{2287}'), // SUPERSET OF OR EQUAL TO
    (0xCB, '\u{2284}'), // NOT A SUBSET OF
    (0xCC, '\u{2282}'), // SUBSET OF
    (0xCD, '\u{2286}'), // SUBSET OF OR EQUAL TO
    (0xCE, '\u{2208}'), // ELEMENT OF
    (0xCF, '\u{2209}'), // NOT AN ELEMENT OF
    (0xD0, '\u{2220}'), // ANGLE
    (0xD1, '\u{2207}'), // NABLA
    (0xD2, '\u{00AE}'), // REGISTERED SIGN
    (0xD3, '\u{00A9}'), // COPYRIGHT SIGN
    (0xD4, '\u{2122}'), // TRADE MARK SIGN
    (0xD5, '\u{220F}'), // N-ARY PRODUCT
    (0xD6, '\u{221A}'), // SQUARE ROOT
    (0xD7, '\u{22C5}'), // DOT OPERATOR
    (0xD8, '\u{00AC}'), // NOT SIGN
    (0xD9, '\u{2227}'), // LOGICAL AND
    (0xDA, '\u{2228}'), // LOGICAL OR
    (0xDB, '\u{21D4}'), // LEFT RIGHT DOUBLE ARROW
    (0xDC, '\u{21D0}'), // LEFTWARDS DOUBLE ARROW
    (0xDD, '\u{21D1}'), // UPWARDS DOUBLE ARROW
    (0xDE, '\u{21D2}'), // RIGHTWARDS DOUBLE ARROW
    (0xDF, '\u{21D3}'), // DOWNWARDS DOUBLE ARROW
    (0xE0, '\u{25CA}'), // LOZENGE
    (0xE1, '\u{2329}'), // LEFT-POINTING ANGLE BRACKET
    (0xE2, '\u{00AE}'), // REGISTERED SIGN
    (0xE3, '\u{00A9}'), // COPYRIGHT SIGN
    (0xE4, '\u{2122}'), // TRADE MARK SIGN
    (0xE5, '\u{2211}'), // N-ARY SUMMATION
    (0xF1, '\u{232A}'), // RIGHT-POINTING ANGLE BRACKET
    (0xF2, '\u{222B}'), // INTEGRAL
    (0xF3, '\u{2320}'), // TOP HALF INTEGRAL
    (0xF5, '\u{2321}'), // BOTTOM HALF INTEGRAL
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
    fn text_in_symbol_fonts_becomes_what_it_draws() {
        let greek: String = "Mixed Case".chars().map(|c| text_char("Symbol", c).unwrap_or(c)).collect();
        assert_eq!(greek, "Μιξεδ Χασε");
        assert_eq!(text_char("Symbol", '\u{F0B0}'), Some('°'));
        assert_eq!(text_char("Symbol", 'Ó'), Some('©'));
        assert_eq!(text_char("Wingdings", '('), Some('\u{1F57F}'));
        assert_eq!(text_char("WINGDINGS", 'X'), Some('✠'));
        assert_eq!(text_char("Wingdings", ' '), Some(' '));
        // No character for it: left to the caller.
        assert_eq!(text_char("Wingdings", '\u{FF}'), None);
        assert_eq!(text_char("Webdings", 'a'), None);
    }

    #[test]
    fn every_entry_maps_to_a_standard_character() {
        for (name, table) in [("Wingdings", WINGDINGS), ("Symbol", SYMBOL)] {
            let mut codes: Vec<u32> = table.iter().map(|(k, _)| *k).collect();
            codes.sort_unstable();
            codes.dedup();
            assert_eq!(codes.len(), table.len(), "{name}: a code is listed twice");
            for (code, u) in table {
                assert!((0x21..=0xFF).contains(code), "{name} {code:#x}");
                assert!(!(0xE000..=0xF8FF).contains(&(*u as u32)), "{name} {code:#x} maps into the private use area");
            }
        }
    }

    #[test]
    fn unknown_symbol_codes_stay_bullets_and_text_fonts_are_untouched() {
        assert_eq!(bullet(Some("Webdings"), 'x'), ('•', true));
        assert_eq!(bullet(Some("Wingdings 2"), 'x'), ('•', true));
        assert_eq!(bullet(Some("Arial"), '–'), ('–', false));
        assert_eq!(bullet(None, '•'), ('•', false));
    }
}
