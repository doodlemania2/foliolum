//! Foliolum's simple editor: a ribbon of plain-worded buttons under the app bar and the pages down
//! the left, in place of DesignCraft's menus and Tools panel. A switch in the app bar goes to
//! DesignCraft's full editor and back.
//!
//! It draws through DesignCraft's embedder hooks (`designcraft_ui_egui::shell`, from
//! fork/patches/0003-ui-embedder-shell.patch). Every button runs a command, tool or panel DesignCraft
//! already has ([`tables`]), so this crate adds layout and words, not behaviour.
//!
//! Clean room: names, groupings and drawings are ours, not Microsoft Publisher's.

pub mod icons;
pub mod pages;
pub mod tables;

use std::sync::atomic::{AtomicBool, Ordering};

use designcraft_ui_egui::DesignApp;
use designcraft_ui_egui::shell::{Chrome, Shell};
use designcraft_ui_egui::theme::Tokens;
use egui::{Color32, Sense, Stroke, StrokeKind, vec2};
use tables::{Action, Button, TABS};

static SIMPLE: AtomicBool = AtomicBool::new(true);

/// What runs the ribbon's [`Action::App`] buttons: the app around the editor sets it at startup.
static APP_ACTIONS: std::sync::OnceLock<fn(&mut DesignApp, &str)> = std::sync::OnceLock::new();

/// Sets what runs the ribbon's [`Action::App`] buttons, once. Without it they are greyed out.
pub fn set_app_actions(run: fn(&mut DesignApp, &str)) -> bool {
    APP_ACTIONS.set(run).is_ok()
}

/// Whether the simple editor is showing (rather than DesignCraft's full one).
pub fn simple() -> bool {
    SIMPLE.load(Ordering::Relaxed)
}

/// Switches between the simple editor and the full one.
pub fn set_simple(on: bool) {
    SIMPLE.store(on, Ordering::Relaxed);
}

/// What Foliolum gives `designcraft_ui_egui::shell::set`.
pub const SHELL: Shell = Shell { chrome, top: Some(ribbon), left: Some(page_strip), app_bar: Some(mode_switch) };

/// The simple editor keeps the app bar, the Control bar (formatting for what's selected), the
/// status bar and the panels, and hides the menus, the workspace menu and the Tools panel.
fn chrome(_: &DesignApp) -> Chrome {
    if simple() { Chrome { menu_bar: false, workspace_menu: false, toolbar: false, control_bar: true } } else { Chrome::ALL }
}

const RIBBON_TAB_ID: &str = "foliolum_ribbon_tab";
const BUTTON: egui::Vec2 = vec2(76.0, 50.0);
const ICON: f32 = 20.0;
const PAGE_STRIP_WIDTH: f32 = 170.0;

fn ribbon(app: &mut DesignApp, ui: &mut egui::Ui) {
    if !simple() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let tab_id = egui::Id::unique(RIBBON_TAB_ID);
    let mut tab: usize = ui.data(|d| d.get_temp(tab_id)).unwrap_or(tables::START_TAB).min(TABS.len() - 1);
    egui::Panel::top("foliolum_ribbon")
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.divider)).inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, tb) in TABS.iter().enumerate() {
                    if ui.selectable_label(i == tab, egui::RichText::new(tb.name).strong()).clicked() {
                        tab = i;
                    }
                }
            });
            ui.add_space(2.0);
            egui::ScrollArea::horizontal().id_salt("foliolum_ribbon_scroll").show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (gi, g) in TABS[tab].groups.iter().enumerate() {
                        if gi > 0 {
                            ui.separator();
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;
                                for b in g.buttons {
                                    ribbon_button(app, ui, b, &t);
                                }
                            });
                            ui.label(egui::RichText::new(g.caption).small().color(t.text_dim));
                        });
                    }
                });
            });
        });
    ui.data_mut(|d| d.insert_temp(tab_id, tab));
}

fn enabled(app: &DesignApp, b: &Button) -> bool {
    match b.action {
        Action::Menu(id, _) => designcraft_ui_egui::menus::menu_enabled(app, id),
        Action::Tool(_) => true,
        Action::App(_) => APP_ACTIONS.get().is_some() && app.session.active().is_some(),
    }
}

fn selected(app: &DesignApp, b: &Button) -> bool {
    matches!(b.action, Action::Tool(id) if app.session.tool_id() == id)
}

fn run(app: &mut DesignApp, b: &Button) {
    match b.action {
        Action::Menu(id, params) => {
            let params = if params.is_empty() { serde_json::Value::Null } else { serde_json::from_str(params).unwrap_or(serde_json::Value::Null) };
            designcraft_ui_egui::menus::activate(app, id, &params);
        }
        Action::Tool(id) => app.select_tool(id),
        Action::App(id) => {
            if let Some(run) = APP_ACTIONS.get() {
                run(app, id);
            }
        }
    }
}

/// A button: icon (if it has one) over a label, wrapped to two lines.
fn ribbon_button(app: &mut DesignApp, ui: &mut egui::Ui, b: &Button, t: &Tokens) {
    let on = enabled(app, b);
    let (rect, resp) = ui.allocate_exact_size(BUTTON, if on { Sense::click() } else { Sense::hover() });
    let resp = resp.on_hover_text(b.label);
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        if selected(app, b) {
            p.rect(rect, t.radius, t.tool_active, Stroke::NONE, StrokeKind::Inside);
        } else if on && resp.hovered() {
            p.rect(rect, t.radius, t.hover, Stroke::NONE, StrokeKind::Inside);
        }
        let color = if on { t.text } else { t.text_disabled };
        let galley = p.layout(b.label.to_string(), egui::FontId::proportional(11.0), color, rect.width() - 6.0);
        // With an icon the label sits under it; without one it is centred in the button.
        let text_top = if let Some(icon) = b.icon {
            let r = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.top() + 4.0 + ICON / 2.0), vec2(ICON, ICON));
            let c = if on { t.icon } else { t.text_disabled };
            if !icons::paint(p, r, icon, c) {
                designcraft_ui_egui::icons::paint(p, r, icon, c);
            }
            r.bottom() + 2.0
        } else {
            rect.center().y - galley.size().y / 2.0
        };
        let pos = egui::pos2(rect.center().x - galley.size().x / 2.0, text_top);
        p.galley(pos, galley, Color32::PLACEHOLDER);
    }
    if resp.clicked() {
        run(app, b);
    }
}

/// The document's pages down the left edge ([`pages`]).
fn page_strip(app: &mut DesignApp, ui: &mut egui::Ui) {
    if !simple() || app.session.active().is_none() {
        return;
    }
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("foliolum_pages")
        .exact_size(PAGE_STRIP_WIDTH)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.0, t.divider)).inner_margin(egui::Margin::same(4)))
        .show(ui, |ui| {
            let current = designcraft_ui_egui::canvas::current_page(app);
            if let Some(abs) = pages::show(app, ui, current) {
                pages::go_to(app, abs);
            }
        });
}

/// The switch between the simple editor and the full one, in the app bar.
fn mode_switch(_: &mut DesignApp, ui: &mut egui::Ui) {
    let (label, tip) = if simple() {
        ("Full editor", "Show every menu, tool and panel")
    } else {
        ("Simple editor", "Go back to the ribbon and the pages on the left")
    };
    if ui.button(label).on_hover_text(tip).clicked() {
        set_simple(!simple());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use designcraft_ui_egui::menus::{Item, menu_tree};

    fn menu_ids() -> Vec<String> {
        fn walk(items: &[Item], out: &mut Vec<String>) {
            for it in items {
                match it {
                    Item::Sub(_, children) => walk(children, out),
                    Item::Cmd { id, .. } => out.push(id.clone()),
                    Item::Sep => {}
                }
            }
        }
        let mut out = Vec::new();
        for (_, items) in menu_tree() {
            walk(&items, &mut out);
        }
        out
    }

    /// The ribbon only runs what DesignCraft's menus offer. Catches an id renamed upstream at a
    /// pin bump.
    #[test]
    fn every_ribbon_command_is_in_designcrafts_menus() {
        let ids = menu_ids();
        assert!(ids.len() > 200, "walked only {} menu items: the check would pass on nothing", ids.len());
        assert!(!ids.iter().any(|i| i == "foliolum.noSuchCommand"));
        for b in tables::buttons() {
            if let Action::Menu(id, params) = b.action {
                assert!(ids.iter().any(|i| i == id), "{}: {id} is not in DesignCraft's menus", b.label);
                assert!(params.is_empty() || serde_json::from_str::<serde_json::Value>(params).is_ok(), "{}: bad params", b.label);
            }
        }
    }

    #[test]
    fn every_ribbon_tool_is_one_of_designcrafts() {
        for b in tables::buttons() {
            if let Action::Tool(id) = b.action {
                assert!(designcraft_tools::TOOL_GROUPS.iter().flat_map(|g| g.iter()).any(|t| t.id == id), "{}: no tool {id}", b.label);
            }
        }
    }

    #[test]
    fn simple_mode_hides_the_menus_and_tools_and_full_mode_shows_everything() {
        let app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
        set_simple(true);
        let c = chrome(&app);
        assert!(!c.menu_bar && !c.workspace_menu && !c.toolbar && c.control_bar);
        set_simple(false);
        assert_eq!(chrome(&app), Chrome::ALL);
        set_simple(true);
    }

    #[test]
    fn a_tool_button_picks_its_tool() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
        let b = tables::buttons().find(|b| b.action == Action::Tool("type")).unwrap();
        run(&mut app, b);
        assert_eq!(app.session.tool_id(), "type");
        assert!(selected(&app, b));
    }

    /// Every button has an icon, and it is one we or DesignCraft draw (DesignCraft draws a plain
    /// square for a name it doesn't know, so a misspelt or renamed one would show as that).
    #[test]
    fn every_button_has_an_icon_someone_draws() {
        let unknown = icons::drawn(|p, r| designcraft_ui_egui::icons::paint(p, r, "fo-no-such-icon", Color32::WHITE));
        let mut n = 0;
        for b in tables::buttons() {
            let icon = b.icon.unwrap_or_else(|| panic!("{} has no icon", b.label));
            let ours = icons::NAMES.contains(&icon);
            let theirs = icons::drawn(|p, r| designcraft_ui_egui::icons::paint(p, r, icon, Color32::WHITE)) != unknown;
            assert!(ours || theirs, "{}: nobody draws {icon}", b.label);
            n += 1;
        }
        assert!(n > 20, "only {n} buttons");
    }

    #[test]
    fn labels_are_short_enough_for_two_lines() {
        for b in tables::buttons() {
            assert!(b.label.chars().count() <= 22, "{} is too long for a ribbon button", b.label);
        }
    }
}
