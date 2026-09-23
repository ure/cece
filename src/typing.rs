//! Guessing that you are typing, without reading the keyboard.
//!
//! The compositor tells us (ext-idle-notify) when all input has stopped for a
//! moment and when it resumes. Input that resumes while the pointer stays
//! where it was is the keyboard (or a click or a scroll): you are typing.
//! Moving the pointer again means you are done.

use std::time::{Duration, Instant};

/// How long to wait after input resumes before looking at the pointer, so
/// the cursor poll has caught up with any motion that woke us.
const SETTLE: Duration = Duration::from_millis(60);
/// Pointer travel, in logical pixels, that counts as using the mouse.
const MOVED: f64 = 24.0;
const STILL: f64 = 3.0;

#[derive(Default)]
pub struct Typing {
    /// Where the pointer was when input stopped.
    idle_at: Option<Option<(f64, f64)>>,
    resumed: Option<(Instant, Option<(f64, f64)>)>,
    anchor: Option<(f64, f64)>,
    pub active: bool,
}

fn apart(a: Option<(f64, f64)>, b: Option<(f64, f64)>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => (a.0 - b.0).hypot(a.1 - b.1),
        _ => 0.0,
    }
}

impl Typing {
    pub fn idled(&mut self, cursor: Option<(f64, f64)>) {
        self.idle_at = Some(cursor);
        self.resumed = None;
    }

    pub fn resumed(&mut self, now: Instant) {
        if let Some(at) = self.idle_at.take() {
            self.resumed = Some((now, at));
        }
    }

    /// Call every tick. Returns true when typing has just started.
    pub fn update(&mut self, now: Instant, cursor: Option<(f64, f64)>) -> bool {
        let mut started = false;
        if let Some((when, at)) = self.resumed {
            if now.duration_since(when) >= SETTLE {
                self.resumed = None;
                if apart(at, cursor) <= STILL && !self.active {
                    self.active = true;
                    self.anchor = cursor;
                    started = true;
                }
            }
        }
        if self.active && apart(self.anchor, cursor) > MOVED {
            self.active = false;
        }
        started
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_after_a_pause_are_typing_and_the_mouse_ends_it() {
        let t0 = Instant::now();
        let mut typing = Typing::default();
        typing.idled(Some((100.0, 100.0)));
        typing.resumed(t0);
        assert!(!typing.update(t0, Some((100.0, 100.0))));
        assert!(typing.update(t0 + SETTLE, Some((100.0, 100.0))));
        assert!(typing.active);
        assert!(!typing.update(t0 + SETTLE * 2, Some((101.0, 100.0))));
        assert!(typing.active);
        typing.update(t0 + SETTLE * 3, Some((200.0, 100.0)));
        assert!(!typing.active);
    }

    #[test]
    fn moving_the_mouse_after_a_pause_is_not_typing() {
        let t0 = Instant::now();
        let mut typing = Typing::default();
        typing.idled(Some((100.0, 100.0)));
        typing.resumed(t0);
        assert!(!typing.update(t0 + SETTLE, Some((140.0, 110.0))));
        assert!(!typing.active);
    }
}
