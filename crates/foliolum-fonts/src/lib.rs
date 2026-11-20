//! The fonts Foliolum ships, and which of them draws each font a Publisher file asks for.
//!
//! Publisher files name Windows and Office fonts, which a browser doesn't have and which we can't
//! redistribute. DesignCraft draws a missing family with Source Sans 3, which is wider or narrower than
//! the original, so headlines overflow and columns break differently. Foliolum bundles open fonts
//! instead: metric-compatible ones (same advance widths, so the text breaks where it did in
//! Publisher) where they exist, and close look-alikes otherwise.
//!
//! The files are fetched by `fetch.sh` (from `fonts.lock`, checked by SHA-256) and embedded here. All
//! are SIL Open Font License 1.1 except Roboto Slab (Apache 2.0); each family's licence text ships
//! beside its fonts (see [`LICENSES`]).
//!
//! A family that is neither installed nor in [`SUBSTITUTES`] gets a guess from its name ([`guess`]):
//! a condensed name gets a condensed font, a serif name a serif, and so on, registered per document
//! by [`register_families`].

use designcraft_fonts::FontDb;

macro_rules! font {
    ($path:literal) => {
        ($path, include_bytes!(concat!("../fonts/", $path)) as &[u8])
    };
}

/// Every bundled font file, by its path under `fonts/`.
pub const FONTS: &[(&str, &[u8])] = &[
    font!("carlito/Carlito-Regular.ttf"),
    font!("carlito/Carlito-Bold.ttf"),
    font!("carlito/Carlito-Italic.ttf"),
    font!("carlito/Carlito-BoldItalic.ttf"),
    font!("caladea/Caladea-Regular.ttf"),
    font!("caladea/Caladea-Bold.ttf"),
    font!("caladea/Caladea-Italic.ttf"),
    font!("caladea/Caladea-BoldItalic.ttf"),
    font!("arimo/Arimo[wght].ttf"),
    font!("arimo/Arimo-Italic[wght].ttf"),
    font!("tinos/Tinos-Regular.ttf"),
    font!("tinos/Tinos-Bold.ttf"),
    font!("tinos/Tinos-Italic.ttf"),
    font!("tinos/Tinos-BoldItalic.ttf"),
    font!("cousine/Cousine-Regular.ttf"),
    font!("cousine/Cousine-Bold.ttf"),
    font!("cousine/Cousine-Italic.ttf"),
    font!("cousine/Cousine-BoldItalic.ttf"),
    font!("gelasio/Gelasio[wght].ttf"),
    font!("gelasio/Gelasio-Italic[wght].ttf"),
    font!("librefranklin/LibreFranklin[wght].ttf"),
    font!("librefranklin/LibreFranklin-Italic[wght].ttf"),
    font!("ebgaramond/EBGaramond[wght].ttf"),
    font!("ebgaramond/EBGaramond-Italic[wght].ttf"),
    font!("comicneue/ComicNeue-Regular.ttf"),
    font!("comicneue/ComicNeue-Bold.ttf"),
    font!("comicneue/ComicNeue-Italic.ttf"),
    font!("comicneue/ComicNeue-BoldItalic.ttf"),
    font!("questrial/Questrial-Regular.ttf"),
    font!("notosanssymbols2/NotoSansSymbols2-Regular.ttf"),
    font!("robotocondensed/RobotoCondensed[wght].ttf"),
    font!("robotocondensed/RobotoCondensed-Italic[wght].ttf"),
    font!("robotoslab/RobotoSlab[wght].ttf"),
    font!("dancingscript/DancingScript[wght].ttf"),
];

/// Each bundled family's licence text, by its path under `fonts/`.
pub const LICENSES: &[(&str, &[u8])] = &[
    font!("carlito/OFL.txt"),
    font!("caladea/OFL.txt"),
    font!("arimo/OFL.txt"),
    font!("tinos/OFL.txt"),
    font!("cousine/OFL.txt"),
    font!("gelasio/OFL.txt"),
    font!("librefranklin/OFL.txt"),
    font!("ebgaramond/OFL.txt"),
    font!("comicneue/OFL.txt"),
    font!("questrial/OFL.txt"),
    font!("notosanssymbols2/OFL.txt"),
    font!("robotocondensed/OFL.txt"),
    font!("robotoslab/LICENSE.txt"),
    font!("dancingscript/OFL.txt"),
];

/// How closely a substitute matches the font it stands in for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// Same advance widths: text breaks into the same lines.
    Metric,
    /// Similar design, different widths: text may break differently.
    LookAlike,
    /// Chosen from the family's name alone (see [`guess`]).
    Guess,
}

/// Which bundled family draws each font a Publisher file may name, when it isn't installed. Families
/// are matched ignoring case. The first entry wins.
pub const SUBSTITUTES: &[(&str, &str, Match)] = &[
    ("Calibri", "Carlito", Match::Metric),
    ("Calibri Light", "Carlito", Match::LookAlike),
    ("Cambria", "Caladea", Match::Metric),
    ("Arial", "Arimo", Match::Metric),
    ("Helvetica", "Arimo", Match::Metric),
    ("Arial Narrow", "Arimo", Match::LookAlike),
    ("Arial Black", "Arimo", Match::LookAlike),
    ("Times New Roman", "Tinos", Match::Metric),
    ("Times", "Tinos", Match::Metric),
    ("Courier New", "Cousine", Match::Metric),
    ("Courier", "Cousine", Match::Metric),
    ("Georgia", "Gelasio", Match::Metric),
    ("Franklin Gothic Book", "Libre Franklin", Match::LookAlike),
    ("Franklin Gothic Medium", "Libre Franklin", Match::LookAlike),
    ("Franklin Gothic Demi", "Libre Franklin", Match::LookAlike),
    ("Garamond", "EB Garamond", Match::LookAlike),
    ("Adobe Garamond Pro", "EB Garamond", Match::LookAlike),
    ("Comic Sans MS", "Comic Neue", Match::LookAlike),
    ("Century Gothic", "Questrial", Match::LookAlike),
    ("Rockwell", "Roboto Slab", Match::LookAlike),
    ("Rockwell Condensed", "Roboto Condensed", Match::LookAlike),
];

/// A bundled family for a font known only by its name: condensed, script, slab, monospaced or serif
/// by the words in the name. `None` leaves it to DesignCraft's default (Source Sans 3), which suits
/// the sans serifs most unknown names are.
pub fn guess(family: &str) -> Option<&'static str> {
    let f = family.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| f.contains(w));
    if has(&["condensed", "narrow", "compressed", " cond", "extra cond"]) {
        Some("Roboto Condensed")
    } else if has(&[
        "script",
        "hand",
        "brush",
        "calligraph",
        "chancery",
        "cursive",
        "vivaldi",
        "edwardian",
        "mistral",
        "freestyle",
        "kunstler",
        "pristina",
        "vladimir",
    ]) {
        Some("Dancing Script")
    } else if has(&["slab", "rockwell", "elephant", "clarendon", "egyptian", "memphis"]) {
        Some("Roboto Slab")
    } else if has(&["mono", "console", "consolas", "typewriter", "courier", "lucida sans typewriter"]) {
        Some("Cousine")
    } else if !has(&["sans"])
        && has(&[
            "serif",
            "roman",
            "garamond",
            "bodoni",
            "caslon",
            "baskerville",
            "century",
            "antiqua",
            "palatino",
            "bookman",
            "goudy",
            "cambria",
            "georgia",
            "sylfaen",
            "perpetua",
            "centaur",
            "californian",
            "constantia",
            "minion",
            "times",
            "book",
            "old style",
            "didot",
        ])
    {
        Some("Source Serif 4")
    } else {
        None
    }
}

/// The font families a document names, with the characters set in each: those text runs name
/// (how foliolum-pub writes Publisher's fonts), and those paragraph and character styles name
/// (counted as 0 characters: a style's font draws text without any run naming it).
pub fn document_families(doc: &designcraft_doc::Document) -> std::collections::BTreeMap<String, usize> {
    let mut out: std::collections::BTreeMap<String, usize> = Default::default();
    for story in doc.stories.values() {
        for (range, cf) in story.runs() {
            if let Some(f) = serde_json::to_value(&cf.over).ok().and_then(|v| v["fontFamily"].as_str().map(str::to_string)) {
                *out.entry(f).or_default() += story.text.get(range.clone()).map_or(range.len(), |t| t.chars().count());
            }
        }
    }
    fn walk(v: &serde_json::Value, out: &mut std::collections::BTreeMap<String, usize>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, v) in m {
                    if (k == "fontFamily" || k == "bulletFont")
                        && let Some(f) = v.as_str()
                    {
                        out.entry(f.to_string()).or_default();
                    }
                    walk(v, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    if let Ok(v) = serde_json::to_value(doc) {
        walk(&v, &mut out);
    }
    out
}

/// Registers a substitute for each of a document's `families` that isn't available and isn't in
/// [`SUBSTITUTES`], from [`guess`]. Call it when a document is opened, before it is drawn.
pub fn register_families<'a>(db: &FontDb, families: impl IntoIterator<Item = &'a str>) {
    for family in families {
        if substitute(family).is_some() || db.has_family(family) {
            continue;
        }
        if let Some(g) = guess(family) {
            db.add_substitute(family, &[g]);
        }
    }
}

/// The bundled family that stands in for `family`, if there is one.
pub fn substitute(family: &str) -> Option<(&'static str, Match)> {
    SUBSTITUTES.iter().find(|(f, ..)| f.eq_ignore_ascii_case(family)).map(|(_, s, m)| (*s, *m))
}

/// Adds the bundled fonts to `db` and registers [`SUBSTITUTES`]. Call it once at startup, before
/// any document is drawn; later calls do nothing.
pub fn install(db: &'static FontDb) {
    static DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    DONE.get_or_init(|| {
        for (_, bytes) in FONTS {
            db.add_font(bytes.to_vec());
        }
        for (family, substitute, _) in SUBSTITUTES {
            db.add_substitute(family, &[substitute]);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_substitute_is_a_bundled_family() {
        install(FontDb::global());
        let db = FontDb::global();
        for (family, substitute, _) in SUBSTITUTES {
            assert!(db.has_family(substitute), "{family} → {substitute} is not bundled");
        }
    }

    #[test]
    fn missing_office_fonts_are_drawn_with_their_substitutes() {
        install(FontDb::global());
        let db = FontDb::global();
        // Only meaningful when the original isn't installed on this machine.
        for (family, substitute, _) in SUBSTITUTES.iter().filter(|(f, ..)| !db.has_family(f)) {
            assert_eq!(db.face(family, "Regular").family, *substitute, "{family}");
        }
        assert_eq!(db.face("Calibri", "Bold").style, "Bold");
    }

    #[test]
    fn unknown_families_are_guessed_from_their_names() {
        assert_eq!(guess("Tw Cen MT Condensed"), Some("Roboto Condensed"));
        assert_eq!(guess("Lucida Handwriting"), Some("Dancing Script"));
        assert_eq!(guess("Elephant"), Some("Roboto Slab"));
        assert_eq!(guess("Book Antiqua"), Some("Source Serif 4"));
        assert_eq!(guess("Gill Sans MT"), None);
        assert_eq!(guess("Microsoft Sans Serif"), None);
        install(FontDb::global());
        let db = FontDb::global();
        if !db.has_family("Bodoni MT") {
            register_families(db, ["Bodoni MT"]);
            assert_eq!(db.face("Bodoni MT", "Regular").family, "Source Serif 4");
        }
    }

    #[test]
    fn lookups_ignore_case() {
        assert_eq!(substitute("times new roman"), Some(("Tinos", Match::Metric)));
        assert_eq!(substitute("Gill Sans MT"), None);
    }
}
