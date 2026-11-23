//! The web app's `favicon.ico`, rendered from `favicon.svg` (Foliolum's mark, the same file the
//! marketing site uses) so the two never drift apart. Browsers that take an SVG icon use the SVG; the
//! ICO is for the rest and for the `/favicon.ico` request browsers make on their own.
//!
//! The ICO holds 16, 32 and 48 px PNGs (PNG-in-ICO, which every current browser reads).

use std::path::Path;

use resvg::tiny_skia::Pixmap;

/// The sizes inside the ICO, smallest first.
pub const SIZES: [u32; 3] = [16, 32, 48];

/// `svg` drawn at `size`×`size` pixels.
pub fn render(svg: &[u8], size: u32) -> Result<Pixmap, String> {
    let tree = resvg::usvg::Tree::from_data(svg, &resvg::usvg::Options::default()).map_err(|e| format!("favicon.svg: {e}"))?;
    let mut pixmap = Pixmap::new(size, size).ok_or("empty size")?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    Ok(pixmap)
}

/// An ICO file holding `pngs` (size, PNG bytes).
pub fn ico(pngs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // 1 = icon
    out.extend_from_slice(&(pngs.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pngs.len();
    for (size, png) in pngs {
        let side = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256
        out.extend_from_slice(&[side, side, 0, 0]); // width, height, no palette, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in pngs {
        out.extend_from_slice(png);
    }
    out
}

/// The images inside an ICO written by [`ico`]: (size, PNG bytes).
#[cfg(test)]
fn ico_images(ico: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, String> {
    let u16_at = |i: usize| ico.get(i..i + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or("short ICO");
    let u32_at = |i: usize| ico.get(i..i + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or("short ICO");
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return Err("not an ICO".into());
    }
    (0..u16_at(4)? as usize)
        .map(|n| {
            let e = 6 + 16 * n;
            let side = *ico.get(e).ok_or("short ICO")?;
            let (len, at) = (u32_at(e + 8)? as usize, u32_at(e + 12)? as usize);
            let png = ico.get(at..at + len).ok_or("ICO image past the end")?.to_vec();
            Ok((if side == 0 { 256 } else { side as u32 }, png))
        })
        .collect()
}

/// Renders `svg_path` into an ICO at `ico_path`.
pub fn write(svg_path: &Path, ico_path: &Path) -> Result<(), String> {
    let svg = std::fs::read(svg_path).map_err(|e| format!("{}: {e}", svg_path.display()))?;
    let pngs = SIZES.iter().map(|&s| Ok((s, render(&svg, s)?.encode_png().map_err(|e| e.to_string())?))).collect::<Result<Vec<_>, String>>()?;
    std::fs::write(ico_path, ico(&pngs)).map_err(|e| format!("{}: {e}", ico_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn web() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/foliolum-web")
    }

    /// The committed ICO is the SVG as it is now. Compared by pixels, not bytes: rasterising can
    /// differ in the last bit between machines. Run `cargo xtask favicon` after changing the SVG.
    #[test]
    fn the_committed_favicon_is_the_svg_rendered() {
        let svg = std::fs::read(web().join("favicon.svg")).unwrap();
        let images = ico_images(&std::fs::read(web().join("favicon.ico")).unwrap()).unwrap();
        assert_eq!(images.iter().map(|(s, _)| *s).collect::<Vec<_>>(), SIZES);
        for (size, png) in images {
            let have = Pixmap::decode_png(&png).unwrap();
            let want = render(&svg, size).unwrap();
            assert_eq!((have.width(), have.height()), (size, size));
            let worst = have.data().iter().zip(want.data()).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(worst <= 2, "{size} px differs from favicon.svg by {worst}: run cargo xtask favicon");
            // Not blank: the mark covers most of the square.
            assert!(have.pixels().iter().filter(|p| p.alpha() == 255).count() as u32 > size * size / 2);
        }
    }

    #[test]
    fn an_ico_reads_back_what_was_written() {
        let pngs = vec![(16, vec![1, 2, 3]), (256, vec![4, 5])];
        assert_eq!(ico_images(&ico(&pngs)).unwrap(), pngs);
        assert!(ico_images(b"PNG").is_err());
    }
}
