//! Dynamic Text, as Photoshop has it: a text that fits its box. Dragging
//! the box's handles scales the type and reflows its lines to fill the new
//! size, and so does editing the words; Enter breaks a line where you want
//! it. TraceDraw's paragraph text frame already does this with Type block
//! (the paragraph style's `type_block`: every line scaled to fill the
//! frame's width, laid out afresh as a frame is dragged). This adds what
//! makes it one tap on a phone:
//!
//! - Text > Dynamic Text (and a button beside the drawing while a text is
//!   selected): an artistic text first becomes a paragraph text in a frame
//!   of its own size, then Type block is switched on for every
//!   selected text; with it on already for all of them it is switched
//!   off. The word gap and the line gap are the paragraph's Word spacing
//!   and Line spacing in the Text docker.
//! - Layout: Block (no shape), Arch, Bulge, Squeeze, Flag, Perspective
//!   and Circle, the envelope presets set on the text, which stay live
//!   as the frame is resized (Clear Envelope or Block takes them off).
//!
//! See docs/behavior/text.md.

use crate::app::App;
use crate::envelope_edit::Preset;
use crate::i18n::tr;
use tracedraw_core::{Shape, ShapeKind};

fn is_artistic(s: &Shape) -> bool {
    matches!(s.kind, ShapeKind::Text { frame: None, .. })
}

fn is_dynamic(s: &Shape) -> bool {
    matches!(&s.kind, ShapeKind::Text { frame: Some(_), para, .. } if para.type_block)
}

impl App {
    /// Whether every selected text is a dynamic one (and there is one).
    pub fn dynamic_text_on(&self) -> bool {
        let texts = self.text_shapes();
        !texts.is_empty() && texts.iter().all(is_dynamic)
    }

    /// Text > Dynamic Text: see the module's text.
    pub fn dynamic_text(&mut self) {
        let texts = self.text_shapes();
        if texts.is_empty() {
            self.status = tr("status.dynamic_text_none");
            return;
        }
        let on = texts.iter().all(is_dynamic);
        // Artistic texts take a frame of their own size first (Convert),
        // only those: converting a frame would make it artistic again.
        if !on && texts.iter().any(is_artistic) {
            let keep = self.selection.clone();
            self.selection = texts.iter().filter(|s| is_artistic(s)).map(|s| s.id).collect();
            self.toggle_text_kind();
            self.selection = keep;
        }
        let frames: Vec<Shape> = self
            .text_shapes()
            .into_iter()
            .filter(|s| matches!(s.kind, ShapeKind::Text { frame: Some(_), .. }))
            .collect();
        for s in frames {
            self.edit_paragraph_style(s.id, |p| p.type_block = !on);
        }
    }

    /// The layout of the selected dynamic texts: a preset, or `None` for
    /// Block, which takes the envelope off.
    pub fn dynamic_layout(&mut self, preset: Option<Preset>) {
        if self.text_shapes().is_empty() {
            self.status = tr("status.dynamic_text_none");
            return;
        }
        match preset {
            Some(p) => self.apply_envelope_preset(p),
            None => self.clear_envelope(),
        }
    }
}

/// The key of a layout's name.
fn layout_key(preset: Option<Preset>) -> &'static str {
    match preset {
        None => "dynamic.layout_block",
        Some(p) => p.key(),
    }
}

/// A button beside the drawing while a text is selected, on a touch
/// screen: Dynamic Text on or off, and the layout list.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: egui::Rect) {
    let ctx = ui.ctx().clone();
    if !crate::touch_modifiers::offered(&ctx)
        || app.artboard_edit.is_some()
        || app.text_shapes().is_empty()
        || canvas.height() < 120.0
    {
        return;
    }
    let on = app.dynamic_text_on();
    let layouts: [Option<Preset>; 7] = [
        None,
        Some(Preset::Arch),
        Some(Preset::Bulge),
        Some(Preset::Squeeze),
        Some(Preset::Flag),
        Some(Preset::Perspective),
        Some(Preset::Circle),
    ];
    egui::Area::new(egui::Id::new("dynamic_text_bar"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::LEFT_TOP)
        .fixed_pos(egui::pos2(canvas.left() + 8.0, canvas.top() + 8.0))
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(crate::theme::Tokens::panel())
                .stroke(egui::Stroke::new(1.0, crate::theme::Tokens::control_border()))
                .corner_radius(8.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let side = egui::vec2(104.0, 40.0);
                        let b = egui::Button::new(tr("dynamic.button"))
                            .min_size(side)
                            .selected(on);
                        if ui
                            .add(b)
                            .on_hover_text(tr("dynamic.button_tip"))
                            .clicked()
                        {
                            app.dynamic_text();
                        }
                        ui.menu_button(tr("dynamic.layout"), |ui| {
                            for l in layouts {
                                if ui.button(tr(layout_key(l))).clicked() {
                                    app.dynamic_layout(l);
                                    ui.close();
                                }
                            }
                        });
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Point;
    use tracedraw_core::{ParagraphStyle, TextAlign, TextSpan};

    fn text(app: &mut App, frame: Option<tracedraw_core::geometry::Size>) -> tracedraw_core::ShapeId {
        app.new_shape(ShapeKind::Text {
            spans: vec![TextSpan::new("Sale today", "Quicksand", 24.0)],
            origin: Point::new(10.0, 10.0),
            frame,
            align: TextAlign::Left,
            para: ParagraphStyle::default(),
            on_path: None,
        })
        .expect("text")
    }

    /// Dynamic Text turns an artistic text into a frame set as a type
    /// block, a second use turns it off, and a frame already there only
    /// has Type block switched on.
    #[test]
    fn dynamic_text_converts_artistic_text_and_toggles_the_fit() {
        let mut app = App::headless();
        app.show_welcome = false;
        let artistic = text(&mut app, None);
        let frame = text(
            &mut app,
            Some(tracedraw_core::geometry::Size::new(80.0, 30.0)),
        );
        // With nothing selected it says so and changes nothing.
        app.select(Vec::new());
        app.dynamic_text();
        assert!(!app.dynamic_text_on());

        app.select(vec![artistic, frame]);
        app.dynamic_text();
        let s = app.doc().find_shape(artistic).cloned().expect("shape");
        assert!(is_dynamic(&s), "the artistic text became a type block frame");
        let f = app.doc().find_shape(frame).cloned().expect("shape");
        assert!(is_dynamic(&f), "the frame keeps its kind and is a type block");
        assert!(app.dynamic_text_on());

        app.dynamic_text();
        assert!(!app.dynamic_text_on());
        let f = app.doc().find_shape(frame).cloned().expect("shape");
        assert!(
            matches!(f.kind, ShapeKind::Text { frame: Some(_), .. }),
            "off keeps the frame"
        );
    }

    /// A layout puts an envelope on the text and Block takes it off.
    #[test]
    fn a_layout_sets_an_envelope_and_block_clears_it() {
        let mut app = App::headless();
        app.show_welcome = false;
        let id = text(
            &mut app,
            Some(tracedraw_core::geometry::Size::new(80.0, 30.0)),
        );
        app.select(vec![id]);
        app.dynamic_layout(Some(Preset::Arch));
        let with = app.doc().find_shape(id).map(|s| s.effects.len()).unwrap_or(0);
        app.dynamic_layout(None);
        let without = app.doc().find_shape(id).map(|s| s.effects.len()).unwrap_or(0);
        assert!(with > without, "{with} effects, then {without}");
    }
}
