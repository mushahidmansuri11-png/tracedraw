//! The fill and outline button for a finger (touch screens). It floats
//! over the drawing: its upper swatch is the fill, its lower one the
//! outline. A tap opens a card with No fill, No outline, Both off, Edit
//! fill and Edit outline, plus Hex, RGB and CMYK rows; each row has a
//! Fill and an Outline button and applies its colour in the model it was
//! typed in (no conversion, so no repaint loop). A drag moves the button
//! over the drawing. Shown only on a touch screen, with the touch keys
//! option on (Options > Display).

use crate::app::App;
use crate::i18n::tr;
use crate::theme::Tokens;
use egui::{Color32, Rect, Sense, Stroke, TextEdit, Vec2};
use tracedraw_core::{Color, Fill};

/// The button's side, in points.
const SIDE: f32 = 40.0;
/// The gap from the drawing's edges, in points.
const MARGIN: f32 = 8.0;

/// The text typed in the three entry rows, kept in egui's memory so it
/// survives frames without touching the document.
#[derive(Clone, Default)]
struct Entry {
    hex: String,
    rgb: String,
    cmyk: String,
}

/// Show the button and, when open, its card.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: Rect) {
    if !app.has_document() || !shown(app, ui.ctx()) {
        return;
    }
    let ctx = ui.ctx().clone();

    let room = Vec2::new(
        (canvas.width() - SIDE - 2.0 * MARGIN).max(1.0),
        (canvas.height() - SIDE - 2.0 * MARGIN).max(1.0),
    );
    let [fx, fy] = app.touch_paint_at;
    let at = egui::pos2(
        canvas.left() + MARGIN + fx.clamp(0.0, 1.0) * room.x,
        canvas.top() + MARGIN + fy.clamp(0.0, 1.0) * room.y,
    );

    let mut button = Rect::NOTHING;
    egui::Area::new(egui::Id::new("touch_paint_button"))
        .order(egui::Order::Middle)
        .fixed_pos(at)
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(SIDE), Sense::click_and_drag());
            button = r;
            if resp.dragged() {
                let d = resp.drag_delta();
                app.touch_paint_at[0] = (fx + d.x / room.x).clamp(0.0, 1.0);
                app.touch_paint_at[1] = (fy + d.y / room.y).clamp(0.0, 1.0);
            } else if resp.clicked() {
                app.touch_paint_open = !app.touch_paint_open;
            }

            let p = ui.painter();
            let bg = if app.touch_paint_open {
                Tokens::tool_active()
            } else {
                Tokens::panel()
            };
            p.rect_filled(r, 4.0, bg);
            p.rect_stroke(
                r,
                4.0,
                Stroke::new(1.0, Tokens::control_border()),
                egui::StrokeKind::Inside,
            );
            let half = SIDE / 2.0 - 4.0;
            p.rect_filled(
                Rect::from_min_size(r.min + Vec2::splat(4.0), Vec2::new(SIDE - 8.0, half)),
                2.0,
                to32(app.current_colors.0.to_rgb8()),
            );
            p.rect_filled(
                Rect::from_min_size(
                    r.min + Vec2::new(4.0, 4.0 + half),
                    Vec2::new(SIDE - 8.0, half - 4.0),
                ),
                2.0,
                to32(app.current_colors.1.to_rgb8()),
            );
        });

    if app.touch_paint_open {
        card(app, &ctx, button, canvas);
    }
}

/// Shown on a touch screen (or Android), with the touch keys option on.
fn shown(app: &App, ctx: &egui::Context) -> bool {
    app.settings.touch_keys && crate::touch_modifiers::offered(ctx)
}

/// The card beside the button, on the side with more room.
fn card(app: &mut App, ctx: &egui::Context, button: Rect, canvas: Rect) {
    let (pivot, at) = if button.center().x > canvas.center().x {
        (egui::Align2::RIGHT_TOP, egui::pos2(button.left() - 8.0, button.top()))
    } else {
        (egui::Align2::LEFT_TOP, egui::pos2(button.right() + 8.0, button.top()))
    };
    let mut done = false;
    egui::Area::new(egui::Id::new("touch_paint_card"))
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(at)
        .constrain_to(canvas)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(Tokens::panel())
                .stroke(Stroke::new(1.0, Tokens::control_border()))
                .corner_radius(6.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        if ui.button(tr("touch_paint.no_fill")).clicked() {
                            app.apply_fill(Fill::None);
                            done = true;
                        }
                        if ui.button(tr("touch_paint.no_outline")).clicked() {
                            app.apply_outline_color(None);
                            done = true;
                        }
                        if ui.button(tr("touch_paint.both_off")).clicked() {
                            app.apply_fill(Fill::None);
                            app.apply_outline_color(None);
                            done = true;
                        }
                        if ui.button(tr("touch_paint.edit_fill")).clicked() {
                            app.open_edit_fill();
                            done = true;
                        }
                        if ui.button(tr("touch_paint.edit_outline")).clicked() {
                            app.open_outline_pen();
                            done = true;
                        }
                        ui.separator();
                        done |= entry_rows(app, ui);
                    });
                });
        });
    if done {
        app.touch_paint_open = false;
    }
}

/// The Hex, RGB and CMYK rows. Returns true when a colour was applied.
fn entry_rows(app: &mut App, ui: &mut egui::Ui) -> bool {
    let id = egui::Id::new("touch_paint_entry");
    let mut e: Entry = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut applied = false;
    {
        let rows: [(&str, &mut String, &str, fn(&str) -> Option<Color>); 3] = [
            ("Hex", &mut e.hex, "#FF8800", parse_hex),
            ("RGB", &mut e.rgb, "255,136,0", parse_rgb),
            ("CMYK", &mut e.cmyk, "0,47,100,0", parse_cmyk),
        ];
        for (label, text, hint, parse) in rows {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(TextEdit::singleline(text).hint_text(hint).desired_width(120.0));
                if ui.button("Fill").clicked() {
                    if let Some(c) = parse(text) {
                        app.apply_fill(Fill::Solid(c));
                        applied = true;
                    }
                }
                if ui.button("Outline").clicked() {
                    if let Some(c) = parse(text) {
                        app.apply_outline_color(Some(c));
                        applied = true;
                    }
                }
            });
        }
    }
    ui.data_mut(|d| d.insert_temp(id, e));
    applied
}

/// "#RRGGBB" or "#RGB" (the # is optional), as an RGB colour.
fn parse_hex(s: &str) -> Option<Color> {
    Color::from_hex(s)
}

/// "r,g,b" with 0 to 255 each, as an RGB colour.
fn parse_rgb(s: &str) -> Option<Color> {
    let v = parts(s, 3)?;
    if v.iter().any(|x| !(0.0..=255.0).contains(x)) {
        return None;
    }
    Some(Color::Rgb {
        r: (v[0] / 255.0) as f32,
        g: (v[1] / 255.0) as f32,
        b: (v[2] / 255.0) as f32,
    })
}

/// "c,m,y,k" with 0 to 100 each (percent of ink), as a CMYK colour.
fn parse_cmyk(s: &str) -> Option<Color> {
    let v = parts(s, 4)?;
    if v.iter().any(|x| !(0.0..=100.0).contains(x)) {
        return None;
    }
    Some(Color::Cmyk {
        c: (v[0] / 100.0) as f32,
        m: (v[1] / 100.0) as f32,
        y: (v[2] / 100.0) as f32,
        k: (v[3] / 100.0) as f32,
    })
}

/// Exactly `n` numbers separated by commas.
fn parts(s: &str, n: usize) -> Option<Vec<f64>> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| x.trim().parse::<f64>().ok())
        .collect::<Option<Vec<f64>>>()?;
    (v.len() == n).then_some(v)
}

/// A colour's screen form.
fn to32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
