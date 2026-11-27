//! Foliolum's own ribbon icons, for buttons DesignCraft has no icon for.
//!
//! Drawn in code the way DesignCraft draws its set (`designcraft_ui_egui::icons`): single-colour
//! strokes 1.3 units wide on a 20×20 grid, scaled to the target rect. Our names start with `fo-` so they
//! can never shadow one of DesignCraft's.
//!
//! Clean room: these drawings are ours. Nothing is traced from Microsoft Publisher, Office or ArtCraft
//! artwork.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

/// Every icon [`paint`] draws.
pub const NAMES: &[&str] = &[
    "fo-new",
    "fo-open",
    "fo-save-copy",
    "fo-print",
    "fo-picture",
    "fo-booklet",
    "fo-sign",
    "fo-smart",
    "fo-undo",
    "fo-redo",
    "fo-copy",
    "fo-paste",
    "fo-spelling",
];

struct Pen<'a> {
    p: &'a Painter,
    r: Rect,
    c: Color32,
    /// Mirror left to right (redo is undo turned round).
    flip: bool,
}

impl Pen<'_> {
    const WIDTH: f32 = 1.3;

    fn s(&self) -> f32 {
        self.r.width().min(self.r.height()) / 20.0
    }
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        let s = self.s();
        let o = self.r.center() - vec2(10.0 * s, 10.0 * s);
        let x = if self.flip { 20.0 - x } else { x };
        pos2(o.x + x * s, o.y + y * s)
    }
    fn stroke(&self) -> Stroke {
        Stroke::new(Self::WIDTH * self.s(), self.c)
    }
    fn line(&self, pts: &[(f32, f32)]) {
        let v = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::line(v, self.stroke()));
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        let v = pts.iter().map(|&(x, y)| self.pt(x, y)).collect();
        self.p.add(Shape::closed_line(v, self.stroke()));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.closed(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.pt(x, y), r * self.s(), self.stroke());
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.pt(x, y), r * self.s(), self.c);
    }
    /// An arc around (cx, cy) from angle `a0` to `a1` in degrees, clockwise on screen (y down).
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        let n = 24;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.line(&pts);
    }
    fn text(&self, x: f32, y: f32, t: &str, size: f32) {
        self.p.text(self.pt(x, y), egui::Align2::CENTER_CENTER, t, designcraft_ui_egui::theme::semibold(size * self.s()), self.c);
    }
}

/// Paints `name` into `r` in colour `c` and says whether it is one of ours.
pub fn paint(p: &Painter, r: Rect, name: &str, c: Color32) -> bool {
    let mut pen = Pen { p, r, c, flip: false };
    match name {
        // A sheet with its corner turned down, and a plus.
        "fo-new" => {
            pen.closed(&[(3.0, 4.0), (9.5, 4.0), (13.0, 7.5), (13.0, 18.0), (3.0, 18.0)]);
            pen.line(&[(9.5, 4.0), (9.5, 7.5), (13.0, 7.5)]);
            pen.line(&[(16.5, 1.5), (16.5, 7.5)]);
            pen.line(&[(13.5, 4.5), (19.5, 4.5)]);
        }
        // A folder with its front tipped open.
        "fo-open" => {
            pen.line(&[(15.5, 9.0), (15.5, 7.0), (9.5, 7.0), (8.0, 5.0), (3.0, 5.0), (3.0, 16.0)]);
            pen.closed(&[(3.0, 16.0), (5.5, 9.0), (18.5, 9.0), (16.0, 16.0)]);
        }
        // Save's tray and arrow, with a plus for the second copy.
        "fo-save-copy" => {
            pen.line(&[(2.5, 11.0), (2.5, 17.0), (15.0, 17.0), (15.0, 11.0)]);
            pen.line(&[(8.75, 4.0), (8.75, 13.0)]);
            pen.line(&[(5.5, 9.75), (8.75, 13.0), (12.0, 9.75)]);
            pen.line(&[(16.5, 1.5), (16.5, 7.5)]);
            pen.line(&[(13.5, 4.5), (19.5, 4.5)]);
        }
        // A printer with a printed sheet coming out of it.
        "fo-print" => {
            pen.line(&[(6.0, 8.0), (6.0, 3.0), (14.0, 3.0), (14.0, 8.0)]);
            pen.line(&[(6.0, 14.0), (3.0, 14.0), (3.0, 8.0), (17.0, 8.0), (17.0, 14.0), (14.0, 14.0)]);
            pen.rect(6.0, 11.0, 14.0, 18.0);
            pen.line(&[(8.0, 13.75), (12.0, 13.75)]);
            pen.line(&[(8.0, 15.75), (12.0, 15.75)]);
            pen.dot(15.0, 10.25, 0.8);
        }
        // A framed landscape: hills and a sun.
        "fo-picture" => {
            pen.rect(3.0, 4.0, 17.0, 16.0);
            pen.line(&[(3.0, 14.5), (7.5, 9.5), (11.0, 13.0), (13.0, 11.0), (17.0, 15.0)]);
            pen.circle(13.0, 7.5, 1.5);
        }
        // An open booklet, two leaves meeting at the fold.
        "fo-booklet" => {
            pen.closed(&[(10.0, 6.0), (2.5, 4.0), (2.5, 15.0), (10.0, 17.0)]);
            pen.closed(&[(10.0, 6.0), (17.5, 4.0), (17.5, 15.0), (10.0, 17.0)]);
            pen.line(&[(4.5, 8.0), (8.0, 9.0)]);
            pen.line(&[(4.5, 11.0), (8.0, 12.0)]);
            pen.line(&[(15.5, 8.0), (12.0, 9.0)]);
            pen.line(&[(15.5, 11.0), (12.0, 12.0)]);
        }
        // A sheet with a signature line, and a pen signing on it.
        "fo-sign" => {
            pen.line(&[(11.0, 18.0), (3.0, 18.0), (3.0, 3.0), (13.0, 3.0), (13.0, 7.0)]);
            pen.line(&[(5.0, 15.0), (10.0, 15.0)]);
            pen.closed(&[(18.5, 6.5), (16.5, 4.5), (9.5, 11.5), (9.0, 14.0), (11.5, 13.5)]);
        }
        // A text box holding a small table.
        "fo-smart" => {
            pen.rect(2.5, 3.0, 17.5, 17.0);
            pen.line(&[(5.0, 7.0), (15.0, 7.0)]);
            pen.line(&[(5.0, 10.5), (15.0, 10.5)]);
            pen.line(&[(5.0, 14.0), (15.0, 14.0)]);
            pen.line(&[(9.0, 7.0), (9.0, 14.0)]);
        }
        // A path that turns back on itself, ending in an arrowhead pointing left.
        "fo-undo" | "fo-redo" => {
            pen.flip = name == "fo-redo";
            pen.line(&[(11.0, 6.0), (4.5, 6.0)]);
            pen.line(&[(7.5, 3.0), (4.5, 6.0), (7.5, 9.0)]);
            pen.arc(11.0, 11.25, 5.25, -90.0, 90.0);
            pen.line(&[(11.0, 16.5), (6.5, 16.5)]);
        }
        // Two sheets, one over the other.
        "fo-copy" => {
            pen.line(&[(7.0, 13.0), (4.0, 13.0), (4.0, 3.0), (13.0, 3.0), (13.0, 7.0)]);
            pen.rect(7.0, 7.0, 16.0, 17.0);
        }
        // A clipboard holding a sheet.
        "fo-paste" => {
            pen.line(&[(7.0, 4.0), (4.0, 4.0), (4.0, 18.0), (16.0, 18.0), (16.0, 4.0), (13.0, 4.0)]);
            pen.rect(7.0, 2.5, 13.0, 5.5);
            pen.line(&[(7.0, 10.0), (13.0, 10.0)]);
            pen.line(&[(7.0, 13.0), (13.0, 13.0)]);
        }
        // Letters with a tick under them.
        "fo-spelling" => {
            pen.text(10.0, 6.0, "abc", 8.5);
            pen.line(&[(5.5, 13.5), (8.5, 16.5), (14.5, 10.5)]);
        }
        _ => return false,
    }
    true
}

/// What painting does, as text: the shapes `paint` adds to a 20×20 rect, in order.
#[cfg(test)]
pub(crate) fn drawn(paint: impl Fn(&Painter, Rect)) -> String {
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    let body = fonts.families[&egui::FontFamily::Proportional].clone();
    fonts.families.insert(egui::FontFamily::Name("semibold".into()), body);
    ctx.set_fonts(fonts);
    let r = Rect::from_min_size(Pos2::ZERO, vec2(20.0, 20.0));
    // Fonts set before a pass take effect from the next one.
    let mut out = String::new();
    for _ in 0..2 {
        let mut o = ctx.run_ui(Default::default(), |ui| paint(ui.painter(), r));
        o.textures_delta.clear();
        out = format!("{:?}", o.shapes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_something_of_its_own() {
        drawn(|p, r| assert!(!paint(p, r, "fo-no-such-icon", Color32::WHITE)));
        let empty = drawn(|_, _| {});
        let mut seen = Vec::new();
        for name in NAMES {
            let d = drawn(|p, r| assert!(paint(p, r, name, Color32::WHITE), "{name} is listed but not drawn"));
            assert_ne!(d, empty, "{name} draws nothing");
            assert!(!seen.contains(&d), "{name} looks the same as another icon");
            seen.push(d);
        }
    }

    /// Ours are checked first, so one named like one of DesignCraft's would hide it.
    #[test]
    fn no_icon_of_ours_shadows_one_of_designcrafts() {
        let unknown = drawn(|p, r| designcraft_ui_egui::icons::paint(p, r, "fo-no-such-icon", Color32::WHITE));
        assert_ne!(unknown, drawn(|p, r| designcraft_ui_egui::icons::paint(p, r, "trash", Color32::WHITE)), "the check would pass on nothing");
        for name in NAMES {
            assert!(name.starts_with("fo-"), "{name}: ours start with fo-");
            assert_eq!(drawn(|p, r| designcraft_ui_egui::icons::paint(p, r, name, Color32::WHITE)), unknown, "DesignCraft has its own {name}");
        }
    }
}
