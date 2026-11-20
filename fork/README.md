# DesignCraft, pinned and patched

Foliolum builds on [DesignCraft](https://github.com/storytold/designcraft) (MIT OR Apache-2.0) without
copying it. This folder is the whole of how:

| File | What it is |
|---|---|
| `pins.env` | The upstream commit we build against. |
| `patches/NNNN-*.patch` | Our changes to DesignCraft, applied in order with `git apply`. |
| `patches/SHA256SUMS` | Checked before any patch is applied; a patch it doesn't list is refused. |
| `fetch.sh` | Clones upstream into `upstream/designcraft` (gitignored), resets it to the pin, verifies, applies the patches, then runs `sync-workspace.sh`. |
| `sync-workspace.sh` | Copies upstream's `[profile.*]` and `[patch.crates-io]` into our root `Cargo.toml` (Cargo only honours them from the root). |

Run `fork/fetch.sh` after cloning this repo and whenever `pins.env` or a patch changes.

## The patches

Each patch: what it changes, why it can't live in our own crates, and what would let us drop it. All are
small and general, so they are good candidates to offer upstream once contributions open.

### 0001-fonts-embedder-hooks

`crates/fonts`: `DESIGNCRAFT_SYSTEM_FONTS=0` leaves the machine's installed fonts out of `FontDb::global()`,
as in a browser, and `FontDb::add_substitute` registers a replacement for a missing family ahead of the
built-in ones. The global font database and its substitution table are internal, so `foliolum-fonts` can't
add to them from outside. Drop it if upstream gains an equivalent.

### 0002-ui-embedder-brand

`crates/ui-egui`: a `brand` module (name, version, tagline, credit line, links) that an app built on
DesignCraft sets at startup, plus an `artcraft-brand` cargo feature (on by default). With Foliolum's brand
the About dialog, start screen, title bar and Help menu show Foliolum and leave out the ArtCraft mark,
community, Discord and sibling apps; with the feature off the mark's artwork isn't compiled in at all.
Menu separators are drawn only between groups that each show something. DesignCraft's own behaviour is
unchanged. The brand licence (`docs/brand/LICENSE-brand.txt` upstream) requires a modified version to
remove the marks, and the UI hardcodes them. Drop it if upstream gains a brand hook.

### 0003-ui-embedder-shell

`crates/ui-egui`: a `shell` module an app built on DesignCraft sets at startup. It holds three hooks,
drawn as part of the window's layout: under the app bar, at the left edge, and inline in the app bar. It
also has a per-frame choice of which of DesignCraft's bars to show (menus, workspace menu, Tools panel,
Control bar). Foliolum's simple editor (`crates/foliolum-shell`: the ribbon and the page strip) draws
through it. `DesignApp::ui` lays out the whole window in one call, so an embedder can't place a panel
between the app bar and the canvas from outside. DesignCraft's own window is unchanged. Most of the
`app_bar.rs` hunk is re-indentation (the workspace menu now sits inside an `if`), so expect that hunk
to need re-applying by hand when upstream changes the app bar. Drop it if upstream gains embedder layout
hooks.

## Upstream test baseline

At pin `47ef1ec` these DesignCraft UI tests already fail on macOS without any of our patches (`cargo test
-p designcraft-ui-egui --lib`, 2026-11-18; 1028 pass). A patch must not add to the list:

- `canvas::tests::the_paste_in_place_shortcut_pastes_in_place_and_plain_paste_offsets`
- `menus::tests::go_back_after_presentation_returns_to_the_last_spread_presented`
- `menus::tests::go_back_and_go_forward_retrace_the_pages_viewed`
- `menus::tests::go_back_returns_to_the_view_a_jump_left_and_scrolling_adds_no_entry`
- `menus::tests::layout_menu_goes_through_pages_and_spreads_with_their_shortcuts`
- `menus::tests::next_spread_at_the_last_spread_is_no_jump`
- `panel_docking::tests::the_dock_is_at_the_right_in_english_and_the_left_in_arabic`
- `panels::styles::tests::named_character_styles_and_redundant_local_values_are_not_paragraph_overrides`
- `panels::styles::tests::paragraph_color_override_is_marked_and_cleared_from_the_row_menu`
- `shortcuts_dialog::tests::a_recorded_chord_runs_nothing_changes_the_menus_at_once_and_ok_keeps_a_custom_set`
- `shortcuts_dialog::tests::an_indesign_set_imports_through_the_review_as_a_new_set`
- `shortcuts_dialog::tests::assign_takes_a_held_shortcut_in_one_step_for_the_menus_and_the_keys`
- `shortcuts_dialog::tests::cancel_puts_the_keys_and_the_sets_back`

## Editing a patch

1. `fork/fetch.sh` (a clean checkout with the current series applied).
2. Commit that state in the throwaway checkout, as the base for the next patch (fetch.sh resets it to the
   pin, so the commit never goes anywhere):
   `git -C upstream/designcraft add -A && git -C upstream/designcraft -c user.name=foliolum -c user.email=foliolum@localhost commit -qm base`.
   To change an existing patch instead, re-apply the series only up to the one before it.
3. Edit inside `upstream/designcraft`, and format with upstream's own config:
   `rustfmt --edition 2024 --config-path upstream/designcraft/rustfmt.toml <files>`.
4. `git -C upstream/designcraft add -N <new files> && git -C upstream/designcraft diff HEAD > fork/patches/NNNN-name.patch`
   (one concern per patch; diffing against the base keeps patches apart even where they touch the same file).
5. Regenerate the checksums: `(cd fork/patches && shasum -a 256 *.patch > SHA256SUMS)`.
6. `fork/fetch.sh` again to prove the series applies cleanly from scratch, and run DesignCraft's UI tests
   against the baseline below.

## Moving the pin

Only through the `fork-sync` skill (`.claude/skills/fork-sync/SKILL.md`), with Derek choosing the target.
