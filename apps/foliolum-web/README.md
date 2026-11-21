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

- Substituted fonts aren't highlighted. DesignCraft puts pink behind text whose named font isn't
  installed, which marked nearly every line of a Publisher file even though a bundled substitute draws
  it. Foliolum starts with that preference off (`new_session` in `src/main.rs`); Preferences › Composition
  can turn it back on, and the Preflight panel still lists every substituted font. Checked in Chrome
  with the sample newsletter.

- The simple editor (`crates/foliolum-shell`, patch 0003): a ribbon with File, Home, Insert and Page
  tabs in place of DesignCraft's menus and Tools panel, the pages down the left, and a "Full editor"
  switch in the app bar. Checked in Chrome (2026-11-20): tabs switch, Text box picks the Type tool,
  buttons grey out when they can't act, and the switch goes to the full editor and back.

- Fonts load after the editor starts (2026-11-21): the 34 bundled font files are served beside the
  `.wasm` under content-hashed names (`fonts/`, cached for good) instead of compiled in, so the `.wasm`
  went from 63.1 MB to 50.8 MB (28.0 MB to 21.6 MB gzip, thin LTO as deployed) and a new deploy no
  longer re-sends 6.4 MB of fonts. A file opened or dropped before they arrive waits ("Loading
  fonts…" in the status bar), so it is never laid out with fallback fonts; if they can't be fetched it
  opens anyway and the status bar says so. Checked in Chrome under the production CSP: Times New Roman
  and Arial files export PDFs set in Tinos and Arimo, including one dropped while the fonts were held
  back 8 seconds.

## Not yet

- Branding: patch 0002 replaces DesignCraft's ArtCraft branding with Foliolum's (checked in Chrome: start
  screen, title bar, Help menu, About) and the ArtCraft mark isn't in the build. The word "ArtCraft" is
  still in the wasm about 128 times, in DesignCraft's translation tables and link constants for screens
  Foliolum no longer shows.
- Simple editor: buttons without one of DesignCraft's icons are text only (ours to draw), the words are
  English only, DesignCraft's dialogs and panels keep their own wording, and the page strip also lists
  parent pages.
