//! The fonts Foliolum ships, and which of them draws each font a Publisher file asks for.
//!
//! Publisher files name Windows and Office fonts, which a browser doesn't have and which we can't
//! redistribute. DesignCraft draws a missing family with Source Sans 3, which is wider or narrower than
//! the original, so headlines overflow and columns break differently. Foliolum bundles open fonts
//! instead: metric-compatible ones (same advance widths, so the text breaks where it did in
//! Publisher) where they exist, and close look-alikes otherwise.
//!
//! The files are fetched by `fetch.sh` (from `fonts.lock`, checked by SHA-256). With the `embed`
//! feature (the default: the CLI, the gate and the tests) they are compiled in. The web app turns it
//! off and fetches them after it starts, from the content-named copies `fetch.sh` writes to `web/`
//! ([`web_files`]), so they stay out of the wasm and stay cached across deploys. All are SIL Open Font
//! License 1.1 except Roboto Slab (Apache 2.0); each family's licence text ships beside its fonts
//! (see [`LICENSES`]).
//!
//! A family that is neither installed nor in [`SUBSTITUTES`] gets a guess from its name ([`guess`]):
//! a condensed name gets a condensed font, a serif name a serif, and so on, registered per document
//! by [`register_families`].

use designcraft_fonts::FontDb;

/// Every bundled font file, by its path under `fonts/` (and in `fonts.lock`).
pub const FONT_PATHS: &[&str] = &[
    "carlito/Carlito-Regular.ttf",
    "carlito/Carlito-Bold.ttf",
    "carlito/Carlito-Italic.ttf",
    "carlito/Carlito-BoldItalic.ttf",
    "caladea/Caladea-Regular.ttf",
    "caladea/Caladea-Bold.ttf",
    "caladea/Caladea-Italic.ttf",
    "caladea/Caladea-BoldItalic.ttf",
    "arimo/Arimo[wght].ttf",
    "arimo/Arimo-Italic[wght].ttf",
    "tinos/Tinos-Regular.ttf",
    "tinos/Tinos-Bold.ttf",
    "tinos/Tinos-Italic.ttf",
    "tinos/Tinos-BoldItalic.ttf",
    "cousine/Cousine-Regular.ttf",
    "cousine/Cousine-Bold.ttf",
    "cousine/Cousine-Italic.ttf",
    "cousine/Cousine-BoldItalic.ttf",
    "gelasio/Gelasio[wght].ttf",
    "gelasio/Gelasio-Italic[wght].ttf",
    "librefranklin/LibreFranklin[wght].ttf",
    "librefranklin/LibreFranklin-Italic[wght].ttf",
    "ebgaramond/EBGaramond[wght].ttf",
    "ebgaramond/EBGaramond-Italic[wght].ttf",
    "comicneue/ComicNeue-Regular.ttf",
    "comicneue/ComicNeue-Bold.ttf",
    "comicneue/ComicNeue-Italic.ttf",
    "comicneue/ComicNeue-BoldItalic.ttf",
    "questrial/Questrial-Regular.ttf",
    "notosanssymbols2/NotoSansSymbols2-Regular.ttf",
    "notosanssymbols/NotoSansSymbols[wght].ttf",
    "robotocondensed/RobotoCondensed[wght].ttf",
    "robotocondensed/RobotoCondensed-Italic[wght].ttf",
    "robotoslab/RobotoSlab[wght].ttf",
    "dancingscript/DancingScript[wght].ttf",
];

/// Each bundled family's licence text, by its path under `fonts/`.
pub const LICENSE_PATHS: &[&str] = &[
    "carlito/OFL.txt",
    "caladea/OFL.txt",
    "arimo/OFL.txt",
    "tinos/OFL.txt",
    "cousine/OFL.txt",
    "gelasio/OFL.txt",
    "librefranklin/OFL.txt",
    "ebgaramond/OFL.txt",
    "comicneue/OFL.txt",
    "questrial/OFL.txt",
    "notosanssymbols2/OFL.txt",
    "notosanssymbols/OFL.txt",
    "robotocondensed/OFL.txt",
    "robotoslab/LICENSE.txt",
    "dancingscript/OFL.txt",
];

#[cfg(feature = "embed")]
macro_rules! embedded {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_bytes!(concat!("../fonts/", $path)) as &[u8])),*]
    };
}

/// Every bundled font file's bytes, by its path under `fonts/` (the same files as [`FONT_PATHS`]).
#[cfg(feature = "embed")]
pub const FONTS: &[(&str, &[u8])] = embedded![
    "carlito/Carlito-Regular.ttf",
    "carlito/Carlito-Bold.ttf",
    "carlito/Carlito-Italic.ttf",
    "carlito/Carlito-BoldItalic.ttf",
    "caladea/Caladea-Regular.ttf",
    "caladea/Caladea-Bold.ttf",
    "caladea/Caladea-Italic.ttf",
    "caladea/Caladea-BoldItalic.ttf",
    "arimo/Arimo[wght].ttf",
    "arimo/Arimo-Italic[wght].ttf",
    "tinos/Tinos-Regular.ttf",
    "tinos/Tinos-Bold.ttf",
    "tinos/Tinos-Italic.ttf",
    "tinos/Tinos-BoldItalic.ttf",
    "cousine/Cousine-Regular.ttf",
    "cousine/Cousine-Bold.ttf",
    "cousine/Cousine-Italic.ttf",
    "cousine/Cousine-BoldItalic.ttf",
    "gelasio/Gelasio[wght].ttf",
    "gelasio/Gelasio-Italic[wght].ttf",
    "librefranklin/LibreFranklin[wght].ttf",
    "librefranklin/LibreFranklin-Italic[wght].ttf",
    "ebgaramond/EBGaramond[wght].ttf",
    "ebgaramond/EBGaramond-Italic[wght].ttf",
    "comicneue/ComicNeue-Regular.ttf",
    "comicneue/ComicNeue-Bold.ttf",
    "comicneue/ComicNeue-Italic.ttf",
    "comicneue/ComicNeue-BoldItalic.ttf",
    "questrial/Questrial-Regular.ttf",
    "notosanssymbols2/NotoSansSymbols2-Regular.ttf",
    "notosanssymbols/NotoSansSymbols[wght].ttf",
    "robotocondensed/RobotoCondensed[wght].ttf",
    "robotocondensed/RobotoCondensed-Italic[wght].ttf",
    "robotoslab/RobotoSlab[wght].ttf",
    "dancingscript/DancingScript[wght].ttf",
];

/// Each bundled family's licence text, by its path under `fonts/`.
#[cfg(feature = "embed")]
pub const LICENSES: &[(&str, &[u8])] = embedded![
    "carlito/OFL.txt",
    "caladea/OFL.txt",
    "arimo/OFL.txt",
    "tinos/OFL.txt",
    "cousine/OFL.txt",
    "gelasio/OFL.txt",
    "librefranklin/OFL.txt",
    "ebgaramond/OFL.txt",
    "comicneue/OFL.txt",
    "questrial/OFL.txt",
    "notosanssymbols2/OFL.txt",
    "notosanssymbols/OFL.txt",
    "robotocondensed/OFL.txt",
    "robotoslab/LICENSE.txt",
    "dancingscript/OFL.txt",
];

/// `fonts.lock`: each file's SHA-256 and path.
const LOCK: &str = include_str!("../fonts.lock");

/// The name `fetch.sh` gives a copy of the file at `path` (under `fonts/`) in `web/`: the first 16
/// hex digits of its SHA-256, then its file name with anything but letters, digits, `.`, `_` and `-`
/// made `_` (`Arimo[wght].ttf` → `Arimo_wght_.ttf`). The name changes whenever the bytes do, so the
/// web server can let browsers cache it for good.
pub fn web_name(path: &str) -> Option<String> {
    let hash = LOCK.lines().filter(|l| !l.starts_with('#')).find_map(|l| {
        let mut f = l.split_whitespace();
        let (hash, name) = (f.next()?, f.next()?);
        (name == path).then_some(hash)
    })?;
    let file: String =
        path.rsplit('/').next()?.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect();
    Some(format!("{}-{file}", &hash[..16]))
}

/// The web copies of the bundled fonts, by their names in `web/` ([`web_name`]), in [`FONT_PATHS`] order.
pub fn web_files() -> Vec<String> {
    FONT_PATHS.iter().map(|p| web_name(p).unwrap_or_else(|| panic!("{p} is not in fonts.lock"))).collect()
}

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
#[cfg(feature = "embed")]
pub fn install(db: &'static FontDb) {
    static DONE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    DONE.get_or_init(|| {
        for (_, bytes) in FONTS {
            db.add_font(bytes.to_vec());
        }
        register_substitutes(db);
    });
}

/// Registers [`SUBSTITUTES`] in `db`. A substitute that isn't loaded yet is skipped at lookup time,
/// so the web app calls this at startup and adds the font files as they arrive.
pub fn register_substitutes(db: &FontDb) {
    for (family, substitute, _) in SUBSTITUTES {
        db.add_substitute(family, &[substitute]);
    }
}

#[cfg(all(test, feature = "embed"))]
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
    fn symbols_from_symbol_fonts_are_drawn_by_a_bundled_font() {
        // As in the browser: no installed fonts, only the bundled ones and DesignCraft's own.
        let db = FontDb::with_font_dirs(Vec::new());
        db.set_system_fallback(false);
        for (_, bytes) in FONTS {
            db.add_font(bytes.to_vec());
        }
        // What foliolum-pub maps Wingdings and Symbol text to that parish documents use: crosses,
        // the Star of David, a telephone, an envelope, scissors, check marks and boxes, circled
        // numbers, stars, arrows, Greek, ©, ® and ™, the degree sign, card suits.
        for c in "✠✞🕆🕈✡🕿✆✉✂✔✓☑☒①❶★✪⇨☞●◆❖αβπΣΩ©®™°±♥♣→".chars() {
            assert!(db.fallback_for(c, u32::MAX, None).is_some(), "no bundled font draws {c} (U+{:04X})", c as u32);
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
    fn every_font_has_a_content_named_web_copy() {
        let names = web_files();
        assert_eq!(names.len(), FONT_PATHS.len());
        assert_eq!(names[0], "f6418f708baede97-Carlito-Regular.ttf");
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "two fonts share a web name");
        assert_eq!(web_name("no/such.ttf"), None);
        assert!(web_name("arimo/Arimo[wght].ttf").unwrap().ends_with("-Arimo_wght_.ttf"));
        // fetch.sh writes the copies; this checks it names them the same way.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
        for name in &names {
            assert!(dir.join(name).is_file(), "fetch.sh wrote no web/{name}");
        }
    }

    #[test]
    fn the_embedded_fonts_are_the_listed_ones() {
        assert_eq!(FONTS.iter().map(|(p, _)| *p).collect::<Vec<_>>(), FONT_PATHS);
        assert_eq!(LICENSES.iter().map(|(p, _)| *p).collect::<Vec<_>>(), LICENSE_PATHS);
    }

    #[test]
    fn lookups_ignore_case() {
        assert_eq!(substitute("times new roman"), Some(("Tinos", Match::Metric)));
        assert_eq!(substitute("Gill Sans MT"), None);
    }
}
