# DesignCraft, pinned and patched

Foliolum builds on [DesignCraft](https://github.com/storytold/designcraft) (MIT OR Apache-2.0) without
copying it. This folder is the whole of how:

| File | What it is |
|---|---|
| `pins.env` | The upstream commit we build against. |
| `patches/NNNN-*.patch` | Our changes to DesignCraft, applied in order with `git apply`. |
| `patches/SHA256SUMS` | Checked before any patch is applied. |
| `fetch.sh` | Clones upstream into `upstream/designcraft` (gitignored), resets it to the pin, verifies, applies the patches, then runs `sync-workspace.sh`. |
| `sync-workspace.sh` | Copies upstream's `[profile.*]` and `[patch.crates-io]` into our root `Cargo.toml` (Cargo only honours them from the root). |

Run `fork/fetch.sh` after cloning this repo and whenever `pins.env` or a patch changes.

## The patches

None yet. Each patch added here gets a section: what it changes, why it can't live in our own crates,
and what would let us drop it.

## Editing a patch

1. `fork/fetch.sh` (a clean checkout with the current series applied).
2. Edit inside `upstream/designcraft`.
3. `git -C upstream/designcraft diff -- <paths> > fork/patches/NNNN-name.patch` (one concern per patch).
4. Regenerate the checksums: `(cd fork/patches && shasum -a 256 *.patch > SHA256SUMS)`.
5. `fork/fetch.sh` again to prove the series applies cleanly from scratch.

## Moving the pin

Only through the `fork-sync` skill (`.claude/skills/fork-sync/SKILL.md`), with Derek choosing the target.
