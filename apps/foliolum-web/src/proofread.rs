//! Review > Proofread (#60): the selected text box's story goes to Foliolum's backend, which asks
//! the fleet's AI relay and sends back suggestions, each naming the words it would change by
//! character offsets into the text that was sent. Nothing changes until the person applies a
//! suggestion; each one applied is one undo step (`text.select` then `text.insert`). Before one is
//! applied its words are checked against the story as it is now, so editing the text while the
//! dialog is open can't make a fix land on the wrong words.
use designcraft_engine::Session;
use serde::Deserialize;
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Suggestion {
    pub start: usize,
    pub end: usize,
    pub original: String,
    pub replacement: String,
    #[serde(default)]
    pub reason: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Open,
    Applied,
    Skipped,
}

/// One proofread story and what became of each suggestion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    pub story: u64,
    /// The story's text as it was sent.
    pub sent: String,
    pub items: Vec<(Suggestion, Mark)>,
}

/// The byte offset of character `n` of `s` (the end when `n` is past it).
fn byte_at(s: &str, n: usize) -> usize {
    s.char_indices().nth(n).map_or(s.len(), |(i, _)| i)
}

/// The text to proofread: the selected text box's story.
pub fn selected_story(s: &Session) -> Option<(u64, String)> {
    let frame = crate::smart::selected_frame(s)?;
    let st = s.doc().ok()?;
    let story = st.doc.item(frame)?.text_frame()?.story;
    Some((story.0, st.doc.stories.get(&story)?.text.clone()))
}

impl Proof {
    /// Suggestions from the backend, kept only where they name the words the sent text has there.
    pub fn new(story: u64, sent: String, suggestions: Vec<Suggestion>) -> Proof {
        let chars = sent.chars().count();
        let items = suggestions
            .into_iter()
            .filter(|x| {
                x.start < x.end && x.end <= chars && {
                    let (a, b) = (byte_at(&sent, x.start), byte_at(&sent, x.end));
                    sent[a..b] == x.original
                }
            })
            .map(|x| (x, Mark::Open))
            .collect();
        Proof { story, sent, items }
    }

    /// Where suggestion `i` sits in the story now: its bytes in the sent text, moved by every fix
    /// already applied before it.
    fn range_now(&self, i: usize) -> (usize, usize) {
        let (x, _) = &self.items[i];
        let (a, b) = (byte_at(&self.sent, x.start), byte_at(&self.sent, x.end));
        let shift: isize =
            self.items[..i].iter().filter(|(_, m)| *m == Mark::Applied).map(|(y, _)| y.replacement.len() as isize - y.original.len() as isize).sum();
        ((a as isize + shift) as usize, (b as isize + shift) as usize)
    }

    /// Applies suggestion `i` to the story, as one undo step.
    pub fn apply(&mut self, s: &mut Session, i: usize) -> Result<(), String> {
        if self.items.get(i).is_none_or(|(_, m)| *m != Mark::Open) {
            return Ok(());
        }
        let (a, b) = self.range_now(i);
        let now = {
            let st = s.doc().map_err(|e| e.to_string())?;
            st.doc.stories.get(&designcraft_engine::doc::StoryId(self.story)).map(|t| t.text.clone()).ok_or("That text box is gone.")?
        };
        let (x, _) = &self.items[i];
        if now.get(a..b) != Some(x.original.as_str()) {
            return Err("The text has changed since it was proofread. Proofread it again.".into());
        }
        s.execute("text.select", &json!({"story": self.story, "anchor": a, "focus": b})).map_err(|e| e.to_string())?;
        s.execute("text.insert", &json!({"text": x.replacement, "raw": true})).map_err(|e| e.to_string())?;
        self.items[i].1 = Mark::Applied;
        Ok(())
    }

    pub fn open(&self) -> usize {
        self.items.iter().filter(|(_, m)| *m == Mark::Open).count()
    }
}

/// What the proofread dialog asks of the shell.
pub enum Want {
    /// Send the selected text box's text.
    Check(u64, String),
}

/// Review > Proofread's dialog.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    pub busy: bool,
    pub message: String,
    pub proof: Option<Proof>,
}

impl Dialog {
    pub fn show(&mut self, ctx: &egui::Context, s: &mut Session) -> Option<Want> {
        if !self.open {
            return None;
        }
        let mut open = true;
        let mut want = None;
        egui::Window::new("Proofread").open(&mut open).collapsible(false).resizable(true).default_width(480.0).show(ctx, |ui| {
            ui.label("Checks the selected text box for spelling, grammar and punctuation. Nothing changes until you apply a suggestion, and Undo takes back each one.");
            ui.add_space(4.0);
            if self.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Proofreading… this can take half a minute.");
                });
            } else if ui.button(if self.proof.is_some() { "Proofread again" } else { "Proofread the selected text box" }).clicked() {
                match selected_story(s) {
                    Some((_, text)) if text.trim().is_empty() => self.message = "That text box is empty.".into(),
                    Some((story, text)) => {
                        self.message.clear();
                        want = Some(Want::Check(story, text));
                    }
                    None => self.message = "Select one text box first.".into(),
                }
            }
            if let Some(proof) = &mut self.proof {
                ui.separator();
                if proof.items.is_empty() {
                    ui.label("No suggestions: it reads well.");
                }
                let mut apply = None;
                egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                    for (i, (x, mark)) in proof.items.iter_mut().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&x.original).strikethrough());
                            ui.label("→");
                            ui.label(egui::RichText::new(&x.replacement).strong());
                            if !x.reason.is_empty() {
                                ui.label(egui::RichText::new(&x.reason).small());
                            }
                            match mark {
                                Mark::Open => {
                                    if ui.small_button("Apply").clicked() {
                                        apply = Some(i);
                                    }
                                    if ui.small_button("Skip").clicked() {
                                        *mark = Mark::Skipped;
                                    }
                                }
                                Mark::Applied => {
                                    ui.label(egui::RichText::new("applied").small().italics());
                                }
                                Mark::Skipped => {
                                    ui.label(egui::RichText::new("skipped").small().italics());
                                }
                            }
                        });
                    }
                });
                if proof.open() > 1 && ui.button("Apply all").clicked() {
                    for i in 0..proof.items.len() {
                        if let Err(e) = proof.apply(s, i) {
                            self.message = e;
                            break;
                        }
                    }
                }
                if let Some(i) = apply
                    && let Err(e) = proof.apply(s, i)
                {
                    self.message = e;
                }
            }
            if !self.message.is_empty() {
                ui.label(egui::RichText::new(&self.message).small());
            }
        });
        self.open = open;
        want
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_engine::doc::ItemId;

    fn session(text: &str) -> (Session, u64) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap_or_default();
        let r = s.execute("frame.create", &json!({"rect": [36, 36, 400, 300], "content": "text", "text": text})).unwrap_or_default();
        let id = ItemId(r["id"].as_u64().unwrap_or_default());
        let story = s.doc().ok().and_then(|d| d.doc.item(id)?.text_frame().map(|f| f.story.0)).unwrap_or_default();
        let _ = s.execute("selection.set", &json!({"ids": [id.0]}));
        (s, story)
    }

    fn text(s: &Session, story: u64) -> String {
        s.doc().ok().and_then(|d| d.doc.stories.get(&designcraft_engine::doc::StoryId(story)).map(|t| t.text.clone())).unwrap_or_default()
    }

    fn sug(start: usize, end: usize, original: &str, replacement: &str) -> Suggestion {
        Suggestion { start, end, original: original.into(), replacement: replacement.into(), reason: String::new() }
    }

    #[test]
    fn suggestions_apply_in_any_order_even_after_accented_text() {
        // "Café" puts a 2-byte character before the fixes, so characters and bytes differ.
        let sent = "Café opens Sunay. Teh coffee is \"free\".";
        let (mut s, story) = session(sent);
        let (got, sent) = selected_story(&s).unwrap();
        assert_eq!(got, story);
        let mut p = Proof::new(story, sent.clone(), vec![sug(11, 16, "Sunay", "Sunday"), sug(18, 21, "Teh", "The"), sug(0, 4, "Nope", "x")]);
        assert_eq!(p.items.len(), 2, "a suggestion whose words aren't there is dropped");
        p.apply(&mut s, 1).unwrap();
        p.apply(&mut s, 0).unwrap();
        assert_eq!(text(&s, story), "Café opens Sunday. The coffee is \"free\".", "inserted raw: the straight quotes stay");
        assert_eq!(p.open(), 0);
        // Each was its own undo step.
        s.execute("edit.undo", &json!({})).unwrap_or_default();
        assert_eq!(text(&s, story), "Café opens Sunay. The coffee is \"free\".");
    }

    #[test]
    fn a_fix_whose_words_were_edited_since_is_refused() {
        let (mut s, story) = session("Join us Sunay.");
        let (_, sent) = selected_story(&s).unwrap();
        let mut p = Proof::new(story, sent, vec![sug(8, 13, "Sunay", "Sunday")]);
        let _ = s.execute("text.select", &json!({"story": story, "anchor": 8, "focus": 13}));
        let _ = s.execute("text.insert", &json!({"text": "Monday", "raw": true}));
        assert!(p.apply(&mut s, 0).unwrap_err().contains("changed"));
        assert_eq!(text(&s, story), "Join us Monday.");
    }
}
