//! The simple editor's page strip: the document's pages down the left edge, as thumbnails, spreads
//! side by side, each numbered underneath. A click goes to that page.
//!
//! Only the document's own pages: parent pages, the panel's options and its page commands stay in
//! DesignCraft's Pages panel in the full editor. Adding and deleting pages are on the ribbon's Page
//! tab. The thumbnails come from the Pages panel's cache (`page_thumbnail`, from
//! fork/patches/0004-ui-page-thumbnails.patch), so a page is not drawn twice.

use designcraft_doc::Document;
use designcraft_ui_egui::DesignApp;
use designcraft_ui_egui::theme::Tokens;
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

/// The tallest a page is drawn, in points.
pub const PAGE_H: f32 = 110.0;
/// Space above each spread, and under it for its number.
const GAP: f32 = 10.0;
const LABEL_H: f32 = 16.0;

/// One spread as the strip draws it: each page's absolute index and rect, relative to the top left of
/// a row `width` wide, and the height of the pages.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub pages: Vec<(usize, Rect)>,
    pub height: f32,
}

/// Every spread of `doc` laid out for a strip `width` wide: scaled to fit across and to at most
/// [`PAGE_H`] high, centred, pages where they sit in the spread.
pub fn rows(doc: &Document, width: f32) -> Vec<Row> {
    let mut first = 0;
    doc.spreads
        .iter()
        .map(|sp| {
            let left = sp.pages.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
            let right = sp.pages.iter().map(|p| p.x + p.width).fold(f64::NEG_INFINITY, f64::max);
            let tall = sp.pages.iter().map(|p| p.height).fold(0.0, f64::max);
            let (span, tall) = ((right - left).max(1.0) as f32, tall.max(1.0) as f32);
            let scale = (PAGE_H / tall).min(width / span);
            let x0 = (width - span * scale) / 2.0;
            let pages = sp
                .pages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let min = pos2(x0 + (p.x - left) as f32 * scale, 0.0);
                    (first + i, Rect::from_min_size(min, vec2(p.width as f32 * scale, p.height as f32 * scale)))
                })
                .collect();
            first += sp.pages.len();
            Row { pages, height: tall * scale }
        })
        .collect()
}

/// A spread's number: its page's label, or its first and last pages' ("2–3").
fn label(app: &DesignApp, row: &Row) -> String {
    match (row.pages.first(), row.pages.last()) {
        (Some((a, _)), Some((b, _))) if a != b => format!("{}–{}", app.session.page_label(*a), app.session.page_label(*b)),
        (Some((a, _)), _) => app.session.page_label(*a),
        _ => String::new(),
    }
}

/// Draws the strip into `ui` and says which page was clicked, if one was. `current` is the page in
/// view; its spread is outlined and scrolled to when it changes.
pub fn show(app: &mut DesignApp, ui: &mut egui::Ui, current: Option<usize>) -> Option<usize> {
    let st = app.session.active()?;
    let (uid, doc) = (st.uid, st.doc.clone());
    let t = Tokens::get(ui.ctx());
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt("foliolum_page_strip").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width() - 8.0;
        let followed = ui.scope_id().with("followed");
        for row in rows(&doc, width) {
            let (area, _) = ui.allocate_exact_size(vec2(ui.available_width(), GAP + row.height + LABEL_H), Sense::hover());
            let origin = area.min + vec2(4.0, GAP);
            let here = current.is_some_and(|c| row.pages.iter().any(|(abs, _)| *abs == c));
            if here && ui.data(|d| d.get_temp::<(u64, Option<usize>)>(followed)) != Some((uid, current)) {
                ui.scroll_to_rect(area, None);
                ui.data_mut(|d| d.insert_temp(followed, (uid, current)));
            }
            for &(abs, r) in &row.pages {
                let r = r.translate(origin.to_vec2());
                let resp = ui.interact(r, ui.scope_id().with(("page", abs)), Sense::click());
                let name = format!("Page {}", app.session.page_label(abs));
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current == Some(abs), &name));
                let tex = ui.is_rect_visible(r).then(|| designcraft_ui_egui::panels::pages::page_thumbnail(app, ui.ctx(), abs, r.height())).flatten();
                match tex {
                    Some(tex) => {
                        ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        ui.painter().rect_filled(r, 0.0, Color32::WHITE);
                    }
                }
                let (w, c) = if current == Some(abs) {
                    (2.5, t.accent)
                } else if resp.hovered() {
                    (1.5, t.text_dim)
                } else {
                    (0.5, Color32::from_gray(0x4a))
                };
                ui.painter().rect_stroke(r, 0.0, Stroke::new(w, c), StrokeKind::Outside);
                if resp.on_hover_text(&name).clicked() {
                    clicked = Some(abs);
                }
            }
            let text = label(app, &row);
            let ink = if here { t.text_strong } else { t.text_dim };
            let at = pos2(area.center().x, origin.y + row.height + LABEL_H / 2.0 + 1.0);
            ui.painter().text(at, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.5), ink);
        }
        ui.add_space(GAP);
    });
    clicked
}

/// Goes to page `abs`, leaving parent editing first if the full editor left it on.
pub fn go_to(app: &mut DesignApp, abs: usize) {
    if app.session.active().is_some_and(|st| st.editing_parents) {
        let _ = app.run("layout.parents.edit", serde_json::json!({"on": false}));
        if let Some(v) = app.view_mut() {
            v.fitted = false;
        }
    }
    designcraft_ui_egui::canvas::go_to_page(app, abs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Five facing pages (1, 2–3, 4–5) and two parents.
    fn app() -> DesignApp {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"pages": 5, "facingPages": true})).unwrap();
        app.run("layout.parents.new", json!({"prefix": "B"})).unwrap();
        app
    }

    #[test]
    fn the_strip_holds_the_documents_pages_and_no_parents() {
        let app = app();
        let doc = &app.session.active().unwrap().doc;
        assert!(doc.parents.len() >= 2, "the check would pass on a document without parents");
        let rows = rows(doc, 150.0);
        assert_eq!(rows.iter().map(|r| r.pages.len()).collect::<Vec<_>>(), [1, 2, 2]);
        let order: Vec<usize> = rows.iter().flat_map(|r| r.pages.iter().map(|(abs, _)| *abs)).collect();
        assert_eq!(order, (0..doc.page_count()).collect::<Vec<_>>());
        for row in &rows {
            for (abs, r) in &row.pages {
                assert!(r.min.x >= -0.01 && r.max.x <= 150.01 && r.height() <= PAGE_H + 0.01, "page {abs} at {r:?}");
            }
        }
        // A spread's pages meet at the spine, left page first.
        let (a, b) = (rows[1].pages[0].1, rows[1].pages[1].1);
        assert!((a.max.x - b.min.x).abs() < 0.01 && a.min.x < b.min.x);
        assert_eq!(label(&app, &rows[1]), "2–3");
        assert_eq!(label(&app, &rows[0]), "1");
    }

    #[test]
    fn a_wide_page_is_scaled_to_fit_across() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"pages": 1, "facingPages": false, "width": 2000, "height": 300})).unwrap();
        let rows = rows(&app.session.active().unwrap().doc, 150.0);
        let r = rows[0].pages[0].1;
        assert!((r.width() - 150.0).abs() < 0.01 && r.height() < PAGE_H, "{r:?}");
    }

    /// Clicks the middle of page `abs` as the strip lays it out and says what the strip reports.
    fn click(app: &mut DesignApp, abs: usize) -> Option<usize> {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(170.0, 2000.0));
        let (mut start, mut width, mut got) = (pos2(0.0, 0.0), 0.0, None);
        let events = [
            vec![],
            vec![egui::Event::PointerMoved(pos2(0.0, 0.0))],
            vec![egui::Event::PointerButton {
                pos: pos2(0.0, 0.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            vec![egui::Event::PointerButton {
                pos: pos2(0.0, 0.0),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        ];
        for (frame, mut ev) in events.into_iter().enumerate() {
            if frame > 0 {
                // Where the page is, from the first frame's layout.
                let mut y = start.y;
                let mut at = None;
                for row in rows(&app.session.active().unwrap().doc, width) {
                    for (a, r) in &row.pages {
                        if *a == abs {
                            at = Some(r.center() + vec2(start.x + 4.0, y + GAP));
                        }
                    }
                    y += GAP + row.height + LABEL_H;
                }
                let at = at.expect("no such page");
                for e in &mut ev {
                    match e {
                        egui::Event::PointerMoved(p) | egui::Event::PointerButton { pos: p, .. } => *p = at,
                        _ => {}
                    }
                }
            }
            let input = egui::RawInput { screen_rect: Some(screen), events: ev, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                start = ui.next_widget_position();
                width = ui.available_width() - 8.0;
                if let Some(a) = show(app, ui, Some(0)) {
                    got = Some(a);
                }
            });
            out.textures_delta.clear();
        }
        got
    }

    #[test]
    fn a_click_on_a_page_picks_that_page() {
        let mut app = app();
        assert_eq!(click(&mut app, 3), Some(3));
        assert_eq!(click(&mut app, 0), Some(0));
    }

    #[test]
    fn going_to_a_page_leaves_parent_editing() {
        let mut app = app();
        app.run("layout.parents.edit", json!({"on": true})).unwrap();
        assert!(app.session.active().unwrap().editing_parents);
        go_to(&mut app, 2);
        assert!(!app.session.active().unwrap().editing_parents);
    }
}
