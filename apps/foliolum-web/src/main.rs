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
