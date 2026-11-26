//! Foliolum in the browser.
//!
//! DesignCraft's editor ([`designcraft_ui_egui::DesignApp`]) through eframe's web runner (wgpu:
//! WebGPU where available, WebGL2 otherwise), with Foliolum's shell around it: Publisher files open
//! from File ▸ Open or by dropping them ([`pub_open`]), Office fonts are drawn with bundled open
//! substitutes (foliolum-fonts), and the simple editor (foliolum-shell) puts a ribbon and the pages
//! around the canvas. Build with `trunk build` from this directory (see README.md).
//!
//! The shell is DesignCraft's `apps/designcraft-web` (MIT OR Apache-2.0) at the pinned commit, renamed
//! and extended; `browser_policy.rs` and `graphics.rs` are unchanged.
//!
//! URL query flags: `?webgl` forces the WebGL2 backend instead of WebGPU; `?sample` opens
//! DesignCraft's sample document on start.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(any(target_arch = "wasm32", test))]
mod browser_policy;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
mod fonts_web;

#[cfg(any(target_arch = "wasm32", test))]
mod graphics;

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
mod pub_open;

/// Foliolum's name and links in DesignCraft's UI, in place of the ArtCraft mark and community, which only
/// DesignCraft may show (fork/patches/0002-ui-embedder-brand.patch).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const BRAND: designcraft_ui_egui::brand::Brand = designcraft_ui_egui::brand::Brand {
    name: "Foliolum",
    version: env!("CARGO_PKG_VERSION"),
    tagline: "Page layout for people Publisher left behind: bulletins, newsletters, flyers and programs.",
    artcraft: false,
    credit: Some("Based on DesignCraft by the ArtCraft team. Not affiliated with Microsoft."),
    links: &[("Source code", "https://github.com/doodlemania2/foliolum"), ("Report a problem", "https://github.com/doodlemania2/foliolum/issues")],
    mark: QUILL,
    mark_view: 64.0,
    accent: Some(ACCENT),
};

/// Foliolum's mark, the quill of `favicon.svg`: a rubric leaf and a paper leaf on a paper tile.
const QUILL: &[designcraft_ui_egui::brand::MarkShape] = {
    use designcraft_ui_egui::brand::MarkShape;
    &[
        MarkShape {
            path: "M12 0H52C58.63 0 64 5.37 64 12V52C64 58.63 58.63 64 52 64H12C5.37 64 0 58.63 0 52V12C0 5.37 5.37 0 12 0Z",
            fill: Some(PAPER),
            stroke: None,
        },
        MarkShape { path: "M14 50C14 29 29 14 50 14Z", fill: Some(RUBRIC), stroke: None },
        MarkShape { path: "M14 50L50 14C50 35 35 50 14 50Z", fill: Some(PAPER), stroke: Some((INK, 3.0)) },
        MarkShape { path: "M14 50L8 56", fill: None, stroke: Some((INK, 4.0)) },
    ]
};

/// The brand colours (foliolum-site's style.css): paper, ink and rubric.
const PAPER: u32 = 0xfbf8f2;
const INK: u32 = 0x1f2430;
const RUBRIC: u32 = 0xb3322a;

/// Rubric in place of DesignCraft's blue. On dark chrome links take the site's dark-mode rubric
/// (#e58a82) and filled buttons the rubric itself, which keeps white text at 6:1.
const ACCENT: designcraft_ui_egui::brand::Accent = {
    use designcraft_ui_egui::brand::{Accent, AccentSet};
    Accent {
        dark: AccentSet { accent: 0xe58a82, strong: RUBRIC, row: 0x7a3631 },
        light: AccentSet { accent: RUBRIC, strong: 0x8e241e, row: 0xf1d2ce },
    }
};

/// A new editing session with Foliolum's defaults.
///
/// DesignCraft highlights text set in a font that isn't installed (Preferences › Composition ›
/// Highlight › Substituted Fonts). Almost every Publisher file names Office fonts, and in Foliolum
/// each of them is drawn by a bundled substitute or, failing that, DesignCraft's default sans
/// ([`foliolum_fonts::register_families`]), so the highlight would mark nearly all the text in a
/// file that came over correctly. Foliolum starts with it off; the checkbox still turns it on, and
/// the Preflight panel (Window ▸ Preflight) still lists every substituted font for anyone sending a file to a printer.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn new_session() -> designcraft_engine::Session {
    let mut session = designcraft_engine::Session::new();
    session.prefs.highlight_substituted_fonts = false;
    session
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("foliolum-web only runs in the browser: build it with `trunk build` in apps/foliolum-web");
}

#[cfg(test)]
mod tests {
    #[test]
    fn substituted_fonts_are_not_highlighted_by_default() {
        assert!(designcraft_engine::Session::new().prefs.highlight_substituted_fonts, "DesignCraft's default, which Foliolum changes");
        assert!(!super::new_session().prefs.highlight_substituted_fonts);
    }
}
