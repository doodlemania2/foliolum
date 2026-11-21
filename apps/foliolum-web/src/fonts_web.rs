//! The bundled fonts, fetched after the editor starts instead of compiled into the wasm.
//!
//! foliolum-fonts' `fetch.sh` writes each font under a name that starts with its SHA-256
//! ([`foliolum_fonts::web_files`]); trunk copies them to `fonts/` beside the wasm, and the web server
//! lets browsers cache them for good. So the wasm is about 12 MB smaller, and a new deploy doesn't
//! make anyone download the fonts again.
//!
//! The shell holds every file that arrives (opened or dropped) until [`settled`], so a document is
//! never laid out with fallback fonts and then left that way.
use std::sync::atomic::{AtomicUsize, Ordering};
use wasm_bindgen::JsCast as _;
use wasm_bindgen_futures::JsFuture;

/// Fonts still on their way. `usize::MAX` until [`start`] runs.
static PENDING: AtomicUsize = AtomicUsize::new(usize::MAX);
/// Fonts that couldn't be fetched or read.
static FAILED: AtomicUsize = AtomicUsize::new(0);

/// Registers the substitutes and starts fetching every bundled font. Each font is added to the
/// global font database as it arrives, and `ctx` repaints so the shell sees it.
pub fn start(ctx: egui::Context) {
    let db = designcraft_fonts::FontDb::global();
    foliolum_fonts::register_substitutes(db);
    let files = foliolum_fonts::web_files();
    PENDING.store(files.len(), Ordering::SeqCst);
    for name in files {
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match fetch(&format!("fonts/{name}")).await {
                Ok(bytes) => {
                    if db.add_font(bytes) == 0 {
                        fail(&name, "not a font this editor can read");
                    }
                }
                Err(e) => fail(&name, &e),
            }
            PENDING.fetch_sub(1, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }
}

/// Has every font arrived or failed?
pub fn settled() -> bool {
    PENDING.load(Ordering::SeqCst) == 0
}

/// How many fonts couldn't be loaded.
pub fn failed() -> usize {
    FAILED.load(Ordering::SeqCst)
}

fn fail(name: &str, why: &str) {
    log::error!("font {name}: {why}");
    FAILED.fetch_add(1, Ordering::SeqCst);
}

async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response =
        JsFuture::from(window.fetch_with_str(url)).await.map_err(|e| format!("{e:?}"))?.dyn_into().map_err(|e| format!("{e:?}"))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = JsFuture::from(response.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}
