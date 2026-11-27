<!-- Copyright (c) 2026 srt8hellcat2-ui. SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Line metrics: how Publisher spaces lines and fits them in a frame

This is about layout, not storage: what Publisher does with text the file describes, worked out
from PDFs that Publisher exported (attached to the gate's lobz bugs, `gate/README.md`) beside the
same files' bytes. The importer's side is `metrics.rs` and `fit_inset` in `lib.rs`.

## Sources

- Publisher-exported PDFs attached to LibreOffice bugs 89393, 116018, 84096, 121800 and 61579
  (`gate/fetch-fixtures.sh`, the lobz references). Text positions were read from the PDFs' content
  (each glyph's baseline and size); the fonts embedded in them were read for their `hhea`/`OS/2`
  metrics. About 60 text frames in all.
- The Microsoft core fonts installed with macOS (`/System/Library/Fonts/Supplemental`), read for
  their `OS/2` and `hhea` metrics.
- The frames' rectangles and insets from the importer's own reading of the same `.pub` files.

## First baseline

OBSERVATION: in a top-aligned frame, Publisher puts the first baseline the font's **Windows ascent**
(`OS/2.usWinAscent`) times the size below the top of the text area (frame top + top inset). Measured
offset ÷ size against the font's ascent: Tahoma 0.991–1.000 (1.0005), Arial Rounded MT Bold
0.937–0.955 (0.946), Rockwell Condensed 0.941 (0.941), Edwardian Script ITC 0.847 (0.851),
Calibri 0.967–0.973 (win 0.952; its `hhea` ascent is 0.75, so it is the Windows value), Century
Gothic Bold 0.989 (1.006), Garamond Bold 0.840 (bug 89393's "Keep In Your Prayers" heading; the
embedded subset's `hhea` ascent is 0.862 and its Windows values are zeroed, so 0.84 is taken from
the PDF).

## Line spacing

OBSERVATION: Publisher's single spacing is the font's Windows line height, win ascent + win descent
+ external leading (`max(0, hhea.lineGap − ((winAscent + winDescent) − (hhea.ascent − hhea.descent)))`),
times the size of the largest text on the line. Its "N lines" spacing (paragraph key 0x34 with low
bits 2, `text-store.md`) multiplies that. Measured baseline-to-baseline ÷ size, within one paragraph
at one size: Tahoma 1.207–1.21 (1.207), Calibri at 1.19 lines 1.449–1.459 (1.19 × 1.221 = 1.453),
Arial at 1.1 lines 1.261 (1.264), Arial Narrow Bold at 1.15 lines 1.316 (1.320), Times New Roman
Bold Italic at 1.14 lines 1.313 (1.311), Times New Roman single 1.15 (1.150).

DesignCraft's default auto leading is 1.2 × size, and the importer used to read "N lines" as N × size,
so single-spaced Times New Roman and Arial were set 4% looser than Publisher's, and N-line spacing
13–22% tighter.

## What fits

OBSERVATION: Publisher keeps a line while its **baseline** is no lower than the frame's bottom edge;
the bottom inset does not limit it, and descenders may hang below the frame. Bug 116018 page 3:
"Pagina 3" (7 pt) has its baseline 2.19 pt into the 2.88 pt bottom inset; bug 61579: "TEL. …" has
its baseline exactly on the frame's bottom edge. No line in the measured frames has its baseline
below its frame's edge.

DesignCraft keeps a line while its baseline is above the bottom inset. Together with a taller
substitute font this hid one-line frames Publisher shows: bug 89393's heading (18 pt Garamond in a
23.3 pt frame) and bug 95894's running headers (10 pt Times New Roman in a 13.5 pt frame).

## What the importer does

- For the fonts in the table below, each paragraph's auto leading becomes the font's line height
  times Publisher's multiple (1 when none is stored), from the paragraph's largest text; exact
  spacing stays exact. The first frame of each story, and each table cell, gets a fixed first
  baseline of the first paragraph's ascent × size. Text in any other font keeps DesignCraft's layout.
- A top-aligned frame's bottom inset becomes 0, so the same lines fit as in Publisher; there it
  changes nothing else. Centred and bottom-aligned frames keep it, as it places their text.

| Font | Ascent (em) | Line height (em) | Source |
|---|---|---|---|
| Arial | 0.9053 | 1.1499 | macOS font file |
| Arial Black | 1.1006 | 1.4102 | macOS font file |
| Arial Narrow | 0.9219 | 1.1474 | macOS font file |
| Arial Rounded MT Bold | 0.9463 | 1.1572 | macOS font file |
| Calibri | 0.9521 | 1.2207 | embedded in bug 121800's PDF |
| Cambria | 0.9502 | 1.1724 | embedded in bug 89393's PDF (`hhea`) |
| Century | 0.9858 | 1.2021 | embedded in bug 116018's PDF (`hhea`) |
| Century Gothic | 1.0059 | 1.2261 | embedded in bug 116018's PDF (`hhea`) |
| Comic Sans MS | 1.1021 | 1.3936 | macOS font file |
| Compacta Blk BT | 1.0068 | 1.2427 | embedded in bug 116018's PDF (`hhea`) |
| Courier New | 0.8325 | 1.1328 | macOS font file |
| Edwardian Script ITC | 0.8506 | 1.1787 | embedded in bug 84096's PDF |
| Felix Titling | 0.9282 | 1.1694 | embedded in bug 89393's PDF (`hhea`) |
| Franklin Gothic Book | 0.9165 | 1.1338 | embedded in bug 61579's PDF (`hhea`) |
| Garamond | 0.84 | 1.125 | first baseline in bug 89393's PDF; `hhea` height of the embedded font |
| Georgia | 0.9170 | 1.1362 | macOS font file |
| Impact | 1.0088 | 1.2197 | macOS font file |
| Microsoft Sans Serif | 0.9219 | 1.1319 | macOS font file |
| Palace Script MT | 0.6841 | 0.9263 | embedded in bug 89393's PDF (`hhea`) |
| Rockwell Condensed | 0.9409 | 1.1758 | embedded in bug 116018's PDF (`hhea`) |
| Tahoma | 1.0005 | 1.2070 | macOS font file |
| Times New Roman | 0.8911 | 1.1499 | macOS font file |
| Trebuchet MS | 0.9390 | 1.1612 | macOS font file |
| Verdana | 1.0054 | 1.2154 | macOS font file |

Where only `hhea` was available (a subset whose Windows values are zeroed), it stands in for the
Windows values; in every complete font above the two agree.

UNKNOWN: fonts not in the table (Maiandra GD, Gill Sans MT, Rockwell, Franklin Gothic Medium,
Lucida Handwriting and others in the gate's files); Publisher's rule for a frame's later lines in a
thread (the importer leaves those frames' first baseline to DesignCraft); how BorderArt and text
wrap around other objects change what fits.
