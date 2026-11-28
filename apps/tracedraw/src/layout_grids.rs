//! Grids for designing a logo, a page of a magazine or a poster, as
//! guidelines. The geometry is pure (millimetres in, guidelines out); the
//! App methods at the bottom write the lines to the page in one undo step
//! and remember them, so applying a grid again replaces the one made last
//! (a magazine grid is edited by changing its columns and applying again).
//!
//! Logo grids: base (square, isometric, golden ratio), construction (the
//! centres, edges, diagonals and nodes of the selection), lockup (the
//! thirds of the selection) and clearspace (a clear distance round it).
//! Page grid: columns and rows with gutters inside the margins.
//! See docs/behavior/rulers-grid-guidelines.md.

use tracedraw_core::document::{Guide, GuideLine};
use tracedraw_core::geometry::{Point, Rect};

/// The most guidelines one grid makes: a square grid of tiny cells would
/// otherwise make thousands.
pub const MAX_LINES: usize = 200;

const INV_PHI: f64 = 0.618_033_988_749_894_9;

fn push_unique(out: &mut Vec<GuideLine>, line: GuideLine) {
    if out.len() >= MAX_LINES {
        return;
    }
    let same = |a: &GuideLine, b: &GuideLine| match (a, b) {
        (GuideLine::Horizontal { y: p }, GuideLine::Horizontal { y: q }) => (p - q).abs() < 1e-9,
        (GuideLine::Vertical { x: p }, GuideLine::Vertical { x: q }) => (p - q).abs() < 1e-9,
        (
            GuideLine::Angled { x: px, y: py, angle: pa },
            GuideLine::Angled { x: qx, y: qy, angle: qa },
        ) => (px - qx).abs() < 1e-9 && (py - qy).abs() < 1e-9 && (pa - qa).abs() < 1e-9,
        _ => false,
    };
    if !out.iter().any(|o| same(o, &line)) {
        out.push(line);
    }
}

fn finite(r: &Rect) -> bool {
    [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite()) && r.x1 > r.x0 && r.y1 > r.y0
}

/// Vertical and horizontal lines every `step` across `b` from its
/// top-left corner.
pub fn square(b: Rect, step: f64) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) || !(step.is_finite() && step > 0.0) {
        return out;
    }
    let mut x = b.x0;
    while x <= b.x1 + 1e-9 && out.len() < MAX_LINES {
        push_unique(&mut out, GuideLine::Vertical { x });
        x += step;
    }
    let mut y = b.y0;
    while y <= b.y1 + 1e-9 && out.len() < MAX_LINES {
        push_unique(&mut out, GuideLine::Horizontal { y });
        y += step;
    }
    out
}

/// Lines at 0.382 and 0.618 of the width and of the height of `b`: the
/// golden section.
pub fn golden(b: Rect) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) {
        return out;
    }
    let (w, h) = (b.width(), b.height());
    for f in [1.0 - INV_PHI, INV_PHI] {
        push_unique(&mut out, GuideLine::Vertical { x: b.x0 + w * f });
        push_unique(&mut out, GuideLine::Horizontal { y: b.y0 + h * f });
    }
    out
}

/// Vertical lines and lines at 30 and 150 degrees, `step` apart measured
/// across the lines, through `b`: the isometric grid of a drawing that
/// sits on a cube's faces.
pub fn isometric(b: Rect, step: f64) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) || !(step.is_finite() && step > 0.0) {
        return out;
    }
    let tan = 30f64.to_radians().tan();
    let along = step / 30f64.to_radians().cos();
    let mut x = b.x0;
    while x <= b.x1 + 1e-9 && out.len() < MAX_LINES {
        push_unique(&mut out, GuideLine::Vertical { x });
        x += step;
    }
    let w = b.width();
    let mut c = b.y0 - w * tan;
    while c <= b.y1 + 1e-9 && out.len() < MAX_LINES {
        push_unique(&mut out, GuideLine::Angled { x: b.x0, y: c, angle: 30.0 });
        c += along;
    }
    let mut c = b.y0;
    while c <= b.y1 + w * tan + 1e-9 && out.len() < MAX_LINES {
        push_unique(&mut out, GuideLine::Angled { x: b.x0, y: c, angle: 150.0 });
        c += along;
    }
    out
}

/// The construction lines of a logo: its centre lines, edges, the two
/// diagonals of its box, and through every node a vertical and a
/// horizontal, so that alignments show up.
pub fn construction(b: Rect, nodes: &[Point]) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) {
        return out;
    }
    let (cx, cy) = (b.x0 + b.width() / 2.0, b.y0 + b.height() / 2.0);
    for x in [b.x0, cx, b.x1] {
        push_unique(&mut out, GuideLine::Vertical { x });
    }
    for y in [b.y0, cy, b.y1] {
        push_unique(&mut out, GuideLine::Horizontal { y });
    }
    let a = b.height().atan2(b.width()).to_degrees();
    push_unique(&mut out, GuideLine::Angled { x: b.x0, y: b.y0, angle: a });
    push_unique(&mut out, GuideLine::Angled { x: b.x0, y: b.y1, angle: -a });
    for p in nodes.iter().filter(|p| p.x.is_finite() && p.y.is_finite()) {
        push_unique(&mut out, GuideLine::Vertical { x: p.x });
        push_unique(&mut out, GuideLine::Horizontal { y: p.y });
    }
    out
}

/// The thirds of `b` in both directions: how a logomark and its logotype
/// are paired and placed.
pub fn lockup(b: Rect) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) {
        return out;
    }
    let (w, h) = (b.width(), b.height());
    for f in [1.0 / 3.0, 2.0 / 3.0] {
        push_unique(&mut out, GuideLine::Vertical { x: b.x0 + w * f });
        push_unique(&mut out, GuideLine::Horizontal { y: b.y0 + h * f });
    }
    out
}

/// Clearspace: the lines `x` outside each edge of `b`, the space no other
/// object should enter round a logo. A negative or NaN `x` gives none.
pub fn clearspace(b: Rect, x: f64) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&b) || !(x.is_finite() && x > 0.0) {
        return out;
    }
    push_unique(&mut out, GuideLine::Vertical { x: b.x0 - x });
    push_unique(&mut out, GuideLine::Vertical { x: b.x1 + x });
    push_unique(&mut out, GuideLine::Horizontal { y: b.y0 - x });
    push_unique(&mut out, GuideLine::Horizontal { y: b.y1 + x });
    out
}

/// The settings of the magazine and poster grid window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGridForm {
    pub columns: u32,
    pub rows: u32,
    /// Space between columns and between rows, mm.
    pub gutter: f64,
    /// Space from each page edge to the grid, mm.
    pub margin: f64,
}

impl Default for PageGridForm {
    fn default() -> Self {
        PageGridForm {
            columns: 3,
            rows: 1,
            gutter: 5.0,
            margin: 10.0,
        }
    }
}

/// Column and row edges of a page: each column and row has its two edges
/// as guidelines. Empty when the margins and gutters leave no room.
pub fn page_grid(page: Rect, form: PageGridForm) -> Vec<GuideLine> {
    let mut out = Vec::new();
    if !finite(&page) || form.columns == 0 || form.rows == 0 {
        return out;
    }
    let (m, g) = (form.margin.max(0.0), form.gutter.max(0.0));
    if !(m.is_finite() && g.is_finite()) {
        return out;
    }
    let inner_w = page.width() - 2.0 * m;
    let inner_h = page.height() - 2.0 * m;
    let cols = form.columns as f64;
    let rows = form.rows as f64;
    let cw = (inner_w - (cols - 1.0) * g) / cols;
    let rh = (inner_h - (rows - 1.0) * g) / rows;
    if cw <= 0.0 || rh <= 0.0 {
        return out;
    }
    for i in 0..form.columns {
        let left = page.x0 + m + i as f64 * (cw + g);
        push_unique(&mut out, GuideLine::Vertical { x: left });
        push_unique(&mut out, GuideLine::Vertical { x: left + cw });
    }
    for j in 0..form.rows {
        let top = page.y0 + m + j as f64 * (rh + g);
        push_unique(&mut out, GuideLine::Horizontal { y: top });
        push_unique(&mut out, GuideLine::Horizontal { y: top + rh });
    }
    out
}

impl crate::app::App {
    /// Write `lines` to the page in one undo step, replacing the lines of
    /// the grid made last (which are taken away first) and skipping any
    /// guideline the page already has. Returns how many were added.
    pub fn apply_grid_lines(&mut self, lines: Vec<GuideLine>, label: &'static str) -> usize {
        let page = self.page;
        let Ok(p) = self.doc().page(page) else {
            return 0;
        };
        let mut guides: Vec<Guide> = p
            .guides
            .iter()
            .filter(|g| !self.grid_lines.iter().any(|l| *l == g.line))
            .cloned()
            .collect();
        let mut added = 0;
        for l in &lines {
            if !guides.iter().any(|g| g.line == *l) {
                guides.push(Guide::from(*l));
                added += 1;
            }
        }
        let cmds = vec![tracedraw_core::Command::SetGuides { page, guides }];
        if let Err(e) = self.engine.run_batch(label, &cmds) {
            self.status = format!("{label}: {e}");
            return 0;
        }
        self.grid_lines = lines;
        added
    }

    /// Take away the grid made last.
    pub fn clear_grid_lines(&mut self) -> usize {
        let n = self.grid_lines.len();
        if n == 0 {
            return 0;
        }
        // The lines are still remembered while they are taken away.
        self.apply_grid_lines(Vec::new(), "Clear Grid");
        n
    }

    /// The bounds and node positions of the selected objects, for the
    /// construction grid. None with nothing selected.
    pub fn selection_geometry(&self) -> Option<(Rect, Vec<Point>)> {
        let bounds = self.selection_bounds()?;
        let mut nodes = Vec::new();
        for s in self.selected_shapes() {
            let path = s.page_path();
            for n in tracedraw_core::nodes::nodes(&path) {
                nodes.push(n.pos);
            }
        }
        Some((bounds, nodes))
    }
}

/// The magazine and poster grid window: columns, rows, gutter and margin,
/// with Apply (which replaces the grid made last) and Clear.
pub fn page_grid_window(app: &mut crate::app::App, ctx: &egui::Context) {
    if !app.show_page_grid {
        return;
    }
    let mut open = true;
    let mut apply = false;
    let mut clear = false;
    crate::ui::chrome::window(crate::i18n::tr("layout_grids.title"))
        .show(ctx, |ui| {
            let f = &mut app.page_grid;
            ui.add(egui::Slider::new(&mut f.columns, 1..=24).text(crate::i18n::tr("layout_grids.columns")));
            ui.add(egui::Slider::new(&mut f.rows, 1..=24).text(crate::i18n::tr("layout_grids.rows")));
            ui.add(egui::Slider::new(&mut f.gutter, 0.0..=50.0).suffix(" mm").text(crate::i18n::tr("layout_grids.gutter")));
            ui.add(egui::Slider::new(&mut f.margin, 0.0..=80.0).suffix(" mm").text(crate::i18n::tr("layout_grids.margin")));
            ui.horizontal_wrapped(|ui| {
                apply = ui.button(crate::i18n::tr("layout_grids.apply")).clicked();
                clear = ui.button(crate::i18n::tr("layout_grids.clear")).clicked();
                if ui.button("Close").clicked() {
                    open = false;
                }
            });
        });
    if apply {
        let page = app.doc().page(app.page).ok().map(|p| {
            tracedraw_core::geometry::Rect::new(0.0, 0.0, p.size.width, p.size.height)
        });
        if let Some(b) = page {
            let lines = page_grid(b, app.page_grid);
            app.apply_grid_lines(lines, "Page Grid");
        }
    }
    if clear {
        app.clear_grid_lines();
    }
    if !open {
        app.show_page_grid = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Rect {
        Rect::new(0.0, 0.0, 210.0, 297.0)
    }

    /// Square cells and the golden section land where they should.
    #[test]
    fn square_and_golden_grids_place_their_lines() {
        let s = square(Rect::new(0.0, 0.0, 30.0, 20.0), 10.0);
        assert!(s.contains(&GuideLine::Vertical { x: 20.0 }));
        assert!(s.contains(&GuideLine::Horizontal { y: 10.0 }));
        assert!(square(page(), 0.0).is_empty());
        assert!(square(page(), f64::NAN).is_empty());
        let g = golden(page());
        assert!(g.iter().any(|l| matches!(l, GuideLine::Vertical { x } if (x - 210.0 * 0.381_966).abs() < 0.01)));
        assert!(g.iter().any(|l| matches!(l, GuideLine::Horizontal { y } if (y - 297.0 * 0.618_034).abs() < 0.01)));
    }

    /// Lockup is the thirds; clearspace sits `x` outside every edge.
    #[test]
    fn lockup_thirds_and_clearspace_distance() {
        let b = Rect::new(10.0, 10.0, 40.0, 25.0);
        let t = lockup(b);
        assert!(t.contains(&GuideLine::Vertical { x: 20.0 }));
        assert!(t.contains(&GuideLine::Horizontal { y: 15.0 }));
        let c = clearspace(b, 5.0);
        assert_eq!(c.len(), 4);
        assert!(c.contains(&GuideLine::Vertical { x: 5.0 }));
        assert!(c.contains(&GuideLine::Vertical { x: 45.0 }));
        assert!(c.contains(&GuideLine::Horizontal { y: 5.0 }));
        assert!(c.contains(&GuideLine::Horizontal { y: 30.0 }));
        assert!(clearspace(b, 0.0).is_empty());
        assert!(clearspace(b, -2.0).is_empty());
    }

    /// Construction has the centres, edges and diagonals, and the nodes'
    /// own lines.
    #[test]
    fn construction_has_centres_edges_and_nodes() {
        let b = Rect::new(0.0, 0.0, 40.0, 20.0);
        let c = construction(b, &[Point::new(7.0, 3.0)]);
        assert!(c.contains(&GuideLine::Vertical { x: 20.0 }));
        assert!(c.contains(&GuideLine::Horizontal { y: 10.0 }));
        assert!(c.contains(&GuideLine::Vertical { x: 7.0 }));
        assert!(c.contains(&GuideLine::Horizontal { y: 3.0 }));
        assert_eq!(c.iter().filter(|l| matches!(l, GuideLine::Angled { .. })).count(), 2);
    }

    /// A magazine grid: 3 columns, 5 mm gutters, 10 mm margins on A4 wide.
    #[test]
    fn page_grid_makes_columns_and_rows_inside_the_margins() {
        let form = PageGridForm {
            columns: 3,
            rows: 1,
            gutter: 5.0,
            margin: 10.0,
        };
        let g = page_grid(page(), form);
        for x in [10.0, 70.0, 75.0, 135.0, 140.0, 200.0] {
            assert!(g.contains(&GuideLine::Vertical { x }), "{x}");
        }
        assert!(g.contains(&GuideLine::Horizontal { y: 10.0 }));
        assert!(g.contains(&GuideLine::Horizontal { y: 287.0 }));
        // Nothing is left for the columns: no lines at all.
        let tight = PageGridForm { columns: 30, gutter: 10.0, ..form };
        assert!(page_grid(page(), tight).is_empty());
        assert!(page_grid(page(), PageGridForm { columns: 0, ..form }).is_empty());
    }

    /// The line count stays under the cap however small the cells.
    #[test]
    fn a_grid_stops_at_the_cap() {
        let s = square(Rect::new(0.0, 0.0, 1000.0, 1000.0), 0.5);
        assert!(s.len() <= MAX_LINES);
    }
}
