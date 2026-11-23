# Foliolum architecture

Decided by Derek on 2026-11-16 (issues #3 and #4). The evidence is in `docs/spike-report.md`.

## Summary

Foliolum is a **distribution of DesignCraft**, kept the way our documenso build is kept:

- **DesignCraft is pinned, not copied.** `fork/pins.env` names the upstream commit.
- **Our upstream changes live in `fork/patches/`** as `git apply` patches. Each is checked against
  `fork/patches/SHA256SUMS` before it is applied. The series stays small.
- **Our own code is our own crates** in a Cargo workspace at the repo root. They depend on
  DesignCraft's crates by path, inside a gitignored, patched checkout at `upstream/designcraft/`.
- **Adopting a newer DesignCraft** goes through the `fork-sync` skill
  (`.claude/skills/fork-sync/SKILL.md`). It produces a weekly bump PR, and Derek merges it.

The spike tested this. A crate outside DesignCraft's workspace, depending on its crates by path, built
against upstream `main` at `47ef1ec` (16 Nov), imported .pub files into the upstream engine, and
exported PDFs, **with no upstream file changed**.

## Repo layout

```
foliolum/
  fork/
    pins.env                   UPSTREAM_REPO, UPSTREAM_COMMIT (+ the upstream tag it follows, if any)
    patches/NNNN-*.patch       our changes to DesignCraft, applied in order
    patches/SHA256SUMS
    fetch.sh                   clone upstream at the pin into upstream/designcraft, verify, apply patches
    README.md                  what each patch does and why, plus how to regenerate them
  upstream/designcraft/        (gitignored) the patched checkout our workspace builds against
  crates/
    foliolum-pub/              .pub import → designcraft_doc::Document          (Phase 1)
    foliolum-fonts/            Publisher font mapping and metric-compatible substitutes (Phase 1)
    foliolum-shell/            Publisher mode: ribbon, simpler dialogs, our words and icons (Phase 1)
  apps/
    foliolum-web/              web entry point (trunk, wasm32), our branding, .pub open wiring
    foliolum-cli/              headless import and PDF export, used by the 50-file gate
  gate/                        gate manifest: per-file hashes and expected results, never the files
  deploy/                      the editor's image (nginx, CSP) and its edge stack (app.foliolum.app)
  xtask/                       fetch, sync-workspace, gate, wasm
  Cargo.toml                   our workspace; [patch.crates-io] and profiles copied from upstream
  rust-toolchain.toml
  presence/features.json
  docs/
```

**Why this model and not a full copy of DesignCraft:** upstream adds about 100 commits a day (2,888
in the last 30 days, from about 80 authors). With a full copy, every sync would be a merge across
thousands of files, and we would have to strip their CI workflows (which use GitHub-hosted runners),
their agent files and the ArtCraft brand on every sync. With a pin and patches, our whole change stays
reviewable, and a sync means re-applying the patches and fixing what no longer applies.

## How our crates attach

DesignCraft's layering, from its `docs/architecture.md` (L0 geom … L5 engine, L6 ui-egui and mcp,
L7 apps), gives us these attach points:

| Attach point | Upstream API | What we use it for |
|---|---|---|
| Document model | `designcraft_doc::Document` and friends | `foliolum-pub` builds a `Document` directly |
| Opening a document | `designcraft_engine::DocState::new(doc, path)` + `Session::add_document` | putting an imported .pub into a session (verified by the probe) |
| Commands | `Session::execute(id, params)` | export PDF, save, every edit (verified: `file.exportPdf`) |
| Platform I/O | `ui_egui::DesignApp::new(Session, Services)` | `foliolum-web` supplies its own `Services` (file read and download, inbox) |
| UI | `ui-egui` (about 33k lines; fixed menu, panel and command tables) | Phase 1 reuses it and patches the menus |

What we can't extend from outside, and what we plan for each. These become our patches; upstream
contributions are on hold.

1. **File ▸ Open routing is hardcoded** (`crates/engine/src/cmd/file/open.rs`). We own the web entry
   app, so we try first to turn a .pub into a `Document` before the engine sees it. If that isn't
   clean, it becomes a small patch adding .pub to the open routing.
2. **Menus, panels and commands are fixed tables, and `DesignApp::ui` lays out the whole window.**
   Patch `0003-ui-embedder-shell` (done) adds layout hooks instead of new tables: our bar under the app
   bar, our strip at the left, our controls in the app bar, and a per-frame choice of DesignCraft's bars.
   Foliolum's simple editor (`crates/foliolum-shell`) is a ribbon whose buttons run DesignCraft's
   existing menu commands, tools and panels, plus a strip of the document's pages down the left (its
   thumbnails from the Pages panel's cache, opened up by patch `0004-ui-page-thumbnails`). A switch
   goes to the full editor. No new commands so far.
3. **The web build has no control channel or MCP.** The AI phases need one. That is a later patch and
   is out of scope for Phase 1.
4. **ArtCraft brand assets are trademarks and not open source** (`docs/brand/LICENSE-brand.txt`), and
   DesignCraft's UI hardcodes them (About, start screen, app bar, Help menu). Patch
   `0002-ui-embedder-brand` (done) lets Foliolum set its own name, credit line and links, hides the
   ArtCraft community, and compiles the mark's artwork out (`artcraft-brand` feature off). The only
   credit we give is the plain-text line "based on DesignCraft by the ArtCraft team" (see `NOTICE`).

5. **Fonts: the font database always includes the machine's installed fonts, and its table of
   replacements for missing families is fixed.** Patch `0001-fonts-embedder-hooks` (done) adds
   `DESIGNCRAFT_SYSTEM_FONTS=0`, which leaves out installed fonts as a browser does, and
   `FontDb::add_substitute`, which lets `foliolum-fonts` register its replacements. Both are small and
   general, so they are good candidates to offer upstream once contributions open.

Expected size of the series in Phase 1: under 5 patches.

## Fonts

Publisher files name Windows and Office fonts. A browser has none of them and we can't ship them, and
DesignCraft draws a missing family with Source Sans 3, so text reflows and headlines overflow.
`crates/foliolum-fonts` bundles open fonts (SIL OFL 1.1, Roboto Slab Apache 2.0; fetched by its
`fetch.sh` from `fonts.lock` at a pinned google/fonts commit, checked by SHA-256, never committed) and
registers which draws each family:

- **Metric-compatible** where one exists, so lines break where they did in Publisher: Calibri → Carlito,
  Cambria → Caladea, Arial → Arimo, Times New Roman → Tinos, Courier New → Cousine, Georgia → Gelasio.
- **Look-alikes** for other common families (Franklin Gothic → Libre Franklin, Garamond → EB Garamond,
  Comic Sans → Comic Neue, Century Gothic → Questrial, Rockwell → Roboto Slab).
- **A guess from the name** for anything else (condensed, script, slab, monospaced or serif), registered
  per document when it opens; plain sans names keep Source Sans 3.

Symbol fonts are handled in the importer: a bullet or text set in Wingdings or Symbol becomes the Unicode
characters it draws, and that text takes the surrounding text font (`crates/foliolum-pub/src/symbols.rs`,
built from Unicode's own mapping documents). DesignCraft draws each character with a loaded font that
has it; Noto Sans Symbols and Noto Sans Symbols 2 are bundled for these (crosses, stars, arrows, check
marks, circled numbers). Codes with no mapping (Webdings, Wingdings 2 and 3 for now) are counted in an
import warning. The gate runs without installed fonts by default and lists
each file's families and what draws them, plus how much text doesn't fit its frame (overset).

## .pub import

**Primary: NiftyDesign's pure-Rust importer** (`srt8hellcat2-ui/niftydesign`, `crates/pub`, MIT OR
Apache-2.0), lifted into `crates/foliolum-pub` with its copyright headers, attribution in `NOTICE`, and
its provenance doc kept alongside.
- It builds a DesignCraft `Document` directly.
- No unsafe code, no C, and no filesystem or threads in the import path, so it is ready for wasm.
- It refuses Publisher 97/98/2000 files.

**Second reader and oracle: Pubwork's patched libmspub WebAssembly reader** (`Daffyllas/Pubwork`,
MPL-2.0, 494 KB `.wasm`).
- It reads every test file, including Publisher 98 and 2000.
- The gate uses it as an oracle: page, text, picture and shape counts are compared per file, so silent
  losses show up.
- If old files turn out to matter, it becomes the fallback reader for them.
- It is kept in its own directory with its MPL notices. MPL-2.0 is file-level copyleft and sits
  inside our MIT/Apache whole.

Stock libmspub 0.1.5 is **not** an option on its own. It returned zero pages, with exit code 0, for
the two most realistic test files.

## The Phase 1 gate

Microsoft Publisher is **not** part of the toolchain: it is out of support, and nothing in Foliolum may
depend on a copy of it being available (Derek, 2026-11-16). We make no Publisher renders ourselves. The
references are the files themselves, two independent readers, DesignCraft's engine and, for one public
set, Publisher's own exports made by someone else.

- **Files:** real-world `.pub` files contributed with permission, plus two public development sets:
  - the 22 test files from the NiftyDesign and Pubwork repositories;
  - **pubsmith's 142 fixtures** (`FuncularLabs/pubsmith`, `tests/fixtures/pub/`, pinned at
    `0ccdf27fc3f381f0897515e226ea485165c699b2`). They were made with Publisher by that project's scripts
    and ship with Publisher's layout export of each file (`truth/*.json`) and a few renders (`golden/`).
    Their notices say the files carry Microsoft material outside pubsmith's MIT licence, so they are
    fetched at the pin into a gitignored folder, **never vendored**. Their truth JSON is used to score
    imports, never to work out the file format (the same rule NiftyDesign follows).

  Real files can carry real names and contact details, so they are **never committed**. The harness
  reads them from a directory given on the command line, and `gate/` keeps only each file's SHA-256, a
  neutral label, and its results.
- **Per file, `cargo xtask gate <dir>` records:**
  1. *Opens:* `foliolum-pub` imports it without error.
  2. *Complete:* against Publisher's layout export when the file has one (pubsmith's), otherwise the
     Pubwork reader: page count, page size, and per-page text, picture, shape and table counts, and the
     text itself after whitespace normalisation. Every difference is listed, not averaged away. Against
     Pubwork, having less than the reader found fails and having more is noted, because the reader has
     blind spots (newer WordArt) and the reviewer judges what is drawn. `gate/README.md` has the rules.
  3. *Exports:* DesignCraft writes a print PDF whose page count and sizes match the import.
  4. *Recognizable:* a per-page PNG is rendered for review, and the reviewer's verdict is stored with the
     file's hash. This is the one judgement a machine does not make.
- **Pass** means all four. The gate's result is the table, not a single number: a file that opens but
  loses text is a fail with its reason.

## Adopting upstream (fork-sync)

- **Derek merges every adoption.** The `fork-sync` skill is run weekly, or when Derek asks.
- **What the skill does:**
  - Reports how far the pin is behind upstream, summarized by area (engine, text, PDF, web, UI, build),
    not commit by commit (that's 500+ commits a week).
  - Flags any upstream change that touches our patches or the APIs our crates use.
  - Proposes a target upstream commit, then waits for Derek's choice.
  - Re-applies the patch series (`git apply --3way`), fixes what no longer applies, and regenerates the
    patches and `SHA256SUMS`.
  - Copies upstream's `[patch.crates-io]` block and profiles into our `Cargo.toml`.
  - Builds native and wasm, runs the gate, and opens one PR with the results.
- **Drift measured in the spike:** NiftyDesign's importer was written against v0.18.0. Lifted onto
  upstream `main` 813 commits later, it failed to compile in **2 places**, both the same API change.
  Weekly syncs should usually cost minutes.

## Toolchain (CI must match)

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.100.0 stable (2026-11-09) | upstream MSRV 1.95, edition 2024; pinned in `rust-toolchain.toml` |
| Target | `wasm32-unknown-unknown` | |
| trunk | 0.22.1 | `cargo install --locked trunk` |
| wasm-bindgen | 0.2.129 | trunk downloads it; pinned in `apps/foliolum-web/Trunk.toml` |
| wasm-opt | binaryen version_131 | trunk downloads it; pinned in `Trunk.toml` |
| Node | 22.23.4 | runs the Pubwork reader for the gate; CI installs this pinned build (the runner's own Node is 10) |

CI (`.github/workflows/ci.yml`) runs on `doodlemanage-foliolum` (label `tdp-linux`), a persistent
Ubuntu container (4 CPUs, 8 GB since 2026-11-21) with only an old Node installed. The job installs the Debian packages it needs
when they are missing (a C compiler, pkg-config, libwayland-dev), then rustup and the toolchain
`rust-toolchain.toml` names, trunk, and the pinned Node the gate's Pubwork reader needs. Cargo's home and the target directory stay under `$HOME` between
runs as the cache. It runs fmt, clippy (native and wasm), the workspace tests, the gate's machine checks
on the pubsmith set and on the 19 dev22 files foliolum-pub opens (`cargo xtask gate --no-review`), and
the web build. That web build turns off the
`web` profile's fat LTO (`CARGO_PROFILE_WEB_LTO=off`) and runs as a non-release trunk build, which
skips wasm-opt: it only proves the app builds, and on the runner's first 2 CPUs the LTO link alone ran past 23 minutes and
wasm-opt took 20. The first run on a cold runner took about two hours (toolchain
23 minutes, tests 49 while the cache filled); the job's timeout is 180 minutes.

The repo is public, so CI never runs on `pull_request`: it runs on push to `main` and on
`workflow_dispatch`, and only for the repository owner. To check a PR branch before merging:
`gh workflow run ci.yml --ref <branch>`. Emscripten is not needed unless we rebuild the libmspub reader;
if we do, K3s provides a pinned image.

Local builds on the shared m4 Mac: `nice -n 15`, `CARGO_BUILD_JOBS=4`, one build at a time.

The toolchain install is shared by CI and the deploy (`.github/actions/rust-toolchain`). The deploy
(`deploy.yml`, after CI passes on `main`) builds the size-tuned web app on the same runner, with thin
LTO instead of the profile's fat LTO (fat peaks at 11.2 GB, the runner has 8; thin peaks at 3.4 GB and
gives a 63 MB wasm instead of 55, 28 MB gzipped instead of 25), packages it
in an nginx image on `doodlemanage-foliolum-build` and rolls it out to app.foliolum.app over edge-ssh;
see `deploy/README.md`.

## Licensing

This repo is **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`), the same as
DesignCraft and NiftyDesign. The fonts `foliolum-fonts` ships keep their own licences (OFL 1.1;
Roboto Slab Apache 2.0), which ship beside them: compiled into the CLI and the gate, and served beside
the web app's `.wasm` (`fonts/`, licence texts included) for it to fetch after it starts. Pubwork-derived files, if we use them, stay MPL-2.0 in their own
directory with their notices.
