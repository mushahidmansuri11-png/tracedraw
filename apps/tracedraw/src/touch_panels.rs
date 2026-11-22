//! Panels that fold away for a finger. On a touch screen the toolbox,
//! the property bar, the standard toolbar, the palette, the dockers and
//! the status bar take most of a phone's screen, and each was hidden only
//! through a menu. A small handle floats over the right edge of the
//! drawing: a tap unfolds a strip of buttons, one per panel, each showing
//! or hiding its panel at once, and "Full" hides every panel (and puts
//! back the ones that were shown) so the drawing has the whole screen. In
//! Full the menu bar, the tabs and the scrollbars are away too, and any
//! panel can still be called back by its button (the toolbox, say) with
//! the drawing staying full screen; the Full button ends it.
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
    /// Full is on: the menu bar, the tabs and the scrollbars are away,
    /// whichever panels were called back since.
    pub full: bool,
}

impl Default for TouchPanels {
    fn default() -> Self {
        TouchPanels {
            open: false,
            y: 0.35,
            stash: None,
            full: false,
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

/// Whether "Full" is on with a drawing open: no menu bar, no tabs and no
/// scrollbars, and the panels the strip's buttons call back are the only
/// ones shown.
pub fn immersive(app: &App) -> bool {
    app.touch_panels.full && app.has_document() && !app.show_welcome
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
    app.touch_panels.full = true;
}

/// Show the panels that [`hide_all`] hid; with none kept (every panel
/// was hidden one by one), the toolbox, the bars and the palette.
pub fn restore(app: &mut App) {
    app.touch_panels.full = false;
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
    if app.touch_panels.full || all_hidden(app) {
        restore(app);
    } else {
        hide_all(app);
    }
}


/// The edge handle and, unfolded, its strip of buttons.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: egui::Rect) {
    if !crate::touch_modifiers::offered(ui.ctx()) || canvas.height() < 120.0 {
        return;
    }
    let ctx = ui.ctx().clone();
    let side = egui::vec2(44.0, 44.0);
    let at = egui::pos2(
        canvas.right() - 26.0,
        canvas.top() + 8.0 + app.touch_panels.y.clamp(0.0, 1.0) * (canvas.height() - 16.0),
    );
    let hidden = app.touch_panels.full;
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
                        for (bar, label, tip) in BARS {
                            let on = bar.visible(app);
                            let b = egui::Button::new(tr(label)).min_size(side).selected(on);
                            if ui.add(b).on_hover_text(tr(tip)).clicked() {
                                // In Full the drawing stays full screen.
                                bar.set_visible(app, !on);
                            }
                        }
                        let on = app.show_dockers;
                        let b = egui::Button::new(tr("touch_panels.docker"))
                            .min_size(side)
                            .selected(on);
                        if ui
                            .add(b)
                            .on_hover_text(tr("touch_panels.docker_tip"))
                            .clicked()
                        {
                            app.show_dockers = !on;
                        }
                        let b = egui::Button::new(tr("touch_panels.full"))
                            .min_size(side)
                            .selected(hidden);
                        if ui
                            .add(b)
                            .on_hover_text(tr("touch_panels.full_tip"))
                            .clicked()
                        {
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
        assert!(!app.touch_panels.full);
    }

    /// A panel called back in Full leaves the menu bar, the tabs and the
    /// scrollbars away; Full again puts everything back.
    #[test]
    fn a_panel_called_back_in_full_keeps_the_screen_full() {
        let mut app = App::headless();
        app.show_welcome = false;
        app.show_toolbox = true;
        app.show_property_bar = true;
        toggle_full(&mut app);
        assert!(app.touch_panels.full);
        assert_eq!(immersive(&app), app.has_document());
        Bar::Toolbox.set_visible(&mut app, true);
        assert!(Bar::Toolbox.visible(&app));
        assert!(app.touch_panels.full, "the toolbox does not end Full");
        assert_eq!(immersive(&app), app.has_document());
        toggle_full(&mut app);
        assert!(!app.touch_panels.full && !immersive(&app));
        assert!(app.show_toolbox && app.show_property_bar);
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
