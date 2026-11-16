//! Fill and outline for a finger. A two-line button floats over the
//! left edge of the drawing: its upper swatch is the fill and its lower
//! one the outline of the selection (or of new objects). A tap opens a
//! small card over the drawing with one button each for "No fill", "No
//! outline", both removed, and the Edit Fill and Outline Pen dialogs;
//! every button acts on the selection through the same paths as the
//! palette and the status bar, so it works under every tool, and the
//! card closes after an action. Only one card is open at a time, and it
//! keeps inside the drawing. Offered where the keys on the drawing are
//! (`touch_modifiers::offered`). See docs/behavior/android.md.

use crate::app::App;
use crate::i18n::tr;
use egui::{Color32, Rect, Sense, Vec2};
use tracedraw_core::Fill;

/// The button's side and the card's button size.
const SIDE: f32 = 44.0;
const CELL: Vec2 = egui::vec2(92.0, 48.0);

/// The fill's and the outline's swatch colours (`None` for nothing).
fn swatches(app: &App) -> (Option<Color32>, Option<Color32>) {
    let (fill, stroke) = match app.first_paint_target() {
        Some(s) => (s.fill, s.stroke),
        None => (app.default_fill(), app.default_stroke()),
    };
    (
        crate::app::fill_preview_color(&fill),
        stroke.map(|s| crate::canvas::to_color32(s.color)),
    )
}

/// Remove the fill, the outline or both from the selection.
pub fn remove(app: &mut App, fill: bool, outline: bool) {
    if fill {
        app.apply_fill(Fill::None);
    }
    if outline {
        app.apply_outline_color(None);
    }
}

fn card_button(ui: &mut egui::Ui, label: String, tip: String) -> bool {
    let b = egui::Button::new(label).min_size(CELL);
    ui.add(b).on_hover_text(tip).clicked()
}

/// The colour grid of the first shown palette: a tap sets the fill, or
/// the outline while the Outline switch is on. The card stays open.
fn colours(app: &mut App, ui: &mut egui::Ui) {
    let outline = app.touch_paint_outline;
    ui.horizontal(|ui| {
        if ui.selectable_label(!outline, tr("touch_paint.target_fill")).clicked() {
            app.touch_paint_outline = false;
        }
        if ui.selectable_label(outline, tr("touch_paint.target_outline")).clicked() {
            app.touch_paint_outline = true;
        }
    });
    let colors: Vec<tracedraw_core::Color> = app
        .visible_palettes
        .first()
        .and_then(|i| app.palettes.get(*i))
        .map(|p| p.colors.iter().map(|(_, c)| *c).collect())
        .unwrap_or_default();
    let mut picked = None;
    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for c in colors.iter().take(240) {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
                ui.painter().rect_filled(r, 3.0, crate::canvas::to_color32(*c));
                ui.painter().rect_stroke(
                    r,
                    3.0,
                    egui::Stroke::new(1.0, Color32::from_gray(0x80)),
                    egui::StrokeKind::Inside,
                );
                if resp.clicked() {
                    picked = Some(*c);
                }
            }
        });
    });
    if let Some(c) = picked {
        if outline {
            app.apply_outline_color(Some(c));
        } else {
            app.apply_fill(Fill::Solid(c));
        }
    }
    ui.add_space(4.0);
}

/// The two-line button and, open, its card.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: Rect) {
    if !crate::touch_modifiers::offered(ui.ctx()) || canvas.height() < 120.0 {
        return;
    }
    let ctx = ui.ctx().clone();
    let at = egui::pos2(canvas.left() + 8.0, canvas.top() + 8.0 + 0.35 * (canvas.height() - 16.0));
    let (fill, outline) = swatches(app);
    let open = app.touch_paint_open;
    let mut button = Rect::NOTHING;
    egui::Area::new(egui::Id::new("touch_paint_button"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::LEFT_TOP)
        .fixed_pos(at)
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(crate::theme::Tokens::panel())
                .stroke(egui::Stroke::new(1.0, crate::theme::Tokens::control_border()))
                .corner_radius(6.0)
                .inner_margin(3.0)
                .show(ui, |ui| {
                    let (r, resp) = ui.allocate_exact_size(Vec2::splat(SIDE), Sense::click());
                    button = r;
                    let painter = ui.painter();
                    if open || resp.hovered() {
                        painter.rect_filled(r, 4.0, crate::theme::Tokens::tool_active());
                    }
                    // Upper line: the fill. Lower line: the outline.
                    let half = Vec2::new(SIDE - 12.0, (SIDE - 14.0) / 2.0);
                    let up = Rect::from_min_size(r.min + Vec2::new(6.0, 4.0), half);
                    let down = Rect::from_min_size(up.left_bottom() + Vec2::new(0.0, 6.0), half);
                    crate::ui::status::draw_swatch(painter, up, fill);
                    crate::ui::status::draw_swatch(painter, down, outline);
                    if resp.on_hover_text(tr("touch_paint.button_tip")).clicked() {
                        app.touch_paint_open = !open;
                    }
                });
        });
    if !app.touch_paint_open {
        return;
    }
    let mut done = false;
    egui::Area::new(egui::Id::new("touch_paint_card"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::LEFT_TOP)
        .fixed_pos(egui::pos2(button.right() + 10.0, button.top()))
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(crate::theme::Tokens::panel())
                .stroke(egui::Stroke::new(1.0, crate::theme::Tokens::control_border()))
                .corner_radius(14.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong(tr("touch_paint.title"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("x").clicked() {
                                done = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if card_button(ui, tr("touch_paint.no_fill"), tr("touch_paint.no_fill_tip")) {
                            remove(app, true, false);
                            done = true;
                        }
                        if card_button(ui, tr("touch_paint.no_outline"), tr("touch_paint.no_outline_tip")) {
                            remove(app, false, true);
                            done = true;
                        }
                        if card_button(ui, tr("touch_paint.none"), tr("touch_paint.none_tip")) {
                            remove(app, true, true);
                            done = true;
                        }
                    });
                    colours(app, ui);
                    ui.horizontal(|ui| {
                        if card_button(ui, tr("touch_paint.edit_fill"), tr("touch_paint.edit_fill_tip")) {
                            app.open_edit_fill();
                            done = true;
                        }
                        if card_button(ui, tr("touch_paint.edit_outline"), tr("touch_paint.edit_outline_tip")) {
                            app.open_outline_pen();
                            done = true;
                        }
                    });
                });
        });
    if done {
        app.touch_paint_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With nothing selected the buttons change the defaults for new
    /// objects and never panic.
    #[test]
    fn removing_with_nothing_selected_is_safe() {
        let mut app = App::headless();
        app.select(Vec::new());
        remove(&mut app, true, true);
        remove(&mut app, true, false);
        remove(&mut app, false, true);
    }
}
