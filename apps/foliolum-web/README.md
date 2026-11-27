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
  Every button has an icon: DesignCraft's where one fits, otherwise one of ours drawn the same way
  (`crates/foliolum-shell/src/icons.rs`). The strip on the left shows only the document's pages,
  spreads side by side and numbered, and a click goes to that page (`crates/foliolum-shell/src/pages.rs`);
  parent pages stay in the full editor's Pages panel.

- The tab icon is Foliolum's mark (`favicon.svg`, the marketing site's file, and `favicon.ico`
  rendered from it by `cargo xtask favicon`), served under fixed names and cached for a day.

- Fonts load after the editor starts (2026-11-21): the 34 bundled font files are served beside the
  `.wasm` under content-hashed names (`fonts/`, cached for good) instead of compiled in, so the `.wasm`
  went from 63.1 MB to 50.8 MB (28.0 MB to 21.6 MB gzip, thin LTO as deployed) and a new deploy no
  longer re-sends 6.4 MB of fonts. A file opened or dropped before they arrive waits ("Loading
  fonts…" in the status bar), so it is never laid out with fallback fonts; if they can't be fetched it
  opens anyway and the status bar says so. Checked in Chrome under the production CSP: Times New Roman
  and Arial files export PDFs set in Tinos and Arimo, including one dropped while the fonts were held
  back 8 seconds.

## Phase 2 tools (Insert and Home tabs)

- **Smart frame** (`src/smart.rs`, #52): select a text box and choose Insert > Smart frame. Type the
  data into a grid, or load a CSV or .xlsx file. The box shows it as a table or as one line per row
  from a pattern such as `{Day}  {Time}  {Intention}`. The data travels in the frame's label, so
  opening the dialog on the box again edits it.
- **Our details** (`src/profile.rs`, #53): name, address, phone, email and website, kept in this
  browser (local storage key `foliolum.profile`). Each detail goes in as a text variable, and saving
  the profile updates every one the document uses.
- **Make it fit** (`src/copyfit.rs`, #54): Home > Make it fit tightens the selected box's tracking
  (by up to 25/1000 em), then its size and fixed leading (by up to 2 pt), until the text fits. If it
  can't fit within those limits, nothing changes.

Each is one undo step (`src/undo.rs`).

## Parish templates (start screen)

`src/starters.rs` (#56): a weekly bulletin, an event flyer, a sign-up sheet and a newsletter, as cards on
the start screen (fork patch 0007). Picking one builds it with the editor's own commands as a new,
untitled document, so no template files ship. The parish details are Our details fields filled from the
details saved in this browser (placeholders where none are saved), and the lists are smart frames
holding sample rows. The tests build each template with the bundled fonts and check that no text
overflows and that it exports. To look at them:
`FOLIOLUM_STARTERS_OUT=<dir> cargo test -p foliolum-web write_template_pdfs -- --ignored`.

## Send for signature

File ▸ Send for signature sends the open document, as a PDF, to TDPdf Sign (https://sign.tdpdf.com),
which emails each person a link to sign it (`src/sign.rs`, `src/sign_web.rs`). People mark where each
signer signs by typing `{{signature, r1}}` (r2 for the second signer, and so on; `{{date, r1}}` and
`{{name, r1}}` too) in a text box: Sign finds them in the PDF and puts fields there. When every signer has
one, the document is sent at once; otherwise it is left as a draft in Sign to finish there.

- The person pastes their own Sign API token. It is kept in memory only, until the tab closes. There is no
  shared or service token.
- The browser calls Sign directly, so the CSP's `connect-src` names `https://sign.tdpdf.com`
  (`deploy/image/prepare.sh`; the smoke test checks it).
- Signing in to Sign from the browser (the PKCE flow TDPdf uses) needs Sign to accept
  `https://app.foliolum.app` as a redirect, which it doesn't yet.

## Not yet

- Branding: patch 0002 replaces DesignCraft's ArtCraft branding with Foliolum's (checked in Chrome: start
  screen, title bar, Help menu, About) and the ArtCraft mark isn't in the build. The word "ArtCraft" is
  still in the wasm about 128 times, in DesignCraft's translation tables and link constants for screens
  Foliolum no longer shows.
- Simple editor: the words are English only, and DesignCraft's dialogs and panels keep their own wording.
