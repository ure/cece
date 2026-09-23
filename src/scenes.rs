//! Renders the screenshots in screenshots/ and the marketplace preview.png,
//! with the same renderer the cat uses, at the size of a 2560x720 panel at
//! scale 2 (a Corsair Xeneon Edge). Regenerate with:
//!
//!     cargo test --release screenshots -- --ignored

use std::path::Path;

use crate::{
    bed::Bed,
    cat::Pose,
    render::{self, Canvas},
    sprites::Sheet,
    toy::{Kind, ToyPose},
    toy_art,
};

const W: usize = 2560;
const H: usize = 720;
/// Buffer pixels per logical pixel, and the cat's size in logical pixels.
const SCALE: f64 = 2.0;
const CAT: f64 = 64.0;

/// A plain dark desktop, a little lighter toward the bottom.
fn desktop(w: usize, h: usize) -> Vec<u32> {
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as f64 / w as f64, (i / w) as f64 / h as f64);
            let v = 0.35 + 0.65 * y;
            let edge = 1.0 - 0.25 * ((x - 0.5) * 2.0).powi(2);
            let c = |base: f64, lift: f64| ((base + lift * v) * edge).round().clamp(0.0, 255.0) as u32;
            0xff00_0000 | c(22.0, 16.0) << 16 | c(24.0, 16.0) << 8 | c(32.0, 20.0)
        })
        .collect()
}

/// Premultiplied src over dst, at (ox, oy).
fn paste(dst: &mut [u32], dw: usize, src: &[u32], sw: usize, ox: i64, oy: i64) {
    let dh = dst.len() / dw;
    for (i, s) in src.iter().enumerate() {
        let (x, y) = (ox + (i % sw) as i64, oy + (i / sw) as i64);
        if x < 0 || y < 0 || x as usize >= dw || y as usize >= dh {
            continue;
        }
        let a = s >> 24;
        if a == 0 {
            continue;
        }
        let d = &mut dst[y as usize * dw + x as usize];
        let ch = |sh: u32| ((((s >> sh) & 0xff) + (((*d >> sh) & 0xff) * (255 - a) + 127) / 255).min(255)) << sh;
        *d = 0xff00_0000 | ch(16) | ch(8) | ch(0);
    }
}

struct Scene<'a> {
    pixels: Vec<u32>,
    w: usize,
    sheet: &'a Sheet,
    /// Zoom around `center` (logical px): 1.0 is true to size.
    k: f64,
    center: (f64, f64),
}

impl Scene<'_> {
    fn new(sheet: &Sheet, w: usize, k: f64) -> Scene<'_> {
        Scene { pixels: desktop(w, H), w, sheet, k, center: (w as f64 / SCALE / 2.0, 280.0) }
    }

    /// A logical stage position to buffer pixels.
    fn at(&self, x: f64, y: f64) -> (f64, f64) {
        let (cx, cy) = self.center;
        ((cx + (x - cx) * self.k) * SCALE, (cy + (y - cy) * self.k) * SCALE)
    }

    /// The cat with its top-left corner at logical (x, y), as main.rs lays
    /// out its surface.
    fn cat(&mut self, x: f64, y: f64, frame: &'static str, bed: Option<Bed>, asleep: Option<f64>, dy: f64) {
        let sf = SCALE * self.k;
        let pad = CAT / 2.0;
        let side = ((CAT + 2.0 * pad) * sf) as usize;
        let mut px = vec![0u32; side * side];
        let mut canvas = Canvas { pixels: &mut px, width: side, height: side };
        let pose = Pose { frame, dx: 0.0, dy, asleep, shadow: 1.0 - (-dy / 12.0).max(0.0), bed: bed.map(|b| (b, 1.0)) };
        render::draw(&mut canvas, self.sheet.get(frame), &pose, (pad * sf, pad * sf), CAT * sf, sf);
        let (bx, by) = self.at(x, y);
        paste(&mut self.pixels, self.w, &px, side, (bx - pad * sf) as i64, (by - pad * sf) as i64);
    }

    /// A toy touching the ground at logical (x, y), `z` logical px up.
    fn toy(&mut self, x: f64, y: f64, pose: ToyPose) {
        let sf = SCALE * self.k;
        let side = (CAT * 1.5).round();
        let ground = ((side / 2.0).round(), (side * 0.66).round());
        let n = (side * sf) as usize;
        let mut px = vec![0u32; n * n];
        let mut canvas = Canvas { pixels: &mut px, width: n, height: n };
        let u = CAT * sf / 32.0;
        let lift = pose.z.min(CAT * 0.45) * sf;
        toy_art::draw(&mut canvas, &pose, (ground.0 * sf, ground.1 * sf), u, lift);
        let (bx, by) = self.at(x, y);
        paste(&mut self.pixels, self.w, &px, n, (bx - ground.0 * sf) as i64, (by - ground.1 * sf) as i64);
    }

    /// A plain arrow pointer with its tip at logical (x, y).
    fn pointer(&mut self, x: f64, y: f64) {
        const ARROW: [&str; 17] = [
            "X", "XX", "XoX", "XooX", "XoooX", "XooooX", "XoooooX", "XooooooX", "XoooooooX", "XooooooooX", "XoooooXXXXX",
            "XooXooX", "XoX XooX", "XX  XooX", "X    XooX", "     XooX", "      XX",
        ];
        let p = (SCALE * self.k).round() as i64;
        let (bx, by) = self.at(x, y);
        let (ox, oy) = (bx as i64, by as i64);
        for (row, line) in ARROW.iter().enumerate() {
            for (col, c) in line.chars().enumerate() {
                let color = match c {
                    'X' => 0xff10_1010,
                    'o' => 0xffff_ffff,
                    _ => continue,
                };
                for dy in 0..p {
                    for dx in 0..p {
                        let (px, py) = (ox + col as i64 * p + dx, oy + row as i64 * p + dy);
                        if px >= 0 && py >= 0 && (px as usize) < self.w && ((py as usize) < self.pixels.len() / self.w) {
                            self.pixels[py as usize * self.w + px as usize] = color;
                        }
                    }
                }
            }
        }
    }
}

fn toy(kind: Kind) -> ToyPose {
    ToyPose {
        kind,
        variant: 1,
        heading: 0.0,
        facing: 1.0,
        z: 0.0,
        tail: 1.2,
        sway: 0.6,
        alpha: 1.0,
        roll: [0.93, 0.2, 0.3, 0.05],
        orbit: 0.5,
        swing: 0.15,
        tilt: 0.0,
        leash: None,
    }
}

fn save(path: &Path, w: usize, h: usize, px: &[u32]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_compression(png::Compression::High);
    let mut writer = enc.write_header().unwrap();
    let data: Vec<u8> = px.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8]).collect();
    writer.write_image_data(&data).unwrap();
}

/// What happens in each picture: the cat's frame and spot, and the toy.
struct Shot {
    name: &'static str,
    draw: fn(&mut Scene, f64),
}

const SHOTS: &[Shot] = &[
    Shot { name: "follow", draw: |s, cx| {
        s.cat(cx - 250.0, 250.0, "right2", None, None, -2.5);
        s.pointer(cx + 60.0, 275.0);
    } },
    Shot { name: "wand", draw: |s, cx| {
        // Flicked: a wand on the pointer's string, and the cat swatting up at it.
        let (px, py) = (cx + 18.0, 150.0);
        let mut t = toy(Kind::MarabouWand);
        let ground = (cx + 8.0, py + (12.0 + 12.0) * 2.0);
        t.leash = Some(((px - ground.0) / 2.0, (py - ground.1) / 2.0));
        t.swing = -0.2;
        s.cat(cx - 40.0, 245.0, "upclaw1", None, None, 0.0);
        s.toy(ground.0, ground.1, t);
        s.pointer(px, py);
    } },
    Shot { name: "sleep-duck", draw: |s, cx| s.cat(cx - 37.0, 240.0, "sleep1", Some(Bed::Duck), Some(1.7), 0.0) },
    Shot { name: "sleep-donut", draw: |s, cx| s.cat(cx - 37.0, 240.0, "sleep2", Some(Bed::Donut), Some(0.9), 0.0) },
    Shot { name: "sleep-scratcher", draw: |s, cx| s.cat(cx - 37.0, 240.0, "sleep1", Some(Bed::Scratcher), Some(2.4), 0.0) },
    Shot { name: "sleep-basket", draw: |s, cx| s.cat(cx - 37.0, 200.0, "sleep2", Some(Bed::Basket), Some(1.3), 0.0) },
    Shot { name: "play-ball", draw: |s, cx| {
        s.cat(cx - 68.0, 250.0, "rightclaw1", None, None, 0.0);
        s.toy(cx + 28.0, 305.0, ToyPose { z: 16.0, ..toy(Kind::RedBall) });
    } },
    Shot { name: "play-butterfly", draw: |s, cx| {
        s.cat(cx - 70.0, 230.0, "upclaw2", None, None, 0.0);
        s.toy(cx + 10.0, 300.0, ToyPose { orbit: 2.6, ..toy(Kind::Butterfly) });
    } },
    Shot { name: "play-radish", draw: |s, cx| {
        s.cat(cx - 200.0, 245.0, "right1", None, None, -3.0);
        s.toy(cx + 40.0, 300.0, ToyPose { z: 8.0, tilt: 0.2, ..toy(Kind::RedRadish) });
    } },
    Shot { name: "play-mouse", draw: |s, cx| {
        s.cat(cx + 60.0, 245.0, "left2", None, None, -2.0);
        s.toy(cx - 60.0, 300.0, ToyPose { facing: -1.0, heading: std::f64::consts::PI, ..toy(Kind::PlushMouse) });
    } },
    Shot { name: "play-fish", draw: |s, cx| {
        s.cat(cx - 110.0, 245.0, "rightclaw2", None, None, 0.0);
        s.toy(cx + 20.0, 300.0, ToyPose { z: 10.0, ..toy(Kind::Sardine) });
    } },
    Shot { name: "play-squirrel", draw: |s, cx| {
        s.cat(cx - 190.0, 240.0, "downright1", None, None, -2.0);
        s.toy(cx + 40.0, 310.0, ToyPose { heading: 0.4, ..toy(Kind::Squirrel) });
    } },
    Shot { name: "play-wand", draw: |s, cx| {
        s.cat(cx - 100.0, 235.0, "upclaw1", None, None, 0.0);
        s.toy(cx + 15.0, 300.0, ToyPose { z: 12.0, ..toy(Kind::RainbowWand) });
    } },
    Shot { name: "play-straw-mouse", draw: |s, cx| {
        s.cat(cx - 180.0, 250.0, "right2", None, None, -2.0);
        s.toy(cx + 30.0, 305.0, ToyPose { heading: -0.3, variant: 0, ..toy(Kind::StrawMouse) });
    } },
    Shot { name: "idle-wash", draw: |s, cx| s.cat(cx - 32.0, 245.0, "wash", None, None, 0.0) },
];

#[test]
#[ignore]
fn screenshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    let sheet = Sheet::load("burmilla").unwrap();
    for shot in SHOTS {
        let mut scene = Scene::new(&sheet, W, 1.0);
        (shot.draw)(&mut scene, W as f64 / SCALE / 2.0);
        save(&dir.join(format!("{}.png", shot.name)), W, H, &scene.pixels);
    }

    // Every toy, lined up.
    let mut scene = Scene::new(&sheet, W, 1.0);
    for (i, kind) in crate::toy::KINDS.into_iter().enumerate() {
        let (col, row) = ((i % 8) as f64, (i / 8) as f64);
        let x = 90.0 + col * 157.0 + row * 78.0;
        let y = 150.0 + row * 150.0;
        let z = if matches!(kind, Kind::RainbowWand | Kind::SpringWand | Kind::MarabouWand) { 10.0 } else { 0.0 };
        scene.toy(x, y, ToyPose { z, variant: i, ..toy(kind) });
    }
    save(&dir.join("toys.png"), W, H, &scene.pixels);

    // The marketplace preview: four panels across, zoomed in as far as the
    // Size setting goes (4), so the cat reads in a thumbnail.
    let panels = ["wand", "play-butterfly", "play-ball", "sleep-donut"];
    let pw = W / panels.len();
    let mut preview = vec![0u32; W * H];
    for (i, name) in panels.iter().enumerate() {
        let shot = SHOTS.iter().find(|s| s.name == *name).unwrap();
        let mut scene = Scene::new(&sheet, pw, 2.0);
        (shot.draw)(&mut scene, pw as f64 / SCALE / 2.0);
        for y in 0..H {
            let row = &mut preview[y * W + i * pw..y * W + (i + 1) * pw];
            row.copy_from_slice(&scene.pixels[y * pw..(y + 1) * pw]);
            if i > 0 {
                row[0] = 0xff3a_3c48;
                row[1] = 0xff3a_3c48;
            }
        }
    }
    save(&root.join("preview.png"), W, H, &preview);
}
