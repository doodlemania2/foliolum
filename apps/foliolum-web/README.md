# foliolum-web

Foliolum in the browser: DesignCraft's editor (`designcraft-ui-egui`) through eframe's web runner, with
Foliolum's shell around it. Publisher files open from File ▸ Open or by dropping them on the window
(`src/pub_open.rs`), and Office fonts are drawn with bundled open substitutes (`crates/foliolum-fonts`).

The shell is DesignCraft's `apps/designcraft-web` (MIT OR Apache-2.0) at the pinned commit, with our
names and element ids. `browser_policy.rs` and `graphics.rs` are unchanged copies; `web.rs` adds .pub to
the open dialog and hands Publisher files to `pub_open` before DesignCraft sees the inbox.

```sh
fork/fetch.sh                 # DesignCraft at the pin, our patches, and the bundled fonts
cd apps/foliolum-web
trunk build                   # → dist/web (the size-tuned "web" profile, then wasm-opt -Oz)
trunk serve                   # http://127.0.0.1:8768 ; ?webgl forces WebGL2
```

On the shared m4: `nice -n 15 env CARGO_BUILD_JOBS=4 trunk build` (about 5 minutes).

## Checked so far (2026-11-18, pin 47ef1ec)

- Starts in Chrome on both WebGPU and WebGL2 (`?webgl`), and a dropped .pub opens as a document, with
  the import summary in the status bar.
- Size: the `.wasm` is 55.2 MB (24.6 MB gzip, 16.8 MB brotli), against 69.6 MB for DesignCraft's own
  web build. The `web` profile (`opt-level = "s"`, fat LTO, one codegen unit) took the code section from
  51.9 MB to 30.3 MB; the data section is 24.8 MB, about 12 MB of it our bundled fonts, which can move to
  on-demand loading.

## Not yet

- Branding: patch 0002 replaces DesignCraft's ArtCraft branding with Foliolum's (checked in Chrome: start
  screen, title bar, Help menu, About) and the ArtCraft mark isn't in the build. The word "ArtCraft" is
  still in the wasm about 128 times, in DesignCraft's translation tables and link constants for screens
  Foliolum no longer shows.
- DesignCraft highlights text whose named font isn't installed, which marks almost every Publisher file
  even though a substitute draws it.
