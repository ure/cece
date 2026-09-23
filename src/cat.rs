//! The cat's brain: steering toward the cursor and the idle routine.
//!
//! Everything is in logical desktop pixels and seconds. Instead of the classic
//! neko's fixed hops per tick, the cat steers: its velocity eases toward the
//! cursor, it slows down as it arrives, its legs cycle with the distance it
//! actually covers and it bounces a little on every stride.

use std::f64::consts::PI;

use crate::{
    bed::{BEDS, Bed},
    toy::{KINDS, LEASHES, Toy},
};

/// How long a round of play with thrown toys lasts before she naps again.
const SESSION: f64 = 600.0;
/// Pointer speeds (logical px/s) that bring out the wand, and that count as
/// slowed down again.
const FLICK: f64 = 900.0;
const CALM: f64 = 150.0;

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
    /// Swatting the toy mouse.
    Bat(Edge),
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
    /// The bed she sleeps in, and how faded in it is.
    pub bed: Option<(Bed, f64)>,
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

/// xorshift64*, plenty for a cat.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }

    fn raw(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in 0..1.
    pub fn next(&mut self) -> f64 {
        (self.raw() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }

    /// An independent generator seeded from this one.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.raw())
    }

    /// A random index below `n`, never `last` (when there is a choice).
    pub fn other(&mut self, n: usize, last: Option<usize>) -> usize {
        match last {
            Some(l) if n > 1 && l < n => (l + 1 + (self.next() * (n - 1) as f64) as usize % (n - 1)) % n,
            _ => (self.next() * n as f64) as usize % n,
        }
    }
}

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
    rng: Rng,
    pub staying: bool,
    pub sounds: Vec<Sound>,
    /// Seconds asleep before someone throws in a toy mouse; 0 for never.
    pub toy_after: f64,
    /// Throw in a toy as soon as possible, nap or not.
    pub toy_now: bool,
    pub toy: Option<Toy>,
    /// Where the cursor was when the toy came in; moving it ends the game.
    toy_cursor: Option<(f64, f64)>,
    /// Never the same toy or bed twice in a row.
    last_toy: Option<usize>,
    last_bed: Option<usize>,
    last_leash: Option<usize>,
    bed: Option<Bed>,
    bed_alpha: f64,
    /// Seconds into a round of play: toy after toy, then a nap.
    session: Option<f64>,
    /// You are typing: the cursor is not to be chased.
    pub typing: bool,
    last_cursor: Option<(f64, f64)>,
    cursor_speed: f64,
    /// How long the pointer has been calm while a wand hangs from it.
    calm: f64,
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
            rng: Rng::new(seed),
            staying: false,
            sounds: Vec::new(),
            toy_after: 60.0,
            toy_now: false,
            toy: None,
            toy_cursor: None,
            last_toy: None,
            last_bed: None,
            last_leash: None,
            session: None,
            typing: false,
            last_cursor: None,
            cursor_speed: 0.0,
            calm: 0.0,
            bed: None,
            bed_alpha: 0.0,
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
        self.rng.next()
    }

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        self.rng.between(lo, hi)
    }

    /// Where the front paws are, where the cat wants the toy.
    fn paws(&self) -> (f64, f64) {
        (self.x + self.size / 2.0, self.y + self.size * 0.9)
    }

    /// Where the toy may roam on `r`: far enough from the edges that the cat,
    /// which stays fully on screen, can still reach it.
    fn arena(&self, r: Rect) -> Rect {
        let s = self.size;
        Rect { x: r.x + s * 0.6, y: r.y + s, w: (r.w - s * 1.2).max(1.0), h: (r.h - s * 1.3).max(1.0) }
    }

    fn enter(&mut self, mode: Mode) {
        self.mode_len = match mode {
            Mode::Startle => 0.45,
            Mode::Alert => self.between(1.4, 3.2),
            Mode::Scratch => self.between(1.0, 1.8),
            Mode::Wash => self.between(1.2, 2.2),
            Mode::Yawn => 2.4,
            Mode::Claw(_) => self.between(2.0, 3.5),
            Mode::Bat(_) => 0.35,
            Mode::Run | Mode::Sleep => f64::INFINITY,
        };
        match mode {
            Mode::Yawn => self.sounds.push(Sound::Yawn),
            Mode::Sleep => {
                self.sounds.push(Sound::Sleep);
                let i = self.rng.other(BEDS.len(), self.last_bed);
                self.last_bed = Some(i);
                self.bed = Some(BEDS[i]);
                self.bed_alpha = 0.0;
            }
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
            Mode::Bat(_) => Mode::Alert,
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

    /// Hop over to monitor `to` (out of the way while you type) and settle
    /// somewhere on it. Any game in progress is over.
    pub fn away(&mut self, to: Rect) {
        let s = self.size;
        let (w, h) = ((to.w - s).max(0.0), (to.h - s).max(0.0));
        self.x = to.x + w * self.between(0.3, 0.7);
        self.y = to.y + h * self.between(0.4, 0.8);
        self.vx = 0.0;
        self.vy = 0.0;
        self.toy = None;
        self.session = None;
        self.bed = None;
        self.bed_alpha = 0.0;
        self.enter(Mode::Startle);
        // A quiet landing: no need to meow at someone typing.
        self.sounds.retain(|s| *s != Sound::Awake);
    }

    /// Advance the simulation. `cursor` is the pointer position, `screen`
    /// the monitor it is on and `home` the monitor the cat is on; all in
    /// logical desktop pixels.
    pub fn update(&mut self, dt: f64, cursor: Option<(f64, f64)>, screen: Option<Rect>, home: Option<Rect>) {
        let dt = dt.clamp(0.0, 0.1);
        self.mode_time += dt;
        let s = self.size;

        // How fast the pointer moves, smoothed.
        if let (Some((x, y)), Some((ox, oy)), true) = (cursor, self.last_cursor, dt > 0.0) {
            let raw = (x - ox).hypot(y - oy) / dt;
            self.cursor_speed += (raw - self.cursor_speed) * (1.0 - (-10.0 * dt).exp());
        }
        self.last_cursor = cursor;

        // Flick the mouse and a wand hangs from the pointer to chase, until
        // the pointer calms down again. It takes over from any thrown toy.
        let leashed = self.toy.as_ref().is_some_and(|t| t.leashed());
        if let (Some(c), false, false, false) = (cursor, leashed, self.typing, self.staying) {
            if self.cursor_speed > FLICK {
                let i = self.rng.other(LEASHES.len(), self.last_leash);
                self.last_leash = Some(i);
                self.toy = Some(Toy::leash(LEASHES[i], c, s, self.max_speed, &mut self.rng));
                self.session = None;
                self.calm = 0.0;
            }
        }

        // After a good nap, someone throws in a toy; during a round of play,
        // the next one (never the same) as soon as the last one is done.
        if let Some(t) = &mut self.session {
            *t += dt;
        }
        let napped = self.mode == Mode::Sleep && self.mode_time >= self.toy_after && self.toy_after > 0.0;
        let more = self.session.is_some_and(|t| t < SESSION);
        if let (true, None, false, Some(r)) = (napped || more || self.toy_now, &self.toy, self.staying, home) {
            self.toy_now = false;
            self.session.get_or_insert(0.0);
            let arena = self.arena(r);
            let paws = self.paws();
            let i = self.rng.other(KINDS.len(), self.last_toy);
            self.last_toy = Some(i);
            self.toy = Some(Toy::throw(KINDS[i], arena, paws, s, self.max_speed, &mut self.rng));
            self.toy_cursor = cursor;
        }
        let arena = home.map(|r| self.arena(r));
        let paws = self.paws();
        let mut woken = false;
        if let Some(toy) = &mut self.toy {
            if toy.leashed() {
                toy.anchor = cursor.or(toy.anchor);
                self.calm = if self.cursor_speed < CALM { self.calm + dt } else { 0.0 };
                if self.calm > 1.2 || self.typing || self.staying {
                    toy.stop();
                }
            } else {
                // Touching the mouse (or telling the cat to stay) ends the game.
                if self.toy_cursor.is_none() {
                    self.toy_cursor = cursor;
                }
                let moved = match (cursor, self.toy_cursor) {
                    (Some((x, y)), Some((ox, oy))) => (x - ox).hypot(y - oy) > s * 0.5,
                    _ => false,
                };
                if moved || self.staying {
                    toy.stop();
                    self.session = None;
                }
            }
            toy.update(dt, arena, paws);
            woken = toy.just_landed;
            if toy.gone() {
                self.toy = None;
                if self.session.is_some_and(|t| t >= SESSION) {
                    self.session = None;
                }
            }
        }
        if woken && matches!(self.mode, Mode::Sleep | Mode::Yawn) {
            self.enter(Mode::Startle);
        }

        // The bed fades in as she dozes off and away while she hops out
        // (she does not move during the startle, so it never slides along).
        if self.mode == Mode::Sleep {
            self.bed_alpha = (self.bed_alpha + dt * 1.25).min(1.0);
        } else {
            self.bed_alpha = (self.bed_alpha - dt * 3.5).max(0.0);
            if self.bed_alpha == 0.0 {
                self.bed = None;
            }
        }

        // Where the cat wants its top-left corner: centred on the cursor, but
        // kept fully on the cursor's monitor. A toy in play wins over the
        // cursor: then the cat goes for a spot just short of it, so it lands
        // between the front paws.
        let mut target = None;
        let mut blocked = None;
        let toy = self.toy.as_ref().filter(|t| t.chaseable()).map(|t| t.prey());
        // A wand on the pointer can be on another monitor than the cat.
        let field = if self.toy.as_ref().is_some_and(|t| t.leashed()) { screen } else { home };
        if let Some((mx, my, _)) = toy {
            let (ax, ay) = (mx - paws.0, my - paws.1);
            let d = ax.hypot(ay).max(1.0);
            let (mut tx, mut ty) = (mx - ax / d * s * 0.3 - s / 2.0, my - ay / d * s * 0.3 - s * 0.9);
            if let Some(r) = field {
                tx = tx.clamp(r.x, (r.x + r.w - s).max(r.x));
                ty = ty.clamp(r.y, (r.y + r.h - s).max(r.y));
            }
            target = Some((tx, ty));
        } else if let (Some((cx, cy)), false, false) = (cursor, self.staying, self.typing) {
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
        let start = if toy.is_some() { s * 0.5 } else { s * 0.9 };
        let settle = if blocked.is_some() { 2.0 } else if toy.is_some() { s * 0.25 } else { s * 0.35 };
        match self.mode {
            Mode::Run => {
                if let (Some((mx, my, high)), true) = (toy, dist < settle) {
                    // Caught it: swat it away, roughly onward; up at it if it
                    // is in the air.
                    let (ax, ay) = (mx - paws.0, my - paws.1);
                    let angle = ay.atan2(ax) + self.between(-0.8, 0.8);
                    let edge = if high > s * 0.1 {
                        Edge::Up
                    } else if ax.abs() > ay.abs() {
                        if ax > 0.0 { Edge::Right } else { Edge::Left }
                    } else if ay > 0.0 {
                        Edge::Down
                    } else {
                        Edge::Up
                    };
                    if let Some(t) = &mut self.toy {
                        t.bat(angle.cos(), angle.sin());
                    }
                    self.enter(Mode::Bat(edge));
                } else if target.is_none() || dist < settle {
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
        let bed = self.bed.filter(|_| self.bed_alpha > 0.0).map(|b| (b, (self.bed_alpha * 32.0).round() / 32.0));
        let mut pose = Pose { frame: "awake", dx: 0.0, dy: 0.0, asleep: None, shadow: 1.0, bed };

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
            Mode::Claw(edge) | Mode::Bat(edge) => {
                let [a, b] = match edge {
                    Edge::Up => ["upclaw1", "upclaw2"],
                    Edge::Right => ["rightclaw1", "rightclaw2"],
                    Edge::Down => ["downclaw1", "downclaw2"],
                    Edge::Left => ["leftclaw1", "leftclaw2"],
                };
                let period = if matches!(self.mode, Mode::Bat(_)) { 0.09 } else { 0.16 };
                pose.frame = [a, b][alternate(period)];
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
            cat.update(1.0 / 60.0, Some(cursor), Some(SCREEN), Some(SCREEN));
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
            cat.update(1.0 / 60.0, Some((532.0, 2.0)), Some(SCREEN), Some(SCREEN));
            clawed |= matches!(cat.mode, Mode::Claw(Edge::Up));
        }
        assert!(clawed, "never clawed; mode {:?}", cat.mode);
        assert!(cat.y.abs() < 3.0, "y {}", cat.y);
    }

    #[test]
    fn plays_with_a_toy_after_a_nap() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        run(&mut cat, 30.0, (532.0, 532.0));
        assert_eq!(cat.mode, Mode::Sleep);
        assert!(cat.toy.is_none());
        run(&mut cat, 61.0, (532.0, 532.0));
        assert!(cat.toy.is_some(), "no toy after a minute asleep");
        let mut batted = false;
        for _ in 0..(20 * 60) {
            cat.update(1.0 / 60.0, Some((532.0, 532.0)), Some(SCREEN), Some(SCREEN));
            batted |= matches!(cat.mode, Mode::Bat(_));
        }
        assert!(batted, "never caught the toy; mode {:?}", cat.mode);
    }

    #[test]
    fn moving_the_cursor_ends_the_game() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.toy_after = 5.0;
        run(&mut cat, 40.0, (532.0, 532.0));
        assert!(cat.toy.as_ref().is_some_and(|t| t.chaseable()));
        run(&mut cat, 6.0, (1200.0, 300.0));
        assert!(cat.toy.is_none());
        let (cx, cy) = cat.center();
        assert!((cx - 1200.0).hypot(cy - 300.0) < 200.0, "cat at {cx},{cy}");
    }

    #[test]
    fn toy_now_without_a_nap() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.toy_after = 0.0;
        cat.toy_now = true;
        run(&mut cat, 0.1, (532.0, 532.0));
        assert!(cat.toy.is_some());
        let mut batted = false;
        for _ in 0..(20 * 60) {
            cat.update(1.0 / 60.0, Some((532.0, 532.0)), Some(SCREEN), Some(SCREEN));
            batted |= matches!(cat.mode, Mode::Bat(_));
        }
        assert!(batted, "never caught the toy; mode {:?}", cat.mode);
    }

    #[test]
    fn never_the_same_twice_in_a_row() {
        let mut rng = Rng::new(5);
        let mut last = None;
        let mut seen = [false; 15];
        for _ in 0..500 {
            let i = rng.other(15, last);
            assert_ne!(Some(i), last);
            seen[i] = true;
            last = Some(i);
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn sleeps_in_a_bed_and_leaves_it_on_waking() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.toy_after = 0.0;
        run(&mut cat, 30.0, (532.0, 532.0));
        assert_eq!(cat.mode, Mode::Sleep);
        assert!(cat.pose().bed.is_some_and(|(_, a)| a == 1.0));
        run(&mut cat, 1.0, (1500.0, 900.0));
        assert!(cat.pose().bed.is_none());
    }

    #[test]
    fn plays_toy_after_toy_then_naps() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.toy_now = true;
        let mut toys = Vec::new();
        let mut t = 0.0;
        while t < 660.0 {
            cat.update(1.0 / 30.0, Some((532.0, 532.0)), Some(SCREEN), Some(SCREEN));
            t += 1.0 / 30.0;
            if let Some(kind) = cat.toy.as_ref().map(|t| t.kind) {
                if toys.last() != Some(&kind) {
                    toys.push(kind);
                }
            }
        }
        assert!((4..=6).contains(&toys.len()), "{toys:?}");
        assert!(toys.windows(2).all(|w| w[0] != w[1]));
        run(&mut cat, 60.0, (532.0, 532.0));
        assert!(cat.session.is_none());
        assert_eq!(cat.mode, Mode::Sleep);
    }

    #[test]
    fn a_flicked_mouse_brings_out_a_wand() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        for i in 0..30 {
            let x = 600.0 + i as f64 * 40.0;
            cat.update(1.0 / 60.0, Some((x, 400.0)), Some(SCREEN), Some(SCREEN));
        }
        assert!(cat.toy.as_ref().is_some_and(|t| t.leashed()));
        run(&mut cat, 4.0, (1800.0, 400.0));
        assert!(cat.toy.is_none());
    }

    #[test]
    fn keeps_away_while_you_type() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        let other = Rect { x: 1920.0, y: 0.0, w: 1280.0, h: 360.0 };
        cat.typing = true;
        cat.away(other);
        for _ in 0..600 {
            cat.update(1.0 / 60.0, Some((900.0, 500.0)), Some(SCREEN), Some(other));
        }
        let (cx, cy) = cat.center();
        assert!(other.contains(cx, cy), "cat at {cx},{cy}");
    }

    #[test]
    fn no_toy_when_disabled() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.toy_after = 0.0;
        run(&mut cat, 120.0, (532.0, 532.0));
        assert!(cat.toy.is_none());
    }

    #[test]
    fn stays_put_when_told() {
        let mut cat = Cat::new(500.0, 500.0, 64.0, 2.0, 3);
        cat.staying = true;
        run(&mut cat, 2.0, (1500.0, 900.0));
        assert_eq!((cat.x, cat.y), (500.0, 500.0));
    }
}
