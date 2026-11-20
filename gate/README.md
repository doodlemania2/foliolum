# The Phase 1 gate

Phase 1 is done when 50 real Publisher files open, render recognizably and export a print PDF, with a
recorded result per file. The design is in `docs/architecture.md`, "The Phase 1 gate".

```sh
fork/fetch.sh                      # DesignCraft at the pin
gate/fetch-fixtures.sh             # pubsmith's 142 Publisher-made fixtures into fixtures/ (gitignored)
cargo xtask gate --set pubsmith --names fixtures/pubsmith
cargo xtask gate --set contributed --record /path/to/contributed/files
```

Each file gets four checks: **opens**, **complete** (against Publisher's layout export when the file has
one), **exports** (the PDF read back: page count and sizes), and **recognizable** (a person's verdict).
A file passes only when all four pass. A check that can't be made yet is shown as `–` and is never a pass.

CI runs `cargo xtask gate --set pubsmith --names --no-review fixtures/pubsmith`: `--no-review` sets the
exit code by the three machine checks alone, since only a person can give the fourth, and every pubsmith
file must pass them. The table still shows all four.

The gate uses only DesignCraft's and Foliolum's bundled fonts, as the browser does, so its results don't
depend on the machine (`--system-fonts` adds the installed ones). `results.jsonl` lists each file's font
families and what draws them, and the table shows overset: text that doesn't fit its frame and so isn't
in the PDF. Overset is reported, not failed on, because Publisher files can hold overset text themselves.

Output goes to `target/gate/<set>/`: one PDF per file, `<label>/page-NN.png` renders for review,
`results.jsonl` with every difference found, and `results.md`. `--record` also writes
`gate/results/<set>.md`, the table that is committed.

## What is committed here, and what never is

- **Committed:** `results/<set>.md` (labels, SHA-256s and results) and `verdicts.json`.
- **Never committed:** the files themselves. Contributed files can carry people's names and contact
  details, so they are labelled by hash. Use `--names` only for public sets. pubsmith's fixtures carry
  Microsoft material outside pubsmith's licence, so they are fetched, never vendored.

## Recording a verdict

Look at `target/gate/<set>/<label>/page-*.png` beside the original, then add the file's SHA-256 (from
`results.jsonl`) to `verdicts.json`:

```json
{ "<sha256>": { "verdict": "recognizable" },
  "<sha256>": { "verdict": "not-recognizable", "note": "headline cut off on page 1" } }
```

"Recognizable" means someone who made the file would know it at a glance, with all its text and
pictures present. It doesn't mean pixel-identical.
