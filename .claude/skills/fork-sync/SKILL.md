---
name: fork-sync
description: Fork-sync for Foliolum. Move the DesignCraft pin forward, re-apply our patch series, rebuild, run the gate, and open one bump PR for Derek to merge. Use when asked to "sync the fork", "bump DesignCraft", "pull in upstream", or on the weekly sync.
---

# Foliolum fork-sync

Foliolum is a DesignCraft distribution kept the documenso way. We don't keep a copy of DesignCraft.
`fork/pins.env` pins an upstream commit, `fork/patches/` holds our changes, and our own crates build
against the patched checkout in `upstream/designcraft/` (gitignored). See `docs/architecture.md`.

A sync moves the pin. It is a routine, weekly job because upstream moves at about 100 commits a day.
It is still **Derek's call each time**: you prepare it, he chooses the target and merges the PR.

## Ground truth (verify, don't assume)

- Upstream: `https://github.com/storytold/designcraft`, trunk `main`. Tags ship every 1–3 days, but the
  release branch and `main` diverge, so pin a `main` commit.
- Our trunk is `main`. Work in a worktree: `/Volumes/Data/repos/_tools/wt new foliolum <issue#>-fork-sync`.
  One branch and one PR, pushed once.
- Builds on the shared m4 Mac: `nice -n 15`, `CARGO_BUILD_JOBS=4`, **one build at a time**. Measure
  release size only on purpose.
- PRs and issues carry **no** "Claude Code" text, footer or session link.

## Procedure

### 1. Establish state

```
source fork/pins.env
git clone --filter=blob:none https://github.com/storytold/designcraft.git <scratch>/dc   # or fetch an existing scratch clone
git -C <scratch>/dc rev-list --count $UPSTREAM_COMMIT..origin/main      # how far behind we are
git -C <scratch>/dc log --oneline --no-merges $UPSTREAM_COMMIT..origin/main | wc -l
git -C <scratch>/dc diff --stat $UPSTREAM_COMMIT origin/main -- crates apps Cargo.toml
```

Also check upstream's `ROADMAP.md` and `docs/roadmap.md` for the stated stage.

### 2. Gap analysis, by area

Summarize what changed by area, not by commit (a week is 500+ commits): engine and commands, document
model, text composer, PDF, render, web app, ui-egui, build and dependencies.

Then flag precisely:

- **Patch conflicts:** for each `fork/patches/*.patch`, run `git apply --check` against the candidate
  commit. List which apply, which need `--3way`, and which fail.
- **API breaks for our crates:** the `designcraft_*` items `crates/foliolum-*` and `apps/foliolum-*`
  use. A quick `cargo check` of our workspace against the candidate (debug, niced) is the real test.
- **Behavior we care about:** .pub-relevant model changes, text composition and PDF output, the web
  build, licensing and NOTICE changes, any new brand assets.

Present it to Derek with `AskUserQuestion`: the proposed target commit (usually the newest `main`
commit whose upstream CI passed), one or two alternatives (for example the latest tag's commit), a size
estimate for the patch work, and a recommendation. **Write no sync code until he picks.**

### 3. Re-apply

```
fork/fetch.sh            # with the new commit in pins.env; it verifies the commit, checks SHA256SUMS, applies patches
```

When a patch doesn't apply, edit `upstream/designcraft/` with `git apply --3way` and fix it by hand.
Then regenerate the patch with `git diff` (one patch per concern, same file names), update
`fork/patches/SHA256SUMS`, and note what changed in `fork/README.md`.

Copy upstream's `[patch.crates-io]` block and `[profile.*]` sections into our root `Cargo.toml` exactly
(`cargo xtask sync-workspace` once it exists). Update `rust-toolchain.toml` only if upstream's
`rust-version` now exceeds it.

### 4. Verify

- `cargo check` and `cargo test` for our workspace, native.
- The web build: `trunk build` in `apps/foliolum-web` (debug unless measuring size).
- **The gate corpus:** `cargo xtask gate`. Compare per-file results with the last run, and treat any
  file that got worse as a blocker to report, not to hide.
- **Branding leak scan:** no `ArtCraft` or `artcraft` strings or assets in anything we ship from
  `apps/foliolum-*` or `crates/foliolum-*`.

### 5. Finalize

- Update `fork/pins.env` (commit and tag, if any) and `CHANGES.md`: one section per sync, naming the
  upstream range adopted and the patches that changed.
- Commit with the trailer from the current session's attribution reminder. Push once, then:
  ```
  gh pr create --repo doodlemania2/foliolum --base main --head <branch> --title "Sync DesignCraft to <short sha>" --body "<area summary, patch changes, gate before/after>"
  ```
- After Derek merges: `wt done foliolum <task>`.

## Guardrails

- Never adopt a newer upstream without Derek picking the target.
- Keep the patch series small. A new patch needs a reason in `fork/README.md`. If one grows past a
  single concern, split it.
- Upstream PRs are on hold (Derek, 2026-11-16). Don't open or comment on anything in
  `storytold/designcraft`.
- Keep upstream's copyright headers and our NOTICE intact. Never ship ArtCraft marks.
