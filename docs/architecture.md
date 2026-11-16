# Foliolum architecture

Status: **proposal from the Phase 1 spike (#2), 2026-11-16.** The integration method is waiting on
Derek's decision (#4). The evidence behind it is in `docs/spike-report.md`.

## Summary

Foliolum is a **distribution of DesignCraft**. DesignCraft lives in this repo as a pinned git submodule.
Our code is a separate Cargo workspace at the repo root that depends on DesignCraft's crates by path.
Upstream code is not edited in place. When a change to upstream is truly needed, it is a commit on a
`foliolum` branch of our DesignCraft fork, and the same change is offered upstream as a pull request.

The spike tested this. A crate outside DesignCraft's workspace, depending on its crates by path,
built against upstream `main` at `47ef1ec` (16 Nov) and imported .pub files into the upstream engine.
It then exported PDFs with **no upstream file changed**.

## Repo layout

```
foliolum/
  upstream/designcraft/        git submodule → doodlemania2/designcraft (fork), pinned commit
  crates/
    foliolum-pub/              .pub import → designcraft_doc::Document   (Phase 1)
    foliolum-fonts/            Publisher font mapping and metric-compatible substitutes (Phase 1)
    foliolum-shell/            Publisher mode: ribbon, simpler dialogs, our words and icons (Phase 1)
  apps/
    foliolum-web/              web entry point (trunk, wasm32), our branding, .pub open wiring
    foliolum-cli/              headless import and PDF export, used by the 50-file gate
  corpus/                      gate manifest: file hashes and expected results, never the files
  xtask/                       sync-upstream, gate, wasm, ci
  Cargo.toml                   our workspace; [patch.crates-io] and profiles synced from upstream
  presence/features.json
  docs/
```

- **Why a submodule, not vendoring:** vendoring copies about 73 MB and 5,500 commits of someone else's
  history into our tree, makes every upstream bump a giant diff, and hides which lines are ours.
- **Why a submodule, not a whole-repo fork (the documenso model):** upstream adds about 100 commits a
  day (2,888 in the last 30 days, from about 80 authors). If our crates lived inside the fork, every
  rebase would carry them through that churn. Their CLAUDE.md, AGENTS.md, CI and ArtCraft brand files
  would also become ours to strip on every bump. The probe showed that our crates do not need to live
  inside their workspace.
- **Why the submodule points at our fork rather than straight at upstream:** the fork is where any
  unavoidable patch lives, as normal commits on a `foliolum` branch, and where our upstream pull
  requests come from. When we carry no patches, the `foliolum` branch is just upstream.

## How our crates attach

DesignCraft's layering, from its `docs/architecture.md` (L0 geom … L5 engine, L6 ui-egui and mcp,
L7 apps), gives us these attach points:

| Attach point | Upstream API | What we use it for |
|---|---|---|
| Document model | `designcraft_doc::Document` and friends | `foliolum-pub` builds a `Document` directly |
| Opening a document | `designcraft_engine::DocState::new(doc, path)` + `Session::add_document` | putting an imported .pub into a session (verified by the probe) |
| Commands | `Session::execute(id, params)` | export PDF, save, every edit (verified: `file.exportPdf`) |
| Platform I/O | `ui_egui::DesignApp::new(Session, Services)` | `foliolum-web` supplies its own `Services` (file read and download, inbox) |
| UI | `ui-egui` (about 33k lines, fixed menu, panel and command tables) | Phase 1 starts by reusing it, see below |

Things upstream does not let us extend from outside, and what we plan for each:

1. **File ▸ Open routing is hardcoded** (`crates/engine/src/cmd/file/open.rs`). For the web we own the
   entry app (`apps/foliolum-web`), so we can turn a .pub into a `Document` before the engine sees it,
   through our `Services` or a pre-open step. This still needs proving in Phase 1. If it fails, it
   becomes our first patch, and we offer upstream a small importer registry.
2. **Menus, panels and commands are fixed tables.** A Publisher-mode ribbon needs one of three things:
   - an upstream extension point (a "UI profile" or menu-table injection), which we offer upstream
     first;
   - a carried patch to `ui-egui/src/menus/tables.rs`;
   - our own shell crate drawing over `engine`, `render` and `tools`, which is a large job.

   Recommendation: start with the patch plus an upstream PR, and decide on our own shell only if
   upstream declines. Expected size of the series: under 5 patches.
3. **The web build has no control channel or MCP.** The AI phases need one. We build it as an
   upstream contribution, not as a fork-only feature.
4. **ArtCraft brand assets are trademarks and not open source** (`docs/brand/LICENSE-brand.txt`).
   `foliolum-web` ships only our own assets. The only credit allowed is the plain-text line "based on
   DesignCraft by the ArtCraft team".

## .pub import

Primary: **NiftyDesign's pure-Rust importer** (`srt8hellcat2-ui/niftydesign`, `crates/pub`, MIT OR
Apache-2.0), lifted into `crates/foliolum-pub` with attribution.
- It builds a DesignCraft `Document` directly, so there is no format bridge.
- It contains no unsafe code, no C, and no filesystem or threads in the import path, so it is ready for
  wasm.
- Upstream PR storytold/designcraft#2126 proposes the same importer. If it merges, we drop our copy.

Second reader: **Pubwork's patched libmspub WebAssembly reader** (`Daffyllas/Pubwork`, MPL-2.0,
494 KB `.wasm`).
- It reads Publisher 97, 98 and 2000 files, which the Rust importer refuses (3 of 22 test files).
- It runs as an oracle in the gate: page, text, picture and shape counts are compared per file, so
  silent losses show up.
- It is kept in its own directory with its MPL notices. MPL-2.0 is file-level copyleft, so it ships
  inside a permissive whole.

Stock libmspub 0.1.5 is **not** an option on its own. It returned zero pages, with exit code 0, for
the two most realistic test files. Pubwork's patch fixes that.

## Tracking upstream

- **Pin to commits, bump weekly.** Upstream tags a release every 1–3 days (v0.21.0 on 14 Nov), but its
  release branch and `main` diverge. Pin a `main` commit that passed upstream CI, and treat bumping as
  routine.
- **`xtask sync-upstream`** copies upstream's `[patch.crates-io]` block and profiles into our
  `Cargo.toml`, bumps the submodule, rebases the `foliolum` patch branch, builds native and wasm, and
  runs the gate corpus.
- **A scheduled bump job** runs on `main` with `workflow_dispatch` and `schedule`, never on fork PRs. It
  runs sync-upstream and opens a PR when the build and gate pass; when they don't, it opens an issue
  with the errors.
- **Drift measured in the spike:** NiftyDesign's importer was written against v0.18.0. Lifted onto
  upstream `main` 813 commits later, it failed to compile in **2 places** (an `Arc<Vec<u8>>` →
  `AssetData::embedded` change). Weekly bumps should usually cost minutes, not days.
- **Upstream-first rule:** every carried patch has an open upstream PR linked in its commit message.
  A patch with no upstream PR is a bug in our process.

## Toolchain (CI must match)

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.100.0 stable (2026-11-09) | upstream MSRV 1.95, edition 2024; pin with `rust-toolchain.toml` |
| Target | `wasm32-unknown-unknown` | |
| trunk | 0.22.1 | `cargo install --locked trunk` |
| wasm-bindgen | 0.2.129 | trunk downloads it; pin it in `Trunk.toml` |
| Node | 22 | already on the runner; used for the Pubwork reader oracle |

CI runner `doodlemanage-foliolum` (label `tdp-linux`) is a persistent Ubuntu container with 12 cores
and only Node installed. The first workflow step installs rustup with the pinned toolchain and the
wasm32 target, plus trunk. `~/.cargo` and `target/` are cached. Emscripten is not needed unless we
rebuild the libmspub reader; if we do, K3s provides a pinned image.

## Licensing (decision in #3)

Recommendation: **MIT OR Apache-2.0** for this repo, the same as DesignCraft and NiftyDesign.
Pubwork-derived files stay MPL-2.0 in their own directory. Options and reasons are in
`docs/spike-report.md`.
