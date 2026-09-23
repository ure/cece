//! The cat's brain: steering toward the cursor and the idle routine.
//!
//! Everything is in logical desktop pixels and seconds. Instead of the classic
//! neko's fixed hops per tick, the cat steers: its velocity eases toward the
//! cursor, it slows down as it arrives, its legs cycle with the distance it
//! actually covers and it bounces a little on every stride.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Awake,
    Yawn,
    Sleep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Up,
    Right,
    Down,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Run,
    /// Woken up: a surprised look and a little jump before running.
    Startle,
    Alert,
    Scratch,
    Wash,
    Yawn,
    Sleep,
    /// Cursor is out of reach past a screen edge: claw at it.
    Claw(Edge),
}

/// A monitor rectangle in logical desktop coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// What to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub frame: &'static str,
    /// Offset of the sprite from the cat's position, in logical pixels.
    pub dx: f64,
    pub dy: f64,
    /// Seconds spent asleep; drives the floating Zzz.
    pub asleep: Option<f64>,
    /// 1.0 grounded, smaller while the cat is in the air.
    pub shadow: f64,
}

const RUN_FRAMES: [[&str; 2]; 8] = [
    ["right1", "right2"],
    ["downright1", "downright2"],
    ["down1", "down2"],
    ["downleft1", "downleft2"],
    ["left1", "left2"],
    ["upleft1", "upleft2"],
    ["up1", "up2"],
    ["upright1", "upright2"],
];

pub struct Cat {
    /// Top-left corner of the cat, logical desktop pixels.
    pub x: f64,
    pub y: f64,
    /// Rendered size of the cat, logical pixels.
    size: f64,
    max_speed: f64,
    vx: f64,
    vy: f64,
    mode: Mode,
    mode_time: f64,
    mode_len: f64,
    stride: f64,
    dir: usize,
    blink_in: f64,
    blink_left: f64,
    washed: bool,
    rng: u64,
    pub staying: bool,
    pub sounds: Vec<Sound>,
}

impl Cat {
    pub fn new(x: f64, y: f64, size: f64, speed: f64, seed: u64) -> Self {
        let mut cat = Cat {
            x,
            y,
            size,
            // `speed` keeps the classic knob: 2.0 is the default.
            max_speed: speed * 110.0,
            vx: 0.0,
            vy: 0.0,
            mode: Mode::Alert,
            mode_time: 0.0,
            mode_len: 0.0,
            stride: 0.0,
            dir: 2,
            blink_in: 2.0,
            blink_left: 0.0,
            washed: false,
            rng: seed | 1,
            staying: false,
            sounds: Vec::new(),
        };
        cat.enter(Mode::Alert);
        cat
    }

    pub fn size(&self) -> f64 {
        self.size
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.size / 2.0, self.y + self.size / 2.0)
    }

    fn random(&mut self) -> f64 {
        // xorshift64*
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.random()
    }

    fn enter(&mut self, mode: Mode) {
        self.mode_len = match mode {
            Mode::Startle => 0.45,
            Mode::Alert => self.between(1.4, 3.2),
            Mode::Scratch => self.between(1.0, 1.8),
            Mode::Wash => self.between(1.2, 2.2),
            Mode::Yawn => 2.4,
            Mode::Claw(_) => self.between(2.0, 3.5),
            Mode::Run | Mode::Sleep => f64::INFINITY,
        };
        match mode {
            Mode::Yawn => self.sounds.push(Sound::Yawn),
            Mode::Sleep => self.sounds.push(Sound::Sleep),
            Mode::Startle => self.sounds.push(Sound::Awake),
            _ => {}
        }
        if matches!(mode, Mode::Alert) && !matches!(self.mode, Mode::Scratch | Mode::Wash) {
            self.washed = false;
        }
        self.mode = mode;
        self.mode_time = 0.0;
    }

    /// Next idle activity after `mode` has run its course.
    fn next_idle(&mut self) -> Mode {
        match self.mode {
            Mode::Alert | Mode::Claw(_) => {
                if self.washed {
                    Mode::Yawn
                } else if self.random() < 0.5 {
                    Mode::Scratch
                } else {
                    Mode::Wash
                }
            }
            Mode::Scratch | Mode::Wash => {
                self.washed = true;
                if self.random() < 0.35 { Mode::Alert } else { Mode::Yawn }
            }
            Mode::Yawn => Mode::Sleep,
            other => other,
        }
    }

    fn resting(&self) -> bool {
        !matches!(self.mode, Mode::Run | Mode::Startle)
    }

    /// Advance the simulation. `cursor` is the pointer position and `screen`
    /// the monitor it is on; both in logical desktop pixels.
    pub fn update(&mut self, dt: f64, cursor: Option<(f64, f64)>, screen: Option<Rect>) {
        let dt = dt.clamp(0.0, 0.1);
        self.mode_time += dt;
        let s = self.size;

        // Where the cat wants its top-left corner: centred on the cursor, but
        // kept fully on the cursor's monitor.
        let mut target = None;
        let mut blocked = None;
        if let (Some((cx, cy)), false) = (cursor, self.staying) {
            let (mut tx, mut ty) = (cx - s / 2.0, cy - s / 2.0);
            if let Some(r) = screen {
                let (lo_x, hi_x) = (r.x, r.x + r.w - s);
                let (lo_y, hi_y) = (r.y, r.y + r.h - s);
                let reach = s * 0.35;
                blocked = if ty < lo_y - reach {
                    Some(Edge::Up)
                } else if ty > hi_y + reach {
                    Some(Edge::Down)
                } else if tx < lo_x - reach {
                    Some(Edge::Left)
                } else if tx > hi_x + reach {
                    Some(Edge::Right)
                } else {
                    None
                };
                tx = tx.clamp(lo_x, hi_x.max(lo_x));
                ty = ty.clamp(lo_y, hi_y.max(lo_y));
            }
            target = Some((tx, ty));
        }

        let (dx, dy) = target.map_or((0.0, 0.0), |(tx, ty)| (tx - self.x, ty - self.y));
        let dist = dx.hypot(dy);

        // Chase with hysteresis: set off when the cursor is more than a body
        // length away, settle once close.
        let start = s * 0.9;
        let settle = if blocked.is_some() { 2.0 } else { s * 0.35 };
        match self.mode {
            Mode::Run => {
                if target.is_none() || dist < settle {
                    let next = match blocked {
                        Some(edge) if !self.staying => Mode::Claw(edge),
                        _ => Mode::Alert,
                    };
                    self.enter(next);
                }
            }
            Mode::Startle => {
                if self.mode_time >= self.mode_len {
                    if target.is_some() && dist > settle {
                        self.enter(Mode::Run);
                    } else {
                        self.enter(Mode::Alert);
                    }
                }
            }
            _ => {
                if target.is_some() && dist > start {
                    if matches!(self.mode, Mode::Sleep | Mode::Yawn) {
                        self.enter(Mode::Startle);
                    } else {
                        self.enter(Mode::Run);
                    }
                } else if self.mode_time >= self.mode_len {
                    let next = self.next_idle();
                    self.enter(next);
                }
            }
        }

        // Steering: ease velocity toward the desired one, braking on arrival.
        // The easing makes velocity lag by about v/EASE seconds' worth of
        // travel, so brake that much earlier to arrive without overshooting.
        const EASE: f64 = 9.0;
        let speed = self.vx.hypot(self.vy);
        let (want_vx, want_vy) = if self.mode == Mode::Run && dist > 0.0 {
            let room = (dist - settle * 0.5 - speed / EASE).max(0.0);
            let limit = self.max_speed.min((2.0 * 900.0 * room).sqrt());
            (dx / dist * limit, dy / dist * limit)
        } else {
            (0.0, 0.0)
        };
        let ease = 1.0 - (-(if self.resting() { 2.5 * EASE } else { EASE }) * dt).exp();
        self.vx += (want_vx - self.vx) * ease;
        self.vy += (want_vy - self.vy) * ease;
        if self.resting() && self.vx.hypot(self.vy) < 4.0 {
            self.vx = 0.0;
            self.vy = 0.0;
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // Never slide off the cursor's monitor once on it.
        if let Some(r) = screen {
            let (cx, cy) = self.center();
            if r.contains(cx, cy) {
                self.x = self.x.clamp(r.x, (r.x + r.w - s).max(r.x));
                self.y = self.y.clamp(r.y, (r.y + r.h - s).max(r.y));
            }
        }

        let speed = self.vx.hypot(self.vy);
        self.stride += speed * dt;

        // Facing follows the velocity, with some hysteresis so the cat does
        // not flicker between two directions on a diagonal.
        if speed > 12.0 {
            let angle = self.vy.atan2(self.vx).to_degrees().rem_euclid(360.0);
            let current = self.dir as f64 * 45.0;
            let off = (angle - current).rem_euclid(360.0);
            let off = off.min(360.0 - off);
            if off > 22.5 + 6.0 {
                self.dir = ((angle + 22.5) / 45.0) as usize % 8;
            }
        }

        // Blinking while sitting.
        if matches!(self.mode, Mode::Alert | Mode::Yawn) {
            if self.blink_left > 0.0 {
                self.blink_left -= dt;
            } else {
                self.blink_in -= dt;
                if self.blink_in <= 0.0 {
                    self.blink_left = 0.13;
                    self.blink_in = self.between(1.8, 4.5);
                }
            }
        } else {
            self.blink_left = 0.0;
        }
    }

    pub fn pose(&self) -> Pose {
        let u = self.size / 32.0; // one classic sprite pixel
        let t = self.mode_time;
        let alternate = |period: f64| (t / period) as usize % 2;
        let blinking = self.blink_left > 0.0;
        let mut pose = Pose { frame: "awake", dx: 0.0, dy: 0.0, asleep: None, shadow: 1.0 };

        match self.mode {
            Mode::Run => {
                let stride_len = self.size * 0.42;
                let phase = self.stride / stride_len;
                pose.frame = RUN_FRAMES[self.dir][phase as usize % 2];
                let speed = self.vx.hypot(self.vy) / self.max_speed.max(1.0);
                // A small bounce per stride, stronger at full gallop.
                let lift = (phase.fract() * PI).sin() * 1.6 * u * speed.min(1.0);
                pose.dy = -lift;
                pose.shadow = 1.0 - lift / (6.0 * u);
            }
            Mode::Startle => {
                // Hop straight up and land.
                let k = (t / self.mode_len).min(1.0);
                let lift = (k * PI).sin() * 5.0 * u;
                pose.dy = -lift;
                pose.shadow = 1.0 - lift / (8.0 * u);
            }
            Mode::Alert => {
                pose.frame = if blinking { "awake_blink" } else { "awake" };
            }
            Mode::Scratch => pose.frame = ["scratch1", "scratch2"][alternate(0.12)],
            Mode::Wash => {
                pose.frame = "wash";
                // Head bobs while licking a paw.
                pose.dy = alternate(0.28) as f64 * u;
            }
            Mode::Yawn => {
                pose.frame = if t < 0.9 {
                    if blinking { "yawn1_blink" } else { "yawn1" }
                } else if t < 2.0 {
                    "yawn2"
                } else {
                    "yawn1_blink"
                };
            }
            Mode::Sleep => {
                // Slow breathing between the two sleeping frames.
                pose.frame = ["sleep1", "sleep2"][alternate(1.15)];
                pose.asleep = Some(t);
            }
            Mode::Claw(edge) => {
                let [a, b] = match edge {
                    Edge::Up => ["upclaw1", "upclaw2"],
                    Edge::Right => ["rightclaw1", "rightclaw2"],
                    Edge::Down => ["downclaw1", "downclaw2"],
                    Edge::Left => ["leftclaw1", "leftclaw2"],
                };
                pose.frame = [a, b][alternate(0.16)];
            }
        }
        pose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0.0, y: 0.0, w: 1920.0, h: 1080.0 };

    fn run(cat: &mut Cat, secs: f64, cursor: (f64, f64)) {
        for _ in 0..(secs * 60.0) as usize {
            cat.update(1.0 / 60.0, Some(cursor), Some(SCREEN));
        }
    }

    #[test]
    fn chases_and_settles_near_the_cursor() {
        let mut cat = Cat::new(100.0, 100.0, 64.0, 2.0, 7);
        run(&mut cat, 8.0, (900.0, 500.0));
        let (cx, cy) = cat.center();
        assert!((cx - 900.0).hypot(cy - 500.0) < 64.0, "cat at {cx},{cy}");
        assert!(matches!(cat.mode, Mode::Alert | Mode::Scratch | Mode::Wash | Mode::Yawn));
    }

    #[test]
    fn faces_where_it_runs() {
        let mut cat = Cat::new(100.0, 500.0, 64.0, 2.0, 7);
        run(&mut cat, 0.5, (1500.0, 530.0));
        assert!(cat.pose().frame.starts_with("right"), "frame {}", cat.pose().frame);
    }

    #[test]
    fn falls_asleep_when_left_alone() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        run(&mut cat, 30.0, (532.0, 532.0));
        assert_eq!(cat.mode, Mode::Sleep);
        assert!(cat.sounds.contains(&Sound::Sleep));
    }

    #[test]
    fn wakes_with_a_startle() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        run(&mut cat, 30.0, (532.0, 532.0));
        run(&mut cat, 0.1, (1500.0, 900.0));
        assert_eq!(cat.mode, Mode::Startle);
        run(&mut cat, 1.0, (1500.0, 900.0));
        assert_eq!(cat.mode, Mode::Run);
    }

    #[test]
    fn claws_at_the_edge_when_cursor_is_out_of_reach() {
        let mut cat = Cat::new(500.0, 300.0, 64.0, 2.0, 3);
        let mut clawed = false;
        for _ in 0..300 {
            cat.update(1.0 / 60.0, Some((532.0, 2.0)), Some(SCREEN));
            clawed |= matches!(cat.mode, Mode::Claw(Edge::Up));
        }
        assert!(clawed, "never clawed; mode {:?}", cat.mode);
        assert!(cat.y.abs() < 3.0, "y {}", cat.y);
    }

    #[test]
    fn stays_put_when_told() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.staying = true;
        run(&mut cat, 2.0, (1500.0, 900.0));
        assert_eq!((cat.x, cat.y), (500.0, 500.0));
    }
}
