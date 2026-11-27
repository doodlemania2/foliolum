# Text wrap around pictures

Sources: the bytes of the lobz files (`gate/README.md`), read with the importer's own `Contents` and
drawing readers, and MS-ODRAW for the drawing properties. The PDF exported from Publisher that is
attached to bug 89393 (`lo-bz89393-113398.ref-113399.pdf`) shows which text wraps. No other reader
was consulted.

## What wraps

OBSERVATION: in lobz 89393, Publisher wraps text around two pictures. The trinity-knot picture
(Contents seq 327) is beside the "Word of the Month" story, and the "Discipleship" picture (seq 299)
is beside "America For Christ Offering". Neither picture's object record has key **0x04**.

OBSERVATION: in the same file, key 0x04 (kind 0x10) is **3** on every text box, **0** on groups, and
**2** or **3** on a few pictures that show nothing on the page. Across the 142 pubsmith pictures and
the dev22 and lobz files, 152 of 163 picture records have no 0x04. The rest hold 2, 3, 0x2D02,
0x3000, 0x3003 and 0xFF00, so 0x04 looks like a bit field.

INFERENCE: an absent 0x04 is Publisher's default wrap for a picture, square. Text boxes (3) don't
wrap the text around them. The meaning of the other values is not known.

## How far

MS-ODRAW's wrap distances are properties 0x0384 (dxWrapDistLeft), 0x0385 (dyWrapDistTop), 0x0386
(dxWrapDistRight) and 0x0387 (dyWrapDistBottom), in EMU.

OBSERVATION: in 89393 the drawing's default shape (the first shape, which has no Contents
sequence) holds all four, at 0x8EE0 = 36,576 EMU = 2.88 pt. Most shapes repeat that value. The two
wrapping pictures don't set them at all. In the PDF the text stops a few points from each picture's
box.

## What the importer does

A picture with an image and no 0x04 gets square (bounding-box) wrap. Each distance is taken from the
shape, else from the drawing's default shape, else 2.88 pt. Nothing else wraps.

Known gap: in 89393 the "Discipleship" picture is drawn lower than its frame, so the "America For
Christ Offering" line is not wrapped although the wrap is set. That is a picture placement issue,
not a wrap one.

Effect on the gate (2026-11-26): text that ran under pictures now flows around them. For example,
lobz 105567's booklet pages now wrap around their drawings. Overset rises in 15 files, because the
wrapped text needs more room in the substitute fonts.
