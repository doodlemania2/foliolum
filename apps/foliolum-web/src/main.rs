//! Foliolum in the browser.
//!
//! DesignCraft's editor ([`designcraft_ui_egui::DesignApp`]) through eframe's web runner (wgpu:
//! WebGPU where available, WebGL2 otherwise), with Foliolum's shell around it: Publisher files open
//! from File ▸ Open or by dropping them ([`pub_open`]), and Office fonts are drawn with bundled open
//! substitutes (foliolum-fonts). Build with `trunk build` from this directory (see README.md).
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

#[cfg(any(target_arch = "wasm32", test))]
mod graphics;

#[cfg_attr(not(any(target_arch = "wasm32", test)), allow(dead_code))]
mod pub_open;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("foliolum-web only runs in the browser: build it with `trunk build` in apps/foliolum-web");
}
