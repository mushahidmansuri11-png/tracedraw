//! Shape Builder. The selected closed objects are cut into every separate
//! region they make together ([`tracedraw_core::shaping::divide`]) and
//! the drawing waits for the hand: a drag (a finger, a pen or the
//! mouse) over regions joins them into one, and in Delete mode (the
//! button, or Alt held) the regions it crosses are removed; a tap on a
//! region removes it in Delete mode. A region sweeps in when the drag's
//! path passes over it, so a finger needs no hairline aim. Each gesture
//! changes the regions shown; Done replaces the originals by the
//! regions that are left, in one undo step (each takes the look of the
//! frontmost object that covers it, a joined one the look of the region
//! the drag began on), Cancel leaves the drawing as it was. See
//! docs/behavior/shaping.md.

use crate::app::App;
use crate::i18n::tr;
use egui::{Color32, Pos2, Rect, Sense, Stroke};
use tracedraw_core::geometry::{BezPath, Point};
use tracedraw_core::shaping::{self, Op};
use tracedraw_core::{Command, Shape, ShapeId, ShapeKind};

/// One region: its outline, its flattened contours (page space) and the
/// index of the object whose look it takes.
#[derive(Debug, Clone)]
pub struct Part {
    pub path: BezPath,
    contours: Vec<Vec<[f64; 2]>>,
    pub from: usize,
}

impl Part {
    fn new(path: BezPath, from: usize) -> Part {
        let contours = shaping::contours(&path);
        Part {
            path,
            contours,
            from,
        }
    }

    /// Whether `p` lies in the region (the even-odd rule over all its
    /// contours, so holes are holes).
    pub fn contains(&self, p: Point) -> bool {
        let mut inside = false;
        for c in &self.contours {
            let n = c.len();
            for i in 0..n {
                let (a, b) = (c[i], c[(i + 1) % n]);
                if (a[1] > p.y) != (b[1] > p.y) {
                    let x = a[0] + (p.y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                    if p.x < x {
                        inside = !inside;
                    }
                }
            }
        }
        inside
    }
}

/// The Shape Builder in progress.
#[derive(Debug, Clone)]
pub struct ShapeBuilder {
    sources: Vec<Shape>,
    pub parts: Vec<Part>,
    /// The regions the drag in progress has crossed, in order.
    pub sweep: Vec<usize>,
    last: Option<Pos2>,
    /// Delete mode from the button (Alt held does the same).
    pub delete: bool,
    /// Something was joined or removed.
    pub edited: bool,
}

impl ShapeBuilder {
    /// The regions the closed objects in `shapes` (back to front) make.
    pub fn new(shapes: &[Shape]) -> Option<ShapeBuilder> {
        let closed: Vec<Shape> = shapes
            .iter()
            .filter(|s| !matches!(s.kind, ShapeKind::Path { closed: false, .. }))
            .cloned()
            .collect();
        if closed.len() < 2 {
            return None;
        }
        let paths: Vec<BezPath> = closed.iter().map(crate::ops::shaping_area).collect();
        let parts: Vec<Part> = shaping::divide(&paths)
            .into_iter()
            .map(|(p, from)| Part::new(p, from))
            .collect();
        if parts.is_empty() {
            return None;
        }
        Some(ShapeBuilder {
            sources: closed,
            parts,
            sweep: Vec::new(),
            last: None,
            delete: false,
            edited: false,
        })
    }

    /// The region at `p`, if any.
    pub fn region_at(&self, p: Point) -> Option<usize> {
        self.parts.iter().position(|r| r.contains(p))
    }

    /// Sweep in the region at `p`.
    fn touch(&mut self, p: Point) {
        if let Some(i) = self.region_at(p) {
            if !self.sweep.contains(&i) {
                self.sweep.push(i);
            }
        }
    }

    /// End the gesture: join the swept regions into one, or remove them.
    pub fn commit(&mut self, delete: bool) {
        let sweep = std::mem::take(&mut self.sweep);
        self.last = None;
        if sweep.is_empty() {
            return;
        }
        if delete {
            let mut gone = sweep;
            gone.sort_unstable();
            for i in gone.into_iter().rev() {
                self.parts.remove(i);
            }
            self.edited = true;
            return;
        }
        if sweep.len() < 2 {
            return;
        }
        let mut path = self.parts[sweep[0]].path.clone();
        for &i in &sweep[1..] {
            path = shaping::overlay(&path, &self.parts[i].path, Op::Weld);
        }
        let from = self.parts[sweep[0]].from;
        let mut others: Vec<usize> = sweep[1..].to_vec();
        others.sort_unstable();
        self.parts[sweep[0]] = Part::new(path, from);
        for i in others.into_iter().rev() {
            self.parts.remove(i);
        }
        self.edited = true;
    }
}

impl App {
    /// Start the Shape Builder on the selection.
    pub fn shape_builder_begin(&mut self) {
        match ShapeBuilder::new(&self.selected_shapes()) {
            Some(sb) => {
                self.shape_builder = Some(sb);
                self.status = tr("status.shape_builder");
            }
            None => self.status = tr("status.shaping_open_cutter"),
        }
    }

    /// Leave the Shape Builder: with `apply` the originals become the
    /// regions that are left, as one undo step.
    pub fn shape_builder_end(&mut self, apply: bool) {
        let Some(sb) = self.shape_builder.take() else {
            return;
        };
        if !apply || !sb.edited {
            self.status = tr("status.cancelled");
            return;
        }
        let Some(layer) = self.active_layer() else {
            return;
        };
        let mut ids: Vec<ShapeId> = Vec::new();
        let mut cmds: Vec<Command> = Vec::new();
        for part in &sb.parts {
            let src = &sb.sources[part.from.min(sb.sources.len() - 1)];
            // A group has no look of its own: the first filled object in it.
            let look = crate::ops::group_leaves(src)
                .into_iter()
                .find(|l| !matches!(l.fill, tracedraw_core::Fill::None))
                .unwrap_or_else(|| src.clone());
            let id = self.engine.new_shape_id();
            let mut shape = Shape::new(
                id,
                ShapeKind::Path {
                    path: part.path.clone(),
                    closed: true,
                },
            );
            shape.fill = look.fill.for_result(look.transform, &shape.page_path());
            shape.stroke = look.stroke.clone();
            shape.opacity = look.opacity;
            ids.push(id);
            cmds.push(Command::AddShape { layer, shape });
        }
        cmds.push(Command::DeleteShapes {
            shapes: sb.sources.iter().map(|s| s.id).collect(),
        });
        if let Err(e) = self.engine.run_batch("Shape Builder", &cmds) {
            self.status = e.to_string();
            return;
        }
        self.select(ids);
    }
}

/// Horizontal lines `step` apart across the polygons (even-odd), the
/// fill of a swept region.
fn hatch(painter: &egui::Painter, polys: &[Vec<Pos2>], color: Color32, step: f32) {
    let (mut top, mut bottom) = (f32::MAX, f32::MIN);
    for p in polys.iter().flatten() {
        top = top.min(p.y);
        bottom = bottom.max(p.y);
    }
    let mut y = top + step / 2.0;
    while y < bottom {
        let mut xs: Vec<f32> = Vec::new();
        for poly in polys {
            let n = poly.len();
            for i in 0..n {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                if (a.y > y) != (b.y > y) {
                    xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
                }
            }
        }
        xs.sort_by(f32::total_cmp);
        for pair in xs.chunks_exact(2) {
            painter.line_segment(
                [Pos2::new(pair[0], y), Pos2::new(pair[1], y)],
                Stroke::new(1.5, color),
            );
        }
        y += step;
    }
}

/// The regions over the drawing, the gestures and the Done bar.
pub fn on_drawing(app: &mut App, ui: &mut egui::Ui, canvas: Rect) {
    let Some(mut sb) = app.shape_builder.take() else {
        return;
    };
    let ctx = ui.ctx().clone();
    let alt = ctx.input(|i| i.modifiers.alt) || app.touch_mods.alt;
    let delete = sb.delete || alt;
    let view = app.view;
    let accent = if delete {
        Color32::from_rgb(0xD0, 0x30, 0x30)
    } else {
        crate::theme::Tokens::ACCENT
    };
    egui::Area::new(egui::Id::new("shape_builder_regions"))
        .order(egui::Order::Middle)
        .fixed_pos(canvas.min)
        .show(&ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(canvas.size(), Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            let screen = |part: &Part| -> Vec<Vec<Pos2>> {
                part.contours
                    .iter()
                    .map(|c| c.iter().map(|p| view.to_screen(Point::new(p[0], p[1]))).collect())
                    .collect()
            };
            for (i, part) in sb.parts.iter().enumerate() {
                let polys = screen(part);
                let swept = sb.sweep.contains(&i);
                if swept {
                    hatch(&painter, &polys, accent, 5.0);
                }
                let stroke = if swept {
                    Stroke::new(3.0, accent)
                } else {
                    Stroke::new(1.5, Color32::from_gray(0x70))
                };
                for poly in &polys {
                    painter.add(egui::Shape::closed_line(poly.clone(), stroke));
                }
            }
            // The drag: every region its path passes over is swept in.
            if let Some(pos) = resp.interact_pointer_pos() {
                if resp.drag_started() || resp.dragged() {
                    let from = sb.last.unwrap_or(pos);
                    let steps = ((pos - from).length() / 3.0).ceil().max(1.0) as usize;
                    for k in 1..=steps {
                        let q = from + (pos - from) * (k as f32 / steps as f32);
                        sb.touch(view.to_page(q));
                    }
                    sb.last = Some(pos);
                }
                if resp.clicked() && delete {
                    sb.touch(view.to_page(pos));
                    sb.commit(true);
                }
            }
            if resp.drag_stopped() {
                sb.commit(delete);
            }
        });
    // Merge or Delete, Done and Cancel, over the top of the drawing.
    let (mut done, mut cancel) = (false, false);
    egui::Area::new(egui::Id::new("shape_builder_bar"))
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
                    ui.horizontal(|ui| {
                        let side = egui::vec2(84.0, 44.0);
                        let merge = egui::Button::new(tr("shape_builder.merge"))
                            .min_size(side)
                            .selected(!delete);
                        if ui
                            .add(merge)
                            .on_hover_text(tr("shape_builder.merge_tip"))
                            .clicked()
                        {
                            sb.delete = false;
                        }
                        let del = egui::Button::new(tr("shape_builder.delete"))
                            .min_size(side)
                            .selected(delete);
                        if ui
                            .add(del)
                            .on_hover_text(tr("shape_builder.delete_tip"))
                            .clicked()
                        {
                            sb.delete = true;
                        }
                        let ok = egui::Button::new(tr("shape_builder.done")).min_size(side);
                        done = ui
                            .add(ok)
                            .on_hover_text(tr("shape_builder.done_tip"))
                            .clicked();
                        let no = egui::Button::new(tr("shape_builder.cancel")).min_size(side);
                        cancel = ui
                            .add(no)
                            .on_hover_text(tr("shape_builder.cancel_tip"))
                            .clicked();
                    });
                });
        });
    app.shape_builder = Some(sb);
    if done {
        app.shape_builder_end(true);
    } else if cancel || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.shape_builder_end(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Rect as MmRect;

    fn square(app: &mut App, x: f64, y: f64, s: f64) -> ShapeId {
        app.new_shape(ShapeKind::Rect {
            rect: MmRect::new(x, y, x + s, y + s),
            radius: 0.0,
            corners: None,
        })
        .expect("rect")
    }

    /// Two overlapping squares make three regions; dragging over the two
    /// left ones joins them, Done replaces the squares by two objects.
    #[test]
    fn a_drag_over_regions_joins_them() {
        let mut app = App::headless();
        app.show_welcome = false;
        let a = square(&mut app, 0.0, 0.0, 40.0);
        let b = square(&mut app, 20.0, 0.0, 40.0);
        app.select(vec![a, b]);
        app.shape_builder_begin();
        let sb = app.shape_builder.as_mut().expect("begun");
        assert_eq!(sb.parts.len(), 3);
        // Left only, the overlap, right only.
        let left = sb.region_at(Point::new(10.0, 20.0)).expect("left");
        let both = sb.region_at(Point::new(30.0, 20.0)).expect("overlap");
        let right = sb.region_at(Point::new(50.0, 20.0)).expect("right");
        assert!(left != both && both != right && left != right);
        assert_eq!(sb.region_at(Point::new(-5.0, 20.0)), None);
        sb.sweep = vec![left, both];
        sb.commit(false);
        assert_eq!(sb.parts.len(), 2);
        assert!(sb.edited);
        app.shape_builder_end(true);
        assert!(app.shape_builder.is_none());
        assert!(app.doc().find_shape(a).is_none() && app.doc().find_shape(b).is_none());
        assert_eq!(app.selection.len(), 2);
    }

    /// Delete mode removes the swept regions; with nothing done, Done
    /// leaves the drawing alone.
    #[test]
    fn delete_removes_regions_and_an_untouched_build_changes_nothing() {
        let mut app = App::headless();
        app.show_welcome = false;
        let a = square(&mut app, 0.0, 0.0, 40.0);
        let b = square(&mut app, 20.0, 0.0, 40.0);
        app.select(vec![a, b]);
        app.shape_builder_begin();
        let sb = app.shape_builder.as_mut().expect("begun");
        let both = sb.region_at(Point::new(30.0, 20.0)).expect("overlap");
        sb.sweep = vec![both];
        sb.commit(true);
        assert_eq!(sb.parts.len(), 2);
        app.shape_builder_end(true);
        assert!(app.doc().find_shape(a).is_none());
        assert_eq!(app.selection.len(), 2);
        // One object alone cannot be built.
        let c = square(&mut app, 100.0, 0.0, 10.0);
        app.select(vec![c]);
        app.shape_builder_begin();
        assert!(app.shape_builder.is_none());
        // A build left as it was changes nothing.
        let d = square(&mut app, 0.0, 100.0, 40.0);
        let e = square(&mut app, 20.0, 100.0, 40.0);
        app.select(vec![d, e]);
        app.shape_builder_begin();
        app.shape_builder_end(true);
        assert!(app.doc().find_shape(d).is_some() && app.doc().find_shape(e).is_some());
    }
}
