//! Make it fit (#54): overflowing text is fitted to its box by tightening it a little at a time.
//! First the tracking (letter spacing) closes by up to [`MAX_TIGHTEN`] thousandths of an em. If
//! that is not enough, the type and any fixed line spacing shrink, a quarter point at a time, by up
//! to [`MAX_SHRINK`] points. It stops at the first step that fits. The words are never touched, and
//! the whole change is one undo step.
use std::sync::Arc;

use designcraft_engine::Session;
use designcraft_engine::compose::compose_story;
use designcraft_engine::doc::{Document, ItemId, Leading, Story, StoryId};

/// The most the tracking closes, in thousandths of an em.
pub const MAX_TIGHTEN: f64 = 25.0;
/// The most the type shrinks, in points.
pub const MAX_SHRINK: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    AlreadyFits,
    /// Fitted with this much tighter tracking and this much smaller type.
    Fitted {
        tighten: f64,
        shrink: f64,
    },
    /// Even at the limits the text overflows; nothing was changed.
    TooLong,
}

impl Outcome {
    pub fn message(self) -> String {
        match self {
            Outcome::AlreadyFits => "This text already fits its box.".into(),
            Outcome::Fitted { tighten, shrink: 0.0 } => format!("Fitted by tightening the letter spacing ({tighten:.0}/1000 em)."),
            Outcome::Fitted { shrink, .. } => format!("Fitted by tightening the letter spacing and making the type {shrink:.2} pt smaller."),
            Outcome::TooLong => {
                format!("Too much text to fit without changing more than {MAX_SHRINK:.0} pt of type size. Shorten it, or make the box bigger.")
            }
        }
    }
}

/// `story` with every run's tracking closed by `tighten` and its size (and any fixed leading)
/// reduced by `shrink` points, from each run's resolved values.
fn adjusted(doc: &Document, story: &Story, tighten: f64, shrink: f64) -> Story {
    let mut out = story.clone();
    let mut changes = Vec::new();
    for (pi, para) in story.para_ranges().into_iter().enumerate() {
        let Some(pf) = story.paras.get(pi) else { continue };
        let base = doc.styles.resolve_para(pf).1;
        for (range, f) in story.runs() {
            let (a, b) = (range.start.max(para.start), range.end.min(para.end.max(para.start + 1)));
            if a >= b {
                continue;
            }
            let r = doc.styles.resolve_char(&base, f);
            let size = (r.size - shrink).max(1.0);
            let leading = match r.leading {
                Leading::Points(p) if r.size > 0.0 => Some(Leading::Points(p * size / r.size)),
                _ => None,
            };
            changes.push((a..b, r.tracking - tighten, size, leading));
        }
    }
    for (range, tracking, size, leading) in changes {
        out.format_chars(range, |cf| {
            cf.over.tracking = Some(tracking);
            cf.over.size = Some(size);
            if let Some(l) = leading {
                cf.over.leading = Some(l);
            }
        });
    }
    out.rev = out.rev.wrapping_add(1);
    out
}

fn overset(doc: &Document, sid: StoryId, story: Story) -> bool {
    let mut d = doc.clone();
    d.stories.insert(sid, Arc::new(story));
    compose_story(&d, sid, &Default::default()).is_overset()
}

/// The tightest step needed, as (tighten, shrink), or `None` when nothing within the limits fits.
fn find_fit(doc: &Document, sid: StoryId) -> Option<(f64, f64)> {
    let story = doc.stories.get(&sid)?;
    let steps = (1..=5).map(|i| (MAX_TIGHTEN * f64::from(i) / 5.0, 0.0)).chain((1..=8).map(|i| (MAX_TIGHTEN, MAX_SHRINK * f64::from(i) / 8.0)));
    for (tighten, shrink) in steps {
        if !overset(doc, sid, adjusted(doc, story, tighten, shrink)) {
            return Some((tighten, shrink));
        }
    }
    None
}

/// Fits the text of frame `id` (its whole story, when it runs through several boxes).
pub fn fit(s: &mut Session, id: ItemId) -> Result<Outcome, String> {
    let (sid, doc) = {
        let d = s.doc().map_err(|e| e.to_string())?;
        let tf = d.doc.item(id).and_then(|i| i.text_frame()).ok_or("Select a text box whose text overflows.")?;
        (tf.story, d.doc.clone())
    };
    if !compose_story(&doc, sid, &Default::default()).is_overset() {
        return Ok(Outcome::AlreadyFits);
    }
    let Some((tighten, shrink)) = find_fit(&doc, sid) else { return Ok(Outcome::TooLong) };
    let story = adjusted(&doc, &doc.stories[&sid], tighten, shrink);
    crate::undo::as_one_step(s, "Make It Fit", |s| {
        s.edit(|d, _| {
            d.stories.insert(sid, Arc::new(story));
            Ok(())
        })
        .map_err(|e| e.to_string())
    })?;
    Ok(Outcome::Fitted { tighten, shrink })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A frame `h` points tall holding `text` at 12 pt.
    fn frame(text: &str, h: f64) -> (Session, ItemId) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [36, 36, 236, 36.0 + h], "content": "text", "text": text})).unwrap();
        (s, ItemId(r["id"].as_u64().unwrap()))
    }

    fn is_overset(s: &Session, id: ItemId) -> bool {
        let d = &s.doc().unwrap().doc;
        compose_story(d, d.item(id).unwrap().text_frame().unwrap().story, &Default::default()).is_overset()
    }

    /// Text that just overflows a box gets fitted, keeps its words, and comes back with undo.
    #[test]
    fn slightly_long_text_is_fitted_and_undo_restores_it() {
        let text = "Coffee and doughnuts after every Mass this Sunday in the parish hall, hosted by the youth group.";
        // The tallest box it overflows: one line short.
        let mut h = 120.0;
        let (mut s, id) = loop {
            let (s, id) = frame(text, h);
            if is_overset(&s, id) {
                break (s, id);
            }
            h -= 0.5;
            assert!(h > 5.0, "never overflowed");
        };
        let before = s.doc().unwrap().doc.clone();
        let outcome = fit(&mut s, id).unwrap();
        let Outcome::Fitted { tighten, shrink } = outcome else { panic!("{outcome:?}") };
        {
            assert!(tighten <= MAX_TIGHTEN && shrink <= MAX_SHRINK);
            assert!(!is_overset(&s, id));
            let d = &s.doc().unwrap().doc;
            let sid = d.item(id).unwrap().text_frame().unwrap().story;
            assert_eq!(d.stories[&sid].text, text, "the words never change");
            s.execute("edit.undo", &json!({})).unwrap();
            assert!(
                Arc::ptr_eq(&s.doc().unwrap().doc.stories[&sid], &before.stories[&sid]) || s.doc().unwrap().doc.stories[&sid] == before.stories[&sid]
            );
        }
    }

    #[test]
    fn text_that_fits_is_left_alone_and_far_too_much_is_refused() {
        let (mut s, id) = frame("Short.", 100.0);
        assert_eq!(fit(&mut s, id).unwrap(), Outcome::AlreadyFits);
        let long = "word ".repeat(400);
        let (mut s, id) = frame(&long, 40.0);
        let before = s.doc().unwrap().doc.clone();
        assert_eq!(fit(&mut s, id).unwrap(), Outcome::TooLong);
        let d = &s.doc().unwrap().doc;
        let sid = d.item(id).unwrap().text_frame().unwrap().story;
        assert_eq!(d.stories[&sid], before.stories[&sid], "nothing changed");
    }
}
