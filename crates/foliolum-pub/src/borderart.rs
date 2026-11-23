// Copyright (c) 2026 srt8hellcat2-ui. SPDX-License-Identifier: MIT OR Apache-2.0
//! BorderArt: the picture borders Publisher draws around a frame; see
//! docs/pub-format/borderart.md.
use crate::{
    Result,
    read::{Prop, number},
};
use std::fmt::Write;

/// Definitions in one publication, and pieces in one definition, that are read at most.
const MAX_ARTS: usize = 1000;
const MAX_PIECES: usize = 8;
/// Tiles along one side; more would only make an unreadable picture.
const MAX_TILES_PER_SIDE: usize = 400;

/// One BorderArt definition: a WMF picture for each of the eight places around a frame,
/// clockwise from the top-left corner (corner, top, corner, right, corner, bottom, corner, left).
#[derive(Clone, Debug)]
pub(crate) struct BorderArt {
    pub name: String,
    /// Key 0x04: the border's width when Publisher put it on (EMU).
    pub size_emu: Option<u32>,
    /// For each place, the index into `pieces`.
    pub places: [usize; 8],
    /// Each piece's WMF data (a memory metafile, no placeable header).
    pub pieces: Vec<Vec<u8>>,
}

/// The definitions of a type-0x46 record, in order (a frame names one by its position). A
/// definition that doesn't check out is `None`, so the positions of the others still hold.
pub(crate) fn parse(p: &[Prop<'_>]) -> Result<Vec<Option<BorderArt>>> {
    let Some(list) = p.iter().find(|v| v.key == 2 && v.kind == 0xa0) else { return Ok(Vec::new()) };
    Ok(list.children()?.iter().take(MAX_ARTS).map(|art| art.children().ok().and_then(|a| definition(&a))).collect())
}

fn definition(a: &[Prop<'_>]) -> Option<BorderArt> {
    let name = a.iter().find(|v| v.key == 3 && v.kind == 0xc0).map(|v| v.text().trim_end_matches('\0').to_string()).unwrap_or_default();
    let offsets: Vec<usize> = a.iter().find(|v| v.key == 8)?.children().ok()?.iter().filter_map(|v| v.number()).map(|n| n as usize).collect();
    let pieces: Vec<Vec<u8>> = a
        .iter()
        .find(|v| v.key == 0xa && v.kind == 0xa0)?
        .children()
        .ok()?
        .iter()
        .take(MAX_PIECES)
        .map(|piece| piece.children().ok().and_then(|c| c.iter().find(|v| v.key == 1).map(|v| v.data.to_vec())))
        .collect::<Option<_>>()?;
    if offsets.len() != 8 || pieces.is_empty() || number(a, 9).is_some_and(|n| n as usize != pieces.len()) {
        return None;
    }
    // The offsets count through the pieces laid end to end; each place's offset must be where one
    // of them starts.
    let base = *offsets.iter().min()?;
    let mut starts = Vec::with_capacity(pieces.len());
    let mut at = 0usize;
    for piece in &pieces {
        starts.push(at);
        at = at.checked_add(piece.len())?;
    }
    let mut places = [0usize; 8];
    for (place, offset) in places.iter_mut().zip(&offsets) {
        *place = starts.iter().position(|s| *s == offset - base)?;
    }
    Some(BorderArt { name, size_emu: number(a, 4), places, pieces })
}

/// One piece drawn as SVG: its viewBox size and its drawing.
pub(crate) struct Piece {
    pub view: (f64, f64),
    pub body: String,
}

/// A piece's drawing, from the SVG the metafile reader makes (`viewBox="0 0 w h"`).
pub(crate) fn piece(svg: &[u8]) -> Option<Piece> {
    let s = std::str::from_utf8(svg).ok()?;
    let open_end = s.find('>')?;
    let open = s.get(..open_end)?;
    let vb = open.split("viewBox=\"").nth(1)?.split('"').next()?;
    let n: Vec<f64> = vb.split_whitespace().filter_map(|v| v.parse().ok()).collect();
    let (&w, &h) = (n.get(2)?, n.get(3)?);
    let body = s.get(open_end + 1..)?.trim_end().strip_suffix("</svg>")?.to_string();
    (w > 0.0 && h > 0.0).then_some(Piece { view: (w, h), body })
}

/// The whole border for a `w` x `h` pt frame with a `b` pt border, as one SVG in pt (viewBox
/// `0 0 w h`). Corners are `b` square; each side is tiled between them with its piece, as many
/// tiles as fit at the piece's own proportions, stretched a little so a whole number fill the side.
pub(crate) fn svg(places: [&Piece; 8], w: f64, h: f64, b: f64) -> Option<String> {
    if !(w.is_finite() && h.is_finite() && b.is_finite()) || b <= 0.0 || w < 2.0 * b || h < 2.0 * b {
        return None;
    }
    let mut s = String::new();
    let _ =
        write!(s, "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 {} {}\"><defs>", n(w), n(h));
    let mut defined: Vec<*const Piece> = Vec::new();
    for p in places {
        let ptr: *const Piece = p;
        if !defined.contains(&ptr) {
            let _ = write!(s, "<g id=\"p{}\">{}</g>", defined.len(), p.body);
            defined.push(ptr);
        }
    }
    s.push_str("</defs>");
    let id = |p: &Piece| defined.iter().position(|d| std::ptr::eq(*d, p)).unwrap_or(0);
    let tile = |s: &mut String, p: &Piece, x: f64, y: f64, tw: f64, th: f64| {
        let _ = write!(
            s,
            "<use xlink:href=\"#p{}\" transform=\"translate({} {}) scale({} {})\"/>",
            id(p),
            n(x),
            n(y),
            n(tw / p.view.0),
            n(th / p.view.1)
        );
    };
    // Corners.
    for (place, x, y) in [(0, 0.0, 0.0), (2, w - b, 0.0), (4, w - b, h - b), (6, 0.0, h - b)] {
        tile(&mut s, places[place], x, y, b, b);
    }
    // Sides: top and bottom run across, right and left run down.
    let count = |length: f64, natural: f64| ((length / natural.max(1e-6)).round() as usize).clamp(1, MAX_TILES_PER_SIDE);
    for (place, y) in [(1, 0.0), (5, h - b)] {
        let p = places[place];
        let length = w - 2.0 * b;
        let k = count(length, b * p.view.0 / p.view.1);
        for i in 0..k {
            tile(&mut s, p, b + length * i as f64 / k as f64, y, length / k as f64, b);
        }
    }
    for (place, x) in [(3, w - b), (7, 0.0)] {
        let p = places[place];
        let length = h - 2.0 * b;
        let k = count(length, b * p.view.1 / p.view.0);
        for i in 0..k {
            tile(&mut s, p, x, b + length * i as f64 / k as f64, b, length / k as f64);
        }
    }
    s.push_str("</svg>\n");
    Some(s)
}

fn n(v: f64) -> String {
    let v = if v.is_finite() { (v * 1000.0).round() / 1000.0 } else { 0.0 };
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(key: u8, kind: u8, data: &[u8]) -> Vec<u8> {
        [vec![key, kind], ((data.len() + 4) as u32).to_le_bytes().to_vec(), data.to_vec()].concat()
    }
    fn short(key: u8, v: u16) -> Vec<u8> {
        [vec![key, 0x18], v.to_le_bytes().to_vec()].concat()
    }

    /// A type-0x46 record with one definition: its pieces, and each place's offset as Publisher
    /// writes it (the pieces end to end, counted from 108).
    fn record(pieces: &[&[u8]], places: [usize; 8]) -> Vec<u8> {
        let mut starts = Vec::new();
        let mut at = 108u16;
        for p in pieces {
            starts.push(at);
            at += p.len() as u16;
        }
        let offsets: Vec<u8> = places.iter().flat_map(|i| short(0, starts[*i])).collect();
        let array: Vec<u8> = pieces.iter().flat_map(|p| list(0, 0x88, &list(1, 0x80, p))).collect();
        let name: Vec<u8> = "Stars\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let art = [
            list(3, 0xc0, &name),
            [vec![4, 0x20], 101_600u32.to_le_bytes().to_vec()].concat(),
            list(8, 0x90, &offsets),
            short(9, pieces.len() as u16),
            list(0xa, 0xa0, &array),
        ]
        .concat();
        [vec![1, 0x20], 1u32.to_le_bytes().to_vec(), list(2, 0xa0, &list(0, 0x88, &art))].concat()
    }

    #[test]
    fn places_map_to_pieces_by_offset() {
        // Seven pieces, the left side sharing the right side's (as "Certificate Banner" does).
        let pieces: Vec<Vec<u8>> = (0..7u8).map(|i| vec![i; 10 + usize::from(i) * 3]).collect();
        let refs: Vec<&[u8]> = pieces.iter().map(Vec::as_slice).collect();
        let arts = parse(&crate::read::properties(&record(&refs, [0, 1, 2, 3, 4, 5, 6, 3])).unwrap()).unwrap();
        let art = arts[0].as_ref().unwrap();
        assert_eq!(art.name, "Stars");
        assert_eq!(art.size_emu, Some(101_600));
        assert_eq!(art.places, [0, 1, 2, 3, 4, 5, 6, 3]);
        assert_eq!(art.pieces[6], pieces[6]);
        // One piece in every place.
        let one = parse(&crate::read::properties(&record(&[&[9; 12]], [0; 8])).unwrap()).unwrap();
        assert_eq!(one[0].as_ref().unwrap().places, [0; 8]);
        // An offset that lands inside a piece: that definition is dropped, keeping its position.
        let mut bad = record(&refs, [0, 1, 2, 3, 4, 5, 6, 3]);
        let at = bad.windows(4).position(|w| w == [0, 0x18, 108, 0]).unwrap();
        bad[at + 2] = 109;
        let arts = parse(&crate::read::properties(&bad).unwrap()).unwrap();
        assert_eq!(arts.len(), 1);
        assert!(arts[0].is_none());
    }

    fn square(id: &str) -> Piece {
        Piece { view: (10.0, 10.0), body: format!("<rect id=\"{id}\" width=\"10\" height=\"10\"/>") }
    }

    #[test]
    fn corners_sit_in_the_corners_and_sides_fill_between_them() {
        let (c, t) = (square("corner"), Piece { view: (20.0, 10.0), body: "<rect width=\"20\" height=\"10\"/>".into() });
        let s = svg([&c, &t, &c, &t, &c, &t, &c, &t], 100.0, 60.0, 10.0).unwrap();
        // Two pieces defined once each, however many times they are used.
        assert_eq!(s.matches("<g id=").count(), 2);
        // Four corners, the top and bottom each 80 pt of 20 x 10 tiles (4 each), the sides 40 pt
        // of tiles 5 pt tall at that piece's proportions (8 each).
        assert_eq!(s.matches("<use").count(), 4 + 4 + 4 + 8 + 8);
        assert!(s.contains("translate(90 50) scale(1 1)"), "bottom-right corner: {s}");
        assert!(s.contains("translate(70 0) scale(1 1)"), "last top tile: {s}");
        assert!(designcraft_images::svg_tree(s.as_bytes()).is_some(), "{s}");
        // Too small for its border: nothing.
        assert!(svg([&c, &t, &c, &t, &c, &t, &c, &t], 15.0, 60.0, 10.0).is_none());
    }

    #[test]
    fn a_metafile_svg_gives_its_view_and_drawing() {
        let p = piece(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"500\" viewBox=\"0 0 200 100\" preserveAspectRatio=\"none\">\n<path d=\"M0 0\"/></svg>\n").unwrap();
        assert_eq!(p.view, (200.0, 100.0));
        assert_eq!(p.body.trim(), "<path d=\"M0 0\"/>");
        assert!(piece(b"<svg viewBox=\"0 0 0 10\"></svg>").is_none());
    }
}
