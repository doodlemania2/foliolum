//! Parish starter pack (#56): ready-made documents a parish opens from the start screen and makes its
//! own in minutes. Each is built with the editor's own commands when it is picked, so nothing ships
//! as a file and every template is always in the current document format.
//!
//! The parish name, address, phone, email and website are Our details fields (#53), filled from the
//! details saved in this browser, so a parish that has typed them once sees its own name at once.
//! Lists (Mass times, the week's events, a sign-up sheet) are smart frames (#52) holding sample rows,
//! ready to be refilled from a spreadsheet.
use std::sync::Arc;

use designcraft_engine::Session;
use designcraft_engine::doc::{Align, ItemId, StoryId};
use serde_json::{Value, json};

use crate::profile::{FIELDS, Profile};
use crate::smart::{Binding, Layout};

/// The swatch the templates colour headings and bands with: Foliolum's rubric.
const RUBRIC: &str = "Rubric";
const SERIF: &str = "EB Garamond";
const SANS: &str = "Libre Franklin";

/// A template in the pack.
#[derive(Clone, Copy, Debug)]
pub struct Starter {
    pub id: &'static str,
    pub name: &'static str,
    /// One line for its card.
    pub detail: &'static str,
    pub pages: usize,
}

pub const STARTERS: [Starter; 4] = [
    Starter { id: "bulletin", name: "Weekly bulletin", detail: "Mass times, the pastor's letter and the week ahead", pages: 2 },
    Starter { id: "flyer", name: "Event flyer", detail: "One event, big and clear, for the noticeboard", pages: 1 },
    Starter { id: "signup", name: "Sign-up sheet", detail: "A table for names, phones and shifts", pages: 1 },
    Starter { id: "newsletter", name: "Newsletter", detail: "Stories, photos and the month's calendar", pages: 2 },
];

/// What shown text should look like.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    /// The parish name in a band.
    Masthead,
    /// Contact lines in a band.
    BandLine,
    Title,
    /// A poster-sized title.
    Display,
    Subtitle,
    Heading,
    Body,
    Small,
}

impl Look {
    /// (family, style, size, colour swatch, space before, space after)
    fn spec(self) -> (&'static str, &'static str, f64, &'static str, f64, f64) {
        match self {
            Look::Masthead => (SERIF, "Bold", 30.0, "[Paper]", 0.0, 6.0),
            Look::BandLine => (SANS, "Regular", 10.0, "[Paper]", 0.0, 0.0),
            Look::Title => (SERIF, "Bold", 28.0, RUBRIC, 0.0, 4.0),
            Look::Display => (SERIF, "Bold", 54.0, RUBRIC, 0.0, 12.0),
            Look::Subtitle => (SANS, "Regular", 13.0, "[Black]", 0.0, 8.0),
            Look::Heading => (SANS, "Bold", 13.0, RUBRIC, 8.0, 4.0),
            Look::Body => (SANS, "Regular", 10.5, "[Black]", 0.0, 6.0),
            Look::Small => (SANS, "Regular", 9.0, "[Black]", 0.0, 2.0),
        }
    }
}

/// One paragraph: its look, alignment and text. `{name}`, `{address}`, `{phone}`, `{email}` and
/// `{website}` become Our details fields.
type Para<'a> = (Look, Align, &'a str);

/// The token for each detail, in [`FIELDS`] order.
const TOKENS: [&str; 5] = ["{name}", "{address}", "{phone}", "{email}", "{website}"];

/// What a detail shows until the parish fills it in.
const PLACEHOLDERS: [&str; 5] = ["Your Parish Name", "Street address, Town, State ZIP", "(555) 555-0100", "office@yourparish.org", "yourparish.org"];

/// The text with the tokens taken out, and where each detail goes: (byte offset, detail index).
fn split_tokens(text: &str) -> (String, Vec<(usize, usize)>) {
    let (mut out, mut at, mut rest) = (String::new(), Vec::new(), text);
    while let Some((pos, i)) = TOKENS.iter().enumerate().filter_map(|(i, t)| rest.find(t).map(|p| (p, i))).min() {
        out.push_str(&rest[..pos]);
        at.push((out.len(), i));
        rest = &rest[pos + TOKENS[i].len()..];
    }
    out.push_str(rest);
    (out, at)
}

struct Builder<'a> {
    s: &'a mut Session,
}

impl Builder<'_> {
    fn run(&mut self, cmd: &str, p: Value) -> Result<Value, String> {
        self.s.execute(cmd, &p).map_err(|e| format!("{cmd}: {e}"))
    }

    fn story(&self, id: ItemId) -> Result<StoryId, String> {
        let d = self.s.doc().map_err(|e| e.to_string())?;
        d.doc.item(id).and_then(|i| i.text_frame()).map(|t| t.story).ok_or_else(|| "not a text frame".to_string())
    }

    /// A filled rectangle with no line.
    fn band(&mut self, page: usize, rect: [f64; 4], swatch: &str) -> Result<ItemId, String> {
        let r = self.run("frame.create", json!({"rect": rect, "spread": page, "content": "unassigned"}))?;
        let id = r["id"].as_u64().ok_or("frame.create gave no id")?;
        self.run("object.fill", json!({"swatch": swatch, "ids": [id]}))?;
        self.run("object.stroke", json!({"swatch": "[None]", "ids": [id]}))?;
        Ok(ItemId(id))
    }

    /// A text frame holding `paras`, each formatted by its look, with the detail tokens made fields.
    fn text(&mut self, page: usize, rect: [f64; 4], paras: &[Para]) -> Result<ItemId, String> {
        let joined = paras.iter().map(|p| p.2).collect::<Vec<_>>().join("\n");
        let (text, fields) = split_tokens(&joined);
        let r = self.run("frame.create", json!({"rect": rect, "spread": page, "content": "text", "text": text}))?;
        let id = ItemId(r["id"].as_u64().ok_or("frame.create gave no id")?);
        let sid = self.story(id)?;
        // Last first, so the earlier offsets still hold.
        for &(at, i) in fields.iter().rev() {
            self.run("text.select", json!({"story": sid.0, "anchor": at}))?;
            self.run("variables.insert", json!({"name": FIELDS[i].1}))?;
        }
        let looks: Vec<(Look, Align)> = paras.iter().map(|p| (p.0, p.1)).collect();
        self.s
            .edit(|d, _| {
                let Some(story) = d.stories.get_mut(&sid) else { return Ok(()) };
                let st = Arc::make_mut(story);
                for (range, (look, align)) in st.para_ranges().into_iter().zip(looks) {
                    let (family, style, size, fill, before, after) = look.spec();
                    let chars = range.start..range.end.max(range.start + 1).min(st.text.len().max(range.start + 1));
                    st.format_paras(range.clone(), |pf| {
                        pf.para.align = Some(align);
                        pf.para.space_before = Some(before);
                        pf.para.space_after = Some(after);
                    });
                    st.format_chars(chars, |cf| {
                        cf.over.font_family = Some(family.into());
                        cf.over.font_style = Some(style.into());
                        cf.over.size = Some(size);
                        cf.over.fill = Some(fill.into());
                    });
                }
                st.rev = st.rev.wrapping_add(1);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// A smart frame showing `rows` (the first row names the columns).
    fn smart(&mut self, page: usize, rect: [f64; 4], layout: Layout, pattern: &str, rows: &[&[&str]]) -> Result<ItemId, String> {
        let r = self.run("frame.create", json!({"rect": rect, "spread": page, "content": "text", "text": ""}))?;
        let id = ItemId(r["id"].as_u64().ok_or("frame.create gave no id")?);
        let binding = Binding {
            layout,
            pattern: pattern.into(),
            header: true,
            source: "sample rows".into(),
            rows: rows.iter().map(|r| r.iter().map(|c| c.to_string()).collect()).collect(),
        };
        crate::smart::fill(self.s, id, &binding)?;
        let sid = self.story(id)?;
        self.s
            .edit(|d, _| {
                let Some(story) = d.stories.get_mut(&sid) else { return Ok(()) };
                let st = Arc::make_mut(story);
                let n = st.text.len().max(1);
                st.format_chars(0..n, |cf| {
                    cf.over.font_family = Some(SANS.into());
                    cf.over.size = Some(if layout == Layout::Lines { 11.0 } else { 10.0 });
                });
                if layout == Layout::Lines {
                    st.format_paras(0..n, |pf| pf.para.space_after = Some(3.0));
                }
                // Table text in the template's type, with the header row white on rubric.
                for table in st.tables.values_mut() {
                    let t = Arc::make_mut(table);
                    let width = t.columns.len().max(1);
                    for (k, cell) in t.cells.iter_mut().enumerate() {
                        let header = k < width;
                        let n = cell.text.text.len().max(1);
                        cell.text.format_chars(0..n, |cf| {
                            cf.over.font_family = Some(SANS.into());
                            cf.over.font_style = Some(if header { "Bold" } else { "Regular" }.into());
                            cf.over.size = Some(10.0);
                            if header {
                                cf.over.fill = Some("[Paper]".into());
                            }
                        });
                        if header {
                            cell.fill = RUBRIC.into();
                        }
                    }
                }
                st.rev = st.rev.wrapping_add(1);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// The parish name and contact line on a rubric band across the top of `page`.
    fn masthead(&mut self, page: usize, kicker: &str) -> Result<(), String> {
        self.band(page, [36.0, 36.0, 576.0, 120.0], RUBRIC)?;
        let line = format!("{kicker}{{address}}  |  {{phone}}  |  {{website}}");
        self.text(page, [52.0, 46.0, 560.0, 116.0], &[(Look::Masthead, Align::Left, "{name}"), (Look::BandLine, Align::Left, &line)])?;
        Ok(())
    }

    /// The contact block along the foot of `page`.
    fn footer(&mut self, page: usize) -> Result<(), String> {
        self.band(page, [36.0, 708.0, 576.0, 709.5], RUBRIC)?;
        self.text(
            page,
            [36.0, 716.0, 576.0, 756.0],
            &[(Look::Small, Align::Center, "{name}  |  {address}"), (Look::Small, Align::Center, "{phone}  |  {email}  |  {website}")],
        )?;
        Ok(())
    }
}

fn bulletin(b: &mut Builder) -> Result<(), String> {
    b.masthead(0, "")?;
    b.text(
        0,
        [36.0, 136.0, 576.0, 196.0],
        &[(Look::Title, Align::Left, "Sunday Bulletin"), (Look::Subtitle, Align::Left, "Week of Sunday, ____________")],
    )?;
    b.text(0, [36.0, 204.0, 290.0, 226.0], &[(Look::Heading, Align::Left, "Mass Times")])?;
    b.smart(
        0,
        [36.0, 228.0, 290.0, 330.0],
        Layout::Table,
        "",
        &[
            &["Day", "Time", "Celebrant"],
            &["Saturday", "5:00 pm", "Fr. Example"],
            &["Sunday", "8:00 am", "Fr. Example"],
            &["Sunday", "10:30 am", "Fr. Example"],
            &["Weekdays", "8:00 am", "Fr. Example"],
        ],
    )?;
    b.text(
        0,
        [36.0, 340.0, 290.0, 700.0],
        &[
            (Look::Heading, Align::Left, "Confession and Adoration"),
            (Look::Body, Align::Left, "Saturdays 3:30 to 4:30 pm, or by appointment. Adoration Fridays after the 8:00 am Mass until noon."),
            (Look::Heading, Align::Left, "Prayer List"),
            (Look::Body, Align::Left, "Please pray for the sick of our parish and for all who have died, especially ____________."),
        ],
    )?;
    b.text(
        0,
        [310.0, 204.0, 576.0, 700.0],
        &[
            (Look::Heading, Align::Left, "From the Pastor"),
            (Look::Body, Align::Left, "Dear friends, write a few words to the parish here. Click in this text, select it and type over it."),
            (Look::Body, Align::Left, "Each week, change the date above and the Mass times at left. To fill the times from a spreadsheet, select the table's box and choose Insert > Smart frame."),
            (Look::Body, Align::Left, "Your parish name, address and phone in this bulletin come from Insert > Our details. Change them there and every page updates."),
        ],
    )?;
    b.footer(0)?;
    b.text(1, [36.0, 36.0, 576.0, 74.0], &[(Look::Title, Align::Left, "This Week at {name}")])?;
    b.smart(
        1,
        [36.0, 80.0, 576.0, 200.0],
        Layout::Lines,
        "{Day}, {Time}: {Event}",
        &[
            &["Day", "Time", "Event"],
            &["Monday", "7:00 pm", "Bible study in the parish hall"],
            &["Wednesday", "6:30 pm", "Choir rehearsal"],
            &["Thursday", "9:30 am", "Rosary group"],
            &["Saturday", "9:00 am", "Food pantry volunteers"],
        ],
    )?;
    b.text(1, [36.0, 212.0, 576.0, 234.0], &[(Look::Heading, Align::Left, "Ministry Contacts")])?;
    b.smart(
        1,
        [36.0, 236.0, 576.0, 340.0],
        Layout::Table,
        "",
        &[
            &["Ministry", "Contact", "Phone"],
            &["Altar servers", "Name", "(555) 555-0101"],
            &["Lectors", "Name", "(555) 555-0102"],
            &["Religious education", "Name", "(555) 555-0103"],
        ],
    )?;
    b.text(
        1,
        [36.0, 352.0, 576.0, 700.0],
        &[
            (Look::Heading, Align::Left, "Stewardship"),
            (Look::Body, Align::Left, "Thank you for your generosity. Last week's offertory: $________. Online giving: {website}"),
            (Look::Heading, Align::Left, "Announcements"),
            (Look::Body, Align::Left, "New parishioners are always welcome. Please register at the parish office or call {phone}."),
            (Look::Body, Align::Left, "Bulletin notices are due in the parish office by noon on Tuesday. Send them to {email}."),
        ],
    )?;
    b.footer(1)
}

fn flyer(b: &mut Builder) -> Result<(), String> {
    b.masthead(0, "")?;
    b.text(
        0,
        [36.0, 190.0, 576.0, 600.0],
        &[
            (Look::Display, Align::Center, "Parish Picnic"),
            (Look::Subtitle, Align::Center, "Sunday, ____________  |  12:00 to 4:00 pm"),
            (Look::Subtitle, Align::Center, "On the parish grounds"),
            (
                Look::Body,
                Align::Center,
                "Food, games for the children and music all afternoon. Everyone is welcome: bring a friend, a neighbour and a dish to share.",
            ),
            (Look::Heading, Align::Center, "Questions?"),
            (Look::Body, Align::Center, "Call the parish office at {phone} or write to {email}."),
        ],
    )?;
    b.footer(0)
}

fn signup(b: &mut Builder) -> Result<(), String> {
    b.masthead(0, "")?;
    b.text(
        0,
        [36.0, 136.0, 576.0, 214.0],
        &[
            (Look::Title, Align::Left, "Volunteer Sign-up"),
            (Look::Subtitle, Align::Left, "Event: ____________   Date: ____________   Coordinator: ____________"),
        ],
    )?;
    let blank: &[&str] = &["", "", "", ""];
    let mut rows: Vec<&[&str]> = vec![&["Name", "Phone", "Email", "Shift"]];
    rows.extend(std::iter::repeat_n(blank, 16));
    b.smart(0, [36.0, 222.0, 576.0, 690.0], Layout::Table, "", &rows)?;
    b.footer(0)
}

fn newsletter(b: &mut Builder) -> Result<(), String> {
    b.masthead(0, "Parish Newsletter  |  ")?;
    b.text(
        0,
        [36.0, 136.0, 576.0, 196.0],
        &[(Look::Title, Align::Left, "Month Year"), (Look::Subtitle, Align::Left, "News from our parish family")],
    )?;
    let story = |title: &'static str| -> [Para<'static>; 3] {
        [
            (Look::Heading, Align::Left, title),
            (Look::Body, Align::Left, "Write the story here. Click in this text, select it and type over it, or paste it from your word processor."),
            (Look::Body, Align::Left, "To add a photo, choose Insert > Picture and drag it where you want it; the text flows around it."),
        ]
    };
    b.text(0, [36.0, 204.0, 296.0, 700.0], &story("A Word from the Pastor"))?;
    b.text(0, [316.0, 204.0, 576.0, 700.0], &story("Parish News"))?;
    b.footer(0)?;
    b.text(1, [36.0, 36.0, 576.0, 74.0], &[(Look::Title, Align::Left, "Calendar")])?;
    b.smart(
        1,
        [36.0, 80.0, 576.0, 250.0],
        Layout::Table,
        "",
        &[
            &["Date", "Time", "Event", "Where"],
            &["Sun 1", "10:30 am", "Family Mass", "Church"],
            &["Wed 4", "7:00 pm", "Parish council", "Hall"],
            &["Sat 14", "9:00 am", "Parish clean-up", "Grounds"],
        ],
    )?;
    b.text(1, [36.0, 262.0, 296.0, 700.0], &story("Ministry Spotlight"))?;
    b.text(1, [316.0, 262.0, 576.0, 700.0], &story("Around the Parish"))?;
    b.footer(1)
}

/// Builds starter `id` in the open document (a fresh one of its page count), with the details in `p`
/// (placeholders where a detail is empty). The document starts with no undo history.
pub fn build(s: &mut Session, id: &str, p: &Profile) -> Result<(), String> {
    let make: fn(&mut Builder) -> Result<(), String> = match id {
        "bulletin" => bulletin,
        "flyer" => flyer,
        "signup" => signup,
        "newsletter" => newsletter,
        _ => return Err(format!("no template `{id}`")),
    };
    s.execute("swatch.create", &json!({"name": RUBRIC, "color": "#b3322a"})).map_err(|e| format!("swatch.create: {e}"))?;
    for (i, (_, var)) in FIELDS.iter().enumerate() {
        let v = p.get(i);
        crate::profile::define(s, var, if v.trim().is_empty() { PLACEHOLDERS[i] } else { v })?;
    }
    make(&mut Builder { s })?;
    let st = s.doc_mut().map_err(|e| e.to_string())?;
    st.history.undo.clear();
    st.history.redo.clear();
    let _ = s.execute("selection.set", &json!({"ids": []}));
    Ok(())
}

/// The `file.new` parameters for starter `st`.
pub fn new_params(st: &Starter) -> Value {
    json!({"preset": "Letter", "pages": st.pages, "facingPages": false, "title": st.name})
}

/// Opens starter `st` in the editor as a new untitled document.
pub fn open(app: &mut designcraft_ui_egui::DesignApp, st: &Starter, p: &Profile) {
    if let Err(e) = app.run("file.new", new_params(st)) {
        app.status_error("Parish templates", e);
        return;
    }
    match build(&mut app.session, st.id, p) {
        Ok(()) => app.status(format!("{} is ready. Click any text to make it yours.", st.name)),
        Err(e) => app.status_error("Parish templates", e),
    }
}

/// The start screen's "Parish templates" cards (Brand::start_section).
pub fn start_section(app: &mut designcraft_ui_egui::DesignApp, ui: &mut egui::Ui) {
    let t = designcraft_ui_egui::theme::Tokens::get(ui.ctx());
    ui.label(egui::RichText::new("Parish templates").size(15.0).strong().color(t.text_strong));
    ui.label(egui::RichText::new("Your details from Insert > Our details fill in as the template opens.").size(12.0).color(t.text_dim));
    ui.add_space(8.0);
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        for st in &STARTERS {
            let (cr, resp) = ui.allocate_exact_size(egui::vec2(150.0, 170.0), egui::Sense::click());
            let painter = ui.painter();
            painter.rect_filled(cr, 6.0, if resp.hovered() { t.hover } else { t.panel });
            // A small picture of the page: a rubric band and lines of text.
            let page = egui::Rect::from_center_size(cr.center() - egui::vec2(0.0, 22.0), egui::vec2(66.0, 85.0));
            painter.rect_filled(page, 0.0, egui::Color32::from_gray(245));
            let rubric = egui::Color32::from_rgb(0xb3, 0x32, 0x2a);
            painter.rect_filled(egui::Rect::from_min_size(page.min + egui::vec2(4.0, 4.0), egui::vec2(58.0, 12.0)), 0.0, rubric);
            for k in 0..6 {
                let y = page.min.y + 24.0 + 9.0 * k as f32;
                let w = if k % 3 == 2 { 36.0 } else { 58.0 };
                painter.line_segment(
                    [egui::pos2(page.min.x + 4.0, y), egui::pos2(page.min.x + 4.0 + w, y)],
                    egui::Stroke::new(2.0, egui::Color32::from_gray(190)),
                );
            }
            painter.text(
                egui::pos2(cr.center().x, cr.max.y - 40.0),
                egui::Align2::CENTER_CENTER,
                st.name,
                egui::FontId::proportional(12.5),
                t.text_strong,
            );
            let galley = painter.layout(st.detail.to_string(), egui::FontId::proportional(10.0), t.text_dim, 136.0);
            painter.galley(egui::pos2(cr.center().x - galley.size().x / 2.0, cr.max.y - 30.0), galley, t.text_dim);
            if resp.on_hover_text(st.detail).clicked() {
                picked = Some(*st);
            }
        }
    });
    if let Some(st) = picked {
        open(app, &st, &stored_profile());
    }
    // The templates the person's parish shares, once signed in (#60).
    crate::account::start_section(ui);
}

/// The details saved in this browser (none outside one).
fn stored_profile() -> Profile {
    #[cfg(target_arch = "wasm32")]
    {
        crate::web::stored_profile()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Profile::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_engine::compose::compose_story;

    fn opened(st: &Starter, p: &Profile) -> Session {
        foliolum_fonts::install(designcraft_fonts::FontDb::global());
        let mut s = Session::new();
        s.execute("file.new", &new_params(st)).unwrap();
        build(&mut s, st.id, p).unwrap_or_else(|e| panic!("{}: {e}", st.id));
        s
    }

    fn value(s: &mut Session, name: &str) -> Option<String> {
        s.execute("variables.list", &json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .map(|v| v["value"].as_str().unwrap_or("").to_string())
    }

    #[test]
    fn tokens_become_field_positions() {
        let (text, at) = split_tokens("Call {phone} or {email}.");
        assert_eq!(text, "Call  or .");
        assert_eq!(at, vec![(5, 2), (9, 3)]);
        assert_eq!(split_tokens("plain"), ("plain".to_string(), vec![]));
    }

    /// Every template opens with its pages, nothing left on undo, every story fitting its frames, the
    /// details as fields, smart frames that know their data, and it exports.
    #[test]
    fn every_template_builds_fits_and_exports() {
        let p = Profile { name: "St. Example Parish".into(), phone: "555-0142".into(), ..Default::default() };
        for st in &STARTERS {
            let mut s = opened(st, &p);
            let d = s.doc().unwrap();
            assert_eq!(d.doc.pages().count(), st.pages, "{}", st.id);
            assert!(d.history.undo.is_empty(), "{}: opens with no undo", st.id);
            let doc = d.doc.clone();
            for sid in doc.stories.keys() {
                assert!(!compose_story(&doc, *sid, &Default::default()).is_overset(), "{}: story {sid:?} overflows", st.id);
            }
            let smart = doc.all_items().into_iter().filter(|&i| crate::smart::binding_of(&s, i).is_some()).count();
            let want = match st.id {
                "bulletin" => 3,
                "flyer" => 0,
                _ => 1,
            };
            assert_eq!(smart, want, "{}: its lists are smart frames", st.id);
            assert_eq!(value(&mut s, "Organization name").as_deref(), Some("St. Example Parish"));
            assert_eq!(value(&mut s, "Organization website").as_deref(), Some(PLACEHOLDERS[4]), "an empty detail shows its placeholder");
            let pdf = s.execute("file.exportPdf", &json!({})).unwrap();
            assert!(pdf["base64"].as_str().is_some_and(|b| b.len() > 1000), "{}: exports", st.id);
        }
    }

    /// The parish name appears as a field, so saving new details updates the template.
    #[test]
    fn saving_details_updates_an_opened_template() {
        let mut s = opened(&STARTERS[0], &Profile::default());
        assert_eq!(value(&mut s, "Organization name").as_deref(), Some(PLACEHOLDERS[0]));
        let p = Profile { name: "Holy Example".into(), ..Default::default() };
        assert_eq!(crate::profile::apply(&mut s, &p).unwrap(), 5, "all five details are in use");
        assert_eq!(value(&mut s, "Organization name").as_deref(), Some("Holy Example"));
    }

    /// Writes each template as a PDF into `$FOLIOLUM_STARTERS_OUT`, to look at them:
    /// `FOLIOLUM_STARTERS_OUT=/tmp/x cargo test -p foliolum-web write_template_pdfs -- --ignored`.
    #[test]
    #[ignore]
    fn write_template_pdfs() {
        let dir = std::env::var("FOLIOLUM_STARTERS_OUT").expect("FOLIOLUM_STARTERS_OUT");
        for st in &STARTERS {
            let mut s = opened(st, &Profile { name: "St. Example Parish".into(), ..Default::default() });
            let pdf = s.execute("file.exportPdf", &json!({})).unwrap();
            let bytes = designcraft_engine::cmd::base64_decode(pdf["base64"].as_str().unwrap());
            std::fs::write(format!("{dir}/{}.pdf", st.id), bytes).unwrap();
        }
    }

    #[test]
    fn an_unknown_template_is_refused() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        assert!(build(&mut s, "nope", &Profile::default()).unwrap_err().contains("nope"));
    }

    /// The cards draw, and picking one opens the template as a new document in the editor.
    #[test]
    fn the_cards_draw_and_a_pick_opens_the_template() {
        let ctx = egui::Context::default();
        let mut app = designcraft_ui_egui::DesignApp::new(Session::new(), Default::default());
        let mut out = ctx.run_ui(Default::default(), |ui| start_section(&mut app, ui));
        out.textures_delta.clear();
        open(&mut app, &STARTERS[1], &Profile::default());
        let d = app.session.doc().unwrap();
        assert_eq!(d.doc.pages().count(), 1);
        assert!(d.doc.all_items().len() >= 4, "band, masthead, body and footer");
    }
}
