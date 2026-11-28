//! Perspective for the picture editor's layer in use: the picture's
//! corners move so that one side gets shorter than the other, sideways
//! (the left or the right side) or up and down (the top or the bottom),
//! as Photoshop's Edit > Transform > Perspective does in one step. It
//! maps every pixel of the picture through a projective transform; the
//! pixels outside the new corners are clear. The Warp tool's grid is a
//! different thing (`pixel_warp`): this is one move of four corners.
//! See docs/behavior/edit-inside-picture.md.

use image::{Rgba, RgbaImage};

/// Which way the perspective narrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Persp {
    /// The left or the right side gets shorter (`amount` below zero makes
    /// the left side the short one).
    #[default]
    Sideways,
    /// The top or the bottom gets narrower (`amount` below zero makes the
    /// bottom the narrow one).
    UpDown,
}

/// Solve the projective map from the picture's corners (0,0), (w,0), (w,h),
/// (0,h) to the four `dst` corners: eight numbers, by Gauss-Jordan.
fn homography(dst: [(f64, f64); 4], w: f64, h: f64) -> Option<[f64; 8]> {
    let src = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    let mut m = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = dst[i];
        let (u, v) = src[i];
        m[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -x * u, -y * u, u];
        m[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -x * v, -y * v, v];
    }
    for c in 0..8 {
        let mut best = c;
        for r in c + 1..8 {
            if m[r][c].abs() > m[best][c].abs() {
                best = r;
            }
        }
        if m[best][c].abs() < 1e-12 {
            return None;
        }
        m.swap(c, best);
        let p = m[c][c];
        for k in c..9 {
            m[c][k] /= p;
        }
        for r in 0..8 {
            if r != c {
                let f = m[r][c];
                if f != 0.0 {
                    for k in c..9 {
                        m[r][k] -= f * m[c][k];
                    }
                }
            }
        }
    }
    let mut out = [0.0; 8];
    for (i, o) in out.iter_mut().enumerate() {
        *o = m[i][8];
    }
    Some(out)
}

/// Nearest-pixel sample of `src` at (x, y) in its pixel units; outside is
/// clear.
fn sample(src: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let (w, h) = src.dimensions();
    if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
        return Rgba([0, 0, 0, 0]);
    }
    *src.get_pixel(x as u32, y as u32)
}

/// The picture with its corners moved to `dst` (top left, top right,
/// bottom right, bottom left, in pixels). None when the corners are
/// degenerate.
pub fn quad(src: &RgbaImage, dst: [(f64, f64); 4]) -> Option<RgbaImage> {
    let (w, h) = src.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let k = homography(dst, w as f64, h as f64)?;
    let mut out = RgbaImage::new(w, h);
    for (x, y, q) in out.enumerate_pixels_mut() {
        let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
        let den = k[6] * cx + k[7] * cy + 1.0;
        if den.abs() < 1e-9 {
            continue;
        }
        let u = (k[0] * cx + k[1] * cy + k[2]) / den;
        let v = (k[3] * cx + k[4] * cy + k[5]) / den;
        *q = sample(src, u, v);
    }
    Some(out)
}

/// The corners for a perspective of `kind` by `amount` (-1 to 1; 0 is the
/// picture as it is). The short side loses `|amount| * 40` per cent of the
/// other dimension at its two ends.
pub fn corners(w: f64, h: f64, kind: Persp, amount: f64) -> [(f64, f64); 4] {
    let a = amount.clamp(-1.0, 1.0);
    let t = a.abs() * 0.4;
    match (kind, a >= 0.0) {
        (Persp::Sideways, true) => {
            // The right side is the long one: the left shrinks top and bottom.
            let d = t * h;
            [(0.0, d), (w, 0.0), (w, h), (0.0, h - d)]
        }
        (Persp::Sideways, false) => {
            let d = t * h;
            [(0.0, 0.0), (w, d), (w, h - d), (0.0, h)]
        }
        (Persp::UpDown, true) => {
            let d = t * w;
            [(d, 0.0), (w - d, 0.0), (w, h), (0.0, h)]
        }
        (Persp::UpDown, false) => {
            let d = t * w;
            [(0.0, 0.0), (w, 0.0), (w - d, h), (d, h)]
        }
    }
}

/// The picture in perspective of `kind` by `amount`; unchanged at 0.
pub fn apply(src: &RgbaImage, kind: Persp, amount: f64) -> RgbaImage {
    if amount.abs() < 1e-9 {
        return src.clone();
    }
    let (w, h) = src.dimensions();
    let dst = corners(w as f64, h as f64, kind, amount);
    quad(src, dst).unwrap_or_else(|| src.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No amount is the picture; a sideways perspective takes the corners
    /// of the short side off, and up and down the narrow end's.
    #[test]
    fn perspective_takes_the_corners_of_the_short_side() {
        let img = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
        assert_eq!(apply(&img, Persp::Sideways, 0.0), img);
        let s = apply(&img, Persp::Sideways, 1.0);
        assert_eq!(s.get_pixel(1, 1)[3], 0, "the short side's top corner");
        assert_eq!(s.get_pixel(98, 1)[3], 255, "the long side stays");
        let v = apply(&img, Persp::UpDown, 1.0);
        assert_eq!(v.get_pixel(1, 98)[3], 0, "the narrow bottom corner");
        assert_eq!(v.get_pixel(98, 98)[3], 0, "the narrow bottom corner, right");
        assert_eq!(v.get_pixel(50, 4)[3], 255, "the top stays");
    }

    /// The corners map back and degenerate ones give nothing, not a panic.
    #[test]
    fn a_degenerate_corner_gives_no_picture() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
        assert!(quad(&img, [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)]).is_none());
        assert!(quad(&RgbaImage::new(0, 0), [(0.0, 0.0); 4]).is_none());
    }
}
