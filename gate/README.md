# The Phase 1 gate

Phase 1 is done when 50 real Publisher files open, render recognizably and export a print PDF, with a
recorded result per file. The design is in `docs/architecture.md`, "The Phase 1 gate".

The real-world files are **dev22** (22) and **lobz** (33, below): 55 files people made in Publisher for
their own use, and any contributed ones on top. No contributed files are expected, so these two public
sets are what the 50 is counted over. pubsmith's 142 fixtures are made with Publisher too, but by a
script, one feature at a time; they test the importer against Publisher's own layout export and don't
count toward the 50. The stress set is ours and doesn't count either.

```sh
fork/fetch.sh                      # DesignCraft at the pin
gate/fetch-fixtures.sh             # the public dev sets and Pubwork's reader into fixtures/ (gitignored)
cargo xtask gate --set pubsmith --names fixtures/pubsmith
cargo xtask gate --set dev22 --names fixtures/dev22
gate/fetch-fixtures.sh --lobz      # also the LibreOffice Bugzilla set (not fetched by default)
cargo xtask gate --set lobz --names --record fixtures/lobz
cargo xtask gate --set contributed --record /path/to/contributed/files
```

Each file gets four checks: **opens**, **complete** (against a reference, below), **exports** (the PDF
read back: page count and sizes), and **recognizable** (a person's verdict). A file passes only when all
four pass. A check that can't be made yet is shown as `–` and is never a pass.

`fetch-fixtures.sh` fetches, each at a pinned commit and checked by SHA-256:

- `fixtures/pubsmith/`: pubsmith's 142 fixtures, made with Publisher, each with Publisher's layout export;
- `fixtures/dev22/`: the spike's 22 public files (21 from Apache POI, one from LibreOffice), no exports;
- `fixtures/pubwork/`: Pubwork's patched libmspub reader (MPL-2.0), the second reference;
- with `--lobz`, `fixtures/lobz/`: the LibreOffice Bugzilla set, below.

## The LibreOffice Bugzilla set (lobz)

People attach Publisher files to LibreOffice import bugs so that importers will get them right. 33 of
those attachments are distinct Publisher files not already in dev22: parish and church bulletins, a
missal, an order of service, newsletters, a sponsorship brochure, flyers, greeting cards, pedigree
certificates, an itinerary, a shape catalogue and a few minimized repros. They are fetched by attachment
id (`bugs.documentfoundation.org/attachment.cgi?id=N`) and SHA-256, named `lo-bz<bug>-<attachment>.pub`,
and never committed: they are public, but they carry real names and addresses. One (bug 89993) is a
Publisher 2000 file, refused by design for now.

They are fetched only with `--lobz`: CI cleans `fixtures/` on every run, and LibreOffice's Bugzilla
shouldn't serve 45 MB to every push. CI doesn't gate on them yet, since some still fail the machine
checks; `results/lobz.md` is the recorded table, and each fix that moves a file is checked against
dev22, pubsmith and the stress set as well. Some bugs also attach the PDF the author exported from
Publisher (116018's shows 4 pages), the best reference there is for those files.

CI runs the gate with `--no-review` on pubsmith and on the 19 dev22 files that open (the other three are
Publisher 97–2000 files, which foliolum-pub refuses by design). `--no-review` sets the exit code by the
three machine checks alone, since only a person can give the fourth, and every file in those runs must
pass them. The table still shows all four.

## The stress set (synthetic)

`cargo xtask synth fixtures/stress` writes 50 publications made up by our own writer (`xtask/src/synth.rs`,
seed 1, so the same files every time), and beside each a reference in the layout-export shape:

```sh
cargo xtask synth fixtures/stress
cargo xtask gate --set stress --names fixtures/stress
```

They have one to eight pages in five page sizes, master pages with footers, columns of text boxes with
headlines and body text in ten fonts, mixed sizes, bold, italic and colours, aligned and bulleted
paragraphs, stories linked across pages, text in Wingdings and Symbol, filled and outlined shapes and PNG
pictures: about 240 pages and 3,300 elements in all. About one text box in ten is overfull on purpose.

**They are not real Publisher files and don't count toward the Phase 1 gate.** Only Publisher writes .pub
files, and ours are written from the same format notes the importer reads with, so they test robustness
(large, mixed documents import, check out complete and export) rather than fidelity to Publisher. Pubwork's
reader can't read them, so their references are the generator's own. CI generates and checks them;
`results/stress.md` is the recorded table.

## The completeness references

A file with a layout export (`truth/<name>.json` beside it) is scored against it, page by page: each
element is matched among the items shown on its own page. Any other file,
including every contributed one, is checked against **Pubwork's reader**, an independent reading of the
same file: `gate/pubwork/oracle.mjs` runs it under Node (`node` on the path, or `$FOLIOLUM_NODE`) and
summarises each page, and `xtask/src/pubwork.rs` summarises the import the same way and compares:

- page count and sizes;
- per page, the number of text boxes, pictures, shapes and tables;
- per page, the text: each story once, and each table cell, whitespace collapsed.

Having **less** than the reader found (a missing text box, picture, shape, table or piece of text, or
changed text) fails. Having **more** is noted in `results.jsonl`, not failed: the reader doesn't see
newer WordArt, for one, and the reviewer judges what is drawn. Where libmspub draws one thing as two,
the summaries count it the way foliolum-pub imports it: a text box's fill or holding shape, an outline
drawn over a shape or picture, a picture cropped to a shape, and WordArt as text. A shortfall in shapes
is excused, with a note, up to the number of extra text boxes we have, because WordArt the reader can't
read may reach it as a bare shape. Empty pages the reader leaves out are noted, and a page with nothing of its own (only what its master
page shows) counts as empty for this. If the reader can't read
a file, or finds no pages where we found content, completeness is unchecked: never a pass.

**Symbols and control characters.** Text compares with control characters left out (no one draws
them; the reader keeps Publisher's 0x1F optional hyphen), and a Latin-1 letter on one side matches a
symbol outside Latin-1 on the other, because we draw symbol-font text (Wingdings, Webdings, Symbol)
as the Unicode symbols it shows where the reader keeps the letters.

**Cells split in two.** Publisher can list one table cell twice, once for each half of a split cell.
We keep both halves' text in the one cell, one after the other, and warn that the split isn't drawn;
the reader lists the halves as separate texts, and sometimes stops reading the table there. A reader
text that is a word-bounded leading or trailing part of one of our unmatched texts is therefore held,
not lost: it is noted, and that part of ours is used up so it can't vouch twice.

With these rules Pubwork finds no loss in any of the 142 pubsmith imports, which the layout export
already confirms (one, `wordart__line-white`, only through the WordArt rule), or in the 19 dev22 files
that open. `results.jsonl` keeps both page summaries whenever the readers differ.

The gate uses only DesignCraft's and Foliolum's bundled fonts, as the browser does, so its results don't
depend on the machine (`--system-fonts` adds the installed ones). `results.jsonl` lists each file's font
families and what draws them, and the table shows overset: text that doesn't fit its frame and so isn't
in the PDF. Overset is reported, not failed on, because Publisher files can hold overset text themselves.

Output goes to `target/gate/<set>/`: one PDF per file, `<label>/page-NN.png` renders for review,
`results.jsonl` with every difference found, and `results.md`. `--record` also writes
`gate/results/<set>.md`, the table that is committed.

## What is committed here, and what never is

- **Committed:** `results/<set>.md` (labels, SHA-256s and results) and `verdicts.json`. The stress set is
  generated, so it is never committed either.
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
