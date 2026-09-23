//! Toys, thrown in when the cat has slept for a while: mice that scurry about
//! like wind-up toys, balls that roll, radishes that hop, feather wands
//! dangled from somewhere off screen and a butterfly that circles its base.
//! They run from the cat until the clockwork runs down, after about two
//! minutes. A wand can also hang from the mouse pointer on a string while the
//! pointer moves fast. What they look like lives in toy_art.rs.

use std::f64::consts::{PI, TAU};

use crate::cat::{Rect, Rng};

/// Gravity, in classic sprite pixels per second squared.
const GRAVITY: f64 = 110.0;
/// Radius of a ball, and of the butterfly's circle, in classic pixels.
pub const BALL: f64 = 3.8;
pub const ORBIT: f64 = 11.0;
/// Height the butterfly flies at, in classic pixels.
pub const FLIGHT: f64 = 9.0;
/// Length of the string a toy hangs from the pointer by, in classic pixels.
const LEASH: f64 = 12.0;
/// Wands that can hang from the pointer.
pub const LEASHES: [Kind; 3] = [Kind::RainbowWand, Kind::SpringWand, Kind::MarabouWand];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    StrawMouse,
    PlushMouse,
    PlaidMouse,
    Squirrel,
    Sardine,
    Candy,
    RedRadish,
    WhiteRadish,
    RedBall,
    BlueBall,
    PinkBall,
    RainbowWand,
    SpringWand,
    MarabouWand,
    Butterfly,
}

pub const KINDS: [Kind; 15] = [
    Kind::StrawMouse,
    Kind::PlushMouse,
    Kind::PlaidMouse,
    Kind::Squirrel,
    Kind::Sardine,
    Kind::Candy,
    Kind::RedRadish,
    Kind::WhiteRadish,
    Kind::RedBall,
    Kind::BlueBall,
    Kind::PinkBall,
    Kind::RainbowWand,
    Kind::SpringWand,
    Kind::MarabouWand,
    Kind::Butterfly,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion {
    Scurry,
    Roll,
    Hop,
    Dangle,
    Spin,
    /// Hanging from the pointer on a string.
    Leash,
}

impl Kind {
    /// How far above its lowest point a dangled toy hangs from its string,
    /// in classic pixels.
    pub fn pivot(self) -> f64 {
        match self {
            Kind::SpringWand => 8.5,
            Kind::MarabouWand => 12.0,
            _ => 7.0,
        }
    }

    fn motion(self) -> Motion {
        match self {
            Kind::RedBall | Kind::BlueBall | Kind::PinkBall => Motion::Roll,
            Kind::RedRadish | Kind::WhiteRadish => Motion::Hop,
            Kind::RainbowWand | Kind::SpringWand | Kind::MarabouWand => Motion::Dangle,
            Kind::Butterfly => Motion::Spin,
            _ => Motion::Scurry,
        }
    }
}

/// What to draw this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToyPose {
    pub kind: Kind,
    /// Picks a colour, for toys that come in several.
    pub variant: usize,
    /// Where it is headed, radians, screen coordinates.
    pub heading: f64,
    /// 1.0 facing right, -1.0 facing left, for toys drawn from the side.
    pub facing: f64,
    /// Height above the ground, logical pixels.
    pub z: f64,
    /// A running phase (radians) for tails, wings and wobbles, and how much
    /// they swing, 0..1.
    pub tail: f64,
    pub sway: f64,
    pub alpha: f64,
    /// A ball's orientation, as a unit quaternion (w, x, y, z).
    pub roll: [f64; 4],
    /// Where the butterfly is on its circle, radians.
    pub orbit: f64,
    /// How far a dangled toy swings on its string, radians.
    pub swing: f64,
    /// Leaning into the run, radians.
    pub tilt: f64,
    /// Where the pointer holding the string is, relative to the toy, in
    /// classic pixels; for a toy on the pointer's leash.
    pub leash: Option<(f64, f64)>,
}

pub struct Toy {
    pub kind: Kind,
    variant: usize,
    motion: Motion,
    /// Where the toy touches the ground (a wand: where it hangs over),
    /// logical desktop pixels.
    pub x: f64,
    pub y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    heading: f64,
    facing: f64,
    aim: f64,
    spin: f64,
    /// Size of the cat, and one of its classic sprite pixels.
    size: f64,
    u: f64,
    top_speed: f64,
    dart_speed: f64,
    landed: bool,
    pub just_landed: bool,
    /// Seconds of clockwork left.
    wind: f64,
    darting: bool,
    burst: f64,
    rest: f64,
    tail: f64,
    sway: f64,
    roll: [f64; 4],
    orbit: f64,
    orbit_dir: f64,
    swing: f64,
    swing_v: f64,
    playing: bool,
    linger: f64,
    alpha: f64,
    /// The pointer a leashed toy hangs from, logical desktop pixels.
    pub anchor: Option<(f64, f64)>,
    rng: Rng,
}

/// Signed smallest turn from `from` to `to`.
fn turn(from: f64, to: f64) -> f64 {
    (to - from + PI).rem_euclid(TAU) - PI
}

/// Hamilton product a*b.
fn qmul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

impl Toy {
    /// Throw a toy of `kind` in from the side of `arena`, landing well away
    /// from the cat at `cat` so it has to run for it. `cat_speed` is the
    /// cat's top speed; the toy is a little slower.
    pub fn throw(kind: Kind, arena: Rect, cat: (f64, f64), size: f64, cat_speed: f64, rng: &mut Rng) -> Toy {
        let mut rng = rng.fork();
        let mut land = (arena.x + arena.w / 2.0, arena.y + arena.h / 2.0);
        let mut best = -1.0;
        for _ in 0..16 {
            let p = (arena.x + rng.next() * arena.w, arena.y + rng.next() * arena.h);
            let d = (p.0 - cat.0).hypot(p.1 - cat.1);
            if (size * 3.0..size * 8.0).contains(&d) {
                land = p;
                break;
            }
            if d > best {
                best = d;
                land = p;
            }
        }
        let from_left = land.0 - arena.x < arena.x + arena.w - land.0;
        let sx = if from_left { arena.x - size * 1.1 } else { arena.x + arena.w + size * 1.1 };
        let u = size / 32.0;
        let motion = kind.motion();
        let (sy, vx, vy, z, vz) = if motion == Motion::Dangle {
            // Dangled in from the side on its string.
            let t = (land.0 - sx).abs() / (cat_speed * 0.8);
            (land.1, (land.0 - sx) / t, 0.0, 6.0 * u, 0.0)
        } else {
            let sy = (land.1 - rng.between(0.0, 0.25) * arena.h).max(arena.y);
            let t = rng.between(0.8, 1.1);
            (sy, (land.0 - sx) / t, (land.1 - sy) / t, 0.0, GRAVITY * u * t / 2.0)
        };
        Toy {
            kind,
            variant: (rng.next() * 3.0) as usize,
            motion,
            x: sx,
            y: sy,
            z,
            vx,
            vy,
            vz,
            heading: vy.atan2(vx),
            facing: vx.signum(),
            aim: 0.0,
            spin: if motion == Motion::Scurry { rng.between(-6.0, 6.0) } else { 0.0 },
            size,
            u,
            top_speed: cat_speed,
            dart_speed: 0.0,
            landed: false,
            just_landed: false,
            wind: rng.between(115.0, 125.0),
            darting: false,
            burst: 0.0,
            rest: 0.0,
            tail: 0.0,
            sway: 0.0,
            roll: [1.0, 0.0, 0.0, 0.0],
            orbit: rng.between(0.0, TAU),
            orbit_dir: if rng.next() < 0.5 { 1.0 } else { -1.0 },
            swing: 0.0,
            swing_v: 0.0,
            playing: true,
            linger: 0.0,
            alpha: 1.0,
            anchor: None,
            rng,
        }
    }

    /// A wand of `kind` hanging from the pointer at `cursor`.
    pub fn leash(kind: Kind, cursor: (f64, f64), size: f64, cat_speed: f64, rng: &mut Rng) -> Toy {
        let spot = Rect { x: cursor.0, y: cursor.1, w: 1.0, h: 1.0 };
        let mut toy = Toy::throw(kind, spot, cursor, size, cat_speed, rng);
        toy.motion = Motion::Leash;
        toy.x = cursor.0;
        toy.y = cursor.1 + (LEASH + kind.pivot()) * toy.u;
        (toy.vx, toy.vy, toy.vz, toy.z) = (0.0, 0.0, 0.0, 0.0);
        toy.landed = true;
        toy.anchor = Some(cursor);
        toy
    }

    pub fn leashed(&self) -> bool {
        self.motion == Motion::Leash
    }

    /// Worth chasing: arrived and still in play.
    pub fn chaseable(&self) -> bool {
        self.playing && self.landed
    }

    /// Faded out after play ended; drop it.
    pub fn gone(&self) -> bool {
        self.alpha <= 0.0
    }

    /// End the game early; the toy fades away.
    pub fn stop(&mut self) {
        if self.playing {
            self.playing = false;
            self.linger = 0.3;
        }
    }

    /// The part the cat goes for, and how high it is: logical pixels.
    pub fn prey(&self) -> (f64, f64, f64) {
        if self.motion == Motion::Spin {
            let r = ORBIT * self.u;
            (self.x + r * self.orbit.cos(), self.y + r * 0.5 * self.orbit.sin(), FLIGHT * self.u)
        } else {
            (self.x, self.y, self.z)
        }
    }

    /// The cat swats the toy in direction (`dx`, `dy`), a unit vector.
    pub fn bat(&mut self, dx: f64, dy: f64) {
        let u = self.u;
        let side = if self.rng.next() < 0.5 { 1.0 } else { -1.0 };
        self.rest = self.rng.between(0.3, 0.6);
        self.darting = false;
        self.burst = 0.0;
        match self.motion {
            Motion::Leash => {
                let v = self.top_speed * 1.2;
                self.vx += dx * v;
                self.vy += dy * v;
            }
            Motion::Dangle => {
                // Whoever holds the string yanks it away.
                let v = self.top_speed * self.rng.between(0.8, 1.1);
                self.vx = dx * v;
                self.vy = dy * v;
                self.z += 5.0 * u;
                self.swing_v += side * self.rng.between(6.0, 10.0);
            }
            Motion::Spin => {
                // The base barely moves; the butterfly turns around.
                let v = self.top_speed * 0.3;
                self.vx = dx * v;
                self.vy = dy * v;
                self.vz = 8.0 * u;
                self.orbit_dir = -self.orbit_dir;
                self.orbit += self.orbit_dir * 0.5;
                self.rest = 0.0;
            }
            Motion::Roll => {
                let v = self.top_speed * self.rng.between(1.3, 1.8);
                self.vx = dx * v;
                self.vy = dy * v;
                self.vz = self.rng.between(8.0, 16.0) * u;
            }
            Motion::Scurry | Motion::Hop => {
                let v = self.top_speed * self.rng.between(1.1, 1.6);
                self.vx = dx * v;
                self.vy = dy * v;
                self.vz = self.rng.between(22.0, 34.0) * u;
                self.spin = side * self.rng.between(7.0, 13.0);
            }
        }
    }

    /// Advance the toy. `arena` is where it may roam and `cat` where the
    /// cat's front paws are, both in logical desktop pixels.
    pub fn update(&mut self, dt: f64, arena: Option<Rect>, cat: (f64, f64)) {
        let u = self.u;
        self.just_landed = false;
        let speed = self.vx.hypot(self.vy);
        let pace = (speed / self.top_speed.max(1.0)).min(1.0);
        let airborne = self.motion != Motion::Dangle && (self.z > 0.0 || self.vz > 0.0);

        // Tails stream behind while running and flutter in the air.
        let want_sway = if airborne || self.motion == Motion::Dangle { 0.8f64.max(pace) } else { pace };
        self.sway += (want_sway - self.sway) * (1.0 - (-4.0 * dt).exp());
        self.tail = (self.tail + dt * (3.0 + 14.0 * pace)) % (TAU * 8.0);

        if !self.playing {
            self.linger -= dt;
            if self.linger <= 0.0 {
                self.alpha = (self.alpha - dt * 1.5).max(0.0);
            }
        }

        if self.motion == Motion::Leash {
            self.swing_on_leash(dt);
            return;
        }
        // The clockwork keeps ticking in the air (a hopper is mostly there).
        let running = self.motion != Motion::Dangle && self.clockwork(dt, arena, cat);
        if self.motion == Motion::Dangle {
            self.dangle(dt, arena, cat);
        } else if airborne {
            self.vz -= GRAVITY * u * dt;
            self.z += self.vz * dt;
            self.heading += self.spin * dt;
            if self.z <= 0.0 {
                self.z = 0.0;
                self.just_landed = !self.landed;
                self.landed = true;
                let hopping = self.motion == Motion::Hop && self.darting;
                if self.vz < -14.0 * u && !hopping {
                    // Bounce, losing most of the energy.
                    self.vz = -self.vz * 0.35;
                    self.vx *= 0.55;
                    self.vy *= 0.55;
                    self.spin *= 0.5;
                } else {
                    self.vz = 0.0;
                    self.spin = 0.0;
                    if !hopping {
                        self.rest = self.rng.between(0.3, 0.7);
                    }
                }
            }
        } else {
            let moving = running && self.darting && self.motion != Motion::Spin;
            let (want_vx, want_vy) = if moving {
                self.heading += turn(self.heading, self.aim) * (1.0 - (-6.0 * dt).exp());
                // A little waddle, and slowing down as it runs out of wind.
                let waddle = if self.motion == Motion::Scurry { (self.tail * 2.0).sin() * 0.25 } else { 0.0 };
                let h = self.heading + waddle;
                let v = self.dart_speed * (self.wind / 3.0).min(1.0);
                (h.cos() * v, h.sin() * v)
            } else {
                (0.0, 0.0)
            };
            let grip = if moving { 10.0 } else if self.motion == Motion::Roll { 1.2 } else { 3.5 };
            let k = 1.0 - (-grip * dt).exp();
            self.vx += (want_vx - self.vx) * k;
            self.vy += (want_vy - self.vy) * k;
            if !moving && self.vx.hypot(self.vy) < 2.0 * u {
                self.vx = 0.0;
                self.vy = 0.0;
            }
            if moving && self.motion == Motion::Hop {
                self.vz = self.rng.between(22.0, 28.0) * u;
            }
            if running && self.motion == Motion::Spin {
                let pace = (self.wind / 3.0).min(1.0);
                self.orbit += self.orbit_dir * 2.4 * pace * dt;
            }
        }
        let (dx, dy) = (self.vx * dt, self.vy * dt);
        self.x += dx;
        self.y += dy;

        // A rolling ball turns about the axis across its path.
        let dist = dx.hypot(dy);
        if self.motion == Motion::Roll && dist > 0.0 {
            let half = dist / (BALL * u) / 2.0;
            let (ax, ay) = (-dy / dist, dx / dist);
            let dq = [half.cos(), ax * half.sin(), ay * half.sin(), 0.0];
            let q = qmul(dq, self.roll);
            let n = q.iter().map(|c| c * c).sum::<f64>().sqrt();
            self.roll = q.map(|c| c / n);
        }

        let c = self.heading.cos();
        if c.abs() > 0.3 {
            self.facing = c.signum();
        }

        // Once in, stay in: bounce off the sides of the arena.
        if let (true, Some(r)) = (self.landed, arena) {
            let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
            let mut hit = false;
            if self.x < r.x || self.x > r.x + r.w {
                self.x = self.x.clamp(r.x, r.x + r.w);
                self.vx = -self.vx * 0.6;
                hit = true;
            }
            if self.y < r.y || self.y > r.y + r.h {
                self.y = self.y.clamp(r.y, r.y + r.h);
                self.vy = -self.vy * 0.6;
                hit = true;
            }
            if hit {
                self.aim = (cy - self.y).atan2(cx - self.x);
                self.heading = self.vy.atan2(self.vx);
            }
        }

        // Run down and back on the floor: the game is over, even if the cat's
        // last swat still has it sliding.
        let grounded = self.z <= 0.0 && self.vz <= 0.0;
        if self.playing && self.landed && self.wind <= 0.0 && grounded {
            self.playing = false;
            self.linger = 6.0;
        }
    }

    /// Run the clockwork: dart, pause, dart again in a new direction.
    /// Returns whether it is running at all.
    fn clockwork(&mut self, dt: f64, arena: Option<Rect>, cat: (f64, f64)) -> bool {
        self.rest -= dt;
        let wound = self.playing && self.landed && self.wind > 0.0;
        if wound {
            self.wind -= dt;
        }
        let running = wound && self.rest <= 0.0;
        if running {
            self.burst -= dt;
            if self.burst <= 0.0 {
                self.darting = !self.darting;
                if self.darting {
                    self.burst = self.rng.between(0.5, 1.4);
                    self.aim = self.pick_aim(arena, cat);
                    self.dart_speed = self.top_speed * self.rng.between(0.55, 0.9);
                    // Now and then the butterfly changes its mind.
                    if self.motion == Motion::Spin && self.rng.next() < 0.3 {
                        self.orbit_dir = -self.orbit_dir;
                    }
                } else {
                    self.burst = self.rng.between(0.2, 0.8);
                }
            }
        } else {
            self.darting = false;
        }
        running
    }

    /// A wand's toy on a string: swoops about in the air, dips to the floor
    /// when it pauses and swings on its string.
    fn dangle(&mut self, dt: f64, arena: Option<Rect>, cat: (f64, f64)) {
        let u = self.u;
        if !self.landed {
            // Carried in from off screen until it is over the arena.
            if arena.is_none_or(|r| r.contains(self.x, self.y)) {
                self.landed = true;
                self.just_landed = true;
                self.rest = 0.3;
            }
        } else {
            let running = self.clockwork(dt, arena, cat);
            let moving = running && self.darting;
            let (want_vx, want_vy) = if moving {
                self.heading += turn(self.heading, self.aim) * (1.0 - (-5.0 * dt).exp());
                let v = self.dart_speed * (self.wind / 3.0).min(1.0);
                (self.heading.cos() * v, self.heading.sin() * v)
            } else {
                (0.0, 0.0)
            };
            let k = 1.0 - (-(if moving { 6.0 } else { 4.0 }) * dt).exp();
            self.vx += (want_vx - self.vx) * k;
            self.vy += (want_vy - self.vy) * k;
            if !moving && self.vx.hypot(self.vy) < 2.0 * u {
                self.vx = 0.0;
                self.vy = 0.0;
            }
        }
        // Up while swooping, down to the floor to twitch there when it
        // pauses, and dropped once the game is over.
        let want_z = if !self.playing || (self.landed && self.wind <= 0.0) {
            0.0
        } else if self.darting || !self.landed {
            (5.0 + 3.0 * (self.tail * 0.7).sin()) * u
        } else {
            (0.8 + 0.6 * (self.tail * 3.0).sin().max(0.0)) * u
        };
        self.z += (want_z - self.z) * (1.0 - (-5.0 * dt).exp());
        if want_z == 0.0 && self.z < 0.2 * u {
            self.z = 0.0;
        }
        self.vz = 0.0;
        // A pendulum on the string, pulled back by the motion.
        let lean = (-self.vx / self.top_speed.max(1.0) * 0.9).clamp(-1.2, 1.2);
        self.swing_v += (-(self.swing - lean) * 40.0 - self.swing_v * 3.0) * dt;
        self.swing += self.swing_v * dt;
    }

    /// A pendulum on a string from the pointer: gravity pulls it down, the
    /// string holds it, and it trails and swings as the pointer moves.
    fn swing_on_leash(&mut self, dt: f64) {
        let u = self.u;
        let Some((ax, ay)) = self.anchor else { return };
        let lift = self.kind.pivot() * u;
        let (mut px, mut py) = (self.x, self.y - lift);
        self.vy += 300.0 * u * dt;
        let damp = (-1.5 * dt).exp();
        self.vx *= damp;
        self.vy *= damp;
        px += self.vx * dt;
        py += self.vy * dt;
        let (dx, dy) = (px - ax, py - ay);
        let d = dx.hypot(dy);
        let len = LEASH * u;
        if d > len * 4.0 {
            // Flung far in one frame: the toy just comes along.
            (px, py) = (ax, ay + len);
            (self.vx, self.vy) = (0.0, 0.0);
        } else if d > len {
            // Taut: back onto the circle, keeping only the sideways motion.
            let (nx, ny) = (dx / d, dy / d);
            px = ax + nx * len;
            py = ay + ny * len;
            let radial = self.vx * nx + self.vy * ny;
            if radial > 0.0 {
                self.vx -= radial * nx;
                self.vy -= radial * ny;
            }
        }
        self.x = px;
        self.y = py + lift;
        self.swing = (-(px - ax)).atan2(py - ay);
    }

    /// Where to dart next: somewhere new, away from the cat if it is close,
    /// back toward the middle if near an edge.
    fn pick_aim(&mut self, arena: Option<Rect>, cat: (f64, f64)) -> f64 {
        let (dx, dy) = (self.x - cat.0, self.y - cat.1);
        let mut aim = if dx.hypot(dy) < self.size * 2.5 {
            dy.atan2(dx) + self.rng.between(-0.9, 0.9)
        } else {
            self.heading + self.rng.between(-1.3, 1.3)
        };
        if let Some(r) = arena {
            let m = self.size * 0.8;
            if self.x < r.x + m || self.x > r.x + r.w - m || self.y < r.y + m || self.y > r.y + r.h - m {
                let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                aim = (cy - self.y).atan2(cx - self.x) + self.rng.between(-0.5, 0.5);
            }
        }
        aim
    }

    pub fn pose(&self) -> ToyPose {
        // Quantized, so a toy lying still does not repaint.
        let q = |v: f64, step: f64| (v / step).round() * step;
        let tilt = match self.motion {
            Motion::Hop => (self.vx / self.top_speed.max(1.0)).clamp(-1.0, 1.0) * 0.3,
            _ => 0.0,
        };
        ToyPose {
            kind: self.kind,
            variant: self.variant,
            heading: q(self.heading.rem_euclid(TAU), TAU / 64.0),
            facing: self.facing,
            z: q(self.z, 0.5),
            tail: q(self.tail, TAU / 48.0),
            sway: q(self.sway, 1.0 / 16.0),
            alpha: q(self.alpha, 1.0 / 32.0),
            roll: self.roll.map(|c| q(c, 1.0 / 256.0)),
            orbit: q(self.orbit, TAU / 128.0),
            swing: q(self.swing, 0.02),
            tilt: q(tilt, 0.02),
            leash: self
                .anchor
                .filter(|_| self.leashed())
                .map(|(ax, ay)| ((ax - self.x) / self.u, (ay - self.y) / self.u)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARENA: Rect = Rect { x: 40.0, y: 60.0, w: 1840.0, h: 1000.0 };

    #[test]
    fn every_toy_arrives_and_runs_down() {
        for kind in KINDS {
            let mut rng = Rng::new(9);
            let mut toy = Toy::throw(kind, ARENA, (500.0, 500.0), 64.0, 220.0, &mut rng);
            let mut arrived = None;
            for i in 0..(140 * 60) {
                toy.update(1.0 / 60.0, Some(ARENA), (500.0, 500.0));
                if toy.just_landed {
                    arrived = Some(i);
                }
                let (x, y, _) = toy.prey();
                assert!(x.is_finite() && y.is_finite(), "{kind:?}");
            }
            assert!(arrived.is_some_and(|i| i < 300), "{kind:?} arrived at {arrived:?}");
            assert!(!toy.playing, "{kind:?} still playing");
            assert!(toy.gone(), "{kind:?} not gone");
        }
    }

    #[test]
    fn a_leashed_toy_hangs_below_the_pointer() {
        let mut rng = Rng::new(4);
        let mut toy = Toy::leash(Kind::SpringWand, (500.0, 300.0), 64.0, 220.0, &mut rng);
        for _ in 0..240 {
            toy.anchor = Some((500.0, 300.0));
            toy.update(1.0 / 60.0, Some(ARENA), (0.0, 0.0));
        }
        let pivot = toy.y - Kind::SpringWand.pivot() * 2.0;
        assert!((toy.x - 500.0).abs() < 2.0 && (pivot - 324.0).abs() < 2.0, "at {},{}", toy.x, pivot);
        assert!(toy.pose().leash.is_some());
    }
}
