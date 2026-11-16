# Phase 1 spike report

Issue #2. Run on 2026-11-16 on the m4 dev Mac (arm64). No product code was written. The recommendations
that follow from this are in `docs/architecture.md`.

## What was tested

| Project | Commit | License |
|---|---|---|
| DesignCraft (`storytold/designcraft`) | `47ef1ec` (main, 16 Nov; after v0.21.0) | MIT OR Apache-2.0 (both files present) |
| NiftyDesign (`srt8hellcat2-ui/niftydesign`) | `ec75466` (branch `nifty-update-0.18.0`) | MIT OR Apache-2.0 |
| Pubwork (`Daffyllas/Pubwork`) | `8dd03e1` | MPL-2.0 |
| libmspub (Homebrew) | 0.1.5 | MPL-2.0 |

**Test files.** 22 public .pub files: 21 from Apache POI's `test-data/publisher` (Apache-2.0) and one
from LibreOffice's libmspub tests. They include Publisher 98, 2000, 2002–2007 and 2010 saves, a
real-looking 4-page newsletter and a 2-page brochure. These are **not** the gate corpus: they are small,
mostly English, made by Microsoft, and not parish bulletins. The 50-file gate still needs real files.

## Findings

### 1. DesignCraft is what the plan says, and it moves even faster than the plan says

- **Stage:** pre-alpha. ROADMAP says "next: alpha, ~5–7 h away" (updated 15 Nov).
- **Age:** first commit 2026-10-01, 5,548 commits since.
- **Pace:** 578 commits in the last 7 days and 2,888 in the last 30, from about 80 authors (people,
  bots and agents). Tags ship every 1–3 days.
- **Layering:** clean, from L0 geometry to L7 apps (`docs/architecture.md`), with an enforced rule
  that nothing below the UI uses egui. The engine (doc, compose, render, pdf, engine) is reusable as
  libraries.
- **What we can't extend from outside:**
  - the command registry, menus, panels and file-open routing are fixed tables compiled in;
  - there is no plugin API and no stability promise;
  - none of the crates are on crates.io: 52 dependencies are pinned to `storytold/craft-libs` git
    revisions, and egui, winit and krilla are patched to storytold forks.
- **Web build:** it has no control channel or MCP, as the plan said.
- **Brand:** the ArtCraft name and logos are trademarks with their own license. A derivative must
  remove them and may only credit "based on DesignCraft by the ArtCraft team" in plain text.

### 2. The web build builds

- `trunk build --release` in `apps/designcraft-web` succeeded: 5.5 min on m4 with 16 cores, after
  trunk 0.22.1 took 5.4 min to install.
- Trunk fetched wasm-bindgen 0.2.129 and wasm-opt `version_131` itself.
- **Output:** `designcraft-web_bg.wasm` is **69.6 MB** (26.6 MB gzipped), plus 164 KB of JS.
  - Upstream's `packaging/web/README.md` says "about 13 MB, or 5 MB with compression".
  - The 5× gap is unexplained. Wasm-opt `-Oz` is configured, so either it didn't apply or upstream's
    number is out of date.
  - **Open item:** size matters for parish volunteers on slow church Wi-Fi. Measured again in Phase 1
    with a single, low-priority build.
- **Not yet verified:** the app running in a real browser. I stopped running heavy jobs because the
  shared Mac was overloaded (see "Process note" below).

### 3. Our crates can attach from outside, without a fork

Probe: a separate Cargo workspace (no files inside DesignCraft changed) with:
- `foliolum-pub`, which is NiftyDesign's `crates/pub` copied as-is;
- a tiny CLI that imports a .pub, calls `DocState::new` + `Session::add_document`, then
  `Session::execute("file.exportPdf")`.

Results:
- It depends on DesignCraft's crates by `path`. DesignCraft's own `workspace = true` inheritance
  resolved, and the `[patch.crates-io]` block copied from upstream was the only setup needed.
- **Drift:** the importer was written against v0.18.0, 813 upstream commits ago. It failed to compile
  in **2 lines**, both the same change (`Arc::new(bytes)` became `AssetData::embedded(bytes)`). After
  that fix it produced **the same 19/22 PDFs** as NiftyDesign's own build.

So the "unchanged engine + our own crates" plan holds for import and export. The places it won't hold
are the UI (menus and ribbon) and the open-file wiring. Those are listed in `docs/architecture.md`
with a plan for each.

### 4. The .pub importers

| File | Stock libmspub 0.1.5 pages | Pubwork reader: pages / text boxes / pictures / shapes | NiftyDesign importer → PDF |
|---|---|---|---|
| `lo-fdo59355-1.pub` | 2 | 1 / 1 / 0 / 0 | **refused** (Publisher 97/98/2000) |
| `poi-51318.pub` | 1 | 1 / 2 / 1 / 11 | PDF, 1 p, clean |
| `poi-60685.pub` | 1 | 1 / 4 / 0 / 53 | PDF, 1 p, clean |
| `poi-LinkAt0And10.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-LinkAt10.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-LinkAt10And20And30.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-LinkAt10And20And30And40.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-LinkAt10Longer.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-LinkAt20.pub` | 1 | 1 / 1 / 0 / 0 | PDF, 1 p, clean |
| `poi-Sample.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample2.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample2000.pub` | 3 | 2 / 5 / 0 / 1 | **refused** (Publisher 97/98/2000) |
| `poi-Sample2_2010.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample3.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample3_2010.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample4.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample4_2010.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Sample98.pub` | 3 | 2 / 5 / 0 / 4 | **refused** (Publisher 97/98/2000) |
| `poi-SampleBrochure.pub` | **0 (silent failure)** | 2 / 20 / 8 / 40 | PDF, 2 p, 3 warning(s) |
| `poi-SampleNewsletter.pub` | **0 (silent failure)** | 4 / 63 / 10 / 76 | PDF, 4 p, 9 warning(s) |
| `poi-Sample_2010.pub` | 2 | 2 / 5 / 0 / 1 | PDF, 2 p, 2 warning(s) |
| `poi-Simple.pub` | 1 | 1 / 2 / 0 / 0 | PDF, 1 p, clean |

**Stock libmspub 0.1.5** returned zero pages, with exit code 0, for the two most realistic files.
That rules out any plan that uses libmspub without Pubwork's patches.

**Pubwork's reader** (prebuilt `pubreader.wasm`, 494 KB, from libmspub `652b2be` plus a 243-line patch,
with librevenge, zlib and header-only Boost; no GPL code) read all 22 files, including Publisher 98 and
2000, in 1–84 ms each under Node.
- Its output is a JSON stream of librevenge drawing calls. Pubwork's `pubmodel.js` turns that into a
  page model: pages, masters, text with paragraphs and runs, images, shapes and tables, in points.
- It is reusable on its own (the `.wasm`, its loader and `pubmodel.js`), and it is the best fidelity
  oracle available.
- **Its limits:** text wrap isn't read, and linked boxes each receive the whole story.

**NiftyDesign's importer** (pure Rust, about 5,600 lines, MIT OR Apache-2.0) produced PDFs for 19 of
22 files and refused the 3 Publisher 97/98/2000 files by design.
- Visual check of the newsletter and brochure: **recognizable**. Layout, columns, colors, pictures and
  WordArt-style headings all come across.
- Losses seen:
  - **Fonts.** Publisher fonts that aren't installed get substituted, so text is wider. Headlines lose
    their last words ("Living and Learning in ~~Class X~~"), and 9 frames overflow. Wingdings bullets
    render as empty boxes.
  - **Text wrap** isn't applied: a clip-art picture sits on top of body text in the brochure.
  - **Fields** (page numbers and dates) are dropped.
  - **Table cell fills, borders and padding** aren't decoded.
- **Provenance:** NiftyDesign documents a clean-room process (`docs/CLEANROOM.md`). That is
  self-reported and only partly applied: 3 modules were rebuilt from notes, and the notes' author
  could see an earlier in-house parser. Even in the worst case (derived from libmspub) the code would
  be MPL-2.0, which we could still use. **The risk is low, but it's not zero, so we keep its
  attribution and its provenance doc with the code.**
- **Maintenance:** one maintainer, 2 stars, sits on v0.18.0 and doesn't merge upstream. The same
  importer is proposed upstream as storytold/designcraft#2126 (open since 1 Nov, +7,844 lines, no
  review yet).

### 5. What the 50-file gate will be fought over

In order of expected impact on "renders recognizably":

1. **Font mapping.** Map Publisher's common fonts (Times New Roman, Arial, Calibri, Cambria,
   Garamond, Wingdings and the rest) to metric-compatible open fonts (Liberation, Carlito, Caladea,
   and others). Map symbol fonts to Unicode. This is likely the single largest fidelity gain.
2. **Publisher 97–2000 files.** Parish archives go back decades. We either add Pubwork's reader as a
   fallback for old files or extend the Rust importer. **The real gate files will show how common
   they are.**
3. Text wrap, fields (page numbers and dates), and table cell formatting.
4. Web bundle size (see 2).

## Process note

My builds overloaded the shared m4 (load 247, Derek's terminal app crashed). I had run three heavy
release builds at once while other sessions were busy. From now on every build runs at `nice -n 15`
with `CARGO_BUILD_JOBS=4`, one at a time, and in debug except for deliberate size measurements.
Nothing was lost.

## Not done in this spike

- Running the web build in a browser (deferred because of load; first task of Phase 1).
- A CI build on the `doodlemanage-foliolum` runner. There is no shell there by design, and the first
  CI workflow comes with the Phase 1 PR. Its contents are recorded in `docs/architecture.md`.
- Testing on real parish files (the gate set is still needed).
