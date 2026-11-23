<!-- Copyright (c) 2026 srt8hellcat2-ui. SPDX-License-Identifier: MIT OR Apache-2.0 -->
# BorderArt

Source: the `Contents` and `Escher/EscherStm` streams of the real-world files in the LibreOffice
Bugzilla set (`gate/README.md`, `fixtures/lobz/`), read against Microsoft's public MS-WMF and MS-ODRAW
specifications. One of them, LibreOffice bug 84096's pedigree certificate, comes with the PDF its author
exported from Publisher 2013 (attachment 106882), which shows the border as Publisher draws it. No other
reader's source code was consulted. The pubsmith fixtures have no BorderArt.

Files with a BorderArt table: bugs 60556, 78739, 79182, 84096, 89061, 92040, 95894, 116018 and 160489.
Files that use one on a frame: 84096 and 89061 ("Celtic Knotwork"), 92040 ("Southwest" and
"Stars...Black"), 160489 ("Basic...Black Dashes").

## The table: `Contents` record type 0x46

One per publication. Key 0x01 (kind 0x20) is the number of definitions; key 0x02 (kind 0xA0) is the
array of them, each a kind-0x88 list:

| Key | Kind | Meaning |
|---|---|---|
| 0x03 | 0xC0 | Name, UTF-16, NUL-terminated (`Celtic Knotwork`, `Stars...Black`, `Candy  Corn`). |
| 0x04 | 0x20 | A width in EMU: in every use seen, the width the frame's line has (below). |
| 0x05, 0x06 | 0x20 | EMU; often equal to 0x04, sometimes 12700000. Not read. |
| 0x07 | 0x20 | Always 8: the number of places around a frame. |
| 0x08 | 0x90 | Eight kind-0x18 numbers: for each place, the offset of its piece (below). |
| 0x09 | 0x18 | The number of pieces stored: 8, 7 or 1. |
| 0x0A | 0xA0 | The pieces: kind-0x88 lists whose key 0x01 (kind 0x80) is the piece's data. |

A file keeps definitions it no longer uses: 92040 has 38, two of them used, and several names repeat.

**Pieces** are memory metafiles (MS-WMF, header type 1, version 0x0300, no placeable header), so the
importer's WMF reader draws them; every piece in the files above is vector art.

**Places** are the eight positions around a frame, clockwise from the top-left corner: top-left
corner, top side, top-right corner, right side, bottom-right corner, bottom side, bottom-left corner,
left side. Drawn out, the eight pieces of "Southwest", "Waveline", "Postage Stamp" and "Celtic Knotwork"
are exactly that: square corners that turn the right way, horizontal pieces in places 1 and 5, vertical
ones in 3 and 7.

**Offsets (key 0x08)** count through the pieces' data laid end to end, from 108 in every file: piece k
starts at 108 plus the lengths of pieces 0 to k-1. So places can share a piece. All eight offsets are
equal when one piece is stored ("Stars...Black", "Apples"); with seven, two places share one
("Certificate Banner": places 3 and 7 use the same side piece; "Deco...Arch": places 4 and 6 the same
corner). The importer measures from the smallest offset and requires every place's offset to be where
a piece starts; a definition that doesn't check out is skipped, keeping the others' positions.

## Which frame has which: object key 0x09

An object record (types 0x01, 0x10, 0x20, 0x30) with key 0x09, kind 0x18, has the BorderArt at that
position in the table: 84096's and 89061's page frames have 0, 92040's two boxes have 0 and 16,
160489's coupon has 0. Across all 197 files in pubsmith, dev22 and lobz, key 0x09 appears only on
objects in files with a table, always as kind 0x18 and always within it.

## How it is drawn

The frame's drawing record has a line whose width (MS-ODRAW `lineWidth`, 0x01CB) equals the
definition's key 0x04 in each use: 304800 EMU (24 pt) for the certificates' Celtic Knotwork, 152400
for Southwest, 101600 for Stars...Black. That is the border's width.

Publisher's PDF of the certificate shows the border inside the frame's edges: square corners of the
border's width in the four corners, and each side filled with its piece repeated, end to end, between
them. The importer draws the same, as one SVG picture over the frame: each side holds as many tiles as
fit at the piece's own proportions (a 2:1 top piece gives tiles twice as long as the border is wide),
stretched a little so a whole number fill the side. The frame's own line is not drawn.

A text frame's text stays inside the border: the border's width is added to each inset. Two of 92040's
bordered frames hold text, and their first lines would otherwise sit under the border (one is the
headline `ATTENTION:`).

## Not done

- **Colour.** Publisher's PDF shows the Celtic Knotwork in dark green and white where the pieces are
  black and white: Publisher can recolour BorderArt. Where it keeps that colour is not established; the
  pieces are drawn in their own colours.
- **Rotated frames** are drawn with the frame's transform, like any picture; no file shows a rotated
  BorderArt to compare with.
- Keys 0x05 and 0x06, and whether a side's tiles are stretched or spaced, are not established.
