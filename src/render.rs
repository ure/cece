//! Software rendering into a premultiplied ARGB8888 buffer.

use crate::{cat::Pose, sprites::FRAME};

/// Everything is in buffer pixels: the surface is `size` logical pixels
/// square and the buffer is `size * scale` pixels.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u32],
    pub width: usize,
    pub height: usize,
}

const SHADOW: u32 = 0x2a;
const ZZZ: u32 = 0xffe4_e2ec;
const ZZZ_EDGE: u32 = 0xff26_232c;

// A 5x5 pixel-font "z".
const Z_GLYPH: [u8; 5] = [0b11111, 0b00010, 0b00100, 0b01000, 0b11111];

impl Canvas<'_> {
    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn blend(&mut self, x: i64, y: i64, src: u32) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let a = src >> 24;
        if a == 0 {
            return;
        }
        let dst = &mut self.pixels[y as usize * self.width + x as usize];
        if a == 255 {
            *dst = src;
            return;
        }
        let inv = 255 - a;
        let ch = |shift: u32| {
            let s = (src >> shift) & 0xff;
            let d = (*dst >> shift) & 0xff;
            (s + (d * inv + 127) / 255).min(255) << shift
        };
        *dst = ch(24) | ch(16) | ch(8) | ch(0);
    }

    /// Nearest-neighbour blit of a sprite frame, `size` buffer pixels square,
    /// with its top-left corner at (`x`, `y`).
    pub fn sprite(&mut self, frame: &[u32], x: f64, y: f64, size: f64) {
        let (x0, y0) = (x.round() as i64, y.round() as i64);
        let n = size.round() as i64;
        for by in 0..n {
            let sy = (by as usize * FRAME) / n as usize;
            for bx in 0..n {
                let sx = (bx as usize * FRAME) / n as usize;
                self.blend(x0 + bx, y0 + by, frame[sy * FRAME + sx]);
            }
        }
    }

    /// Soft elliptical shadow centred at (`cx`, `cy`).
    pub fn shadow(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, strength: f64) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        for y in (cy - ry).floor() as i64..=(cy + ry).ceil() as i64 {
            for x in (cx - rx).floor() as i64..=(cx + rx).ceil() as i64 {
                let (nx, ny) = ((x as f64 + 0.5 - cx) / rx, (y as f64 + 0.5 - cy) / ry);
                let d = nx * nx + ny * ny;
                if d < 1.0 {
                    let a = (SHADOW as f64 * strength * (1.0 - d).powf(0.6)) as u32;
                    self.blend(x, y, a << 24);
                }
            }
        }
    }

    /// A pixel-font "z", each glyph pixel `px` buffer pixels, faded by
    /// `alpha`: a light glyph over a dark offset copy, legible on any theme.
    pub fn z(&mut self, x: f64, y: f64, px: f64, alpha: f64) {
        let edge = (px * 0.5).max(1.0);
        self.glyph(x + edge, y + edge, px, alpha * 0.8, ZZZ_EDGE);
        self.glyph(x, y, px, alpha, ZZZ);
    }

    fn glyph(&mut self, x: f64, y: f64, px: f64, alpha: f64, rgb: u32) {
        let a = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
        let premul = |c: u32| ((c & 0xff) * a / 255) & 0xff;
        let color = a << 24 | premul(rgb >> 16) << 16 | premul(rgb >> 8) << 8 | premul(rgb);
        let p = px.max(1.0).round() as i64;
        for (row, bits) in Z_GLYPH.iter().enumerate() {
            for col in 0..5 {
                if bits & (0b10000 >> col) != 0 {
                    let (gx, gy) = (x.round() as i64 + col * p, y.round() as i64 + row as i64 * p);
                    for dy in 0..p {
                        for dx in 0..p {
                            self.blend(gx + dx, gy + dy, color);
                        }
                    }
                }
            }
        }
    }
}

/// Draw the cat, its shadow and any sleep bubbles.
///
/// `origin` is where the cat's top-left corner lands in the buffer and `cat`
/// its size, both in buffer pixels.
pub fn draw(canvas: &mut Canvas, frame: &[u32], pose: &Pose, origin: (f64, f64), cat: f64, scale: f64) {
    canvas.clear();
    let u = cat / 32.0; // one classic sprite pixel, in buffer pixels
    let (ox, oy) = origin;

    let ground = oy + 29.5 * u;
    let wide = if pose.asleep.is_some() { 11.0 } else { 8.5 };
    canvas.shadow(ox + cat / 2.0, ground, wide * u * pose.shadow.max(0.4), 2.2 * u, pose.shadow.max(0.3));

    canvas.sprite(frame, ox + pose.dx * scale, oy + pose.dy * scale, cat);

    if let Some(t) = pose.asleep {
        // Three staggered Zzz drifting up and to the right.
        let period = 3.6;
        for i in 0..3 {
            let age = t - 1.2 * i as f64;
            if age < 0.0 {
                continue;
            }
            let k = (age % period) / period;
            let sway = (k * std::f64::consts::TAU * 1.3 + i as f64).sin() * 2.0 * u;
            let x = ox + cat * 0.72 + k * cat * 0.35 + sway;
            let y = oy + cat * 0.4 - k * cat * 0.55;
            let glyph = u * (0.55 + 0.35 * k);
            let alpha = (k * 5.0).min(1.0) * (1.0 - k).powf(0.8);
            canvas.z(x, y, glyph, alpha);
        }
    }
}
