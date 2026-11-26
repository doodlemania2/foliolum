// Copyright (c) 2026 srt8hellcat2-ui. SPDX-License-Identifier: MIT OR Apache-2.0
//! Publisher lays text out with the metrics of the font the text asks for, not of the font that
//! draws it here (docs/pub-format/font-metrics.md). Its single line spacing is that font's Windows
//! line height (ascent + descent + external leading) times the size, and the first baseline sits
//! that font's ascent below the top of the text area.
//!
//! DesignCraft spaces lines at 120% of the size and puts the first baseline at the drawing font's
//! ascent. Where a substitute is taller than the original (EB Garamond for Garamond: 1.007 em
//! against 0.84), short frames that held a line in Publisher hold none here, and long stories run
//! out of room. So for the fonts below, each paragraph's auto leading becomes the original's line
//! height (times Publisher's own multiple, when it stores one), and each frame's and table cell's
//! first baseline is fixed at the original's ascent. Text in any other font keeps DesignCraft's
//! layout.
use std::collections::HashSet;

use designcraft_doc::{CharFormat, Document, FirstBaseline, Leading, Story, StoryId};

/// A font's ascent and single line height, in em.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineMetrics {
    pub ascent: f64,
    pub height: f64,
}

/// Windows ascent and line height (win ascent + win descent + external leading, as GDI reports
/// them) of fonts Publisher files ask for. Sources are in font-metrics.md: the font files that ship
/// with macOS, the fonts embedded in Publisher's own PDFs attached to the gate's files, and, for
/// Garamond's ascent, where Publisher's PDF puts a first baseline.
const FONTS: &[(&str, f64, f64)] = &[
    ("Arial", 0.9053, 1.1499),
    ("Arial Black", 1.1006, 1.4102),
    ("Arial Narrow", 0.9219, 1.1474),
    ("Arial Rounded MT Bold", 0.9463, 1.1572),
    ("Calibri", 0.9521, 1.2207),
    ("Cambria", 0.9502, 1.1724),
    ("Century", 0.9858, 1.2021),
    ("Century Gothic", 1.0059, 1.2261),
    ("Comic Sans MS", 1.1021, 1.3936),
    ("Compacta Blk BT", 1.0068, 1.2427),
    ("Courier New", 0.8325, 1.1328),
    ("Edwardian Script ITC", 0.8506, 1.1787),
    ("Felix Titling", 0.9282, 1.1694),
    ("Franklin Gothic Book", 0.9165, 1.1338),
    ("Garamond", 0.84, 1.125),
    ("Georgia", 0.9170, 1.1362),
    ("Impact", 1.0088, 1.2197),
    ("Microsoft Sans Serif", 0.9219, 1.1319),
    ("Palace Script MT", 0.6841, 0.9263),
    ("Rockwell Condensed", 0.9409, 1.1758),
    ("Tahoma", 1.0005, 1.2070),
    ("Times New Roman", 0.8911, 1.1499),
    ("Trebuchet MS", 0.9390, 1.1612),
    ("Verdana", 1.0054, 1.2154),
];

pub(crate) fn of(family: &str) -> Option<LineMetrics> {
    FONTS.iter().find(|(name, ..)| name.eq_ignore_ascii_case(family.trim())).map(|&(_, ascent, height)| LineMetrics { ascent, height })
}

/// DesignCraft's default size, for text that doesn't set one.
const DEFAULT_SIZE: f64 = 12.0;

/// The run that sets a paragraph's line spacing: its largest text (the first of equals), as auto
/// leading follows the largest size on the line. Returns its metrics and size.
fn dominant(story: &Story, para: std::ops::Range<usize>) -> Option<(LineMetrics, f64)> {
    let mut best: Option<(&CharFormat, f64)> = None;
    for (range, format) in story.runs() {
        if range.end <= para.start || range.start >= para.end.max(para.start + 1) {
            continue;
        }
        let size = format.over.size.unwrap_or(DEFAULT_SIZE);
        if best.is_none_or(|(_, s)| size > s) {
            best = Some((format, size));
        }
    }
    let (format, size) = best?;
    Some((of(format.over.font_family.as_deref()?)?, size))
}

/// Gives each paragraph whose text is in a known font that font's line spacing, and returns where
/// the first baseline of the story belongs (below the top of the text area), when its first
/// paragraph is in a known font.
fn space_lines(story: &mut Story) -> Option<f64> {
    let ranges = story.para_ranges();
    let mut first = None;
    for (i, range) in ranges.into_iter().enumerate() {
        let Some((m, size)) = dominant(story, range.clone()) else { continue };
        if i == 0 {
            first = Some(m.ascent * size);
        }
        let Some(pf) = story.paras.get_mut(i) else { continue };
        if matches!(pf.chars.leading, Some(Leading::Points(_))) {
            continue;
        }
        // Publisher's "N lines" spacing was imported as auto leading N; single is 1.
        let lines = pf.para.auto_leading.unwrap_or(1.0);
        pf.para.auto_leading = Some(lines * m.height);
    }
    if first.is_some() {
        story.rev = story.rev.wrapping_add(1);
    }
    first
}

/// Applies Publisher's font metrics to every story, frame and table cell of `doc`, except the
/// stories in `skip` (WordArt, which is sized to its frame instead).
pub(crate) fn apply(doc: &mut Document, skip: &HashSet<StoryId>) {
    let ids: Vec<_> = doc.stories.keys().copied().filter(|s| !skip.contains(s)).collect();
    for sid in ids {
        let Some(story) = doc.stories.get_mut(&sid) else { continue };
        let first = space_lines(std::sync::Arc::make_mut(story));
        let story = &**story;
        // Cell text: each cell's own first baseline.
        let mut tables = story.tables.clone();
        let mut changed = false;
        for table in tables.values_mut() {
            let table = std::sync::Arc::make_mut(table);
            for cell in &mut table.cells {
                if let Some(base) = space_lines(&mut cell.text) {
                    cell.first_baseline = FirstBaseline::Fixed;
                    cell.first_baseline_min = base;
                    changed = true;
                }
            }
        }
        let frames = story.frames.clone();
        if changed && let Some(story) = doc.stories.get_mut(&sid) {
            let story = std::sync::Arc::make_mut(story);
            story.tables = tables;
            story.rev = story.rev.wrapping_add(1);
        }
        // The first frame holds the first line; later frames of a thread begin wherever the text
        // breaks, so they keep DesignCraft's own first baseline.
        if let (Some(base), Some(frame)) = (first, frames.first())
            && let Some(tf) = doc.item_mut(*frame).and_then(|i| i.text_frame_mut())
        {
            tf.options.first_baseline = FirstBaseline::Fixed;
            tf.options.first_baseline_min = base;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_doc::{ParaFormat, SpreadRef, build::NewDocument};
    use designcraft_geom::Rect;

    fn frame(text: &str, family: &str, size: f64) -> (Document, designcraft_doc::ItemId, designcraft_doc::StoryId) {
        let mut d = Document::new(&NewDocument { pages: 1, ..Default::default() });
        let layer = d.default_layer();
        let (id, sid) = d.add_text_frame(SpreadRef::Doc(0), Rect::new(10.0, 10.0, 210.0, 33.3), layer, text, ParaFormat::default()).unwrap();
        let st = d.story_mut(sid).unwrap();
        let n = st.text.len();
        st.format_chars(0..n, |cf| {
            cf.over.font_family = Some(family.into());
            cf.over.size = Some(size);
        });
        (d, id, sid)
    }

    #[test]
    fn known_fonts_space_lines_and_place_the_first_baseline_as_publisher_does() {
        let (mut d, id, sid) = frame("Keep In Your Prayers\nSecond", "Garamond", 18.0);
        apply(&mut d, &HashSet::new());
        let tf = d.item(id).unwrap().text_frame().unwrap();
        assert_eq!(tf.options.first_baseline, FirstBaseline::Fixed);
        assert!((tf.options.first_baseline_min - 15.12).abs() < 1e-9, "{}", tf.options.first_baseline_min);
        let st = d.story(sid).unwrap();
        assert!(
            st.paras.iter().all(|p| p.para.auto_leading == Some(1.125)),
            "{:?}",
            st.paras.iter().map(|p| p.para.auto_leading).collect::<Vec<_>>()
        );
    }

    #[test]
    fn publisher_multiples_and_exact_spacing_are_kept() {
        let (mut d, _, sid) = frame("One\nTwo", "times new roman", 10.0);
        let st = d.story_mut(sid).unwrap();
        st.paras[0].para.auto_leading = Some(1.5);
        st.paras[1].chars.leading = Some(Leading::Points(14.0));
        apply(&mut d, &HashSet::new());
        let st = d.story(sid).unwrap();
        assert!((st.paras[0].para.auto_leading.unwrap() - 1.5 * 1.1499).abs() < 1e-9);
        assert_eq!(st.paras[1].para.auto_leading, None, "exact spacing stays exact");
    }

    #[test]
    fn unknown_fonts_keep_designcrafts_layout() {
        let (mut d, id, sid) = frame("Text", "Maiandra GD", 12.0);
        apply(&mut d, &HashSet::new());
        assert_eq!(d.item(id).unwrap().text_frame().unwrap().options.first_baseline, FirstBaseline::Ascent);
        assert_eq!(d.story(sid).unwrap().paras[0].para.auto_leading, None);
    }

    #[test]
    fn the_heading_that_fits_in_publisher_fits_here() {
        // Bug 89393's heading: 18 pt Garamond Bold in a 23.3 pt frame with 2.88 pt insets. Publisher
        // shows it; at EB Garamond's ascent its baseline fell below the text area.
        let (mut d, id, sid) = frame("Keep In Your Prayers", "Garamond", 18.0);
        if let Some(tf) = d.item_mut(id).and_then(|i| i.text_frame_mut()) {
            tf.options.inset = [2.88; 4];
        }
        apply(&mut d, &HashSet::new());
        let composed = designcraft_compose::compose_story(&d, sid, &Default::default());
        assert!(!composed.is_overset());
    }
}
