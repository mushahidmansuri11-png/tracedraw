//! Pack Studio (2D): the flat design side of a pack. File > Pack Studio
//! draws a carton's dieline on the page (the page is the trim size, the
//! drawing's bleed is the carton's bleed), with the panel fills and
//! starter text on one printing layer and the cut, fold, trim, bleed,
//! safe and panel-name lines on layers that do not print, and builds a
//! nutrition table for a country as editable curves and text. The
//! Export for print tab opens Export as PDF set to a PDF/X level, with
//! the carton's bleed and printers' marks. See docs/behavior/pack-studio.md.
//!
//! The geometry is worked out in "sheet space": millimetres, origin at
//! the top left of the trim box, y growing down. Items are turned into
//! shapes at the end, which is the only place the page's y axis (up) is
//! met.

use crate::app::App;
use crate::i18n::tr;
use tracedraw_core::document::{Shape, ShapeKind};
use tracedraw_core::geometry::{BezPath, Point, Rect};
use tracedraw_core::{Color, Command, Fill, LayerId, Stroke, TextAlign, TextSpan};

/// Millimetres in a point.
pub const MM_PT: f64 = 25.4 / 72.0;

// ----- drawing items ---------------------------------------------------

/// One thing to draw, in sheet space.
#[derive(Debug, Clone)]
pub enum Item {
    /// A polygon or a polyline.
    Poly {
        pts: Vec<Point>,
        closed: bool,
        fill: Option<[u8; 3]>,
        line: Option<([u8; 3], f64)>,
        /// Dash lengths in multiples of the line width; empty is solid.
        dash: Vec<f64>,
    },
    /// Text on a baseline starting (or centred, or ending) at `at`;
    /// each span is (text, bold).
    Text {
        at: Point,
        spans: Vec<(String, bool)>,
        size_pt: f64,
        align: TextAlign,
        color: [u8; 3],
    },
}

fn rect_poly(r: Rect, fill: Option<[u8; 3]>, line: Option<([u8; 3], f64)>, dash: &[f64]) -> Item {
    Item::Poly {
        pts: vec![
            Point::new(r.x0, r.y0),
            Point::new(r.x1, r.y0),
            Point::new(r.x1, r.y1),
            Point::new(r.x0, r.y1),
        ],
        closed: true,
        fill,
        line,
        dash: dash.to_vec(),
    }
}

fn seg(a: Point, b: Point, color: [u8; 3], width: f64, dash: &[f64]) -> Item {
    Item::Poly {
        pts: vec![a, b],
        closed: false,
        fill: None,
        line: Some((color, width)),
        dash: dash.to_vec(),
    }
}

fn label(at: Point, text: &str, size_pt: f64, bold: bool, align: TextAlign, color: [u8; 3]) -> Item {
    Item::Text {
        at,
        spans: vec![(text.to_string(), bold)],
        size_pt,
        align,
        color,
    }
}

// ----- the carton ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxStyle {
    /// Straight tuck-end carton: a lid with a tuck lip, a bottom and a
    /// dust flap on each side panel.
    TuckEnd,
    /// A sleeve: four panels and a glue flap, open at both ends.
    Sleeve,
    /// A pillow pouch laid flat: the back's left half, the front and the
    /// back's right half side by side. The glue width is the fin seal at
    /// both outer edges, the tuck height the crimp seal at the top and
    /// the bottom; the depth is not used.
    Pouch,
}

impl BoxStyle {
    pub const ALL: [BoxStyle; 3] = [BoxStyle::TuckEnd, BoxStyle::Sleeve, BoxStyle::Pouch];

    pub fn key(self) -> &'static str {
        match self {
            BoxStyle::TuckEnd => "pack.style_tuck",
            BoxStyle::Sleeve => "pack.style_sleeve",
            BoxStyle::Pouch => "pack.style_pouch",
        }
    }
}

/// The carton's size and the margins of its artwork, millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxSpec {
    pub style: BoxStyle,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// Width of the glue flap.
    pub glue: f64,
    /// Height of the tuck lip (tuck-end cartons).
    pub tuck: f64,
    pub bleed: f64,
    pub safe: f64,
}

impl Default for BoxSpec {
    fn default() -> Self {
        BoxSpec {
            style: BoxStyle::TuckEnd,
            width: 100.0,
            height: 150.0,
            depth: 60.0,
            glue: 12.0,
            tuck: 12.0,
            bleed: 3.0,
            safe: 5.0,
        }
    }
}

/// The edges of the dieline: columns `x0 = 0 < xf < x1 < x2 < x3 < x4`
/// (glue, front, side, back, side) and rows `0 <= yt <= yb < ye <= yz`
/// (tuck lip, lid, body, bottom).
#[derive(Debug, Clone, Copy)]
struct Edges {
    xf: f64,
    x1: f64,
    x2: f64,
    x3: f64,
    x4: f64,
    yt: f64,
    yb: f64,
    ye: f64,
    yz: f64,
    /// Height of a dust flap.
    df: f64,
    /// Chamfer of the flaps' corners.
    c: f64,
}

/// One panel of the carton.
#[derive(Debug, Clone)]
pub struct Panel {
    pub name: &'static str,
    pub rect: Rect,
    /// Carries artwork (a face of the carton) rather than a flap.
    pub face: bool,
    /// A side, the lid or the bottom (the darker fill).
    pub minor: bool,
}

impl BoxSpec {
    /// The spec with every size kept to something that can be drawn.
    pub fn sane(&self) -> BoxSpec {
        let c = |v: f64, lo: f64| if v.is_finite() { v.max(lo) } else { lo };
        BoxSpec {
            style: self.style,
            width: c(self.width, 5.0),
            height: c(self.height, 5.0),
            depth: c(self.depth, 5.0),
            glue: c(self.glue, 3.0),
            tuck: c(self.tuck, 3.0),
            bleed: c(self.bleed, 0.0),
            safe: c(self.safe, 0.0),
        }
    }

    fn edges(&self) -> Edges {
        let s = self.sane();
        let xf = s.glue;
        let x1 = xf + s.width;
        let x2 = x1 + s.depth;
        let x3 = x2 + s.width;
        let x4 = x3 + s.depth;
        let (yt, yb, ye, yz) = match s.style {
            BoxStyle::TuckEnd => {
                let yt = s.tuck;
                let yb = yt + s.depth;
                let ye = yb + s.height;
                (yt, yb, ye, ye + s.depth)
            }
            BoxStyle::Sleeve | BoxStyle::Pouch => (0.0, 0.0, s.height, s.height),
        };
        Edges {
            xf,
            x1,
            x2,
            x3,
            x4,
            yt,
            yb,
            ye,
            yz,
            df: (s.depth - 3.0).max(1.0),
            c: 3.0_f64.min(s.depth / 2.0).min(s.width / 2.0),
        }
    }

    /// The trim size: what the page is set to.
    pub fn trim_size(&self) -> (f64, f64) {
        if self.style == BoxStyle::Pouch {
            let s = self.sane();
            return (2.0 * s.width, s.height);
        }
        let e = self.edges();
        (e.x4, e.yz)
    }

    /// The panels, glue flap first.
    pub fn panels(&self) -> Vec<Panel> {
        if self.style == BoxStyle::Pouch {
            let s = self.sane();
            let (w, h) = (s.width, s.height);
            let panel = |name, x0, x1, minor| Panel {
                name,
                rect: Rect::new(x0, 0.0, x1, h),
                face: true,
                minor,
            };
            return vec![
                panel("BACK", 0.0, w / 2.0, true),
                panel("FRONT", w / 2.0, 1.5 * w, false),
                panel("BACK", 1.5 * w, 2.0 * w, true),
            ];
        }
        let e = self.edges();
        let mut out = vec![
            Panel {
                name: "GLUE",
                rect: Rect::new(0.0, e.yb, e.xf, e.ye),
                face: false,
                minor: true,
            },
            Panel {
                name: "FRONT",
                rect: Rect::new(e.xf, e.yb, e.x1, e.ye),
                face: true,
                minor: false,
            },
            Panel {
                name: "SIDE",
                rect: Rect::new(e.x1, e.yb, e.x2, e.ye),
                face: true,
                minor: true,
            },
            Panel {
                name: "BACK",
                rect: Rect::new(e.x2, e.yb, e.x3, e.ye),
                face: true,
                minor: false,
            },
            Panel {
                name: "SIDE",
                rect: Rect::new(e.x3, e.yb, e.x4, e.ye),
                face: true,
                minor: true,
            },
        ];
        if self.style == BoxStyle::TuckEnd {
            out.push(Panel {
                name: "TOP",
                rect: Rect::new(e.xf, e.yt, e.x1, e.yb),
                face: true,
                minor: true,
            });
            out.push(Panel {
                name: "BOTTOM",
                rect: Rect::new(e.xf, e.ye, e.x1, e.yz),
                face: true,
                minor: true,
            });
            out.push(Panel {
                name: "TUCK",
                rect: Rect::new(e.xf, 0.0, e.x1, e.yt),
                face: false,
                minor: true,
            });
        }
        out
    }

    /// The front panel.
    pub fn front(&self) -> Rect {
        if self.style == BoxStyle::Pouch {
            let s = self.sane();
            return Rect::new(s.width / 2.0, 0.0, 1.5 * s.width, s.height);
        }
        let e = self.edges();
        Rect::new(e.xf, e.yb, e.x1, e.ye)
    }

    /// The panel the back's text goes on: the back of a carton, the
    /// left half of a pouch's back.
    pub fn back_panel(&self) -> Rect {
        if self.style == BoxStyle::Pouch {
            let s = self.sane();
            return Rect::new(0.0, 0.0, s.width / 2.0, s.height);
        }
        let e = self.edges();
        Rect::new(e.x2, e.yb, e.x3, e.ye)
    }

    /// A panel's safe area: inset by the safe margin, and for a pouch
    /// by the seals as well.
    pub fn safe_of(&self, r: Rect) -> Rect {
        let s = self.sane();
        if s.style != BoxStyle::Pouch {
            return r.inflate(-s.safe, -s.safe);
        }
        let (tw, th) = s.trim_size();
        let fin_l = if r.x0 <= 1e-9 { s.glue } else { 0.0 };
        let fin_r = if r.x1 >= tw - 1e-9 { s.glue } else { 0.0 };
        Rect::new(
            r.x0 + s.safe + fin_l,
            s.tuck + s.safe,
            r.x1 - s.safe - fin_r,
            th - s.tuck - s.safe,
        )
    }

    /// The seals of a pouch, where nothing may be placed.
    pub fn seal_zones(&self) -> Vec<Rect> {
        if self.style != BoxStyle::Pouch {
            return Vec::new();
        }
        let s = self.sane();
        let (tw, th) = s.trim_size();
        vec![
            Rect::new(0.0, 0.0, s.glue, th),
            Rect::new(tw - s.glue, 0.0, tw, th),
            Rect::new(0.0, 0.0, tw, s.tuck),
            Rect::new(0.0, th - s.tuck, tw, th),
        ]
    }

    /// The cut line: the whole dieline as one closed polygon.
    pub fn outline(&self) -> Vec<Point> {
        if self.style == BoxStyle::Pouch {
            let (tw, th) = self.trim_size();
            return vec![
                Point::new(0.0, 0.0),
                Point::new(tw, 0.0),
                Point::new(tw, th),
                Point::new(0.0, th),
            ];
        }
        let e = self.edges();
        let p = Point::new;
        let cg = (e.xf * 0.5).min(8.0);
        let mut v: Vec<Point> = Vec::new();
        if self.style == BoxStyle::Sleeve {
            v.extend([
                p(e.xf, 0.0),
                p(e.x4, 0.0),
                p(e.x4, e.ye),
                p(e.xf, e.ye),
                p(cg, e.ye),
                p(0.0, e.ye - cg),
                p(0.0, cg),
                p(cg, 0.0),
            ]);
            return v;
        }
        let c = e.c;
        // Tuck lip and lid, top edge, left to right.
        v.extend([
            p(e.xf, e.yt),
            p(e.xf, c),
            p(e.xf + c, 0.0),
            p(e.x1 - c, 0.0),
            p(e.x1, c),
            p(e.x1, e.yb - e.df),
        ]);
        // Dust flap of the left side panel (top), then the back's top
        // edge, then the right side panel's flap.
        for (xa, xb) in [(e.x1, e.x2), (e.x3, e.x4)] {
            v.extend([
                p(xa, e.yb - e.df + c),
                p(xa + c, e.yb - e.df),
                p(xb - c, e.yb - e.df),
                p(xb, e.yb - e.df + c),
                p(xb, e.yb),
            ]);
            if xb == e.x2 {
                v.push(p(e.x3, e.yb));
            }
        }
        // Down the right edge, then the bottom flaps from right to left.
        v.push(p(e.x4, e.ye));
        for (xa, xb) in [(e.x3, e.x4), (e.x1, e.x2)] {
            v.extend([
                p(xb, e.ye + e.df - c),
                p(xb - c, e.ye + e.df),
                p(xa + c, e.ye + e.df),
                p(xa, e.ye + e.df - c),
                p(xa, e.ye),
            ]);
            if xa == e.x3 {
                v.push(p(e.x2, e.ye));
            }
        }
        // The bottom panel, then the glue flap up the left edge.
        v.extend([
            p(e.x1, e.yz),
            p(e.xf, e.yz),
            p(e.xf, e.ye),
            p(cg, e.ye),
            p(0.0, e.ye - cg),
            p(0.0, e.yb + cg),
            p(cg, e.yb),
            p(e.xf, e.yb),
        ]);
        v
    }

    /// The fold and crease lines as segments.
    pub fn folds(&self) -> Vec<(Point, Point)> {
        if self.style == BoxStyle::Pouch {
            let s = self.sane();
            let h = s.height;
            return vec![
                (Point::new(s.width / 2.0, 0.0), Point::new(s.width / 2.0, h)),
                (Point::new(1.5 * s.width, 0.0), Point::new(1.5 * s.width, h)),
            ];
        }
        let e = self.edges();
        let p = Point::new;
        let mut v = vec![
            (p(e.xf, e.yb), p(e.xf, e.ye)),
            (p(e.x1, e.yb), p(e.x1, e.ye)),
            (p(e.x2, e.yb), p(e.x2, e.ye)),
            (p(e.x3, e.yb), p(e.x3, e.ye)),
        ];
        if self.style == BoxStyle::TuckEnd {
            v.extend([
                (p(e.xf, e.yb), p(e.x1, e.yb)),
                (p(e.xf, e.ye), p(e.x1, e.ye)),
                (p(e.xf, e.yt), p(e.x1, e.yt)),
                (p(e.x1, e.yb), p(e.x2, e.yb)),
                (p(e.x3, e.yb), p(e.x4, e.yb)),
                (p(e.x1, e.ye), p(e.x2, e.ye)),
                (p(e.x3, e.ye), p(e.x4, e.ye)),
            ]);
        }
        v
    }
}

// ----- what the dialog sets besides the size ----------------------------

/// A front-of-pack area a country asks for, as fractions of the front
/// panel's safe area: (x, y, width, height), y from the top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FopZone {
    pub label: &'static str,
    pub frac: [f64; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoxOptions {
    pub fill_panels: bool,
    pub main_color: [u8; 3],
    pub minor_color: [u8; 3],
    pub starter_text: bool,
    pub product: String,
    pub tagline: String,
    pub net_weight: String,
    pub show_bleed: bool,
    pub show_trim: bool,
    pub show_safe: bool,
    pub show_names: bool,
    pub show_fop: bool,
}

impl Default for BoxOptions {
    fn default() -> Self {
        BoxOptions {
            fill_panels: true,
            main_color: [0x12, 0x35, 0x5b],
            minor_color: [0x0e, 0x29, 0x47],
            starter_text: true,
            product: "Product name".into(),
            tagline: "Tagline here".into(),
            net_weight: "NET WT. 200 g".into(),
            show_bleed: true,
            show_trim: true,
            show_safe: true,
            show_names: true,
            show_fop: false,
        }
    }
}

const RED: [u8; 3] = [0xff, 0x3b, 0x30];
const BLACK: [u8; 3] = [0, 0, 0];
const WHITE: [u8; 3] = [0xff, 0xff, 0xff];
const BLUE: [u8; 3] = [0x00, 0xb7, 0xff];
const AMBER: [u8; 3] = [0xff, 0xb0, 0x00];
const PINK: [u8; 3] = [0xff, 0x2d, 0xb5];
const MAGENTA: [u8; 3] = [0xe5, 0x00, 0x7d];

/// What a carton is drawn as: the printing artwork, the dieline (cut and
/// fold) and the guides, each for its own layer.
#[derive(Debug, Clone, Default)]
pub struct BoxItems {
    pub artwork: Vec<Item>,
    pub dieline: Vec<Item>,
    pub guides: Vec<Item>,
}

/// A panel's fill grown over the bleed on the edges that lie on the trim.
fn with_bleed(r: Rect, tw: f64, th: f64, b: f64) -> Rect {
    Rect::new(
        if r.x0 <= 1e-9 { r.x0 - b } else { r.x0 },
        if r.y0 <= 1e-9 { r.y0 - b } else { r.y0 },
        if r.x1 >= tw - 1e-9 { r.x1 + b } else { r.x1 },
        if r.y1 >= th - 1e-9 { r.y1 + b } else { r.y1 },
    )
}

fn is_light(c: [u8; 3]) -> bool {
    0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64 > 140.0
}

/// A closed circle-like polygon, for the veg mark's dot.
fn disc(c: Point, r: f64, fill: [u8; 3]) -> Item {
    let pts: Vec<Point> = (0..24)
        .map(|i| {
            let a = i as f64 / 24.0 * std::f64::consts::TAU;
            Point::new(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect();
    Item::Poly {
        pts,
        closed: true,
        fill: Some(fill),
        line: None,
        dash: Vec::new(),
    }
}

/// The veg mark: a green-edged square with a green dot, `side` mm wide.
fn veg_mark(at: Point, side: f64) -> Vec<Item> {
    let green = [0x00, 0x8a, 0x3c];
    vec![
        rect_poly(
            Rect::new(at.x, at.y, at.x + side, at.y + side),
            Some(WHITE),
            Some((green, side * 0.08)),
            &[],
        ),
        disc(Point::new(at.x + side / 2.0, at.y + side / 2.0), side * 0.27, green),
    ]
}

impl BoxSpec {
    /// The artwork's starter text and boxes: the front's name, tagline
    /// and net weight, the sides' and lid's name, and the back's details
    /// (a pouch has them on its two back halves).
    fn starter(&self, o: &BoxOptions, country: Country, ink: [u8; 3]) -> Vec<Item> {
        let s = self.sane();
        let panels = s.panels();
        let mut out: Vec<Item> = Vec::new();
        let ctr = TextAlign::Center;
        let pouch = s.style == BoxStyle::Pouch;
        let f = s.front();
        let safe = s.safe_of(f);
        out.push(label(
            Point::new(f.center().x, f.y0 + f.height() * 0.42),
            &o.product,
            26.0,
            true,
            ctr,
            ink,
        ));
        out.push(label(
            Point::new(f.center().x, f.y0 + f.height() * 0.42 + 9.0),
            &o.tagline,
            11.0,
            false,
            ctr,
            ink,
        ));
        out.push(label(
            Point::new(f.center().x, safe.y1 - 2.0),
            &o.net_weight,
            9.0,
            true,
            ctr,
            ink,
        ));
        if !pouch {
            for pn in panels.iter().filter(|p| p.name == "SIDE" || p.name == "TOP") {
                let size = if pn.name == "TOP" { 14.0 } else { 9.0 };
                out.push(label(pn.rect.center(), &o.product, size, true, ctr, ink));
            }
        }
        // The back: a pouch's left half holds the ingredients, the right
        // half the product details; a carton's back holds both.
        let back = s.safe_of(s.back_panel());
        let details = |x: f64, y: f64| -> Vec<Item> {
            let lines = [
                format!("{}: {}", tr("pack.text_net"), o.net_weight),
                format!("{} ____ ({})", tr("pack.text_mrp"), tr("pack.text_taxes")),
                format!("{}: __________", tr("pack.text_batch")),
                format!("{}: __________", tr("pack.text_mfg")),
                format!("{}: __________", tr("pack.text_use_by")),
            ];
            lines
                .iter()
                .enumerate()
                .map(|(i, l)| label(Point::new(x, y + i as f64 * 4.2), l, 6.5, false, TextAlign::Left, ink))
                .collect()
        };
        let ingredients = |x: f64, y: f64| -> Vec<Item> {
            let lines = [
                format!("{}: ____________", tr("pack.text_ingredients")),
                format!("{}: ____________", tr("pack.text_allergen")),
                format!("{}", tr("pack.text_storage")),
            ];
            lines
                .iter()
                .enumerate()
                .map(|(i, l)| label(Point::new(x, y + i as f64 * 5.0), l, 6.5, i == 0, TextAlign::Left, ink))
                .collect()
        };
        if pouch {
            if back.width() > 8.0 {
                out.extend(ingredients(back.x0 + 2.0, back.y0 + 10.0));
            }
            if let Some(right) = panels.last() {
                let r = s.safe_of(right.rect);
                if r.width() > 8.0 {
                    out.push(label(
                        Point::new(r.center().x, r.y0 + 8.0),
                        &o.product,
                        12.0,
                        true,
                        ctr,
                        ink,
                    ));
                    out.push(label(
                        Point::new(r.center().x, r.y0 + 14.0),
                        &o.tagline,
                        7.0,
                        false,
                        ctr,
                        ink,
                    ));
                    let by = r.y1 - 28.0;
                    out.push(rect_poly(
                        Rect::new(r.x0, by - 4.0, r.x1, r.y1),
                        None,
                        Some((ink, 0.2)),
                        &[],
                    ));
                    out.extend(details(r.x0 + 2.0, by + 1.0));
                    if country == Country::India {
                        out.extend(veg_mark(Point::new(r.x0, by - 14.0), 6.0));
                        out.push(label(
                            Point::new(r.x0 + 8.0, by - 9.5),
                            "100% VEG",
                            6.5,
                            true,
                            TextAlign::Left,
                            ink,
                        ));
                    }
                }
            }
        } else if back.width() > 8.0 {
            out.extend(ingredients(back.x0, back.y0 + 8.0));
            out.extend(details(back.x0, back.y0 + 28.0));
            if country == Country::India {
                out.extend(veg_mark(Point::new(back.x1 - 6.0, back.y0), 6.0));
            }
        }
        out
    }

    /// Everything the carton is drawn with.
    pub fn items(&self, o: &BoxOptions, country: Country) -> BoxItems {
        let s = self.sane();
        let (tw, th) = s.trim_size();
        let panels = s.panels();
        let mut out = BoxItems::default();

        // The printing artwork.
        if o.fill_panels {
            for pn in panels.iter().filter(|p| p.face || p.name == "TUCK") {
                let fill = if pn.minor { o.minor_color } else { o.main_color };
                out.artwork
                    .push(rect_poly(with_bleed(pn.rect, tw, th, s.bleed), Some(fill), None, &[]));
            }
        }
        if o.starter_text {
            // Text on the back halves of a pouch sits on the minor
            // colour, the rest on the main one.
            let ink = if o.fill_panels && !is_light(o.main_color) {
                WHITE
            } else {
                BLACK
            };
            let ink = if s.style == BoxStyle::Pouch && o.fill_panels && is_light(o.minor_color) {
                BLACK
            } else {
                ink
            };
            out.artwork.extend(s.starter(o, country, ink));
        }

        // The dieline.
        out.dieline.push(Item::Poly {
            pts: s.outline(),
            closed: true,
            fill: None,
            line: Some((MAGENTA, 0.35)),
            dash: Vec::new(),
        });
        for (a, b) in s.folds() {
            out.dieline
                .push(seg(a, b, AMBER, 0.33, &[5.0, 2.0, 1.0, 2.0]));
        }

        // The guides.
        if o.show_bleed && s.bleed > 0.0 {
            out.guides.push(rect_poly(
                Rect::new(-s.bleed, -s.bleed, tw + s.bleed, th + s.bleed),
                None,
                Some((RED, 0.25)),
                &[],
            ));
        }
        if o.show_trim {
            out.guides
                .push(rect_poly(Rect::new(0.0, 0.0, tw, th), None, Some((BLACK, 0.25)), &[]));
        }
        if o.show_safe {
            for pn in panels.iter().filter(|p| p.face) {
                let r = s.safe_of(pn.rect);
                if r.width() > 0.0 && r.height() > 0.0 {
                    out.guides
                        .push(rect_poly(r, None, Some((BLUE, 0.25)), &[5.0, 3.0]));
                }
            }
            // The seals of a pouch are no-go zones.
            for z in s.seal_zones() {
                out.guides
                    .push(rect_poly(z, None, Some((AMBER, 0.25)), &[3.0, 3.0]));
            }
        }
        if o.show_names {
            for pn in &panels {
                out.guides.push(label(
                    pn.rect.center(),
                    pn.name,
                    7.0,
                    true,
                    TextAlign::Center,
                    PINK,
                ));
            }
        }
        if o.show_fop {
            if let Some(z) = country.fop_zone() {
                let safe = s.safe_of(s.front());
                let r = Rect::new(
                    safe.x0 + safe.width() * z.frac[0],
                    safe.y0 + safe.height() * z.frac[1],
                    safe.x0 + safe.width() * (z.frac[0] + z.frac[2]),
                    safe.y0 + safe.height() * (z.frac[1] + z.frac[3]),
                );
                out.guides
                    .push(rect_poly(r, None, Some((PINK, 0.3)), &[4.0, 2.0]));
                out.guides.push(label(
                    Point::new(r.x0 + 1.0, r.y0 + 3.5),
                    z.label,
                    6.0,
                    true,
                    TextAlign::Left,
                    PINK,
                ));
            }
        }
        out
    }
}

// ----- countries and nutrition -------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Country {
    India,
    Usa,
    Canada,
    Uk,
    Eu,
    AusNz,
    UaeGcc,
    Mexico,
    Chile,
    Singapore,
}

impl Country {
    pub const ALL: [Country; 10] = [
        Country::India,
        Country::Usa,
        Country::Canada,
        Country::Uk,
        Country::Eu,
        Country::AusNz,
        Country::UaeGcc,
        Country::Mexico,
        Country::Chile,
        Country::Singapore,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Country::India => "pack.country_india",
            Country::Usa => "pack.country_usa",
            Country::Canada => "pack.country_canada",
            Country::Uk => "pack.country_uk",
            Country::Eu => "pack.country_eu",
            Country::AusNz => "pack.country_ausnz",
            Country::UaeGcc => "pack.country_uae",
            Country::Mexico => "pack.country_mexico",
            Country::Chile => "pack.country_chile",
            Country::Singapore => "pack.country_singapore",
        }
    }

    /// The short note on the country's labelling rules.
    pub fn note_key(self) -> &'static str {
        match self {
            Country::India => "pack.note_india",
            Country::Usa => "pack.note_usa",
            Country::Canada => "pack.note_canada",
            Country::Uk => "pack.note_uk",
            Country::Eu => "pack.note_eu",
            Country::AusNz => "pack.note_ausnz",
            Country::UaeGcc => "pack.note_uae",
            Country::Mexico => "pack.note_mexico",
            Country::Chile => "pack.note_chile",
            Country::Singapore => "pack.note_singapore",
        }
    }

    /// Where the country's front-of-pack mark or statement goes, as a
    /// planning guide (not a regulation).
    pub fn fop_zone(self) -> Option<FopZone> {
        let z = |label, x, y, w, h| {
            Some(FopZone {
                label,
                frac: [x, y, w, h],
            })
        };
        match self {
            Country::India => z("VEG / NON-VEG MARK", 0.0, 0.0, 0.2, 0.1),
            Country::Usa => z("NET QUANTITY (BOTTOM 30%)", 0.0, 0.7, 1.0, 0.3),
            Country::Canada => z("HIGH IN SYMBOL", 0.7, 0.0, 0.3, 0.14),
            Country::Uk => z("TRAFFIC LIGHTS (OPTIONAL)", 0.0, 0.8, 1.0, 0.2),
            Country::Eu | Country::UaeGcc => None,
            Country::AusNz => z("HEALTH STAR / ORIGIN", 0.0, 0.82, 0.5, 0.18),
            Country::Mexico | Country::Chile => z("WARNING OCTAGONS", 0.62, 0.0, 0.38, 0.22),
            Country::Singapore => z("NUTRI-GRADE MARK", 0.7, 0.0, 0.3, 0.15),
        }
    }

    fn is_facts_panel(self) -> bool {
        matches!(self, Country::Usa | Country::Canada)
    }
}

/// Amounts for one serving.
#[derive(Debug, Clone, PartialEq)]
pub struct Nutrients {
    pub serving_g: f64,
    pub servings: f64,
    pub energy_kcal: f64,
    pub fat: f64,
    pub sat_fat: f64,
    pub trans_fat: f64,
    pub cholesterol_mg: f64,
    pub sodium_mg: f64,
    pub carbs: f64,
    pub fibre: f64,
    pub sugars: f64,
    pub added_sugars: f64,
    pub protein: f64,
    pub vit_d_mcg: f64,
    pub calcium_mg: f64,
    pub iron_mg: f64,
    pub potassium_mg: f64,
}

impl Default for Nutrients {
    /// Sample amounts, to be replaced by the lab's.
    fn default() -> Self {
        Nutrients {
            serving_g: 25.0,
            servings: 8.0,
            energy_kcal: 120.0,
            fat: 4.5,
            sat_fat: 2.0,
            trans_fat: 0.0,
            cholesterol_mg: 0.0,
            sodium_mg: 95.0,
            carbs: 18.0,
            fibre: 1.0,
            sugars: 6.0,
            added_sugars: 5.0,
            protein: 2.0,
            vit_d_mcg: 0.0,
            calcium_mg: 20.0,
            iron_mg: 0.4,
            potassium_mg: 60.0,
        }
    }
}

/// What the dialog hands to the table maker.
#[derive(Debug, Clone, PartialEq)]
pub struct NutritionSpec {
    pub country: Country,
    pub nut: Nutrients,
    pub width_mm: f64,
    pub font_pt: f64,
    /// A column per serving beside the one per 100 g (tables with
    /// columns only).
    pub both_columns: bool,
}

impl Default for NutritionSpec {
    fn default() -> Self {
        NutritionSpec {
            country: Country::India,
            nut: Nutrients::default(),
            width_mm: 60.0,
            font_pt: 7.0,
            both_columns: false,
        }
    }
}

/// Kilojoules in a kilocalorie.
pub const KJ_PER_KCAL: f64 = 4.184;

/// An amount for a serving scaled to 100 g (or ml).
pub fn per_100(v: f64, serving_g: f64) -> f64 {
    if serving_g > 0.0 {
        v * 100.0 / serving_g
    } else {
        v
    }
}

/// An amount with at most one decimal and no trailing ".0".
pub fn num(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{}", r.round() as i64)
    } else {
        format!("{r:.1}")
    }
}

/// Calories as the FDA rounds them: to 5 up to 50, to 10 above, zero
/// under 5.
pub fn round_calories(v: f64) -> f64 {
    if v < 5.0 {
        0.0
    } else if v <= 50.0 {
        (v / 5.0).round() * 5.0
    } else {
        (v / 10.0).round() * 10.0
    }
}

/// Fats as the FDA rounds them: zero under 0.5 g, to 0.5 g under 5 g,
/// to 1 g above.
pub fn round_fat(v: f64) -> f64 {
    if v < 0.5 {
        0.0
    } else if v < 5.0 {
        (v * 2.0).round() / 2.0
    } else {
        v.round()
    }
}

/// Carbohydrate, fibre, sugars and protein: zero under 0.5 g, else to 1 g.
pub fn round_gram(v: f64) -> f64 {
    if v < 0.5 {
        0.0
    } else {
        v.round()
    }
}

/// Sodium: zero under 5 mg, to 5 mg up to 140, to 10 mg above.
pub fn round_sodium(v: f64) -> f64 {
    if v < 5.0 {
        0.0
    } else if v <= 140.0 {
        (v / 5.0).round() * 5.0
    } else {
        (v / 10.0).round() * 10.0
    }
}

/// Cholesterol: zero under 2 mg, else to 5 mg (5 at least).
pub fn round_cholesterol(v: f64) -> f64 {
    if v < 2.0 {
        0.0
    } else {
        ((v / 5.0).round() * 5.0).max(5.0)
    }
}

/// A share of a daily value, in whole percent.
pub fn pct_dv(v: f64, dv: f64) -> i64 {
    if dv <= 0.0 {
        0
    } else {
        (v / dv * 100.0).round() as i64
    }
}

/// Daily values of the US label (2016 rule).
pub mod us_dv {
    pub const FAT: f64 = 78.0;
    pub const SAT_FAT: f64 = 20.0;
    pub const CHOLESTEROL: f64 = 300.0;
    pub const SODIUM: f64 = 2300.0;
    pub const CARBS: f64 = 275.0;
    pub const FIBRE: f64 = 28.0;
    pub const ADDED_SUGARS: f64 = 50.0;
    pub const VIT_D: f64 = 20.0;
    pub const CALCIUM: f64 = 1300.0;
    pub const IRON: f64 = 18.0;
    pub const POTASSIUM: f64 = 4700.0;
}

/// Daily values of the Canadian label.
pub mod ca_dv {
    pub const FAT: f64 = 75.0;
    pub const SAT_TRANS: f64 = 20.0;
    pub const CARBS: f64 = 275.0;
    pub const FIBRE: f64 = 28.0;
    pub const SUGARS: f64 = 100.0;
    pub const SODIUM: f64 = 2300.0;
    pub const POTASSIUM: f64 = 3400.0;
    pub const CALCIUM: f64 = 1300.0;
    pub const IRON: f64 = 14.0;
}

/// Which basis a column of a table is worked out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basis {
    Per100,
    PerServing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nut {
    Kcal,
    Kj,
    Protein,
    Fat,
    Sat,
    Trans,
    Carbs,
    Sugars,
    Added,
    Fibre,
    Cholesterol,
    Sodium,
    Salt,
}

fn amount(n: &Nutrients, nut: Nut, basis: Basis) -> f64 {
    let raw = match nut {
        Nut::Kcal => n.energy_kcal,
        Nut::Kj => n.energy_kcal * KJ_PER_KCAL,
        Nut::Protein => n.protein,
        Nut::Fat => n.fat,
        Nut::Sat => n.sat_fat,
        Nut::Trans => n.trans_fat,
        Nut::Carbs => n.carbs,
        Nut::Sugars => n.sugars,
        Nut::Added => n.added_sugars,
        Nut::Fibre => n.fibre,
        Nut::Cholesterol => n.cholesterol_mg,
        Nut::Sodium => n.sodium_mg,
        // Salt in grams from sodium in milligrams.
        Nut::Salt => n.sodium_mg * 2.5 / 1000.0,
    };
    match basis {
        Basis::PerServing => raw,
        Basis::Per100 => per_100(raw, n.serving_g),
    }
}

fn cell(n: &Nutrients, nut: Nut, basis: Basis) -> String {
    let v = amount(n, nut, basis);
    match nut {
        Nut::Kcal | Nut::Kj | Nut::Sodium | Nut::Cholesterol => format!("{}", v.round() as i64),
        Nut::Salt => format!("{v:.2}"),
        _ => num(v),
    }
}

/// A row of a table with columns: its label, what to show and how far
/// it is set in.
struct ColRow {
    label: String,
    nut: Nut,
    indent: u8,
}

fn cr(label: &str, nut: Nut, indent: u8) -> ColRow {
    ColRow {
        label: label.to_string(),
        nut,
        indent,
    }
}

/// The table of a country that does not use the facts panel: title,
/// lines under it, the column headings with their bases, and the rows.
struct ColTable {
    title: String,
    lines: Vec<String>,
    cols: Vec<(String, Basis)>,
    rows: Vec<ColRow>,
}

fn col_table(country: Country, n: &Nutrients, both: bool) -> ColTable {
    let g = num(n.serving_g);
    let servings = num(n.servings);
    let per_serve = |s: &str| format!("{s} ({g} g)");
    let mut table = match country {
        Country::India => ColTable {
            title: "NUTRITIONAL INFORMATION".into(),
            lines: vec![
                "(Approx.)".into(),
                format!("Servings per pack: {servings}"),
            ],
            cols: vec![
                ("Per 100 g".into(), Basis::Per100),
                (per_serve("Per serve"), Basis::PerServing),
            ],
            rows: vec![
                cr("Energy (kcal)", Nut::Kcal, 0),
                cr("Protein (g)", Nut::Protein, 0),
                cr("Carbohydrate (g)", Nut::Carbs, 0),
                cr("Total sugars (g)", Nut::Sugars, 1),
                cr("Added sugars (g)", Nut::Added, 1),
                cr("Total fat (g)", Nut::Fat, 0),
                cr("Saturated fat (g)", Nut::Sat, 1),
                cr("Trans fat (g)", Nut::Trans, 1),
                cr("Cholesterol (mg)", Nut::Cholesterol, 0),
                cr("Sodium (mg)", Nut::Sodium, 0),
            ],
        },
        Country::Uk | Country::Eu => ColTable {
            title: if country == Country::Uk {
                "Nutrition".into()
            } else {
                "Nutrition declaration".into()
            },
            lines: vec![if country == Country::Uk {
                "Typical values".into()
            } else {
                format!("Servings per pack: {servings}")
            }],
            cols: vec![
                ("Per 100 g".into(), Basis::Per100),
                (per_serve("Per portion"), Basis::PerServing),
            ],
            rows: vec![
                cr("Energy (kJ)", Nut::Kj, 0),
                cr("Energy (kcal)", Nut::Kcal, 0),
                cr("Fat (g)", Nut::Fat, 0),
                cr("of which saturates (g)", Nut::Sat, 1),
                cr("Carbohydrate (g)", Nut::Carbs, 0),
                cr("of which sugars (g)", Nut::Sugars, 1),
                cr("Fibre (g)", Nut::Fibre, 0),
                cr("Protein (g)", Nut::Protein, 0),
                cr("Salt (g)", Nut::Salt, 0),
            ],
        },
        Country::AusNz => ColTable {
            title: "NUTRITION INFORMATION".into(),
            lines: vec![
                format!("Servings per package: {servings}"),
                format!("Serving size: {g} g"),
            ],
            cols: vec![
                ("Quantity per serving".into(), Basis::PerServing),
                ("Quantity per 100 g".into(), Basis::Per100),
            ],
            rows: vec![
                cr("Energy (kJ)", Nut::Kj, 0),
                cr("Protein (g)", Nut::Protein, 0),
                cr("Fat, total (g)", Nut::Fat, 0),
                cr("saturated (g)", Nut::Sat, 1),
                cr("Carbohydrate (g)", Nut::Carbs, 0),
                cr("sugars (g)", Nut::Sugars, 1),
                cr("Sodium (mg)", Nut::Sodium, 0),
            ],
        },
        Country::UaeGcc => ColTable {
            title: "NUTRITION FACTS".into(),
            lines: vec![format!("Servings per pack: {servings}")],
            cols: vec![
                ("Per 100 g".into(), Basis::Per100),
                (per_serve("Per serving"), Basis::PerServing),
            ],
            rows: vec![
                cr("Energy (kcal)", Nut::Kcal, 0),
                cr("Fat (g)", Nut::Fat, 0),
                cr("saturated fat (g)", Nut::Sat, 1),
                cr("Carbohydrate (g)", Nut::Carbs, 0),
                cr("sugars (g)", Nut::Sugars, 1),
                cr("Protein (g)", Nut::Protein, 0),
                cr("Sodium (mg)", Nut::Sodium, 0),
            ],
        },
        Country::Mexico => ColTable {
            title: "INFORMACIÓN NUTRIMENTAL".into(),
            lines: vec![format!("Porciones por envase: {servings}")],
            cols: vec![
                ("Por 100 g".into(), Basis::Per100),
                (per_serve("Por porción"), Basis::PerServing),
            ],
            rows: vec![
                cr("Contenido energético (kcal)", Nut::Kcal, 0),
                cr("Proteínas (g)", Nut::Protein, 0),
                cr("Grasas totales (g)", Nut::Fat, 0),
                cr("Grasas saturadas (g)", Nut::Sat, 1),
                cr("Grasas trans (g)", Nut::Trans, 1),
                cr("Carbohidratos (g)", Nut::Carbs, 0),
                cr("Azúcares totales (g)", Nut::Sugars, 1),
                cr("Azúcares añadidos (g)", Nut::Added, 1),
                cr("Fibra dietética (g)", Nut::Fibre, 0),
                cr("Sodio (mg)", Nut::Sodium, 0),
            ],
        },
        Country::Chile => ColTable {
            title: "INFORMACIÓN NUTRICIONAL".into(),
            lines: vec![format!("Porciones por envase: {servings}")],
            cols: vec![
                ("100 g".into(), Basis::Per100),
                (per_serve("1 porción"), Basis::PerServing),
            ],
            rows: vec![
                cr("Energía (kcal)", Nut::Kcal, 0),
                cr("Proteínas (g)", Nut::Protein, 0),
                cr("Grasa total (g)", Nut::Fat, 0),
                cr("Grasa saturada (g)", Nut::Sat, 1),
                cr("Hidratos de carbono disp. (g)", Nut::Carbs, 0),
                cr("Azúcares totales (g)", Nut::Sugars, 1),
                cr("Sodio (mg)", Nut::Sodium, 0),
            ],
        },
        // Singapore, and the fall-back for the facts-panel countries.
        _ => ColTable {
            title: "NUTRITION INFORMATION".into(),
            lines: vec![format!("Servings per pack: {servings}")],
            cols: vec![
                (per_serve("Per serving"), Basis::PerServing),
                ("Per 100 g".into(), Basis::Per100),
            ],
            rows: vec![
                cr("Energy (kcal)", Nut::Kcal, 0),
                cr("Protein (g)", Nut::Protein, 0),
                cr("Total fat (g)", Nut::Fat, 0),
                cr("Saturated fat (g)", Nut::Sat, 1),
                cr("Carbohydrate (g)", Nut::Carbs, 0),
                cr("Total sugars (g)", Nut::Sugars, 1),
                cr("Sodium (mg)", Nut::Sodium, 0),
            ],
        },
    };
    // One column, per 100 g, when the table is not asked to carry the
    // serving as well; the lines about servings go with it.
    if !both {
        table.cols.retain(|(_, b)| *b == Basis::Per100);
        table.lines.retain(|l| !l.contains("ervings per") && !l.starts_with("Serving size"));
    }
    table
}

/// A line cursor: items are added top to bottom inside a framed panel.
struct Cur {
    items: Vec<Item>,
    y: f64,
    w: f64,
    pad: f64,
}

impl Cur {
    fn new(w: f64, pad: f64) -> Cur {
        Cur {
            items: Vec::new(),
            y: pad,
            w,
            pad,
        }
    }

    fn left(&self, indent: f64) -> f64 {
        self.pad + indent
    }

    fn right(&self) -> f64 {
        self.w - self.pad
    }

    /// Height of a line of `pt` text.
    fn line_h(pt: f64) -> f64 {
        pt * MM_PT * 1.4
    }

    fn text(&mut self, x: f64, pt: f64, spans: Vec<(String, bool)>, align: TextAlign) {
        self.items.push(Item::Text {
            at: Point::new(x, self.y + pt * MM_PT * 1.05),
            spans,
            size_pt: pt,
            align,
            color: BLACK,
        });
    }

    /// One line with text at the left and (optionally) at the right,
    /// then the cursor moves down.
    fn row(&mut self, pt: f64, left: Vec<(String, bool)>, right: Option<(String, bool)>, indent: f64) {
        if !left.is_empty() {
            self.text(self.left(indent), pt, left, TextAlign::Left);
        }
        if let Some((t, b)) = right {
            self.text(self.right(), pt, vec![(t, b)], TextAlign::Right);
        }
        self.y += Cur::line_h(pt);
    }

    /// A rule across the inner width, then a small gap.
    fn rule(&mut self, width: f64) {
        self.y += 0.3;
        self.items.push(seg(
            Point::new(self.pad, self.y),
            Point::new(self.w - self.pad, self.y),
            BLACK,
            width,
            &[],
        ));
        self.y += 0.4;
    }

    /// A solid bar `h` high across the inner width.
    fn bar(&mut self, h: f64) {
        self.y += 0.2;
        self.items.push(rect_poly(
            Rect::new(self.pad, self.y, self.w - self.pad, self.y + h),
            Some(BLACK),
            None,
            &[],
        ));
        self.y += h + 0.4;
    }

    /// Close the panel: the frame round everything.
    fn finish(mut self) -> (Vec<Item>, f64) {
        let h = self.y + self.pad;
        self.items.insert(
            0,
            rect_poly(
                Rect::new(0.0, 0.0, self.w, h),
                Some(WHITE),
                Some((BLACK, 0.35)),
                &[],
            ),
        );
        (self.items, h)
    }
}

/// The text broken into lines of at most `max` characters.
fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > max {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn plain(s: &str) -> Vec<(String, bool)> {
    vec![(s.to_string(), false)]
}

fn bold_then(b: &str, rest: &str) -> Vec<(String, bool)> {
    vec![(b.to_string(), true), (format!(" {rest}"), false)]
}

/// The table with columns (every country but the US and Canada).
fn column_panel(country: Country, n: &Nutrients, w: f64, f: f64, both: bool) -> (Vec<Item>, f64) {
    let t = col_table(country, n, both);
    let mut c = Cur::new(w, 1.6);
    c.row(f * 1.5, vec![(t.title.clone(), true)], None, 0.0);
    for l in &t.lines {
        c.row(f * 0.95, plain(l), None, 0.0);
    }
    c.bar(0.7);
    // Column heads, right aligned on the columns' right edges.
    let inner = w - 2.0 * c.pad;
    let label_w = inner * if t.cols.len() > 2 { 0.4 } else { 0.46 };
    let cw = (inner - label_w) / t.cols.len().max(1) as f64;
    let edge = |i: usize| c.pad + label_w + (i as f64 + 1.0) * cw;
    let edges: Vec<f64> = (0..t.cols.len()).map(edge).collect();
    for (i, (head, _)) in t.cols.iter().enumerate() {
        let e = edges[i];
        c.text(e, f * 0.9, vec![(head.clone(), true)], TextAlign::Right);
    }
    c.y += Cur::line_h(f * 0.9);
    c.rule(0.25);
    for r in &t.rows {
        let indent = r.indent as f64 * 2.0;
        c.text(c.left(indent), f, plain(&r.label), TextAlign::Left);
        for (i, (_, basis)) in t.cols.iter().enumerate() {
            c.text(edges[i], f, plain(&cell(n, r.nut, *basis)), TextAlign::Right);
        }
        c.y += Cur::line_h(f);
        c.rule(0.1);
    }
    c.finish()
}

/// One row of a facts panel.
struct Fact {
    bold: String,
    rest: String,
    pct: Option<String>,
    indent: f64,
}

fn fact(bold: &str, rest: &str, pct: Option<i64>, indent: f64) -> Fact {
    Fact {
        bold: bold.to_string(),
        rest: rest.to_string(),
        pct: pct.map(|p| format!("{p}%")),
        indent,
    }
}

/// The US Nutrition Facts panel (and the Canadian one) as items.
fn facts_panel(country: Country, n: &Nutrients, w: f64, f: f64) -> (Vec<Item>, f64) {
    let usa = country == Country::Usa;
    let g = |v: f64| format!("{}g", num(v));
    let mg = |v: f64| format!("{}mg", num(v));
    let mcg = |v: f64| format!("{}mcg", num(v));
    let mut c = Cur::new(w, 1.8);
    let kcal = round_calories(n.energy_kcal);
    let serving = format!("{}g", num(n.serving_g));

    let rows: Vec<Fact>;
    let vitamins: Vec<Fact>;
    if usa {
        c.row(f * 2.4, vec![("Nutrition Facts".into(), true)], None, 0.0);
        c.rule(0.12);
        c.row(f, plain(&format!("{} servings per container", num(n.servings))), None, 0.0);
        c.row(
            f,
            vec![("Serving size".into(), true)],
            Some((serving, true)),
            0.0,
        );
        c.bar(f * MM_PT * 0.8);
        c.row(f * 0.85, vec![("Amount per serving".into(), true)], None, 0.0);
        c.row(
            f * 1.9,
            vec![("Calories".into(), true)],
            Some((num(kcal), true)),
            0.0,
        );
        c.bar(f * MM_PT * 0.35);
        c.row(f * 0.85, vec![], Some(("% Daily Value*".into(), true)), 0.0);
        c.rule(0.12);
        let fat = round_fat(n.fat);
        let sat = round_fat(n.sat_fat);
        let trans = round_fat(n.trans_fat);
        let chol = round_cholesterol(n.cholesterol_mg);
        let sod = round_sodium(n.sodium_mg);
        let carb = round_gram(n.carbs);
        let fib = round_gram(n.fibre);
        let sug = round_gram(n.sugars);
        let add = round_gram(n.added_sugars);
        let pro = round_gram(n.protein);
        rows = vec![
            fact("Total Fat", &g(fat), Some(pct_dv(n.fat, us_dv::FAT)), 0.0),
            fact(
                "",
                &format!("Saturated Fat {}", g(sat)),
                Some(pct_dv(n.sat_fat, us_dv::SAT_FAT)),
                2.4,
            ),
            fact("", &format!("Trans Fat {}", g(trans)), None, 2.4),
            fact(
                "Cholesterol",
                &mg(chol),
                Some(pct_dv(n.cholesterol_mg, us_dv::CHOLESTEROL)),
                0.0,
            ),
            fact(
                "Sodium",
                &mg(sod),
                Some(pct_dv(n.sodium_mg, us_dv::SODIUM)),
                0.0,
            ),
            fact(
                "Total Carbohydrate",
                &g(carb),
                Some(pct_dv(n.carbs, us_dv::CARBS)),
                0.0,
            ),
            fact(
                "",
                &format!("Dietary Fiber {}", g(fib)),
                Some(pct_dv(n.fibre, us_dv::FIBRE)),
                2.4,
            ),
            fact("", &format!("Total Sugars {}", g(sug)), None, 2.4),
            fact(
                "",
                &format!("Includes {} Added Sugars", g(add)),
                Some(pct_dv(n.added_sugars, us_dv::ADDED_SUGARS)),
                4.8,
            ),
            fact("Protein", &g(pro), None, 0.0),
        ];
        vitamins = vec![
            fact(
                "",
                &format!("Vitamin D {}", mcg(n.vit_d_mcg)),
                Some(pct_dv(n.vit_d_mcg, us_dv::VIT_D)),
                0.0,
            ),
            fact(
                "",
                &format!("Calcium {}", mg(n.calcium_mg)),
                Some(pct_dv(n.calcium_mg, us_dv::CALCIUM)),
                0.0,
            ),
            fact(
                "",
                &format!("Iron {}", mg(n.iron_mg)),
                Some(pct_dv(n.iron_mg, us_dv::IRON)),
                0.0,
            ),
            fact(
                "",
                &format!("Potassium {}", mg(n.potassium_mg)),
                Some(pct_dv(n.potassium_mg, us_dv::POTASSIUM)),
                0.0,
            ),
        ];
    } else {
        c.row(f * 2.0, vec![("Nutrition Facts".into(), true)], None, 0.0);
        c.row(f * 1.3, vec![("Valeur nutritive".into(), true)], None, 0.0);
        c.rule(0.12);
        c.row(
            f,
            plain(&format!("Per 1 serving ({serving}) / par 1 portion ({serving})")),
            None,
            0.0,
        );
        c.bar(f * MM_PT * 0.8);
        c.row(
            f * 1.7,
            vec![("Calories".into(), true)],
            Some((num(kcal), true)),
            0.0,
        );
        c.bar(f * MM_PT * 0.35);
        c.row(
            f * 0.85,
            vec![],
            Some(("% Daily Value* / % valeur quotidienne*".into(), true)),
            0.0,
        );
        c.rule(0.12);
        let st = n.sat_fat + n.trans_fat;
        rows = vec![
            fact(
                "Fat / Lipides",
                &g(round_fat(n.fat)),
                Some(pct_dv(n.fat, ca_dv::FAT)),
                0.0,
            ),
            fact(
                "",
                &format!("Saturated + Trans / saturés + trans {}", g(round_fat(st))),
                Some(pct_dv(st, ca_dv::SAT_TRANS)),
                2.4,
            ),
            fact(
                "Carbohydrate / Glucides",
                &g(round_gram(n.carbs)),
                Some(pct_dv(n.carbs, ca_dv::CARBS)),
                0.0,
            ),
            fact(
                "",
                &format!("Fibre / Fibres {}", g(round_gram(n.fibre))),
                Some(pct_dv(n.fibre, ca_dv::FIBRE)),
                2.4,
            ),
            fact(
                "",
                &format!("Sugars / Sucres {}", g(round_gram(n.sugars))),
                Some(pct_dv(n.sugars, ca_dv::SUGARS)),
                2.4,
            ),
            fact("Protein / Protéines", &g(round_gram(n.protein)), None, 0.0),
            fact(
                "Cholesterol / Cholestérol",
                &mg(round_cholesterol(n.cholesterol_mg)),
                None,
                0.0,
            ),
            fact(
                "Sodium",
                &mg(round_sodium(n.sodium_mg)),
                Some(pct_dv(n.sodium_mg, ca_dv::SODIUM)),
                0.0,
            ),
        ];
        vitamins = vec![
            fact(
                "",
                &format!("Potassium {}", mg(n.potassium_mg)),
                Some(pct_dv(n.potassium_mg, ca_dv::POTASSIUM)),
                0.0,
            ),
            fact(
                "",
                &format!("Calcium {}", mg(n.calcium_mg)),
                Some(pct_dv(n.calcium_mg, ca_dv::CALCIUM)),
                0.0,
            ),
            fact(
                "",
                &format!("Iron / Fer {}", mg(n.iron_mg)),
                Some(pct_dv(n.iron_mg, ca_dv::IRON)),
                0.0,
            ),
        ];
    }
    let draw = |c: &mut Cur, r: &Fact| {
        let left = if r.bold.is_empty() {
            plain(&r.rest)
        } else {
            bold_then(&r.bold, &r.rest)
        };
        c.row(f, left, r.pct.clone().map(|p| (p, true)), r.indent);
        c.rule(0.1);
    };
    for r in &rows {
        draw(&mut c, r);
    }
    // The thick bar closes the main rows (their last rule is replaced).
    c.bar(f * MM_PT * 0.8);
    for r in &vitamins {
        draw(&mut c, r);
    }
    c.bar(f * MM_PT * 0.35);
    let foot = if usa {
        "* The % Daily Value (DV) tells you how much a nutrient in a serving of food contributes to a daily diet. 2,000 calories a day is used for general nutrition advice."
    } else {
        "* 5% or less is a little, 15% or more is a lot / 5 % ou moins c'est peu, 15 % ou plus c'est beaucoup"
    };
    let max = ((w - 2.0 * c.pad) / (f * 0.8 * MM_PT * 0.5)).floor().max(10.0) as usize;
    for l in wrap(foot, max) {
        c.row(f * 0.8, plain(&l), None, 0.0);
    }
    c.finish()
}

/// The nutrition table for the country: items in the panel's own space
/// (origin top left, y down), its width and its height.
pub fn nutrition_items(spec: &NutritionSpec) -> (Vec<Item>, f64, f64) {
    let w = if spec.width_mm.is_finite() {
        spec.width_mm.clamp(20.0, 400.0)
    } else {
        60.0
    };
    let f = if spec.font_pt.is_finite() {
        spec.font_pt.clamp(3.0, 40.0)
    } else {
        7.0
    };
    let (items, h) = if spec.country.is_facts_panel() {
        facts_panel(spec.country, &spec.nut, w, f)
    } else {
        column_panel(spec.country, &spec.nut, w, f, spec.both_columns)
    };
    (items, w, h)
}

// ----- shapes ------------------------------------------------------------

fn color_of(c: [u8; 3]) -> Color {
    Color::rgb8(c[0], c[1], c[2])
}

/// A shape for an item; `map` takes a point of sheet space to the page.
fn shape_from(app: &mut App, item: &Item, map: &dyn Fn(Point) -> Point) -> Shape {
    let id = app.engine.new_shape_id();
    match item {
        Item::Poly {
            pts,
            closed,
            fill,
            line,
            dash,
        } => {
            let mut p = BezPath::new();
            for (i, q) in pts.iter().enumerate() {
                if i == 0 {
                    p.move_to(map(*q));
                } else {
                    p.line_to(map(*q));
                }
            }
            if *closed {
                p.close_path();
            }
            let mut s = Shape::new(
                id,
                ShapeKind::Path {
                    path: p,
                    closed: *closed,
                },
            );
            s.fill = match fill {
                Some(c) => Fill::Solid(color_of(*c)),
                None => Fill::None,
            };
            s.stroke = line.map(|(c, w)| {
                let mut st = Stroke::new(color_of(c), w);
                st.dash = dash.clone();
                st
            });
            s
        }
        Item::Text {
            at,
            spans,
            size_pt,
            align,
            color,
        } => {
            let font = app.text_font.clone();
            let spans: Vec<TextSpan> = spans
                .iter()
                .map(|(t, bold)| {
                    let mut sp = TextSpan::new(t.clone(), font.clone(), *size_pt);
                    sp.bold = *bold;
                    sp
                })
                .collect();
            let mut s = Shape::new(
                id,
                ShapeKind::Text {
                    spans,
                    origin: map(*at),
                    frame: None,
                    align: *align,
                    para: Default::default(),
                    on_path: None,
                },
            );
            s.fill = Fill::Solid(color_of(*color));
            s.stroke = None;
            s
        }
    }
}

/// A group of the items, named.
fn group_of(
    app: &mut App,
    items: &[Item],
    map: &dyn Fn(Point) -> Point,
    name: String,
) -> Shape {
    let children: Vec<Shape> = items.iter().map(|i| shape_from(app, i, map)).collect();
    let id = app.engine.new_shape_id();
    let mut g = Shape::new(id, ShapeKind::Group { children });
    g.name = Some(name);
    g
}

impl App {
    /// Pack Studio > Create box: the page takes the carton's trim size,
    /// the drawing's bleed the carton's, and the carton's layers are
    /// made (the ones of an earlier box go). False with a status message
    /// when it cannot be done.
    pub fn apply_pack_box(&mut self, spec: &BoxSpec, opts: &BoxOptions, country: Country) -> bool {
        let spec = spec.sane();
        let (tw, th) = spec.trim_size();
        let page = self.page;
        let items = spec.items(opts, country);
        let has_art = !items.artwork.is_empty();
        let names = [
            tr("pack.layer_artwork"),
            tr("pack.layer_dieline"),
            tr("pack.layer_guides"),
        ];
        let old: Vec<LayerId> = self
            .doc()
            .page(page)
            .map(|p| {
                p.layers
                    .iter()
                    .filter(|l| names.contains(&l.name))
                    .map(|l| l.id)
                    .collect()
            })
            .unwrap_or_default();
        let mut cmds: Vec<Command> = old
            .into_iter()
            .map(|layer| Command::DeleteLayer { layer })
            .collect();
        cmds.push(Command::ResizePage {
            page,
            size: tracedraw_core::Size::new(tw, th),
        });
        let mut md = self.doc().metadata.clone();
        md.bleed = spec.bleed;
        cmds.push(Command::SetMetadata { metadata: md });
        let mut added = 0;
        for (i, name) in names.iter().enumerate() {
            if i == 0 && !has_art {
                continue;
            }
            cmds.push(Command::AddLayer {
                page,
                name: name.clone(),
            });
            added += 1;
        }
        if let Err(e) = self.engine.run_batch("Pack Studio", &cmds) {
            self.status = format!("Pack Studio: {e}");
            return false;
        }
        let mut ids: Vec<LayerId> = self
            .doc()
            .page(page)
            .ok()
            .map(|p| p.layers.iter().rev().take(added).map(|l| l.id).collect())
            .unwrap_or_default();
        ids.reverse();
        if ids.len() != added {
            return false;
        }
        let mut it = ids.into_iter();
        let art = if has_art { it.next() } else { None };
        let (Some(die), Some(guides)) = (it.next(), it.next()) else {
            return false;
        };
        let map = move |p: Point| Point::new(p.x, th - p.y);
        let mut cmds: Vec<Command> = vec![
            Command::SetLayerPrintable {
                layer: die,
                printable: false,
            },
            Command::SetLayerPrintable {
                layer: guides,
                printable: false,
            },
        ];
        let groups = [
            (art, &items.artwork, "pack.layer_artwork"),
            (Some(die), &items.dieline, "pack.layer_dieline"),
            (Some(guides), &items.guides, "pack.layer_guides"),
        ];
        for (layer, list, name) in groups {
            let Some(layer) = layer else { continue };
            for item in list.iter() {
                let shape = shape_from(self, item, &map);
                cmds.push(Command::AddShape { layer, shape });
            }
            let _ = name;
        }
        if let Err(e) = self.engine.run_batch("Pack Studio", &cmds) {
            self.status = format!("Pack Studio: {e}");
            return false;
        }
        self.selection.clear();
        self.raster.borrow_mut().invalidate();
        self.zoom_to_page();
        self.status = format!("{}: {} x {} mm", tr("pack.status_box"), num(tw), num(th));
        true
    }

    /// Pack Studio > Insert nutrition table: the table for the country
    /// as one group on a panel (the middle of the page without one),
    /// selected, in one undo step.
    pub fn insert_nutrition(&mut self, spec: &NutritionSpec, on: Option<(Rect, f64)>) -> bool {
        let Some(layer) = self.active_layer() else {
            return false;
        };
        let (items, w, h) = nutrition_items(spec);
        let page = self.page_rect();
        // On a panel (its rectangle in sheet space and the trim height),
        // else in the middle of the page.
        let (cx, cy) = match on {
            Some((r, th)) => (r.center().x, th - r.center().y),
            None => (page.width() / 2.0, page.height() / 2.0),
        };
        let ox = cx - w / 2.0;
        let oy = cy - h / 2.0;
        let map = move |p: Point| Point::new(ox + p.x, oy + h - p.y);
        let name = format!("{} ({})", tr("pack.nutrition_group"), tr(spec.country.key()));
        let group = group_of(self, &items, &map, name);
        let id = group.id;
        if let Err(e) = self.engine.run_batch(
            "Nutrition table",
            &[Command::AddShape {
                layer,
                shape: group,
            }],
        ) {
            self.status = format!("Pack Studio: {e}");
            return false;
        }
        self.selection = vec![id];
        self.status = tr("pack.status_nutrition");
        true
    }
}

// ----- the dialog's state ------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Box,
    Nutrition,
    Print,
}

/// File > Pack Studio: what the three tabs hold. The country belongs to
/// the nutrition spec and is the one the box's guides use as well.
#[derive(Debug, Clone, PartialEq)]
pub struct PackStudioState {
    pub tab: Tab,
    pub spec: BoxSpec,
    pub opts: BoxOptions,
    pub nutrition: NutritionSpec,
    /// Insert the nutrition table on the box's back panel instead of
    /// the middle of the page.
    pub on_back_panel: bool,
    pub print: PrintChoice,
}

impl Default for PackStudioState {
    fn default() -> Self {
        PackStudioState {
            tab: Tab::Box,
            spec: BoxSpec::default(),
            opts: BoxOptions::default(),
            nutrition: NutritionSpec::default(),
            on_back_panel: true,
            print: PrintChoice::default(),
        }
    }
}

impl PackStudioState {
    /// The nutrition table's target: the back panel of the box and the
    /// trim height, when the table goes on the panel.
    pub fn table_target(&self) -> Option<(Rect, f64)> {
        if self.on_back_panel {
            Some((self.spec.back_panel(), self.spec.trim_size().1))
        } else {
            None
        }
    }
}

// ----- export for print --------------------------------------------------

/// Which of the box's layers the PDF holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportContent {
    /// The printing layer only.
    ArtworkOnly,
    /// The artwork with the cut and fold lines.
    WithCutlines,
    /// The artwork with the cut and fold lines and the guides.
    WithCutAndGuides,
    /// The cut and fold lines alone, for the die maker.
    DielineOnly,
}

impl ExportContent {
    pub const ALL: [ExportContent; 4] = [
        ExportContent::ArtworkOnly,
        ExportContent::WithCutlines,
        ExportContent::WithCutAndGuides,
        ExportContent::DielineOnly,
    ];

    pub fn key(self) -> &'static str {
        match self {
            ExportContent::ArtworkOnly => "pack.content_art",
            ExportContent::WithCutlines => "pack.content_cut",
            ExportContent::WithCutAndGuides => "pack.content_guides",
            ExportContent::DielineOnly => "pack.content_die",
        }
    }

    /// Whether the artwork, dieline and guides layers print.
    pub fn printable(self) -> [bool; 3] {
        match self {
            ExportContent::ArtworkOnly => [true, false, false],
            ExportContent::WithCutlines => [true, true, false],
            ExportContent::WithCutAndGuides => [true, true, true],
            ExportContent::DielineOnly => [false, true, false],
        }
    }
}

/// What Export for print sets in the Export as PDF dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintChoice {
    pub content: ExportContent,
    /// Index in `PDF_STANDARDS`: 1 X-1a, 2 X-3, 3 X-4.
    pub standard: usize,
    pub crop: bool,
    pub bleed_marks: bool,
    pub registration: bool,
    pub color_bars: bool,
    pub file_info: bool,
}

impl Default for PrintChoice {
    fn default() -> Self {
        PrintChoice {
            content: ExportContent::ArtworkOnly,
            standard: 3,
            crop: true,
            bleed_marks: false,
            registration: true,
            color_bars: true,
            file_info: true,
        }
    }
}

impl App {
    /// Makes the box's layers print or not as the content asks (one undo
    /// step); a page without a box is left as it is.
    fn set_pack_export_layers(&mut self, content: ExportContent) {
        let page = self.page;
        let names = [
            tr("pack.layer_artwork"),
            tr("pack.layer_dieline"),
            tr("pack.layer_guides"),
        ];
        let want = content.printable();
        let cmds: Vec<Command> = self
            .doc()
            .page(page)
            .map(|p| {
                p.layers
                    .iter()
                    .filter_map(|l| {
                        let i = names.iter().position(|n| *n == l.name)?;
                        (l.printable != want[i]).then(|| Command::SetLayerPrintable {
                            layer: l.id,
                            printable: want[i],
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !cmds.is_empty() {
            let _ = self.engine.run_batch("Pack Studio", &cmds);
        }
    }

    /// Pack Studio > Export for print: Export as PDF, opened with a
    /// PDF/X level, the carton's bleed and the printers' marks chosen.
    pub fn export_for_print(&mut self, bleed_mm: f64, c: &PrintChoice) {
        self.set_pack_export_layers(c.content);
        let mut st = crate::ui::publish_pdf::PublishState::new(self);
        st.prefs.standard = c.standard.clamp(1, 3);
        st.prefs.bookmarks = false;
        st.bleed_mm = if bleed_mm.is_finite() {
            bleed_mm.max(0.0)
        } else {
            3.0
        };
        st.prefs.marks = crate::ui::publish_pdf::MarkPrefs {
            crop: c.crop,
            bleed: c.bleed_marks,
            registration: c.registration,
            color_bars: c.color_bars,
            densitometer: false,
            file_info: c.file_info,
        };
        self.dialog = crate::ui::dialogs::Dialog::PublishPdf(Box::new(st));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether two rectangles share area (touching edges do not).
    fn overlap(a: Rect, b: Rect) -> bool {
        a.x0.max(b.x0) < a.x1.min(b.x1) - 1e-9 && a.y0.max(b.y0) < a.y1.min(b.y1) - 1e-9
    }

    fn polygon_area(v: &[Point]) -> f64 {
        let mut a = 0.0;
        for i in 0..v.len() {
            let (p, q) = (v[i], v[(i + 1) % v.len()]);
            a += p.x * q.y - q.x * p.y;
        }
        a / 2.0
    }

    #[test]
    fn the_default_carton_has_the_reference_trim_size() {
        // 100 x 150 x 60 with a 12 mm glue flap and tuck lip: the sheet
        // the reference package was made for, 332 x 282 mm.
        let (w, h) = BoxSpec::default().trim_size();
        assert!((w - 332.0).abs() < 1e-9 && (h - 282.0).abs() < 1e-9, "{w} {h}");
        let s = BoxSpec {
            style: BoxStyle::Sleeve,
            ..Default::default()
        };
        assert_eq!(s.trim_size(), (332.0, 150.0));
    }

    #[test]
    fn the_cut_line_closes_inside_the_trim_box() {
        for style in BoxStyle::ALL {
            let s = BoxSpec {
                style,
                ..Default::default()
            };
            let (tw, th) = s.trim_size();
            let v = s.outline();
            assert!(v.len() >= if style == BoxStyle::Pouch { 4 } else { 8 }, "{style:?}");
            for p in &v {
                assert!(
                    p.x >= -1e-9 && p.x <= tw + 1e-9 && p.y >= -1e-9 && p.y <= th + 1e-9,
                    "{style:?} {p:?}"
                );
            }
            // A simple polygon never has zero area, and it covers the
            // five body panels at least.
            let body = if style == BoxStyle::Pouch {
                2.0 * s.width * s.height - 1.0
            } else {
                2.0 * (s.width + s.depth) * s.height
            };
            assert!(polygon_area(&v).abs() > body, "{style:?}");
        }
    }

    #[test]
    fn panels_lie_inside_the_trim_box_and_do_not_overlap() {
        let s = BoxSpec::default();
        let (tw, th) = s.trim_size();
        let panels = s.panels();
        for p in &panels {
            assert!(
                p.rect.x0 >= 0.0 && p.rect.y0 >= 0.0 && p.rect.x1 <= tw && p.rect.y1 <= th,
                "{}",
                p.name
            );
        }
        for (i, a) in panels.iter().enumerate() {
            for b in panels.iter().skip(i + 1) {
                assert!(!overlap(a.rect, b.rect), "{} {}", a.name, b.name);
            }
        }
    }

    #[test]
    fn a_sleeve_has_no_flaps_and_a_tuck_end_carton_has_creases_for_them() {
        let sleeve = BoxSpec {
            style: BoxStyle::Sleeve,
            ..Default::default()
        };
        assert_eq!(sleeve.panels().len(), 5);
        assert_eq!(sleeve.folds().len(), 4);
        let tuck = BoxSpec::default();
        assert_eq!(tuck.panels().len(), 8);
        assert_eq!(tuck.folds().len(), 11);
    }

    #[test]
    fn sizes_that_cannot_be_drawn_are_kept_drawable() {
        let s = BoxSpec {
            width: f64::NAN,
            height: -4.0,
            depth: 0.0,
            glue: -1.0,
            tuck: f64::INFINITY,
            bleed: -2.0,
            safe: -2.0,
            ..Default::default()
        }
        .sane();
        assert!(s.width >= 5.0 && s.height >= 5.0 && s.depth >= 5.0);
        assert!(s.bleed >= 0.0 && s.safe >= 0.0);
        let (w, h) = s.trim_size();
        assert!(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0);
    }

    #[test]
    fn artwork_runs_over_the_bleed_on_the_trim_edges_only() {
        let s = BoxSpec::default();
        let (tw, _) = s.trim_size();
        let items = s.items(&BoxOptions::default(), Country::India);
        let glue_side = items.artwork.iter().any(|i| match i {
            Item::Poly { pts, fill: Some(_), .. } => pts.iter().any(|p| (p.x - (tw + 3.0)).abs() < 1e-9),
            _ => false,
        });
        assert!(glue_side, "the right side panel reaches the bleed");
        // The front panel's left edge is a fold, not the trim.
        let front = s.front();
        let covered = items.artwork.iter().any(|i| match i {
            Item::Poly { pts, fill: Some(_), .. } => {
                pts.iter().any(|p| (p.x - (front.x0 - 3.0)).abs() < 1e-9)
            }
            _ => false,
        });
        assert!(!covered);
    }

    #[test]
    fn the_guides_hold_what_is_switched_on() {
        let s = BoxSpec::default();
        let mut o = BoxOptions {
            show_bleed: false,
            show_trim: false,
            show_safe: false,
            show_names: false,
            show_fop: false,
            ..Default::default()
        };
        assert!(s.items(&o, Country::Usa).guides.is_empty());
        o.show_fop = true;
        let with_zone = s.items(&o, Country::Usa).guides.len();
        assert_eq!(with_zone, 2, "the zone and its label");
        // The EU asks for no front-of-pack zone.
        assert!(s.items(&o, Country::Eu).guides.is_empty());
    }

    #[test]
    fn the_fda_rounds_amounts_its_way() {
        assert_eq!(round_calories(3.0), 0.0);
        assert_eq!(round_calories(47.0), 45.0);
        assert_eq!(round_calories(123.0), 120.0);
        assert_eq!(round_fat(0.3), 0.0);
        assert_eq!(round_fat(2.2), 2.0);
        assert_eq!(round_fat(7.4), 7.0);
        assert_eq!(round_sodium(3.0), 0.0);
        assert_eq!(round_sodium(97.0), 95.0);
        assert_eq!(round_sodium(233.0), 230.0);
        assert_eq!(round_cholesterol(1.0), 0.0);
        assert_eq!(round_cholesterol(3.0), 5.0);
        assert_eq!(round_gram(0.4), 0.0);
        assert_eq!(round_gram(17.6), 18.0);
    }

    #[test]
    fn daily_values_are_shares_of_the_reference_amounts() {
        assert_eq!(pct_dv(230.0, us_dv::SODIUM), 10);
        assert_eq!(pct_dv(5.0, us_dv::SAT_FAT), 25);
        assert_eq!(pct_dv(78.0, us_dv::FAT), 100);
        assert_eq!(pct_dv(10.0, 0.0), 0);
    }

    #[test]
    fn per_100_g_scales_the_serving_and_energy_converts() {
        let n = Nutrients {
            serving_g: 25.0,
            energy_kcal: 100.0,
            sodium_mg: 100.0,
            ..Default::default()
        };
        assert_eq!(cell(&n, Nut::Kcal, Basis::Per100), "400");
        assert_eq!(cell(&n, Nut::Kcal, Basis::PerServing), "100");
        assert_eq!(cell(&n, Nut::Kj, Basis::PerServing), "418");
        // 100 mg of sodium is 0.25 g of salt.
        assert_eq!(cell(&n, Nut::Salt, Basis::PerServing), "0.25");
        assert_eq!(num(4.0), "4");
        assert_eq!(num(4.25), "4.3");
        assert_eq!(per_100(5.0, 0.0), 5.0);
    }

    #[test]
    fn every_country_makes_a_table_that_stays_in_its_width() {
        for country in Country::ALL {
            let spec = NutritionSpec {
                country,
                ..Default::default()
            };
            let (items, w, h) = nutrition_items(&spec);
            assert!(h > 10.0 && w == 60.0, "{country:?} {h}");
            assert!(items.len() > 10, "{country:?}");
            for i in &items {
                if let Item::Poly { pts, .. } = i {
                    for p in pts {
                        assert!(
                            p.x >= -1e-9 && p.x <= w + 1e-9 && p.y >= -1e-9 && p.y <= h + 1e-9,
                            "{country:?} {p:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_us_panel_shows_calories_and_daily_values() {
        let spec = NutritionSpec {
            country: Country::Usa,
            ..Default::default()
        };
        let (items, _, _) = nutrition_items(&spec);
        let text: Vec<String> = items
            .iter()
            .filter_map(|i| match i {
                Item::Text { spans, .. } => Some(spans.iter().map(|s| s.0.as_str()).collect::<String>()),
                _ => None,
            })
            .collect();
        assert!(text.iter().any(|t| t == "Nutrition Facts"));
        assert!(text.iter().any(|t| t.starts_with("8 servings per container")));
        assert!(text.iter().any(|t| t == "120"), "calories rounded to 120");
        // 95 mg of sodium is 4% of 2300 mg.
        assert!(text.iter().any(|t| t == "4%"));
    }

    #[test]
    fn a_pouch_is_two_back_halves_round_a_front_with_seals_kept_clear() {
        let s = BoxSpec {
            style: BoxStyle::Pouch,
            width: 120.0,
            height: 200.0,
            glue: 10.0,
            tuck: 8.0,
            ..Default::default()
        };
        assert_eq!(s.trim_size(), (240.0, 200.0));
        let panels = s.panels();
        assert_eq!(panels.len(), 3);
        assert_eq!(panels[1].name, "FRONT");
        assert!((panels[1].rect.width() - 120.0).abs() < 1e-9);
        assert!((panels[0].rect.width() - 60.0).abs() < 1e-9);
        for (i, a) in panels.iter().enumerate() {
            for b in panels.iter().skip(i + 1) {
                assert!(!overlap(a.rect, b.rect));
            }
        }
        assert_eq!(s.folds().len(), 2);
        // The left half's safe area keeps off the fin seal and the crimps.
        let r = s.safe_of(s.back_panel());
        assert!((r.x0 - 15.0).abs() < 1e-9 && (r.x1 - 55.0).abs() < 1e-9, "{r:?}");
        assert!((r.y0 - 13.0).abs() < 1e-9 && (r.y1 - 187.0).abs() < 1e-9, "{r:?}");
        // The front's only has the crimps and the margin.
        let f = s.safe_of(s.front());
        assert!((f.x0 - 65.0).abs() < 1e-9 && (f.x1 - 175.0).abs() < 1e-9, "{f:?}");
        assert_eq!(s.seal_zones().len(), 4);
        assert!(BoxSpec::default().seal_zones().is_empty());
    }

    #[test]
    fn the_india_pouch_gets_a_veg_mark_and_the_table_follows_the_reference() {
        let s = BoxSpec {
            style: BoxStyle::Pouch,
            width: 120.0,
            height: 200.0,
            ..Default::default()
        };
        let green = |items: &BoxItems| {
            items.artwork.iter().any(|i| match i {
                Item::Poly { line: Some((c, _)), .. } => *c == [0x00, 0x8a, 0x3c],
                _ => false,
            })
        };
        assert!(green(&s.items(&BoxOptions::default(), Country::India)));
        assert!(!green(&s.items(&BoxOptions::default(), Country::Usa)));
        // One column per 100 g, with cholesterol, as the reference
        // design's table; the serving column is a choice.
        let texts = |both: bool| -> Vec<String> {
            let spec = NutritionSpec {
                both_columns: both,
                ..Default::default()
            };
            nutrition_items(&spec)
                .0
                .iter()
                .filter_map(|i| match i {
                    Item::Text { spans, .. } => Some(spans.iter().map(|s| s.0.clone()).collect()),
                    _ => None,
                })
                .collect()
        };
        let one = texts(false);
        assert!(one.iter().any(|t| t == "NUTRITIONAL INFORMATION"));
        assert!(one.iter().any(|t| t == "Cholesterol (mg)"));
        assert!(one.iter().any(|t| t == "Per 100 g"));
        assert!(!one.iter().any(|t| t.starts_with("Per serve")));
        assert!(texts(true).iter().any(|t| t.starts_with("Per serve")));
    }

    #[test]
    fn a_box_is_made_in_layers_and_goes_in_one_undo_step() {
        let mut app = crate::app::App::headless();
        let spec = BoxSpec::default();
        let layers_before = app.doc().pages[0].layers.len();
        assert!(app.apply_pack_box(&spec, &BoxOptions::default(), Country::India));
        let size = app.page_size();
        assert!((size.width - 332.0).abs() < 1e-9 && (size.height - 282.0).abs() < 1e-9);
        assert!((app.doc().metadata.bleed - 3.0).abs() < 1e-9);
        let layers = &app.doc().pages[0].layers;
        assert_eq!(layers.len(), layers_before + 3);
        let n = layers.len();
        assert!(layers[n - 3].printable, "the artwork prints");
        assert!(!layers[n - 2].printable, "the dieline does not");
        assert!(!layers[n - 1].printable, "the guides do not");
        // Made again, the old layers go: still three.
        assert!(app.apply_pack_box(&spec, &BoxOptions::default(), Country::India));
        assert_eq!(app.doc().pages[0].layers.len(), layers_before + 3);
    }

    #[test]
    fn a_nutrition_table_is_one_selected_group() {
        let mut app = crate::app::App::headless();
        let depth = app.engine.history_labels().0.len();
        assert!(app.insert_nutrition(&NutritionSpec::default(), None));
        assert_eq!(app.engine.history_labels().0.len(), depth + 1);
        assert_eq!(app.selection.len(), 1);
        let g = app.doc().find_shape(app.selection[0]).expect("group").clone();
        let ShapeKind::Group { children } = &g.kind else {
            panic!("a group")
        };
        assert!(children.len() > 10);
    }

    #[test]
    fn a_table_can_be_put_on_a_panel_of_the_box() {
        let mut app = crate::app::App::headless();
        let spec = BoxSpec::default();
        assert!(app.apply_pack_box(&spec, &BoxOptions::default(), Country::India));
        let back = spec.back_panel();
        let (_, th) = spec.trim_size();
        assert!(app.insert_nutrition(&NutritionSpec::default(), Some((back, th))));
        let b = app
            .doc()
            .find_shape(app.selection[0])
            .expect("group")
            .page_path()
            .bounding_box();
        // The sheet's y runs down, the page's up.
        let centre_y = th - back.center().y;
        assert!((b.center().x - back.center().x).abs() < 1.0, "{b:?}");
        assert!((b.center().y - centre_y).abs() < 1.0, "{b:?}");
    }

    #[test]
    fn the_dialog_starts_on_the_box_with_the_table_going_on_the_back() {
        let st = PackStudioState::default();
        assert_eq!(st.tab, Tab::Box);
        let (r, th) = st.table_target().expect("a target");
        assert_eq!(r, st.spec.back_panel());
        assert_eq!(th, st.spec.trim_size().1);
        let off = PackStudioState {
            on_back_panel: false,
            ..Default::default()
        };
        assert!(off.table_target().is_none());
    }

    #[test]
    fn export_for_print_makes_the_chosen_layers_print() {
        let mut app = crate::app::App::headless();
        assert!(app.apply_pack_box(&BoxSpec::default(), &BoxOptions::default(), Country::India));
        let state = |app: &crate::app::App| -> Vec<bool> {
            let l = &app.doc().pages[0].layers;
            l[l.len() - 3..].iter().map(|l| l.printable).collect()
        };
        assert_eq!(state(&app), vec![true, false, false]);
        for content in ExportContent::ALL {
            app.export_for_print(
                3.0,
                &PrintChoice {
                    content,
                    ..Default::default()
                },
            );
            assert_eq!(state(&app), content.printable().to_vec(), "{content:?}");
        }
    }

    #[test]
    fn export_for_print_opens_the_pdf_dialog_with_the_level_and_marks() {
        let mut app = crate::app::App::headless();
        app.export_for_print(
            3.0,
            &PrintChoice {
                standard: 1,
                ..Default::default()
            },
        );
        let crate::ui::dialogs::Dialog::PublishPdf(p) = &app.dialog else {
            panic!("the Export as PDF dialog")
        };
        assert_eq!(p.prefs.standard, 1);
        assert!((p.bleed_mm - 3.0).abs() < 1e-9);
        assert!(p.prefs.marks.crop && p.prefs.marks.registration);
    }
}
