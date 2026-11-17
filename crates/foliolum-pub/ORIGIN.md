# Where this crate comes from

This crate is NiftyDesign's .pub importer, `crates/pub` in
[srt8hellcat2-ui/niftydesign](https://github.com/srt8hellcat2-ui/niftydesign), branch
`nifty-update-0.18.0`, commit `ec754668e83d0b8fbab634f256b9c04eb711de12`. Copyright (c) 2026 srt8hellcat2-ui,
licensed MIT OR Apache-2.0. The per-file copyright headers are kept as they were.

`docs/CLEANROOM.md`, `docs/publisher-import.md` and `docs/pub-format/` are NiftyDesign's provenance and
format notes, copied as they were. Its authors describe a clean-room process (no libmspub, Apache POI
or pubsmith source was read). That claim is self-reported; see `docs/spike-report.md` in this repo.

## What we changed

- Crate renamed `designcraft-pub` → `foliolum-pub` (and `designcraft_pub` → `foliolum_pub` in the code and tests).
- Two lines of API drift fixed for DesignCraft after v0.18.0: picture bytes are now
  `designcraft_doc::AssetData::embedded(..)` instead of `Arc::new(..)`.

- `tests/corpus.rs` (the opt-in report over a folder of real files) moved to `xtask/src/truth.rs`, where the
  gate uses its scoring against Publisher's layout exports. The scoring is unchanged; its entry points are
  public and the old report wrapper is gone (`cargo xtask gate` replaces it).
- `src/symbols.rs` (ours): a bullet set in Wingdings or Symbol becomes the Unicode character it draws
  (Wingdings "l" → ●), set in the paragraph's font, instead of a code no browser font draws as intended.
  Called from the bullet code in `lib.rs`, marked "Foliolum:". Text runs in symbol fonts are not mapped yet.

Further changes are recorded here as they are made.
