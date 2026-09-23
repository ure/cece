//! Pixel-art helpers for the toys and beds. They are drawn in code on the
//! same grid as the cat's 128px sprites: each cell asks a shape function what
//! colour it is, and shading comes in a few flat bands lit from the top left.

use crate::render::Canvas;

/// Paints cells around an origin. Coordinates given to it are in classic
/// sprite pixels (`u` buffer pixels each) relative to that origin.
pub struct Painter<'a, 'b> {
    pub canvas: &'a mut Canvas<'b>,
    pub origin: (f64, f64),
    pub u: f64,
    pub alpha: f64,
}

impl Painter<'_, '_> {
    fn cell(&self) -> f64 {
        (self.u / 4.0).round().max(1.0)
    }

    /// Ask `f` for the colour of every cell whose centre lies in the box
    /// (`x0`, `y0`)..(`x1`, `y1`).
    pub fn fill(&mut self, (x0, y0): (f64, f64), (x1, y1): (f64, f64), f: impl Fn(f64, f64) -> Option<u32>) {
        let (c, u) = (self.cell(), self.u);
        let (ox, oy) = self.origin;
        let span = |o: f64, a: f64, b: f64| ((o + a * u) / c).floor() as i64..((o + b * u) / c).ceil() as i64;
        let n = c as i64;
        for cy in span(oy, y0, y1) {
            for cx in span(ox, x0, x1) {
                let x = ((cx as f64 + 0.5) * c - ox) / u;
                let y = ((cy as f64 + 0.5) * c - oy) / u;
                if let Some(rgb) = f(x, y) {
                    self.canvas.cell(cx * n, cy * n, n, rgb, self.alpha);
                }
            }
        }
    }

    /// A round dot of radius `r`.
    pub fn dot(&mut self, x: f64, y: f64, r: f64, rgb: u32) {
        let r = r.max(0.13);
        self.fill((x - r, y - r), (x + r, y + r), |px, py| ((px - x).hypot(py - y) <= r).then_some(rgb));
    }

    /// A line `width` wide along `points`. `keep(t)`, with `t` running 0..1
    /// along the line, can thin it out (dithered) to fade it away.
    pub fn stroke(&mut self, points: &[(f64, f64)], width: f64, rgb: u32, keep: impl Fn(f64) -> f64) {
        let total: f64 = points.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
        if total <= 0.0 {
            return;
        }
        let step = (self.cell() / self.u * 0.5).max(0.05);
        let mut run = 0.0;
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = (b.0 - a.0).hypot(b.1 - a.1);
            let n = (len / step).ceil().max(1.0) as usize;
            for i in 0..n {
                let k = i as f64 / n as f64;
                let (x, y) = (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
                let t = (run + len * k) / total;
                if dither(x, y) < keep(t) {
                    self.dot(x, y, width / 2.0, rgb);
                }
            }
            run += len;
        }
    }
}

/// How lit a surface with normal (`nx`, `ny`) facing the viewer is, light
/// coming from the top left: roughly -1..1, plus a bulge toward the middle.
pub fn light(nx: f64, ny: f64) -> f64 {
    let r2 = (nx * nx + ny * ny).min(1.0);
    -(nx * 0.5 + ny * 0.85) * 0.9 + (1.0 - r2) * 0.35
}

/// Pick from a four-step ramp (dark..highlight) by `light`.
pub fn tone(ramp: &[u32; 4], light: f64) -> u32 {
    ramp[band(light)]
}

pub fn band(light: f64) -> usize {
    match light {
        l if l > 0.7 => 3,
        l if l > 0.25 => 2,
        l if l > -0.25 => 1,
        _ => 0,
    }
}

/// Deterministic noise in 0..1 for integer lattice points.
pub fn hash(x: i64, y: i64) -> f64 {
    let mut h = (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (y as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
    h ^= h >> 29;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 32;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// Noise on a grid of `size` classic pixels.
pub fn speckle(x: f64, y: f64, size: f64) -> f64 {
    hash((x / size).floor() as i64, (y / size).floor() as i64)
}

/// 4x4 ordered dither threshold for a point, on a quarter-pixel grid.
pub fn dither(x: f64, y: f64) -> f64 {
    const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    let (i, j) = (((x * 4.0).floor() as i64).rem_euclid(4), ((y * 4.0).floor() as i64).rem_euclid(4));
    (BAYER[(j * 4 + i) as usize] as f64 + 0.5) / 16.0
}

/// Rotate a point by `-angle`: screen offsets into a shape's own frame.
pub fn local(x: f64, y: f64, angle: f64) -> (f64, f64) {
    let (s, c) = angle.sin_cos();
    (x * c + y * s, -x * s + y * c)
}

/// Distance from a point to the segment a..b, and how far along it (0..1).
pub fn segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((p.0 - a.0 - dx * t).hypot(p.1 - a.1 - dy * t), t)
}

/// Scale a colour's brightness by `f`.
pub fn scale(rgb: u32, f: f64) -> u32 {
    let ch = |shift: u32| ((((rgb >> shift) & 0xff) as f64 * f).round().clamp(0.0, 255.0) as u32) << shift;
    ch(16) | ch(8) | ch(0)
}

/// Shade a flat colour into one of four bands by `light`.
pub fn shade(rgb: u32, light: f64) -> u32 {
    scale(rgb, [0.7, 0.86, 1.0, 1.1][band(light)])
}

/// Where a point sits in an ellipse: its normalized offset and radius, if
/// inside.
pub fn ellipse(x: f64, y: f64, cx: f64, cy: f64, rx: f64, ry: f64) -> Option<(f64, f64, f64)> {
    let (nx, ny) = ((x - cx) / rx, (y - cy) / ry);
    let r = nx.hypot(ny);
    (r < 1.0).then_some((nx, ny, r))
}

/// Roughly how far inside a shape of radius `r` (0..1, 1 on the edge) and
/// size `rx` by `ry` a point is, in classic pixels: for outlines.
pub fn depth(r: f64, rx: f64, ry: f64) -> f64 {
    (1.0 - r) * rx.min(ry)
}
