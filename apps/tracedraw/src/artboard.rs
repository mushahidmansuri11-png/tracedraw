//! Edit Artboard. The page is the artboard of the design; a drawing often
//! has to grow or shrink it to the artwork (a banner longer than planned,
//! a card with a margin more). Page > Edit Artboard puts handles on the
//! page's edges and corners over the drawing: a drag (a finger, a pen or
//! the mouse) pulls an edge or a corner, a drag inside the page moves the
//! artboard over the artwork. Fit to Selection and Fit to Drawing set the
//! box in one tap, Done gives the page the new box (`App::fit_pages_to`,
//! the way Fit Page to Selection does: the objects move so the new box's
//! bottom-left corner is the page's, one undo step "Edit Artboard"),
//! Cancel and Esc leave it as it was. See docs/behavior/pages.md.

use crate::app::App;
use crate::i18n::tr;
use egui::{Color32, Pos2, Rect as ScreenRect, Sense, Stroke, Vec2};
use tracedraw_core::geometry::{Point, Rect};
use tracedraw_core::PageId;

/// The side of a handle and how near a press must be to take it, in
/// interface points: sized for a finger.
const HANDLE: f32 = 18.0;
const REACH: f32 = 30.0;
/// The smallest artboard, mm.
const MIN_SIDE: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grip {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
    Move,
}

/// The artboard being edited.
#[derive(Debug, Clone)]
pub struct ArtboardEdit {
    pub page: PageId,
    /// The page as it is: where the box started.
    pub original: Rect,
    /// The box the page will take, in the page's coordinates now.
    pub rect: Rect,
    grip: Option<Grip>,
}

impl ArtboardEdit {
    pub fn new(page: PageId, width: f64, height: f64) -> ArtboardEdit {
        let r = Rect::new(0.0, 0.0, width, height);
        ArtboardEdit {
            page,
            original: r,
            rect: r,
            grip: None,
        }
    }

    /// Pull the handle `grip` by `d` (page units); the opposite edges stay
    /// and the box keeps at least [`MIN_SIDE`] on each side.
    pub fn pull(&mut self, grip: Grip, d: Point) {
        let mut r = self.rect;
        let (dx, dy) = (d.x, d.y);
        match grip {
            Grip::Move => {
                r = Rect::new(r.x0 + dx, r.y0 + dy, r.x1 + dx, r.y1 + dy);
            }
            _ => {
                if matches!(grip, Grip::W | Grip::NW | Grip::SW) {
                    r.x0 = (r.x0 + dx).min(r.x1 - MIN_SIDE);
                }
                if matches!(grip, Grip::E | Grip::NE | Grip::SE) {
                    r.x1 = (r.x1 + dx).max(r.x0 + MIN_SIDE);
                }
                if matches!(grip, Grip::S | Grip::SW | Grip::SE) {
                    r.y0 = (r.y0 + dy).min(r.y1 - MIN_SIDE);
                }
                if matches!(grip, Grip::N | Grip::NW | Grip::NE) {
                    r.y1 = (r.y1 + dy).max(r.y0 + MIN_SIDE);
                }
            }
        }
        if r.x0.is_finite() && r.y0.is_finite() && r.x1.is_finite() && r.y1.is_finite() {
            self.rect = r;
        }
    }

    /// Whether the box differs from the page.
    pub fn changed(&self) -> bool {
        let (a, b) = (self.rect, self.original);
        (a.x0 - b.x0).abs() > 1e-6
            || (a.y0 - b.y0).abs() > 1e-6
            || (a.x1 - b.x1).abs() > 1e-6
            || (a.y1 - b.y1).abs() > 1e-6
    }
}

/// The handles of a screen box, with their grips.
fn handles(r: ScreenRect) -> [(Grip, Pos2); 8] {
    let (l, rt, t, b) = (r.left(), r.right(), r.top(), r.bottom());
    let (cx, cy) = (r.center().x, r.center().y);
    [
        (Grip::NW, Pos2::new(l, t)),
        (Grip::N, Pos2::new(cx, t)),
        (Grip::NE, Pos2::new(rt, t)),
        (Grip::E, Pos2::new(rt, cy)),
        (Grip::SE, Pos2::new(rt, b)),
        (Grip::S, Pos2::new(cx, b)),
        (Grip::SW, Pos2::new(l, b)),
        (Grip::W, Pos2::new(l, cy)),
    ]
}

/// The grip a press at `p` takes: the nearest handle in reach, else the
/// inside of the box, else none.
pub fn grip_at(r: ScreenRect, p: Pos2) -> Option<Grip> {
    let near = handles(r)
        .into_iter()
        .filter(|(_, h)| h.distance(p) <= REACH)
        .min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
        .map(|(g, _)| g);
    near.or_else(|| r.contains(p).then_some(Grip::Move))
}

impl App {
    /// Start Edit Artboard on the current page.
    pub fn artboard_begin(&mut self) {
        let Ok(p) = self.doc().page(self.page) else {
            return;
        };
        let (w, h) = (p.size.width, p.size.height);
        self.artboard_edit = Some(ArtboardEdit::new(self.page, w, h));
        self.status = tr("status.artboard");
    }

    /// Leave Edit Artboard; with `apply` the page takes the box.
    pub fn artboard_end(&mut self, apply: bool) {
        let Some(edit) = self.artboard_edit.take() else {
            return;
        };
        if !apply || !edit.changed() {
            self.status = tr("status.cancelled");
            return;
        }
        self.fit_pages_to(&[(edit.page, edit.rect)], "Edit Artboard");
    }
}

/// The box and its handles over the drawing, the gestures and the bar.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: ScreenRect) {
    let Some(mut edit) = app.artboard_edit.take() else {
        return;
    };
    // The page may have changed under the edit (undo, another tab).
    if app.page != edit.page || app.doc().page(edit.page).is_err() {
        app.status = tr("status.cancelled");
        return;
    }
    let ctx = ui.ctx().clone();
    let view = app.view;
    let to_screen_rect = |r: Rect| {
        ScreenRect::from_two_pos(
            view.to_screen(Point::new(r.x0, r.y0)),
            view.to_screen(Point::new(r.x1, r.y1)),
        )
    };
    egui::Area::new(egui::Id::new("artboard_edit_box"))
        .order(egui::Order::Middle)
        .fixed_pos(canvas.min)
        .show(&ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(canvas.size(), Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            let accent = crate::theme::Tokens::ACCENT;
            // The page as it is, dashed grey; the new box in the accent.
            let was = to_screen_rect(edit.original);
            painter.rect_stroke(
                was,
                0.0,
                Stroke::new(1.0, Color32::from_gray(0x80)),
                egui::StrokeKind::Middle,
            );
            let now = to_screen_rect(edit.rect);
            painter.rect_filled(now, 0.0, accent.gamma_multiply(0.08));
            painter.rect_stroke(now, 0.0, Stroke::new(2.0, accent), egui::StrokeKind::Middle);
            for (_, h) in handles(now) {
                let sq = ScreenRect::from_center_size(h, Vec2::splat(HANDLE));
                painter.rect_filled(sq, 3.0, Color32::WHITE);
                painter.rect_stroke(sq, 3.0, Stroke::new(2.0, accent), egui::StrokeKind::Middle);
            }
            if resp.drag_started() {
                edit.grip = resp.interact_pointer_pos().and_then(|p| grip_at(now, p));
            }
            if resp.dragged() {
                if let (Some(g), Some(pos)) = (edit.grip, resp.interact_pointer_pos()) {
                    let from = pos - resp.drag_delta();
                    let (a, b) = (view.to_page(from), view.to_page(pos));
                    edit.pull(g, Point::new(b.x - a.x, b.y - a.y));
                }
            }
            if resp.drag_stopped() {
                edit.grip = None;
            }
        });
    // The size, Fit to Selection, Fit to Drawing, Done and Cancel.
    let (mut done, mut cancel) = (false, false);
    egui::Area::new(egui::Id::new("artboard_edit_bar"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(egui::pos2(canvas.center().x, canvas.top() + 8.0))
        .constrain_to(canvas)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(crate::theme::Tokens::panel())
                .stroke(Stroke::new(1.0, crate::theme::Tokens::control_border()))
                .corner_radius(10.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(format!(
                            "{:.1} x {:.1} mm",
                            edit.rect.width(),
                            edit.rect.height()
                        ));
                        ui.horizontal(|ui| {
                            let side = egui::vec2(96.0, 44.0);
                            let sel = egui::Button::new(tr("artboard.fit_selection")).min_size(side);
                            if ui
                                .add_enabled(app.selection_bounds().is_some(), sel)
                                .on_hover_text(tr("artboard.fit_selection_tip"))
                                .clicked()
                            {
                                if let Some(b) = app.selection_bounds() {
                                    edit.rect = b;
                                }
                            }
                            let dr = egui::Button::new(tr("artboard.fit_drawing")).min_size(side);
                            if ui
                                .add_enabled(app.drawing_bounds().is_some(), dr)
                                .on_hover_text(tr("artboard.fit_drawing_tip"))
                                .clicked()
                            {
                                if let Some(b) = app.drawing_bounds() {
                                    edit.rect = b;
                                }
                            }
                        });
                        // The bleed and the page margin, drawn once set.
                        ui.horizontal(|ui| {
                            let mut md = app.doc().metadata.clone();
                            for is_bleed in [true, false] {
                                ui.label(tr(if is_bleed { "artboard.bleed" } else { "artboard.margin" }));
                                let mut v = app.units.from_mm(if is_bleed { md.bleed } else { md.margin });
                                if ui
                                    .add_sized(
                                        [64.0, 28.0],
                                        crate::ui::field::NumField::new(&mut v)
                                            .speed(0.1)
                                            .max_decimals(2),
                                    )
                                    .changed()
                                {
                                    let mm = app.units.to_mm(v).max(0.0);
                                    if is_bleed {
                                        md.bleed = mm;
                                        app.show_bleed |= mm > 0.0;
                                    } else {
                                        md.margin = mm;
                                        app.show_margin |= mm > 0.0;
                                    }
                                    app.run(tracedraw_core::Command::SetMetadata {
                                        metadata: md.clone(),
                                    });
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            let side = egui::vec2(96.0, 44.0);
                            done = ui
                                .add(egui::Button::new(tr("artboard.done")).min_size(side))
                                .on_hover_text(tr("artboard.done_tip"))
                                .clicked();
                            cancel = ui
                                .add(egui::Button::new(tr("artboard.cancel")).min_size(side))
                                .on_hover_text(tr("artboard.cancel_tip"))
                                .clicked();
                        });
                    });
                });
        });
    app.artboard_edit = Some(edit);
    if done {
        app.artboard_end(true);
    } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.artboard_end(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pulling a corner moves its two edges and keeps the opposite ones;
    /// the box never gets thinner than the smallest side.
    #[test]
    fn a_pull_moves_the_pulled_edges_only() {
        let mut e = ArtboardEdit::new(App::headless().page, 100.0, 50.0);
        e.pull(Grip::NE, Point::new(20.0, 10.0));
        assert_eq!((e.rect.x1, e.rect.y1), (120.0, 60.0));
        assert_eq!((e.rect.x0, e.rect.y0), (0.0, 0.0));
        e.pull(Grip::W, Point::new(-30.0, 5.0));
        assert_eq!((e.rect.x0, e.rect.y0), (-30.0, 0.0));
        e.pull(Grip::Move, Point::new(5.0, 5.0));
        assert_eq!((e.rect.x0, e.rect.y1), (-25.0, 65.0));
        e.pull(Grip::E, Point::new(-1000.0, 0.0));
        assert!((e.rect.width() - MIN_SIDE).abs() < 1e-9);
        assert!(e.changed());
    }

    /// The nearest handle takes the press; inside the box it moves it.
    #[test]
    fn a_press_takes_the_nearest_handle_or_moves_the_box() {
        let r = ScreenRect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(300.0, 200.0));
        assert_eq!(grip_at(r, Pos2::new(102.0, 98.0)), Some(Grip::NW));
        assert_eq!(grip_at(r, Pos2::new(200.0, 101.0)), Some(Grip::N));
        assert_eq!(grip_at(r, Pos2::new(298.0, 150.0)), Some(Grip::E));
        assert_eq!(grip_at(r, Pos2::new(200.0, 150.0)), Some(Grip::Move));
        assert_eq!(grip_at(r, Pos2::new(500.0, 500.0)), None);
    }

    /// The bleed and the page margin are set on the document and shown
    /// once they are above 0; an old drawing has no margin.
    #[test]
    fn the_bleed_and_the_margin_are_kept_on_the_document() {
        let mut app = App::headless();
        app.show_welcome = false;
        assert_eq!(app.doc().metadata.margin, 0.0);
        let mut md = app.doc().metadata.clone();
        md.bleed = 3.0;
        md.margin = 5.0;
        app.run(tracedraw_core::Command::SetMetadata { metadata: md });
        assert_eq!(app.doc().metadata.bleed, 3.0);
        assert_eq!(app.doc().metadata.margin, 5.0);
        app.undo();
        assert_eq!(app.doc().metadata.margin, 0.0);
        let prefs = app.view_prefs();
        assert!(!prefs.margin);
        app.show_margin = true;
        assert!(app.view_prefs().margin);
    }

    /// Done gives the page the box and moves the objects; Cancel and a
    /// box that did not change leave it.
    #[test]
    fn done_resizes_the_page_in_one_step_and_cancel_keeps_it() {
        use tracedraw_core::{ShapeKind};
        let mut app = App::headless();
        app.show_welcome = false;
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(10.0, 10.0, 30.0, 30.0),
                radius: 0.0,
                corners: None,
            })
            .expect("rect");
        let before = app.doc().page(app.page).expect("page").size;
        app.artboard_begin();
        app.artboard_end(false);
        assert_eq!(app.doc().page(app.page).expect("page").size, before);
        app.artboard_begin();
        let edit = app.artboard_edit.as_mut().expect("begun");
        edit.pull(Grip::NE, Point::new(-20.0, -20.0));
        let (w, h) = (edit.rect.width(), edit.rect.height());
        app.artboard_end(true);
        let size = app.doc().page(app.page).expect("page").size;
        assert!((size.width - w).abs() < 1e-6 && (size.height - h).abs() < 1e-6);
        assert!(app.doc().find_shape(id).is_some());
    }
}
