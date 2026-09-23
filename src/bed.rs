//! Beds the cat sleeps in: a duck comforter, a fleece donut with a pink paw,
//! a cardboard scratcher bowl and a woven basket on a sisal post. They are
//! drawn in the cat's own surface: the back of the bed behind the cat and
//! the front rim over it, so she lies in it rather than on top of it.
//!
//! Coordinates are classic sprite pixels around the point under the middle
//! of the sleeping cat, y up negative. The curled-up cat covers about
//! x -12.5..12.5, y -17.5..0.5.

use std::f64::consts::PI;

use crate::{
    art::{Painter, depth, ellipse, light, speckle, tone},
    render::Canvas,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bed {
    Duck,
    Donut,
    Scratcher,
    Basket,
}

pub const BEDS: [Bed; 4] = [Bed::Duck, Bed::Donut, Bed::Scratcher, Bed::Basket];

/// Where a bed's soft shadow goes and how big it is.
pub fn shadow(bed: Bed) -> (f64, f64, f64, f64) {
    match bed {
        Bed::Duck => (0.0, 2.5, 21.0, 4.0),
        Bed::Donut => (0.0, 6.5, 20.5, 3.5),
        Bed::Scratcher => (0.0, 4.5, 20.0, 3.5),
        Bed::Basket => (0.0, 14.5, 10.5, 2.4),
    }
}

/// Paint the part of the bed behind the cat (`front` false) or in front of
/// it. `ground` is the point under the cat, in buffer pixels.
pub fn draw(canvas: &mut Canvas, bed: Bed, ground: (f64, f64), u: f64, alpha: f64, front: bool) {
    let mut p = Painter { canvas, origin: ground, u, alpha };
    let part = match bed {
        Bed::Duck => duck,
        Bed::Donut => donut,
        Bed::Scratcher => scratcher,
        Bed::Basket => basket,
    };
    p.fill((-22.0, -20.0), (22.0, 18.5), |x, y| part(x, y).filter(|c| c.1 == front).map(|c| c.0));
}

// Each part function gives a colour and whether it is in front of the cat.

const BACK: bool = false;
const FRONT: bool = true;

// --- Duck comforter ----------------------------------------------------------

fn duck(x: f64, y: f64) -> Option<(u32, bool)> {
    const PLUSH: [u32; 4] = [0xd4a82c, 0xecc84a, 0xf6dc76, 0xfdeeac];
    const LINE: u32 = 0x8a6010;
    // The duck's head and the wing hugging the blanket, off to one side.
    if let Some((nx, ny, r)) = ellipse(x, y, -13.0, -4.6, 2.3, 1.6) {
        // Beak, with two nostrils.
        if depth(r, 2.3, 1.6) < 0.35 {
            return Some((0x9a5a3a, BACK));
        }
        if (x + 12.3).abs() < 0.25 && (y + 5.0).abs() < 0.25 || (x + 13.7).abs() < 0.25 && (y + 5.0).abs() < 0.25 {
            return Some((0xa05c40, BACK));
        }
        return Some((tone(&[0xc87a58, 0xe89a72, 0xf4b690, 0xfbd0b0], light(nx, ny)), BACK));
    }
    for ex in [-17.0, -12.6] {
        if (x - ex).hypot(y + 7.6) < 0.45 {
            return Some((0x5a3018, BACK));
        }
    }
    if let Some((nx, ny, r)) = ellipse(x, y, -12.2, -1.2, 2.8, 1.8) {
        if depth(r, 2.8, 1.8) < 0.4 {
            return Some((LINE, BACK));
        }
        return Some((tone(&PLUSH, light(nx, ny) + 0.3), BACK));
    }
    if let Some((nx, ny, r)) = ellipse(x, y, -15.4, -5.6, 5.4, 5.0) {
        if depth(r, 5.4, 5.0) < 0.45 {
            return Some((LINE, BACK));
        }
        let fuzz = (speckle(x, y, 0.5) - 0.5) * 0.3;
        return Some((tone(&PLUSH, light(nx, ny) + fuzz), BACK));
    }
    // The blanket: a soft triangle lying flat, with a stitched hem.
    let corners = [(-21.0, 2.8), (20.0, 4.6), (3.0, -9.0)];
    let d = triangle(x, y, corners) - 2.0;
    if d < 0.0 {
        if d > -0.45 {
            return Some((LINE, BACK));
        }
        if d > -1.3 {
            let stitch = ((x + y) * 1.2).rem_euclid(1.0) < 0.5 && d < -0.8 && d > -1.0;
            return Some((if stitch { 0xfdeeac } else { PLUSH[0] }, BACK));
        }
        // Loose folds in the cloth.
        let fold = ((x * 0.35 - y * 0.8).sin() + (x * 0.13 + y * 0.4).sin()) * 0.35;
        return Some((tone(&PLUSH, 0.2 + fold + (speckle(x, y, 0.5) - 0.5) * 0.2), BACK));
    }
    None
}

/// Signed distance from a point to a triangle (negative inside).
fn triangle(x: f64, y: f64, [a, b, c]: [(f64, f64); 3]) -> f64 {
    let edge = |p: (f64, f64), q: (f64, f64)| {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        ((x - p.0) * dy - (y - p.1) * dx) / dx.hypot(dy)
    };
    let e = [edge(a, b), edge(b, c), edge(c, a)];
    // Inside when on the same side of all three edges, whichever way round
    // the corners go.
    if e.iter().all(|d| *d <= 0.0) || e.iter().all(|d| *d >= 0.0) {
        return -e.iter().map(|d| d.abs()).fold(f64::INFINITY, f64::min);
    }
    let seg = |p: (f64, f64), q: (f64, f64)| crate::art::segment((x, y), p, q).0;
    seg(a, b).min(seg(b, c)).min(seg(c, a))
}

// --- Fleece donut with a paw -------------------------------------------------

fn donut(x: f64, y: f64) -> Option<(u32, bool)> {
    const FLEECE: [u32; 4] = [0xb4aea4, 0xd6d1c8, 0xece8e0, 0xfaf8f3];
    const LINE: u32 = 0x7a746a;
    const PINK: [u32; 3] = [0xd87a88, 0xf0a0ac, 0xf8c2ca];
    const PINK_LINE: u32 = 0xa04858;
    let fleece = |l: f64| tone(&FLEECE, l + (speckle(x, y, 0.7) - 0.5) * 0.45);

    // The paw: a white fleece paw on the front of the rim with pink pads.
    for (tx, ty) in [(10.4, -1.6), (12.6, -2.7), (15.0, -2.5), (16.8, -0.8)] {
        if let Some((_, _, r)) = ellipse(x, y, tx, ty, 1.15, 1.0) {
            return Some((if depth(r, 1.15, 1.0) < 0.3 { PINK_LINE } else { PINK[1] }, FRONT));
        }
    }
    if let Some((nx, ny, r)) = ellipse(x, y, 13.4, 1.6, 3.2, 2.4) {
        if depth(r, 3.2, 2.4) < 0.35 {
            return Some((PINK_LINE, FRONT));
        }
        return Some((PINK[if light(nx, ny) > 0.3 { 2 } else { 1 }], FRONT));
    }
    if let Some((nx, ny, r)) = ellipse(x, y, 13.6, -0.2, 5.8, 5.2) {
        if depth(r, 5.8, 5.2) < 0.45 {
            return Some((LINE, FRONT));
        }
        return Some((fleece(light(nx, ny)), FRONT));
    }

    // A little black label on the front.
    if (-6.6..-4.8).contains(&x) && (1.8..4.4).contains(&y) {
        let text = (x + 5.7).abs() < 0.2 && ((y * 3.0).floor() as i64) % 2 == 0 && y < 4.0;
        return Some((if text { 0xe0e0e0 } else { 0x141414 }, FRONT));
    }

    let (cx, cy, rx, ry) = (0.0, -3.5, 19.5, 10.0);
    let (hx, hy, hrx, hry) = (0.0, -7.5, 13.0, 5.5);
    let outer = ellipse(x, y, cx, cy, rx, ry);
    let hole = ellipse(x, y, hx, hy, hrx, hry);
    if let (Some((_, _, r)), None) = (outer, hole) {
        // The rim: a fat fleece tube around the hole.
        let edge = depth(r, rx, ry);
        if edge < 0.45 {
            return Some((LINE, y > hy));
        }
        let hole_r = ((x - hx) / hrx).hypot((y - hy) / hry);
        let across = ((hole_r - 1.0) / 0.55).clamp(0.0, 1.0); // 0 at the hole
        let l = light(x / rx * 0.6, -(across * PI).cos() * 0.9);
        return Some((fleece(l), y > hy));
    }
    if let Some((nx, ny, r)) = hole {
        // The cushion inside, in shadow under the rim at the back.
        if depth(r, hrx, hry) < 0.35 {
            return Some((LINE, BACK));
        }
        return Some((fleece(light(nx, ny) - 0.4 - (1.0 - ny) * 0.2), BACK));
    }
    // A second roll underneath.
    if let Some((nx, _, r)) = ellipse(x, y, 0.0, -1.5, rx, ry) {
        if depth(r, rx, ry) < 0.45 || y < cy {
            return Some((LINE, FRONT));
        }
        return Some((fleece(-0.1 - nx.abs() * 0.3), FRONT));
    }
    None
}

// --- Cardboard scratcher bowl ------------------------------------------------

fn scratcher(x: f64, y: f64) -> Option<(u32, bool)> {
    const CARD: [u32; 3] = [0x7e5e3e, 0xa2805a, 0xbc9a72];
    const WALL: [u32; 4] = [0xbdbdbd, 0xdedede, 0xf2f2f2, 0xffffff];
    const EDGE: u32 = 0x5e4228;
    const INK: u32 = 0x1a1a1a;
    let (cy, rx, ry, wall) = (-8.0, 19.0, 7.0, 5.0);
    let k = (1.0 - (x / rx).powi(2)).max(0.0).sqrt();
    if x.abs() >= rx {
        return None;
    }
    let (rim_back, rim_front) = (cy - ry * k, cy + ry * k);
    // Headrest: the back wall rises toward the middle.
    let rest = 6.0 * (1.0 - (x / 13.0).powi(2)).max(0.0).powf(1.5);
    let top = rim_back - rest;

    if y >= rim_front && y < rim_front + wall {
        // Outer wall, white with a printed vine and butterflies.
        let v = (y - rim_front) / wall;
        if v > 0.9 || y < rim_front + 0.35 {
            return Some((EDGE, FRONT));
        }
        let vine = (v - 0.55 - 0.18 * (x * 0.55).sin()).abs() < 0.05;
        let sprig = ((x * 0.55).rem_euclid(PI) - 1.2).abs() < 0.1 && v > 0.25 && v < 0.55;
        let fly = |bx: f64| {
            let (dx, dy) = ((x - bx).abs(), v - 0.3);
            (dx - 0.5).hypot(dy * wall) < 0.45 && dx > 0.1
        };
        if vine || sprig || fly(-9.0) || fly(4.0) || fly(13.0) {
            return Some((INK, FRONT));
        }
        return Some((tone(&WALL, light(x / rx, 0.2) + 0.1), FRONT));
    }
    if y >= top && y < rim_back + 0.6 {
        // Inside of the back wall: corrugated flutes.
        if y < top + 0.4 {
            return Some((EDGE, BACK));
        }
        let flute = (x * 1.6).rem_euclid(1.0);
        return Some((CARD[if flute < 0.35 { 0 } else if flute < 0.7 { 1 } else { 2 }], BACK));
    }
    if y >= rim_back && y < rim_front {
        // The floor: rolled corrugated card in rings.
        let r = (x / (rx - 1.0)).hypot((y - cy) / (ry - 0.6));
        if r > 0.97 {
            return Some((EDGE, BACK));
        }
        let ring = (r * 14.0).rem_euclid(1.0);
        let dimple = speckle(x * 2.0, r * 14.0, 1.0) > 0.7;
        return Some((CARD[if ring < 0.3 || dimple { 0 } else if ring < 0.7 { 1 } else { 2 }], BACK));
    }
    None
}

// --- Woven basket on a sisal post --------------------------------------------

fn basket(x: f64, y: f64) -> Option<(u32, bool)> {
    const STRAW: [u32; 4] = [0xa07f4b, 0xc9aa70, 0xe2cb96, 0xf3e5bf];
    const LINE: u32 = 0x5a4428;
    const BLANKET: [u32; 3] = [0xcfc2ac, 0xe8dfcd, 0xf6f0e4];
    const SISAL: [u32; 3] = [0xbdb29c, 0xd8cfba, 0xeee7d6];
    const WOOD: [u32; 3] = [0xc8c0b2, 0xe6e0d4, 0xf6f2ea];
    let (cx, cy, rx, ry) = (0.0, -5.0, 18.5, 6.5);
    let k = (1.0 - (x / rx).powi(2)).max(0.0).sqrt();
    let (rim_back, rim_front) = (cy - ry * k, cy + ry * k);
    let depth_front = 7.0 * k.powf(0.5);
    let weave = |row: f64, along: f64| {
        let n = row.floor() as i64;
        let twist = if n % 2 == 0 { 1.0 } else { -1.0 };
        let f = row.fract();
        if f < 0.14 {
            return 0usize;
        }
        if ((along * 0.9 * twist + f * 1.3) / 1.1).rem_euclid(1.0) < 0.3 { 1 } else { 2 }
    };

    // Two little toys hanging from the rim.
    let hang = |sx: f64, len: f64| -> Option<u32> {
        let top = rim_front_at(sx, cx, cy, rx, ry) + 2.0;
        if (x - sx).abs() < 0.13 && y > top && y < top + len {
            return Some(0x2a2a2a);
        }
        let (bx, by) = (sx, top + len + 1.1);
        let d = (x - bx).hypot(y - by);
        if d < 1.3 {
            return Some(if d > 1.0 { 0x9a8a70 } else { 0xe6d6ba });
        }
        None
    };
    if let Some(c) = hang(-14.0, 4.0).or_else(|| hang(13.5, 5.0)) {
        return Some((c, FRONT));
    }
    // Ears on the little mouse on the left.
    for ex in [-12.8, -11.2] {
        if (x - ex - 2.0).hypot(y - (rim_front_at(-14.0, cx, cy, rx, ry) + 6.6)) < 0.55 {
            return Some((0xd08a6a, FRONT));
        }
    }

    if x.abs() < rx && y >= rim_front - 0.8 && y < rim_front + depth_front {
        // Front of the basket, tapering to its base.
        let v = (y - rim_front) / depth_front.max(0.1);
        let taper = 1.0 - 0.18 * v.max(0.0);
        if x.abs() > rx * taper {
            return None;
        }
        if y < rim_front - 0.4 || x.abs() > rx * taper - 0.45 || v > 0.93 {
            return Some((LINE, FRONT));
        }
        let level = weave((y - rim_front + 0.8) / 1.5, x);
        let l = light(x / rx, 0.1) + [-0.6, -0.1, 0.25][level];
        return Some((tone(&STRAW, l), FRONT));
    }
    if x.abs() < rx && y >= rim_back - 1.6 && y < rim_front {
        // The rim at the back and the blanket inside.
        let inner = ellipse(x, y, cx, cy + 0.4, rx - 2.2, ry - 1.4);
        if let Some((nx, ny, _)) = inner {
            let fold = ((x * 0.5 + y * 0.3).sin() + (x * 0.2 - y * 0.9).sin()) * 0.4;
            let l = light(nx, ny) - 0.2 + fold;
            return Some((BLANKET[if l > 0.3 { 2 } else if l > -0.3 { 1 } else { 0 }], BACK));
        }
        if y < rim_back - 1.2 {
            return Some((LINE, BACK));
        }
        let level = weave(x.atan2(1.0) * 4.0 + 20.0, y * 1.5);
        return Some((STRAW[level + 1], BACK));
    }
    // The post, wrapped in sisal rope.
    if x.abs() < 2.3 && (3.0..13.6).contains(&y) {
        if x.abs() > 1.9 {
            return Some((LINE, BACK));
        }
        let rope = ((y - x * 0.3) * 2.2).rem_euclid(1.0);
        let l = light(x / 2.3, 0.0);
        return Some((SISAL[if rope < 0.25 { 0 } else if l > 0.2 { 2 } else { 1 }], BACK));
    }
    // The round base.
    if let Some((nx, ny, r)) = ellipse(x, y, 0.0, 14.0, 9.5, 2.6) {
        if depth(r, 9.5, 2.6) < 0.4 {
            return Some((LINE, BACK));
        }
        return Some((WOOD[crate::art::band(light(nx, ny)).min(2)], BACK));
    }
    if let Some((_, _, r)) = ellipse(x, y, 0.0, 14.9, 9.5, 2.6) {
        return Some((if depth(r, 9.5, 2.6) < 0.4 { LINE } else { WOOD[0] }, BACK));
    }
    None
}

fn rim_front_at(x: f64, _cx: f64, cy: f64, rx: f64, ry: f64) -> f64 {
    cy + ry * (1.0 - (x / rx).powi(2)).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bed_has_a_back_and_most_a_front() {
        for bed in BEDS {
            let mut pixels = vec![0u32; 256 * 256];
            let mut canvas = Canvas { pixels: &mut pixels, width: 256, height: 256 };
            draw(&mut canvas, bed, (128.0, 128.0), 4.0, 1.0, false);
            let back = pixels.iter().filter(|p| *p >> 24 == 255).count();
            assert!(back > 2000, "{bed:?}: {back} pixels behind");
        }
    }
}
