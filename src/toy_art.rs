//! What the toys look like. Each is a set of shape functions in classic
//! sprite pixels around the point where the toy touches the ground (y up is
//! negative), painted on the cat's sprite grid with a dark outline and flat
//! bands of shading, like the cat.

use std::f64::consts::{PI, TAU};

use crate::{
    art::{Painter, depth, ellipse, light, local, scale, segment, shade, speckle, tone},
    render::Canvas,
    toy::{BALL, FLIGHT, Kind, ORBIT, ToyPose},
};

/// Draw the toy and its shadow. `ground` is where it touches the ground and
/// `lift` its height, in buffer pixels; `u` is one classic sprite pixel.
pub fn draw(canvas: &mut Canvas, pose: &ToyPose, ground: (f64, f64), u: f64, lift: f64) {
    canvas.clear();
    let k = 1.0 / (1.0 + lift / (8.0 * u));
    let (rx, ry) = match pose.kind {
        Kind::RedBall | Kind::BlueBall | Kind::PinkBall => (3.6, 1.2),
        Kind::RedRadish | Kind::WhiteRadish => (3.8, 1.3),
        Kind::RainbowWand | Kind::SpringWand | Kind::MarabouWand => (3.0, 1.0),
        Kind::Sardine | Kind::Squirrel => (7.5, 1.9),
        Kind::Butterfly => (7.5, 2.6),
        _ => (6.0, 1.7),
    };
    // Hanging from the pointer, it is nowhere near the floor.
    if pose.leash.is_none() {
        canvas.shadow(ground.0, ground.1, rx * u * k, ry * u * k, k * pose.alpha);
    }
    if pose.kind == Kind::Butterfly {
        let (bx, by) = (ORBIT * pose.orbit.cos(), ORBIT * 0.5 * pose.orbit.sin());
        canvas.shadow(ground.0 + bx * u, ground.1 + by * u, 2.2 * u, 0.8 * u, 0.6 * pose.alpha);
    }

    let mut p = Painter { canvas, origin: (ground.0, ground.1 - lift), u, alpha: pose.alpha };
    if let Some(anchor) = pose.leash {
        // A string up to the pointer, sagging a little.
        let pivot = (0.0, -pose.kind.pivot());
        let string: Vec<(f64, f64)> = (0..=12)
            .map(|i| {
                let t = i as f64 / 12.0;
                let sag = (t * PI).sin() * 0.8;
                (pivot.0 + (anchor.0 - pivot.0) * t + sag, pivot.1 + (anchor.1 - pivot.1) * t)
            })
            .collect();
        // Mid grey, so it shows on dark and light desktops alike.
        p.stroke(&string, 0.35, 0x8e8e98, |_| 1.0);
    }
    match pose.kind {
        Kind::StrawMouse => straw_mouse(&mut p, pose),
        Kind::PlushMouse => plush_mouse(&mut p, pose),
        Kind::PlaidMouse => plaid_mouse(&mut p, pose),
        Kind::Squirrel => squirrel(&mut p, pose),
        Kind::Sardine => sardine(&mut p, pose),
        Kind::Candy => candy(&mut p, pose),
        Kind::RedRadish => red_radish(&mut p, pose),
        Kind::WhiteRadish => white_radish(&mut p, pose),
        Kind::RedBall => ball(&mut p, pose, [0xcbb5a8, 0x1d1719, 0xb4263a]),
        Kind::BlueBall => ball(&mut p, pose, [0x9db7cf, 0x14223b, 0x2d7fd1]),
        Kind::PinkBall => ball(&mut p, pose, [0xc79ad0, 0x2a1729, 0xdf3f8e]),
        Kind::RainbowWand => rainbow_wand(&mut p, pose),
        Kind::SpringWand => spring_wand(&mut p, pose),
        Kind::MarabouWand => marabou_wand(&mut p, pose),
        Kind::Butterfly => butterfly(&mut p, pose),
    }
}

/// Black, ginger, white and peach feathers: vane, second tone, fringe, quill.
const BLACK_FEATHER: [u32; 4] = [0x2b2731, 0x3f3a47, 0x18161b, 0x77707c];
const GINGER_FEATHER: [u32; 4] = [0xe3b37b, 0xc98a4c, 0xf3d6ab, 0x8a5a2e];
const WHITE_FEATHER: [u32; 4] = [0xf4f1eb, 0xdcd6cb, 0xc8c0b3, 0xb3aa9c];
const PEACH_FEATHER: [u32; 4] = [0xf3cfae, 0xe0a878, 0xfbe6d2, 0xc08050];

/// A fluffy feather tuft, widest toward its end. `t` runs 0..1 from root to
/// tip, `dy` is across it and `width` its widest half-width, in classic px.
fn plume(t: f64, dy: f64, len: f64, width: f64, colors: [u32; 4]) -> Option<u32> {
    if !(0.0..1.0).contains(&t) {
        return None;
    }
    let a = dy.abs();
    let side = if dy < 0.0 { 2.1 } else { 0.0 };
    // Ragged edges: tufts of different lengths.
    let ragged = 0.78 + 0.22 * (t * 23.0 + side).sin() * (t * 7.3 + 1.0 + side).sin();
    let half = (0.5 + (width - 0.5) * (PI * t.powf(0.8)).sin().powf(0.6)) * ragged;
    if a > half {
        return None;
    }
    // Wisps sweep back from the quill, with gaps between them near the edge.
    let wisp = (t * len * 0.8 - a * 0.7 + side * 0.4).rem_euclid(1.0);
    if wisp < 0.14 && a > 0.55 * half {
        return None;
    }
    let [vane, second, fringe, quill] = colors;
    Some(if a < 0.3 && t < 0.75 {
        quill
    } else if a > 0.8 * half {
        fringe
    } else if wisp < 0.55 {
        vane
    } else {
        second
    })
}

/// A plume trailing behind (toward -x) from `root`, waving with the pose.
fn tail_plume(x: f64, y: f64, root: f64, len: f64, width: f64, pose: &ToyPose, colors: [u32; 4]) -> Option<u32> {
    let t = (root - x) / len;
    let amp = 0.3 + 1.6 * pose.sway;
    let mid = amp * (pose.tail - t * 3.2).sin() * t.max(0.0).powf(1.2);
    plume(t, y - mid, len, width, colors)
}

// --- Woven straw mouse (top view) -------------------------------------------

const STRAW_LINE: u32 = 0x3b2e1e;
const GROOVE: u32 = 0x7d5f37;
const STRAW: [u32; 4] = [0xa07f4b, 0xc9aa70, 0xe2cb96, 0xf3e5bf];
const EAR: u32 = 0xe9c3a0;
const HALF_LEN: f64 = 4.4;
const HALF_WIDTH: f64 = 2.8;

/// An egg narrowing toward the nose (+x), in the toy's own frame. Gives the
/// screen-space normal (for lighting) or the outline colour.
fn egg(x: f64, y: f64, heading: f64, line: u32) -> Option<Result<(f64, f64, f64), u32>> {
    let half = HALF_WIDTH * (1.0 - 0.2 * x / HALF_LEN);
    let (nx, ny) = (x / HALF_LEN, y / half);
    let r = nx.hypot(ny);
    if r >= 1.0 {
        return None;
    }
    if (1.0 - r) * half.min(HALF_LEN) < 0.55 {
        return Some(Err(line));
    }
    let (sx, sy) = local(nx, ny, -heading);
    Some(Ok((sx, sy, r)))
}

fn straw_mouse(p: &mut Painter, pose: &ToyPose) {
    let colors = [BLACK_FEATHER, GINGER_FEATHER, WHITE_FEATHER][pose.variant % 3];
    let h = pose.heading;
    p.fill((-16.0, -17.6), (16.0, 14.4), |sx, sy| {
        let (x, y) = local(sx, sy + 1.6, h);
        if let Some(body) = egg(x, y, h, STRAW_LINE) {
            let (sx, sy, _) = match body {
                Err(line) => return Some(line),
                Ok(n) => n,
            };
            // The open end of the weave, at the nose.
            if (x - (HALF_LEN - 1.0)).hypot(y) < 0.45 {
                return Some(STRAW_LINE);
            }
            let mut level = crate::art::band(light(sx, sy));
            // Rings of twisted straw around the body, each twisted the other way.
            let ring = 1.35;
            let along = (x + 20.0) / ring;
            let (n, f) = (along.floor() as i64, along.fract());
            if f < 0.17 {
                return Some(GROOVE);
            }
            let twist = if n % 2 == 0 { 0.8 } else { -0.8 };
            if ((y * twist + f * ring) / 1.1).rem_euclid(1.0) < 0.25 {
                level = level.saturating_sub(1);
            }
            return Some(STRAW[level]);
        }
        for side in [-1.0, 1.0] {
            let d = (x - HALF_LEN * 0.25).hypot(y - side * HALF_WIDTH * 0.95);
            if d < 1.2 {
                return Some(if d > 0.7 { STRAW_LINE } else if d > 0.45 { STRAW[2] } else { EAR });
            }
        }
        tail_plume(x, y, -HALF_LEN + 0.8, 10.0, 3.2, pose, colors)
    });
}

// --- Plaid mouse and candy (top view) ----------------------------------------

const PLAID_LINE: u32 = 0x33401f;
const SAGE: [u32; 4] = [0x6e8050, 0x8fa46c, 0xaebf88, 0xc8d6a4];

/// Woven cloth: cream with green checks and a thin yellow line.
fn plaid(x: f64, y: f64) -> u32 {
    let (g, h) = ((x / 2.4).rem_euclid(1.0), (y / 2.4).rem_euclid(1.0));
    let (gx, gy) = (g < 0.22, h < 0.22);
    // Tiny over-under weave so the cloth does not look flat.
    let weave = ((x * 4.0).floor() + (y * 4.0).floor()) as i64 % 2 == 0;
    match (gx, gy) {
        (true, true) => 0x3f6a28,
        (true, false) | (false, true) => if weave { 0x5e8a3a } else { 0x6e9a48 },
        _ if (g - 0.6).abs() < 0.06 || (h - 0.6).abs() < 0.06 => 0xd9b43a,
        _ => if weave { 0xe4e1cf } else { 0xd8d4bf },
    }
}

fn plaid_mouse(p: &mut Painter, pose: &ToyPose) {
    let h = pose.heading;
    p.fill((-16.0, -17.6), (16.0, 14.4), |sx, sy| {
        let (x, y) = local(sx, sy + 1.6, h);
        for side in [-1.0, 1.0] {
            let (ex, ey) = (HALF_LEN * 0.35, side * (HALF_WIDTH + 0.4));
            if let Some((nx, ny, r)) = ellipse(x, y, ex, ey, 1.4, 1.9) {
                if depth(r, 1.4, 1.9) < 0.5 {
                    return Some(PLAID_LINE);
                }
                // Cupped: a darker hollow inside the ear.
                let (lx, ly) = local(nx, ny, -h);
                return Some(if r < 0.5 { SAGE[0] } else { tone(&SAGE, light(lx, ly)) });
            }
        }
        if let Some(body) = egg(x, y, h, PLAID_LINE) {
            let (sx, sy, _) = match body {
                Err(line) => return Some(line),
                Ok(n) => n,
            };
            for side in [-1.0, 1.0] {
                if (x - HALF_LEN * 0.62).hypot(y - side * 1.1) < 0.4 {
                    return Some(0x141414);
                }
            }
            return Some(shade(plaid(x, y), light(sx, sy)));
        }
        tail_plume(x, y, -HALF_LEN + 0.8, 9.0, 3.0, pose, PEACH_FEATHER)
    });
}

fn candy(p: &mut Painter, pose: &ToyPose) {
    let h = pose.heading;
    p.fill((-16.0, -17.6), (16.0, 14.4), |sx, sy| {
        let (x, y) = local(sx, sy + 1.8, h);
        // A soft cylinder, pinched at the twisted ends.
        let (hx, hy) = (3.6, 2.3);
        let pinch = 1.0 - 0.25 * (x / hx).abs().powi(6);
        let (nx, ny) = (x / hx, y / (hy * pinch));
        let r = nx.abs().powf(4.0).max(0.0) + ny.abs().powf(2.0);
        if r < 1.0 && nx.abs() < 1.0 {
            if nx.abs() > 0.9 || (1.0 - r) * hy < 0.5 {
                return Some(PLAID_LINE);
            }
            let (lx, ly) = local(0.0, ny, -h);
            return Some(shade(plaid(x, y), light(lx, ly)));
        }
        // A feather out of each end.
        let front = tail_plume(-x, y, -hx + 0.6, 7.0, 2.6, pose, PEACH_FEATHER);
        front.or_else(|| {
            let back = ToyPose { tail: pose.tail + 1.7, ..*pose };
            tail_plume(x, y, -hx + 0.6, 7.0, 2.6, &back, PEACH_FEATHER)
        })
    });
}

// --- Plush mouse (side view) -------------------------------------------------

const GREY: [u32; 4] = [0x5e5b60, 0x8a878c, 0xaeabb0, 0xcac7cb];
const GREY_LINE: u32 = 0x2e2b31;
const FELT: [u32; 3] = [0xb88a68, 0xdcb08c, 0xefcaa8];
const ROPE: [u32; 3] = [0x7d5e36, 0xa8844f, 0xc9a674];

fn plush_mouse(p: &mut Painter, pose: &ToyPose) {
    let f = pose.facing;
    // The braided rope tail, as a wiggly line behind the body.
    let tail: Vec<(f64, f64)> = (0..=14)
        .map(|i| {
            let t = i as f64 / 14.0;
            let wave = (pose.tail - t * 4.0).sin() * (0.3 + pose.sway) * t;
            (-4.2 - 7.5 * t, -1.2 - 1.8 * (t * PI).sin() * 0.6 + wave)
        })
        .collect();
    p.fill((-13.0, -8.0), (8.0, 1.0), |sx, sy| {
        let x = sx * f;
        let y = sy;
        // Face first: nose, eye, near ear.
        if (x - 4.7).hypot(y + 2.3) < 0.95 {
            return Some(0x121214);
        }
        if (x - 2.9).hypot(y + 3.4) < 0.42 {
            return Some(0x121214);
        }
        if let Some((_, _, r)) = ellipse(x, y, 1.3, -5.0, 1.1, 1.5) {
            return Some(if depth(r, 1.1, 1.5) < 0.45 { GREY_LINE } else if r < 0.55 { FELT[1] } else { FELT[2] });
        }
        // The body: a teardrop, rounder at the back, flat underneath.
        let (bx, by) = (x + 0.3, y + 2.7);
        let half = 2.7 * (1.0 - 0.28 * (bx / 5.0));
        let (nx, ny) = (bx / 5.0, by / half);
        let r = nx.hypot(ny);
        if r < 1.0 && y < -0.5 {
            if depth(r, 5.0, half) < 0.5 || y > -1.0 {
                return Some(GREY_LINE);
            }
            let fuzz = (speckle(x, y, 0.5) - 0.5) * 0.35;
            return Some(tone(&GREY, light(nx * f, ny) + fuzz));
        }
        // Far ear, peeking out behind the head.
        if let Some((_, _, r)) = ellipse(x, y, 2.4, -4.6, 0.9, 1.3) {
            return Some(if depth(r, 0.9, 1.3) < 0.45 { GREY_LINE } else { FELT[0] });
        }
        // Felt feet.
        for fx in [1.8, -2.2] {
            if let Some((_, _, r)) = ellipse(x, y, fx, -0.5, 1.2, 0.55) {
                return Some(if depth(r, 1.2, 0.55) < 0.3 { GREY_LINE } else { FELT[1] });
            }
        }
        let rope = tail
            .windows(2)
            .enumerate()
            .map(|(i, w)| {
                let (d, k) = segment((x, y), w[0], w[1]);
                (d, (i as f64 + k) / 14.0)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        let (d, t) = rope;
        if t > 0.97 && d < 1.0 {
            return Some(ROPE[2]); // frayed end
        }
        if d < 0.62 {
            if d > 0.42 {
                return Some(0x4a3620);
            }
            let braid = (t * 22.0 + d * 1.5).rem_euclid(1.0);
            return Some(if braid < 0.5 { ROPE[1] } else { ROPE[2] });
        }
        None
    });
    // Whiskers.
    for end in [(6.4, -3.0), (6.2, -1.5)] {
        p.stroke(&[(4.9 * f, -2.3), (end.0 * f, end.1)], 0.25, 0x121214, |_| 1.0);
    }
}

// --- Shaggy squirrel (top view) ----------------------------------------------

const FUR: [u32; 4] = [0x4a3320, 0x7a5a38, 0xa88458, 0xcfae80];
const FUR_LINE: u32 = 0x2e2016;

fn squirrel(p: &mut Painter, pose: &ToyPose) {
    let h = pose.heading;
    // The bushy tail curls off to one side and swishes.
    let mut spine = vec![(-3.8, 0.0)];
    let mut angle = PI;
    for i in 1..=10 {
        let t = i as f64 / 10.0;
        angle -= 0.16 + 0.12 * (pose.tail - t * 2.5).sin() * pose.sway;
        let (x, y) = spine[i - 1];
        spine.push((x + angle.cos() * 1.0, y + angle.sin() * 1.0));
    }
    p.fill((-17.0, -18.0), (17.0, 16.0), |sx, sy| {
        let (x, y) = local(sx, sy + 1.6, h);
        let shag = |a: f64, b: f64| (speckle(a * 0.6, b * 3.0, 1.0) - 0.5) * 0.6;
        // Face.
        if (x - 6.4).hypot(y) < 0.85 {
            return Some(0x101012);
        }
        for side in [-1.0, 1.0] {
            if ellipse(x, y, 4.0, side * 1.25, 0.75, 0.6).is_some() {
                return Some(0x101012);
            }
        }
        // Body: long and flat, narrowing to the snout.
        let half = if x > 3.0 { 2.6 - (x - 3.0) * 0.4 } else { 2.6 };
        let edge = half + shag(x, (y / half).atan()) ;
        if (-4.5..6.8).contains(&x) && y.abs() < edge {
            let d = edge - y.abs();
            if d < 0.35 || x > 6.5 {
                return Some(FUR_LINE);
            }
            let (lx, ly) = local(0.0, y / half, -h);
            let streak = (speckle(x * 0.35, y * 2.5, 1.0) - 0.5) * 1.4;
            return Some(tone(&FUR, light(lx, ly) + streak));
        }
        let (d, t) = spine
            .windows(2)
            .enumerate()
            .map(|(i, w)| {
                let (d, k) = segment((x, y), w[0], w[1]);
                (d, (i as f64 + k) / 10.0)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))?;
        // Bushier than the body, with long wisps and light tips.
        let width = 2.2 + 1.6 * (t * PI * 0.8).sin() + shag(t * 10.0, d) * 2.0;
        if d < width {
            if width - d < 0.35 {
                return if speckle(t * 30.0, d, 1.0) < 0.5 { Some(FUR_LINE) } else { None };
            }
            let streak = (speckle(t * 18.0, d * 2.5, 1.0) - 0.5) * 1.6;
            return Some(tone(&FUR, 0.45 - d / width * 0.5 + streak));
        }
        None
    });
}

// --- Sardine (side view) -----------------------------------------------------

const FISH_LINE: u32 = 0x1d2c52;
const FISH_WHITE: [u32; 3] = [0xc3cbd8, 0xe2e7ee, 0xf6f8fb];
const FISH_BLUE: [u32; 3] = [0x223f80, 0x2f56a8, 0x6f8fd0];

fn sardine(p: &mut Painter, pose: &ToyPose) {
    let f = pose.facing;
    // Flops about while it moves.
    let flop = (pose.tail * 1.5).sin() * 0.2 * pose.sway;
    p.fill((-10.0, -9.0), (10.0, 2.0), |sx, sy| {
        let (x, y) = local(sx * f, sy + 2.3, flop * f);
        // Eye.
        let e = (x - 5.0).hypot(y + 0.4);
        if e < 0.95 {
            return Some(if e > 0.6 || (x - 5.15).hypot(y + 0.4) < 0.35 { 0x101018 } else { 0xf6f8fb });
        }
        // Spindle body.
        let (x0, x1) = (-5.2, 7.2);
        if (x0..x1).contains(&x) {
            let t = (x - x0) / (x1 - x0);
            let half = 2.1 * (4.0 * t * (1.0 - t)).powf(0.6);
            if y.abs() < half {
                if half - y.abs() < 0.45 {
                    return Some(FISH_LINE);
                }
                let v = y / half; // -1 back .. 1 belly
                let l = light(0.0, v);
                // Blue back with a streaky edge, a stripe along the side, and a
                // gill mark behind the eye.
                let streak = (speckle(x * 0.5, v * 6.0, 1.0) - 0.5) * 0.35;
                if v < -0.35 + streak {
                    return Some(FISH_BLUE[crate::art::band(l).min(2)]);
                }
                if (v + 0.05).abs() < 0.07 && (-3.5..3.5).contains(&x) && speckle(x * 2.0, 0.0, 1.0) > 0.25 {
                    return Some(FISH_BLUE[1]);
                }
                if ((x - 3.6) - v * v * 0.8).abs() < 0.18 && v > -0.4 && v < 0.5 {
                    return Some(FISH_BLUE[0]);
                }
                return Some(FISH_WHITE[crate::art::band(l).min(2)]);
            }
        }
        // Forked tail fin.
        if (-8.2..=-4.8).contains(&x) {
            let k = (-4.8 - x) / 3.4;
            let spread = 0.4 + 1.9 * k;
            let notch = (1.0 - k) * 0.0 + k * 1.2 * (1.0 - (k - 0.6).abs() * 1.5).max(0.0);
            if y.abs() < spread && y.abs() > notch * 0.6 {
                if spread - y.abs() < 0.35 || x < -8.0 {
                    return Some(FISH_LINE);
                }
                return Some(if speckle(x * 2.0, y * 2.0, 1.0) > 0.55 { FISH_BLUE[1] } else { FISH_WHITE[1] });
            }
        }
        None
    });
}

// --- Radishes (front view) ---------------------------------------------------

const LEAF: [u32; 4] = [0x1c5e24, 0x2a8a34, 0x3fae48, 0x6ccc62];
const LEAF_LINE: u32 = 0x0f3314;

/// Embroidered eyes looking where the radish goes, and whether (x, y) hit one.
fn eyes(x: f64, y: f64, at: [(f64, f64); 2], r: f64, look: f64) -> Option<u32> {
    for (ex, ey) in at {
        let d = (x - ex).hypot(y - ey);
        if d < r {
            let pupil = (x - ex - look * r * 0.25).hypot(y - ey);
            return Some(if d > r - 0.3 {
                0x202020
            } else if pupil < r * 0.25 && pupil > 0.0 && (x - ex - look * r * 0.25 + 0.15).hypot(y - ey + 0.15) < 0.2 {
                0xffffff
            } else if pupil < r * 0.58 {
                0x121212
            } else {
                0xf4f4f4
            });
        }
    }
    None
}

fn leaves(x: f64, y: f64, lobes: &[(f64, f64, f64)]) -> Option<u32> {
    for &(cx, cy, r) in lobes {
        if let Some((nx, ny, rr)) = ellipse(x, y, cx, cy, r * 0.8, r) {
            if depth(rr, r * 0.8, r) < 0.4 {
                return Some(LEAF_LINE);
            }
            // A seam down the middle of each felt leaf.
            if (x - cx).abs() < 0.12 && ny > -0.6 {
                return Some(LEAF[0]);
            }
            return Some(tone(&LEAF, light(nx, ny) + (speckle(x, y, 0.5) - 0.5) * 0.3));
        }
    }
    None
}

fn red_radish(p: &mut Painter, pose: &ToyPose) {
    const RED: [u32; 4] = [0x961420, 0xc8232f, 0xe4443e, 0xf26a58];
    let (tilt, look) = (pose.tilt, pose.facing);
    p.fill((-8.0, -14.0), (8.0, 1.0), |sx, sy| {
        let (x, y) = local(sx, sy, tilt);
        if let Some(c) = eyes(x, y, [(-1.35, -5.0), (1.35, -5.0)], 1.0, look) {
            return Some(c);
        }
        // Open smile.
        if let Some((_, ny, _)) = ellipse(x, y, 0.0, -3.4, 1.6, 1.1) {
            if ny > -0.1 {
                return Some(if ny < 0.25 { 0x5c0c16 } else { 0x9a1c2a });
            }
        }
        // A rounded, slightly pointy-bottomed body.
        let (bx, by) = (x, y + 4.2);
        let half = 3.7 * (1.0 - 0.18 * (by / 3.6).max(0.0));
        if let Some((nx, ny, r)) = ellipse(bx, by, 0.0, 0.0, half, 3.6) {
            if depth(r, half, 3.6) < 0.5 {
                return Some(0x4a0a10);
            }
            return Some(tone(&RED, light(nx, ny) + (speckle(x, y, 0.5) - 0.5) * 0.3));
        }
        leaves(x, y, &[(-1.8, -8.4, 1.6), (1.8, -8.4, 1.6), (0.0, -9.2, 1.8)])
    });
}

fn white_radish(p: &mut Painter, pose: &ToyPose) {
    const WHITE: [u32; 4] = [0xa9b0b8, 0xd2d7dc, 0xecf0f2, 0xffffff];
    let (tilt, look) = (pose.tilt, pose.facing);
    p.fill((-7.0, -15.0), (7.0, 1.0), |sx, sy| {
        let (x, y) = local(sx, sy, tilt);
        if let Some(c) = eyes(x, y, [(-1.1, -6.6), (1.1, -6.6)], 0.85, look) {
            return Some(c);
        }
        if let Some((_, ny, _)) = ellipse(x, y, 0.0, -4.8, 1.2, 0.8) {
            if ny > 0.0 {
                return Some(0xc82838);
            }
        }
        // A plump root, rounded on top, pointed at the bottom.
        let (top, tip) = (-9.4, -0.4);
        if (top..tip).contains(&y) {
            let t = (tip - y) / (tip - top); // 0 at the tip
            let half = 2.5 * t.powf(0.55) * (1.0 - ((t - 0.9) / 0.1).max(0.0).powi(2) * 0.6);
            if x.abs() < half {
                if half - x.abs() < 0.45 || y < top + 0.4 || t < 0.06 {
                    return Some(0x4a5058);
                }
                let fuzz = (speckle(x, y, 0.5) - 0.5) * 0.3;
                return Some(tone(&WHITE, light(x / half, (t - 0.6) * -1.2) + fuzz));
            }
        }
        leaves(x, y, &[(-1.3, -11.4, 1.8), (1.2, -12.0, 2.0)])
    });
}

// --- Leopard balls -----------------------------------------------------------

/// Spots on the ball: evenly spread points on the sphere (a Fibonacci
/// lattice), each with its own size.
fn spots() -> impl Iterator<Item = ([f64; 3], f64)> {
    const N: usize = 24;
    (0..N).map(|i| {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / N as f64;
        let r = (1.0 - y * y).sqrt();
        let a = i as f64 * PI * (3.0 - 5f64.sqrt());
        ([r * a.cos(), y, r * a.sin()], 0.22 + 0.12 * crate::art::hash(i as i64, 7))
    })
}

/// Rotate `v` by the inverse of unit quaternion `q`.
fn unrotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let (w, x, y, z) = (q[0], -q[1], -q[2], -q[3]);
    // v' = v + 2w(u×v) + 2u×(u×v), with u = (x, y, z)
    let c1 = [y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0]];
    let c2 = [y * c1[2] - z * c1[1], z * c1[0] - x * c1[2], x * c1[1] - y * c1[0]];
    [v[0] + 2.0 * (w * c1[0] + c2[0]), v[1] + 2.0 * (w * c1[1] + c2[1]), v[2] + 2.0 * (w * c1[2] + c2[2])]
}

fn ball(p: &mut Painter, pose: &ToyPose, [base, ring, inner]: [u32; 3]) {
    let spots: Vec<_> = spots().collect();
    let r = BALL;
    p.fill((-r - 0.5, -2.0 * r - 0.5), (r + 0.5, 0.5), |x, y| {
        let (nx, ny) = (x / r, (y + r) / r);
        let d2 = nx * nx + ny * ny;
        // Fuzzy felt edge.
        let fuzz = 1.0 + (speckle(x, y, 0.25) - 0.5) * 0.12;
        if d2 >= fuzz * fuzz {
            return None;
        }
        let l = light(nx, ny);
        if d2 > 0.84 {
            return Some(scale(base, 0.45));
        }
        let nz = (1.0 - d2.min(1.0)).sqrt();
        let v = unrotate(pose.roll, [nx, ny, nz]);
        let mut color = base;
        for (s, size) in &spots {
            let d = (v[0] - s[0]).hypot(v[1] - s[1]).hypot(v[2] - s[2]);
            let wobble = (speckle(v[0] * 9.0, v[1] * 9.0 + v[2] * 5.0, 1.0) - 0.5) * 0.08;
            if d < size * 0.5 + wobble {
                color = inner;
                break;
            }
            if d < size + wobble {
                color = ring;
                break;
            }
        }
        Some(shade(color, l + (speckle(x, y, 0.5) - 0.5) * 0.2))
    });
}

// --- Wands -------------------------------------------------------------------

/// Offset of `(x, y)` from the pivot, turned into a frame hanging at `swing`.
fn hanging(x: f64, y: f64, pivot: (f64, f64), swing: f64) -> (f64, f64) {
    local(x - pivot.0, y - pivot.1, swing)
}

fn rainbow_wand(p: &mut Painter, pose: &ToyPose) {
    const COLORS: [u32; 8] = [0xf0409a, 0xc0209a, 0x8a4ad0, 0xf07a28, 0xe03a3a, 0x40b0e8, 0x3ab060, 0xf0d040];
    let pivot = (0.0, -Kind::RainbowWand.pivot());
    let s = pose.swing;
    // The stick, rising off toward whoever holds it and fading away.
    let (dx, dy) = local(0.42, -1.0, s * 0.3);
    let stick = [pivot, (pivot.0 + dx * 26.0, pivot.1 + dy * 26.0)];
    if pose.leash.is_none() {
        p.stroke(&stick, 0.7, 0x5a5a66, |t| 1.4 - t * 1.6);
    }
    p.fill((-9.0, -12.0), (9.0, 2.0), |x, y| {
        let (hx, hy) = hanging(x, y, pivot, s);
        if hx.hypot(hy) < 0.6 {
            return Some(0x18181c);
        }
        // A bunch of feathers fanning down from the tip of the stick.
        for (i, &c) in COLORS.iter().enumerate() {
            let a = PI / 2.0 + (i as f64 - 3.5) * 0.24 + (pose.tail * 1.3 + i as f64).sin() * 0.08 * pose.sway;
            let (fx, fy) = local(hx, hy, a);
            let len = 5.2 + (i % 3) as f64 * 0.8;
            let colors = [c, scale(c, 0.8), scale(c, 1.15), scale(c, 0.6)];
            if let Some(col) = plume(fx / len, fy, len, 1.3, colors) {
                return Some(col);
            }
        }
        None
    });
}

fn spring_wand(p: &mut Painter, pose: &ToyPose) {
    let pivot = (0.0, -Kind::SpringWand.pivot());
    let s = pose.swing;
    // A springy wire with little curls, rising away and fading out.
    let wire: Vec<(f64, f64)> = (0..=60)
        .map(|i| {
            let t = i as f64 / 60.0;
            let curl = t * 9.0 * TAU;
            let r = 0.5 * (1.0 - t * 0.5);
            (pivot.0 + t * 6.0 + curl.cos() * r + (pose.tail * 0.5 + t * 3.0).sin() * 1.2 * t, pivot.1 - t * 22.0 + curl.sin() * r)
        })
        .collect();
    if pose.leash.is_none() {
        p.stroke(&wire, 0.3, 0x8a8a94, |t| 1.4 - t * 1.6);
    }
    p.fill((-8.0, -11.0), (8.0, 2.0), |x, y| {
        let (hx, hy) = hanging(x, y, pivot, s);
        if ellipse(hx, hy, 0.0, 0.4, 0.55, 0.9).is_some() {
            return Some(0x141414);
        }
        // Three long feathers: spotted purple, speckled grey and dark green.
        for (i, a) in [-0.28, 0.02, 0.3].into_iter().enumerate() {
            let (fx, fy) = local(hx, hy - 1.0, PI / 2.0 + a + (pose.tail + i as f64).sin() * 0.05 * pose.sway);
            let len = 7.5 - i as f64 * 0.6;
            let t = fx / len;
            if !(0.0..1.0).contains(&t) {
                continue;
            }
            let half = 0.35 + 0.85 * (t * PI).sin().powf(0.6);
            if fy.abs() < half {
                if half - fy.abs() < 0.22 {
                    return Some(0x1c1020);
                }
                if fy.abs() < 0.15 {
                    return Some(0x2a1830);
                }
                let (base, dot) = match i {
                    0 => (0x6a2c7e, 0xe8e0f0),
                    1 => (0x4a3a52, 0xd0c8d8),
                    _ => (0x264a3a, 0x5a8a6a),
                };
                let spot = ((fx * 2.2).rem_euclid(1.0) - 0.5).hypot((fy * 2.2 + 0.25).rem_euclid(1.0) - 0.5) < 0.2;
                return Some(if spot { dot } else { base });
            }
        }
        None
    });
}

fn marabou_wand(p: &mut Painter, pose: &ToyPose) {
    const GREEN: [u32; 4] = [0x3a8e18, 0x5cc42a, 0x86e04a, 0xb4f47a];
    let pivot = (0.0, -Kind::MarabouWand.pivot());
    let s = pose.swing;
    // A loopy black cord, rising away and fading out.
    let cord: Vec<(f64, f64)> = (0..=48)
        .map(|i| {
            let t = i as f64 / 48.0;
            let loop_ = (t * 3.0 * TAU).sin() * 1.6 * (t * 3.0).min(1.0);
            (pivot.0 + loop_ - t * 5.0, pivot.1 - t * 18.0 + (t * 3.0 * TAU).cos() * 1.2 * (t * 3.0).min(1.0))
        })
        .collect();
    if pose.leash.is_none() {
        p.stroke(&cord, 0.35, 0x7a7a86, |t| 1.4 - t * 1.6);
    }
    p.fill((-8.0, -14.0), (8.0, 2.0), |x, y| {
        let (hx, hy) = hanging(x, y, pivot, s);
        if ellipse(hx, hy, 0.0, 0.3, 0.5, 0.9).is_some() {
            return Some(0x141414);
        }
        // A fluffy boa hanging from the cord.
        let t = (hy - 1.0) / 10.5;
        if !(0.0..1.0).contains(&t) {
            return None;
        }
        let bend = (pose.tail * 1.1 - t * 3.0).sin() * 0.6 * pose.sway * t;
        let dx = hx - bend;
        let wisp = (speckle(t * 30.0, dx * 1.5, 1.0) - 0.5) * 1.4;
        let half = 1.5 + 0.4 * (t * PI).sin() + wisp;
        if dx.abs() < half {
            if half - dx.abs() < 0.35 && speckle(t * 40.0, dx, 1.0) < 0.5 {
                return None; // gaps in the fluff
            }
            return Some(tone(&GREEN, light(dx / half, 0.0) + (speckle(t * 25.0, dx * 3.0, 1.0) - 0.5) * 0.7));
        }
        None
    });
}

// --- Butterfly spinner -------------------------------------------------------

const BASE: [u32; 4] = [0x4a8a1a, 0x74b42a, 0x96d23a, 0xc0ec66];
const BASE_LINE: u32 = 0x2a4a10;

fn butterfly(p: &mut Painter, pose: &ToyPose) {
    let (ox, oy) = (ORBIT * pose.orbit.cos(), ORBIT * 0.5 * pose.orbit.sin() - FLIGHT);
    let behind = pose.orbit.sin() < 0.0;
    if behind {
        wings(p, pose, (ox, oy));
    }
    // The wire from the button up to the butterfly, in a gentle arc.
    let wire: Vec<(f64, f64)> = (0..=16)
        .map(|i| {
            let t = i as f64 / 16.0;
            let lift = (t * PI).sin() * 2.5;
            (ox * t, -4.2 + (oy + 4.2) * t - lift)
        })
        .collect();
    p.stroke(&wire, 0.25, 0xa0a0a8, |_| 1.0);
    p.fill((-9.0, -6.0), (9.0, 4.0), |x, y| {
        // White button on top of the dome.
        if let Some((_, ny, _)) = ellipse(x, y, 0.0, -4.1, 1.1, 0.6) {
            return Some(if ny > 0.4 { 0xb8b8b8 } else { 0xf4f4f4 });
        }
        if let Some((nx, ny, r)) = ellipse(x, y, 0.0, -2.6, 2.4, 1.9) {
            if depth(r, 2.4, 1.9) < 0.4 {
                return Some(BASE_LINE);
            }
            return Some(tone(&BASE, light(nx, ny)));
        }
        // Four rounded arms on the floor (seen at an angle), with a rim.
        let arm = |y: f64| {
            let (gx, gy) = (x, y * 2.0);
            let a = gy.atan2(gx) + PI / 4.0;
            let reach = 3.2 + 4.8 * (a * 2.0).cos().abs().powf(3.0);
            (gx.hypot(gy), reach)
        };
        let (d, reach) = arm(y + 1.0);
        if d < reach {
            if reach - d < 0.5 {
                return Some(BASE_LINE);
            }
            return Some(tone(&BASE, 0.5 - d / reach * 0.8));
        }
        let (d, reach) = arm(y + 0.2);
        if d < reach {
            return Some(if reach - d < 0.5 { BASE_LINE } else { BASE[0] });
        }
        None
    });
    if !behind {
        wings(p, pose, (ox, oy));
    }
}

fn wings(p: &mut Painter, pose: &ToyPose, (bx, by): (f64, f64)) {
    const UPPER: [u32; 3] = [0x4a2288, 0x7a44c8, 0xa87ae6];
    const LOWER: [u32; 3] = [0x1f7a8a, 0x3ab4c0, 0x8ae4dc];
    let flap = 0.35 + 0.65 * (pose.tail * 4.0).sin().abs();
    p.stroke(&[(bx - 0.3, by - 1.6), (bx - 1.2, by - 3.0)], 0.2, 0x141018, |_| 1.0);
    p.stroke(&[(bx + 0.3, by - 1.6), (bx + 1.2, by - 3.0)], 0.2, 0x141018, |_| 1.0);
    p.fill((bx - 5.0, by - 4.0), (bx + 5.0, by + 3.0), |x, y| {
        let (x, y) = (x - bx, y - by);
        if ellipse(x, y, 0.0, 0.0, 0.45, 1.9).is_some() {
            return Some(0x141018);
        }
        let wx = x.abs() / flap;
        let wing = |cx: f64, cy: f64, rx: f64, ry: f64, ramp: [u32; 3]| {
            let (nx, ny, r) = ellipse(wx, y, cx, cy, rx, ry)?;
            if r > 0.78 {
                // Black border with white dots, like the real one.
                let dot = ((r - 0.89).abs() < 0.06) && ((ny.atan2(nx) * 5.0).rem_euclid(1.0) < 0.3);
                return Some(if dot { 0xf0f0f0 } else { 0x141018 });
            }
            // Veins radiating from the body.
            if ((ny.atan2(nx + 1.2) * 6.0).rem_euclid(1.0)) < 0.12 {
                return Some(0x241836);
            }
            Some(ramp[if r < 0.4 { 0 } else if r < 0.65 { 1 } else { 2 }])
        };
        wing(2.1, -1.1, 2.3, 1.7, UPPER).or_else(|| wing(1.6, 1.0, 1.5, 1.3, LOWER))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toy::KINDS;

    #[test]
    fn every_toy_draws_something() {
        for kind in KINDS {
            let mut pixels = vec![0u32; 200 * 200];
            let mut canvas = Canvas { pixels: &mut pixels, width: 200, height: 200 };
            let pose = ToyPose {
                kind,
                variant: 0,
                heading: 1.0,
                facing: 1.0,
                z: 0.0,
                tail: 0.5,
                sway: 0.5,
                alpha: 1.0,
                roll: [1.0, 0.0, 0.0, 0.0],
                orbit: 1.0,
                swing: 0.1,
                tilt: 0.0,
                leash: None,
            };
            draw(&mut canvas, &pose, (100.0, 130.0), 4.0, 0.0);
            let solid = pixels.iter().filter(|p| *p >> 24 == 255).count();
            assert!(solid > 300, "{kind:?}: {solid} pixels");
        }
    }
}
