//! Panels that fold away for a finger. On a touch screen the toolbox,
//! the property bar, the standard toolbar, the palette, the dockers and
//! the status bar take most of a phone's screen, and each was hidden only
//! through a menu. A small handle floats over the right edge of the
//! drawing: a tap unfolds a strip of buttons, one per panel, each showing
//! or hiding its panel at once, and "Full" hides every panel (and puts
//! back the ones that were shown) so the drawing has the whole screen.
//! The handle can be dragged up and down the edge out of the way. It is
//! offered where the keys on the drawing are (`touch_modifiers::offered`).
//! See docs/behavior/android.md.

use crate::app::App;
use crate::i18n::tr;
use crate::ui::bars::Bar;

/// The bars the strip shows and hides, in the order of its buttons, with
/// the key of each button's label and its tip.
pub const BARS: [(Bar, &str, &str); 5] = [
    (Bar::Toolbox, "touch_panels.toolbox", "touch_panels.toolbox_tip"),
    (Bar::Property, "touch_panels.property", "touch_panels.property_tip"),
    (Bar::Standard, "touch_panels.standard", "touch_panels.standard_tip"),
    (Bar::Palette, "touch_panels.palette", "touch_panels.palette_tip"),
    (Bar::Status, "touch_panels.status", "touch_panels.status_tip"),
];

/// What was shown before "Full" hid it all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stash {
    bars: [bool; 5],
    extra: [bool; 3],
    dockers: bool,
    rulers: bool,
}

/// The strip's state: unfolded or not, where its handle sits on the
/// edge (0 at the top of the drawing, 1 at the bottom) and what "Full"
/// put away.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPanels {
    pub open: bool,
    pub y: f32,
    pub stash: Option<Stash>,
}

impl Default for TouchPanels {
    fn default() -> Self {
        TouchPanels {
            open: false,
            y: 0.35,
            stash: None,
        }
    }
}

/// Whether no panel is shown at all.
pub fn all_hidden(app: &App) -> bool {
    !app.show_dockers
        && !app.show_rulers
        && !app.show_text_toolbar
        && !app.show_zoom_toolbar
        && !app.show_transform_toolbar
        && BARS.iter().all(|(b, _, _)| !b.visible(app))
}

/// Whether "Full" has the drawing alone on the screen: no panel, no
/// menu bar, no tabs and no scrollbars.
pub fn immersive(app: &App) -> bool {
    app.touch_panels.stash.is_some() && all_hidden(app) && app.has_document() && !app.show_welcome
}

/// Hide every panel, keeping what was shown for [`restore`].
pub fn hide_all(app: &mut App) {
    let mut bars = [false; 5];
    for (slot, (bar, _, _)) in bars.iter_mut().zip(BARS.iter()) {
        *slot = bar.visible(app);
        bar.set_visible(app, false);
    }
    let extra = [
        app.show_text_toolbar,
        app.show_zoom_toolbar,
        app.show_transform_toolbar,
    ];
    app.show_text_toolbar = false;
    app.show_zoom_toolbar = false;
    app.show_transform_toolbar = false;
    app.touch_panels.stash = Some(Stash {
        bars,
        extra,
        dockers: app.show_dockers,
        rulers: app.show_rulers,
    });
    app.show_dockers = false;
    app.show_rulers = false;
}

/// Show the panels that [`hide_all`] hid; with none kept (every panel
/// was hidden one by one), the toolbox, the bars and the palette.
pub fn restore(app: &mut App) {
    match app.touch_panels.stash.take() {
        Some(s) => {
            for (on, (bar, _, _)) in s.bars.iter().zip(BARS.iter()) {
                bar.set_visible(app, *on);
            }
            app.show_text_toolbar = s.extra[0];
            app.show_zoom_toolbar = s.extra[1];
            app.show_transform_toolbar = s.extra[2];
            app.show_dockers = s.dockers;
            app.show_rulers = s.rulers;
        }
        None => {
            for (bar, _, _) in BARS.iter().filter(|(b, _, _)| *b != Bar::Status) {
                bar.set_visible(app, true);
            }
        }
    }
}

/// The "Full" button: every panel away, or back.
pub fn toggle_full(app: &mut App) {
    if all_hidden(app) {
        restore(app);
    } else {
        hide_all(app);
    }
}


/// The strip's icons, drawn as simple line glyphs (no font needed).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Glyph {
    Tools,
    Props,
    Bar,
    Colors,
    Status,
    Docker,
    Full,
}

fn glyph_of(bar: Bar) -> Glyph {
    match bar {
        Bar::Toolbox => Glyph::Tools,
        Bar::Property => Glyph::Props,
        Bar::Standard => Glyph::Bar,
        Bar::Palette => Glyph::Colors,
        _ => Glyph::Status,
    }
}

fn paint_glyph(p: &egui::Painter, r: egui::Rect, g: Glyph, col: egui::Color32) {
    let st = egui::Stroke::new(1.6, col);
    let c = r.center();
    let u = r.width() / 24.0;
    let pt = |x: f32, y: f32| egui::pos2(c.x + x * u, c.y + y * u);
    match g {
        Glyph::Tools => {
            // A selection arrow.
            p.add(egui::Shape::convex_polygon(
                vec![pt(-6.0, -9.0), pt(-6.0, 7.0), pt(-2.0, 3.0), pt(1.0, 9.0), pt(4.0, 7.5), pt(1.0, 2.0), pt(7.0, 2.0)],
                col,
                egui::Stroke::NONE,
            ));
        }
        Glyph::Props => {
            for (y, k) in [(-6.0, -3.0), (0.0, 4.0), (6.0, -1.0)] {
                p.line_segment([pt(-8.0, y), pt(8.0, y)], st);
                p.circle_filled(pt(k, y), 2.6 * u, col);
            }
        }
        Glyph::Bar => {
            for (x, y) in [(-8.0, -8.0), (1.0, -8.0), (-8.0, 1.0), (1.0, 1.0)] {
                p.rect_stroke(
                    egui::Rect::from_min_size(pt(x, y), egui::vec2(7.0 * u, 7.0 * u)),
                    1.5,
                    st,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Glyph::Colors => {
            p.circle_stroke(pt(0.0, 0.0), 9.0 * u, st);
            for (x, y) in [(-3.5, -3.0), (3.0, -3.5), (-2.0, 4.0)] {
                p.circle_filled(pt(x, y), 2.2 * u, col);
            }
        }
        Glyph::Status => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(20.0 * u, 8.0 * u)),
                2.0,
                st,
                egui::StrokeKind::Inside,
            );
            p.circle_filled(pt(-5.0, 0.0), 1.8 * u, col);
            p.line_segment([pt(-1.0, 0.0), pt(6.0, 0.0)], st);
        }
        Glyph::Docker => {
            for y in [-6.0, 0.0, 6.0] {
                p.rect_stroke(
                    egui::Rect::from_center_size(pt(0.0, y), egui::vec2(16.0 * u, 4.5 * u)),
                    1.5,
                    st,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Glyph::Full => {
            // Four corner brackets: the drawing alone.
            for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = pt(sx * 8.0, sy * 8.0);
                p.line_segment([corner, pt(sx * 8.0, sy * 3.0)], st);
                p.line_segment([corner, pt(sx * 3.0, sy * 8.0)], st);
            }
        }
    }
}

/// A square icon button; true when tapped.
fn icon_button(ui: &mut egui::Ui, g: Glyph, on: bool, tip: String) -> bool {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
    let p = ui.painter();
    if on {
        p.rect_filled(r, 6.0, crate::theme::Tokens::tool_active());
    } else if resp.hovered() {
        p.rect_filled(r, 6.0, crate::theme::Tokens::tool_hover());
    }
    paint_glyph(p, r.shrink(8.0), g, crate::theme::Tokens::icon());
    resp.on_hover_text(tip).clicked()
}

/// The edge handle and, unfolded, its strip of buttons.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: egui::Rect) {
    // The selection handles grow for a finger (`App::handle_size`).
    app.touch_ui = crate::touch_modifiers::offered(ui.ctx());
    if !app.touch_ui || canvas.height() < 120.0 {
        return;
    }
    let ctx = ui.ctx().clone();
    let at = egui::pos2(
        canvas.right() - 26.0,
        canvas.top() + 8.0 + app.touch_panels.y.clamp(0.0, 1.0) * (canvas.height() - 16.0),
    );
    let hidden = all_hidden(app);
    egui::Area::new(egui::Id::new("touch_panels_on_drawing"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(at)
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(crate::theme::Tokens::panel())
                .stroke(egui::Stroke::new(1.0, crate::theme::Tokens::control_border()))
                .corner_radius(6.0)
                .inner_margin(3.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // The handle: a tap folds or unfolds the strip,
                        // a drag along the edge moves it.
                        let open = app.touch_panels.open;
                        let handle = egui::Button::new(if open { "»" } else { "«" })
                            .min_size(egui::vec2(44.0, 44.0))
                            .selected(open)
                            .sense(egui::Sense::click_and_drag());
                        let r = ui.add(handle).on_hover_text(tr("touch_panels.handle_tip"));
                        if r.dragged() {
                            let room = (canvas.height() - 16.0).max(1.0);
                            let y = app.touch_panels.y + r.drag_delta().y / room;
                            app.touch_panels.y = y.clamp(0.0, 1.0);
                        }
                        if r.clicked() {
                            app.touch_panels.open = !open;
                        }
                        if !app.touch_panels.open {
                            return;
                        }
                        for (bar, _, tip) in BARS {
                            let on = bar.visible(app);
                            if bar == Bar::Palette {
                                // Colours open as a card over the drawing.
                                let open = app.touch_paint_open;
                                if icon_button(ui, Glyph::Colors, open, tr(tip)) {
                                    app.touch_paint_open = !open;
                                    bar.set_visible(app, false);
                                    app.touch_panels.stash = None;
                                }
                                continue;
                            }
                            if icon_button(ui, glyph_of(bar), on, tr(tip)) {
                                bar.set_visible(app, !on);
                                // One panel by hand ends what "Full" kept.
                                app.touch_panels.stash = None;
                            }
                        }
                        let on = app.show_dockers;
                        if icon_button(ui, Glyph::Docker, on, tr("touch_panels.docker_tip")) {
                            app.show_dockers = !on;
                            app.touch_panels.stash = None;
                        }
                        if icon_button(ui, Glyph::Full, hidden, tr("touch_panels.full_tip")) {
                            toggle_full(app);
                        }
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Full" puts every panel away and the next tap puts back the ones
    /// that were shown, not the ones that were hidden before.
    #[test]
    fn full_hides_every_panel_and_brings_back_the_same_ones() {
        let mut app = App::headless();
        app.show_toolbox = true;
        app.show_property_bar = true;
        app.show_standard_toolbar = true;
        app.show_status_bar = false;
        app.show_dockers = true;
        app.show_rulers = false;
        assert!(!all_hidden(&app));
        toggle_full(&mut app);
        assert!(all_hidden(&app));
        assert!(!app.show_toolbox && !app.show_dockers && !app.show_property_bar);
        toggle_full(&mut app);
        assert!(app.show_toolbox && app.show_property_bar && app.show_standard_toolbar);
        assert!(app.show_dockers);
        assert!(!app.show_status_bar && !app.show_rulers);
        assert!(app.touch_panels.stash.is_none());
    }

    /// With everything hidden by hand, "Full" shows the working panels.
    /// Full is the drawing alone only with a drawing open and a panel
    /// stash; hiding by hand keeps the menu bar.
    #[test]
    fn only_full_takes_the_menu_bar_away() {
        let mut app = App::headless();
        app.show_welcome = false;
        assert!(!immersive(&app));
        hide_all(&mut app);
        assert_eq!(immersive(&app), app.has_document());
        restore(&mut app);
        assert!(!immersive(&app));
    }

    #[test]
    fn full_after_hiding_one_by_one_shows_the_working_panels() {
        let mut app = App::headless();
        for (bar, _, _) in BARS {
            bar.set_visible(&mut app, false);
        }
        app.show_dockers = false;
        app.show_rulers = false;
        app.touch_panels.stash = None;
        assert!(all_hidden(&app));
        toggle_full(&mut app);
        assert!(app.show_toolbox && app.show_property_bar && app.show_standard_toolbar);
        assert!(Bar::Palette.visible(&app));
        assert!(!app.show_status_bar && !app.show_dockers);
    }
}
