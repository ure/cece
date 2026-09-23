//! cece: a cat that chases the cursor, as a Hyprland layer-shell overlay.
//!
//! The cat lives on a small click-through overlay surface that follows it
//! around by updating the surface's margins; its toy mouse, when it has one,
//! gets a second one. The cursor position comes from
//! Hyprland's IPC (see cursor.rs).

mod art;
mod bed;
mod cat;
mod config;
mod cursor;
mod render;
mod sound;
mod sprites;
mod toy;
mod toy_art;
mod typing;
#[cfg(test)]
mod scenes;

use std::{
    os::unix::net::{SocketAddr, UnixListener},
    process::ExitCode,
    time::{Duration, Instant},
};

use calloop::{
    EventLoop,
    signals::{Signal, Signals},
    timer::{TimeoutAction, Timer},
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    reexports::{
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, Proxy, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
        },
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
    dispatch2::Dispatch2,
    reexports::{
        client::protocol::wl_seat::WlSeat,
        protocols::ext::idle_notify::v1::client::{
            ext_idle_notification_v1::{self, ExtIdleNotificationV1},
            ext_idle_notifier_v1::ExtIdleNotifierV1,
        },
    },
};

use cat::{Cat, Pose, Rect};
use toy::ToyPose;

const TICK: Duration = Duration::from_millis(16);

struct Monitor {
    output: wl_output::WlOutput,
    name: String,
    rect: Rect,
    scale: i32,
}

/// How long all input must stop before the compositor says we are idle; the
/// shortest pause between keys that lets us notice typing.
const IDLE_MS: u32 = 800;

/// User data for the seat and the idle notifications, which only matter for
/// telling when you type (see typing.rs).
struct Idle;

impl Dispatch2<WlSeat, App> for Idle {
    fn event(&self, _: &mut App, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &Connection, _: &QueueHandle<App>) {}
}

impl Dispatch2<ExtIdleNotifierV1, App> for Idle {
    fn event(
        &self,
        _: &mut App,
        _: &ExtIdleNotifierV1,
        _: <ExtIdleNotifierV1 as Proxy>::Event,
        _: &Connection,
        _: &QueueHandle<App>,
    ) {
    }
}

impl Dispatch2<ExtIdleNotificationV1, App> for Idle {
    fn event(
        &self,
        app: &mut App,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &Connection,
        _: &QueueHandle<App>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => app.typing.idled(app.cursor.get()),
            ext_idle_notification_v1::Event::Resumed => app.typing.resumed(Instant::now()),
            _ => {}
        }
    }
}

/// A small click-through overlay surface on one output. `P` describes what
/// was last presented; a new buffer is only committed when it changes.
struct Overlay<P> {
    layer: LayerSurface,
    output: wl_output::WlOutput,
    configured: bool,
    presented: Option<P>,
}

#[derive(PartialEq)]
struct Presented<T> {
    pose: T,
    margin: (i32, i32),
    sub: (i64, i64),
    scale: i32,
}

/// Split a logical offset into whole logical pixels, for the surface margin,
/// and a remainder in buffer pixels, so HiDPI motion is smooth.
fn split(x: f64, y: f64, scale: i32) -> ((i32, i32), (i64, i64)) {
    let sf = scale as f64;
    let margin = (x.floor() as i32, y.floor() as i32);
    let sub = (((x - x.floor()) * sf).round() as i64, ((y - y.floor()) * sf).round() as i64);
    (margin, sub)
}

/// The monitor under (`x`, `y`); between monitors, stay on `current`.
fn pick<'a>(monitors: &'a [Monitor], (x, y): (f64, f64), current: Option<&wl_output::WlOutput>) -> Option<&'a Monitor> {
    monitors
        .iter()
        .find(|m| m.rect.contains(x, y))
        .or_else(|| monitors.iter().find(|m| Some(&m.output) == current))
        .or(monitors.first())
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    qh: QueueHandle<Self>,

    surface: Option<Overlay<Presented<Pose>>>,
    toy_surface: Option<Overlay<Presented<ToyPose>>>,
    frame_requested: Option<Instant>,
    last_tick: Instant,

    cat: Cat,
    sheet: sprites::Sheet,
    cursor: cursor::Cursor,
    audio: sound::Audio,
    typing: typing::Typing,
    /// Kept alive so the compositor keeps sending idle events.
    _idle: Option<(WlSeat, ExtIdleNotificationV1)>,
    exit: bool,
}

impl App {
    fn monitors(&self) -> Vec<Monitor> {
        self.output_state
            .outputs()
            .filter_map(|output| {
                let info = self.output_state.info(&output)?;
                let (x, y) = info.logical_position?;
                let (w, h) = info.logical_size?;
                Some(Monitor {
                    name: info.name.clone().unwrap_or_default(),
                    output,
                    rect: Rect { x: x as f64, y: y as f64, w: w as f64, h: h as f64 },
                    scale: info.scale_factor.max(1),
                })
            })
            .collect()
    }

    /// Padding around the cat inside its surface, for the shadow and Zzz.
    fn pad(&self) -> f64 {
        (self.cat.size() / 2.0).ceil()
    }

    /// Side of the toy's surface, and where its ground point sits in it.
    fn toy_side(&self) -> (f64, (f64, f64)) {
        let side = (self.cat.size() * 1.5).round();
        (side, ((side / 2.0).round(), (side * 0.66).round()))
    }

    fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        let monitors = self.monitors();
        let cursor = self.cursor.get();
        let screen = cursor
            .and_then(|(x, y)| monitors.iter().find(|m| m.rect.contains(x, y)))
            .map(|m| m.rect);
        // While you type, the cat keeps out of the way on another monitor:
        // the smallest one that is not the one you type on.
        if self.typing.update(now, cursor) && !self.cat.staying {
            let focus = self.cursor.focused_monitor();
            let typing_on = monitors
                .iter()
                .find(|m| Some(&m.name) == focus.as_ref())
                .or_else(|| cursor.and_then(|(x, y)| monitors.iter().find(|m| m.rect.contains(x, y))));
            let refuge = monitors
                .iter()
                .filter(|m| typing_on.is_none_or(|t| t.output != m.output))
                .min_by(|a, b| (a.rect.w * a.rect.h).total_cmp(&(b.rect.w * b.rect.h)));
            let (cx, cy) = self.cat.center();
            if let Some(r) = refuge.filter(|r| !r.rect.contains(cx, cy)) {
                self.cat.away(r.rect);
            }
        }
        self.cat.typing = self.typing.active;

        let current = self.surface.as_ref().map(|s| &s.output);
        let home = pick(&monitors, self.cat.center(), current).map(|m| m.rect);
        self.cat.update(dt, cursor, screen, home);
        for sound in self.cat.sounds.drain(..) {
            self.audio.play(sound);
        }

        // Each surface belongs to one output: the one under the cat's centre
        // (or the toy). Between monitors, stay on the current one.
        let current = self.surface.as_ref().map(|s| &s.output);
        let Some(home) = pick(&monitors, self.cat.center(), current) else {
            self.surface = None;
            self.toy_surface = None;
            return;
        };
        if self.surface.as_ref().is_none_or(|s| s.output != home.output) {
            let side = self.cat.size() + 2.0 * self.pad();
            self.surface = Some(self.create_surface(qh, &home.output, side, "cece"));
        }
        let (rect, scale) = (home.rect, home.scale);
        let cat_output = home.output.clone();
        self.draw(qh, rect, scale);

        let Some(spot) = self.cat.toy.as_ref().map(|t| (t.x, t.y)) else {
            self.toy_surface = None;
            return;
        };
        // While it flies in from off screen, it is on the cat's monitor.
        let current = self.toy_surface.as_ref().map(|s| &s.output).or(Some(&cat_output));
        let Some(home) = pick(&monitors, spot, current) else { return };
        if self.toy_surface.as_ref().is_none_or(|s| s.output != home.output) {
            let side = self.toy_side().0;
            self.toy_surface = Some(self.create_surface(qh, &home.output, side, "cece-toy"));
        }
        let (rect, scale) = (home.rect, home.scale);
        self.draw_toy(qh, rect, scale);
    }

    fn create_surface<P>(
        &self,
        qh: &QueueHandle<Self>,
        output: &wl_output::WlOutput,
        side: f64,
        namespace: &str,
    ) -> Overlay<P> {
        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some(namespace),
            Some(output),
        );
        let side = side as u32;
        layer.set_anchor(Anchor::TOP | Anchor::LEFT);
        layer.set_size(side, side);
        // -1: position relative to the output edge, ignoring bars.
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        // An empty input region makes the cat click-through.
        if let Ok(region) = Region::new(&self.compositor) {
            layer.set_input_region(Some(region.wl_region()));
        }
        layer.commit();
        Overlay { layer, output: output.clone(), configured: false, presented: None }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>, rect: Rect, scale: i32) {
        let pad = self.pad();
        let size = self.cat.size();
        let Some(surface) = self.surface.as_mut().filter(|s| s.configured) else { return };
        let side = (size + 2.0 * pad) as i32;
        let (margin, sub) = split(self.cat.x - pad - rect.x, self.cat.y - pad - rect.y, scale);

        let mut pose = self.cat.pose();
        // The Zzz animate at 15 fps; no need to repaint a sleeping cat faster.
        pose.asleep = pose.asleep.map(|t| (t * 15.0).floor() / 15.0);
        let presented = Presented { pose, margin, sub, scale };
        if surface.presented.as_ref() == Some(&presented) {
            return;
        }

        let sf = scale as f64;
        let origin = (pad * sf + sub.0 as f64, pad * sf + sub.1 as f64);
        let frame = self.sheet.get(pose.frame);
        let clock = self.frame_requested.is_none();
        let painted = present(&mut self.pool, qh, &surface.layer, side, scale, margin, clock, |canvas| {
            render::draw(canvas, frame, &pose, origin, size * sf, sf)
        });
        if painted {
            surface.presented = Some(presented);
            if clock {
                self.frame_requested = Some(Instant::now());
            }
        }
    }

    fn draw_toy(&mut self, qh: &QueueHandle<Self>, rect: Rect, scale: i32) {
        let (side, ground) = self.toy_side();
        let size = self.cat.size();
        let Some(toy) = &self.cat.toy else { return };
        let Some(surface) = self.toy_surface.as_mut().filter(|s| s.configured) else { return };
        let (margin, sub) = split(toy.x - ground.0 - rect.x, toy.y - ground.1 - rect.y, scale);
        let pose = toy.pose();
        let presented = Presented { pose, margin, sub, scale };
        if surface.presented.as_ref() == Some(&presented) {
            return;
        }

        let sf = scale as f64;
        let u = size * sf / 32.0;
        let origin = (ground.0 * sf + sub.0 as f64, ground.1 * sf + sub.1 as f64);
        let lift = pose.z.min(size * 0.45) * sf;
        let clock = self.frame_requested.is_none();
        let painted = present(&mut self.pool, qh, &surface.layer, side as i32, scale, margin, clock, |canvas| {
            toy_art::draw(canvas, &pose, origin, u, lift)
        });
        if painted {
            surface.presented = Some(presented);
            if clock {
                self.frame_requested = Some(Instant::now());
            }
        }
    }

    /// Forget what was presented on every surface, forcing a repaint.
    fn invalidate(&mut self) {
        if let Some(s) = &mut self.surface {
            s.presented = None;
        }
        if let Some(s) = &mut self.toy_surface {
            s.presented = None;
        }
    }
}

/// Paint a new buffer, `side` logical pixels square, with `paint` and commit
/// it to `layer` at `margin` from the output's top-left corner. With `clock`,
/// also ask for a frame callback: only one surface paces the animation.
#[allow(clippy::too_many_arguments)]
fn present(
    pool: &mut SlotPool,
    qh: &QueueHandle<App>,
    layer: &LayerSurface,
    side: i32,
    scale: i32,
    margin: (i32, i32),
    clock: bool,
    paint: impl FnOnce(&mut render::Canvas),
) -> bool {
    let width = side * scale;
    let (buffer, canvas) = match pool.create_buffer(width, width, width * 4, wl_shm::Format::Argb8888) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("cece: buffer: {e}");
            return false;
        }
    };
    // wl_shm's ARGB8888 is native-endian 32-bit pixels.
    let pixels: &mut [u32] = bytemuck_cast(canvas);
    paint(&mut render::Canvas { pixels, width: width as usize, height: width as usize });

    let wl = layer.wl_surface();
    layer.set_margin(margin.1, 0, 0, margin.0);
    let _ = layer.set_buffer_scale(scale as u32);
    wl.damage_buffer(0, 0, width, width);
    if clock {
        wl.frame(qh, FrameCallbackData(wl.clone()));
    }
    if let Err(e) = buffer.attach_to(wl) {
        eprintln!("cece: attach: {e}");
        return false;
    }
    layer.commit();
    true
}

/// View a byte buffer as native-endian u32 pixels.
fn bytemuck_cast(bytes: &mut [u8]) -> &mut [u32] {
    // wl_shm pools are page aligned and our buffers start at 4-byte aligned
    // offsets, so this never actually needs to shift anything.
    let (head, pixels, _) = unsafe { bytes.align_to_mut::<u32>() };
    assert!(head.is_empty(), "shm buffer is not 4-byte aligned");
    pixels
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {
        self.invalidate();
    }

    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}

    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        // While the cat is moving, the compositor's frame callbacks pace the
        // animation at the monitor's refresh rate.
        self.frame_requested = None;
        self.tick(qh);
    }

    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}

    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.invalidate();
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.surface.as_ref().is_some_and(|s| s.output == output) {
            self.surface = None;
        }
        if self.toy_surface.as_ref().is_some_and(|s| s.output == output) {
            self.toy_surface = None;
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.surface.as_ref().is_some_and(|s| &s.layer == layer) {
            self.surface = None;
        }
        if self.toy_surface.as_ref().is_some_and(|s| &s.layer == layer) {
            self.toy_surface = None;
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        let first = |configured: &mut bool| !std::mem::replace(configured, true);
        let fresh = match (&mut self.surface, &mut self.toy_surface) {
            (Some(s), _) if &s.layer == layer => first(&mut s.configured),
            (_, Some(s)) if &s.layer == layer => first(&mut s.configured),
            _ => false,
        };
        if fresh {
            self.tick(qh);
        }
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(App);

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_dispatch2!(App);

/// Hold an abstract socket for the lifetime of the process so only one cat
/// runs per user session.
fn single_instance() -> Result<UnixListener, String> {
    use std::os::linux::net::SocketAddrExt;
    let user = std::env::var("USER").unwrap_or_default();
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
    let name = format!("cece-{user}-{display}");
    let addr = SocketAddr::from_abstract_name(name.as_bytes()).map_err(|e| e.to_string())?;
    UnixListener::bind_addr(&addr).map_err(|_| "cece is already running".to_string())
}

fn run(cfg: config::Config) -> Result<(), String> {
    let _lock = single_instance()?;
    // Creating the signal source blocks these signals on this thread. Do it
    // before any other thread starts so they inherit the mask; otherwise a
    // signal can land on a helper thread and take its default action, which
    // for SIGUSR1/SIGUSR2 is to kill the process.
    let signals = Signals::new(&[
        Signal::SIGUSR1,
        Signal::SIGUSR2,
        Signal::SIGINT,
        Signal::SIGTERM,
        Signal::SIGHUP,
    ])
    .map_err(|e| e.to_string())?;
    let sheet = sprites::Sheet::load(&cfg.skin)?;
    let cursor = cursor::Cursor::spawn()?;

    let conn = Connection::connect_to_env().map_err(|e| format!("Wayland: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).map_err(|_| "no wl_compositor")?;
    let layer_shell =
        LayerShell::bind(&globals, &qh).map_err(|_| "compositor lacks wlr-layer-shell")?;
    let shm = Shm::bind(&globals, &qh).map_err(|_| "no wl_shm")?;
    // Optional: without idle notifications the cat just cannot tell typing.
    let idle = match (globals.bind::<WlSeat, _, _>(&qh, 1..=1, Idle), globals.bind::<ExtIdleNotifierV1, _, _>(&qh, 1..=2, Idle)) {
        (Ok(seat), Ok(notifier)) => {
            // Version 2 can ignore idle inhibitors (a playing video, say).
            let n = if notifier.version() >= 2 {
                notifier.get_input_idle_notification(IDLE_MS, &seat, &qh, Idle)
            } else {
                notifier.get_idle_notification(IDLE_MS, &seat, &qh, Idle)
            };
            Some((seat, n))
        }
        _ => None,
    };
    let size = (32.0 * cfg.scale).round();
    let side = (size * 2.0) as usize * 4;
    let pool = SlotPool::new(side * side * 4 * 2, &shm).map_err(|e| e.to_string())?;

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64);
    let mut app = App {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        shm,
        pool,
        qh: qh.clone(),
        surface: None,
        toy_surface: None,
        frame_requested: None,
        last_tick: Instant::now(),
        cat: Cat::new(0.0, 0.0, size, cfg.speed, seed),
        sheet,
        cursor,
        audio: sound::Audio::new(cfg.quiet),
        typing: typing::Typing::default(),
        _idle: idle,
        exit: false,
    };

    // Learn the outputs, then start the cat in the middle of the monitor the
    // cursor is on, like the original.
    for _ in 0..2 {
        queue.roundtrip(&mut app).map_err(|e| e.to_string())?;
    }
    let monitors = app.monitors();
    let pointer = app.cursor.get();
    let start = pointer
        .and_then(|(x, y)| monitors.iter().find(|m| m.rect.contains(x, y)))
        .or(monitors.first())
        .map(|m| m.rect)
        .ok_or("no monitors")?;
    app.cat.toy_after = cfg.toy;
    app.cat.toy_now = cfg.play;
    app.cat.x = start.x + (start.w - size) / 2.0;
    app.cat.y = start.y + (start.h - size) / 2.0;

    let mut event_loop: EventLoop<App> = EventLoop::try_new().map_err(|e| e.to_string())?;
    let handle = event_loop.handle();
    WaylandSource::new(conn, queue)
        .insert(handle.clone())
        .map_err(|e| e.to_string())?;

    // Fallback clock for when no frame callback is pending (the cat is idle,
    // or the compositor is not currently showing the surface).
    handle
        .insert_source(Timer::from_duration(TICK), |_, _, app| {
            let stale = app
                .frame_requested
                .is_none_or(|t| t.elapsed() > Duration::from_millis(100));
            if stale {
                app.frame_requested = None;
                let qh = app.qh.clone();
                app.tick(&qh);
            }
            TimeoutAction::ToDuration(TICK)
        })
        .map_err(|e| e.to_string())?;

    handle
        .insert_source(signals, |event, _, app| match event.signal() {
            Signal::SIGUSR1 => app.cat.staying = !app.cat.staying,
            Signal::SIGUSR2 => app.audio.muted = !app.audio.muted,
            _ => app.exit = true,
        })
        .map_err(|e| e.to_string())?;

    while !app.exit {
        event_loop
            .dispatch(Some(TICK), &mut app)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match config::load(std::env::args().skip(1)) {
        Ok(config::Action::Help) => {
            println!("{}", config::USAGE);
            ExitCode::SUCCESS
        }
        Ok(config::Action::Version) => {
            println!("cece {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(config::Action::Run(cfg)) => match run(cfg) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("cece: {e}");
                ExitCode::FAILURE
            }
        },
        Err(e) => {
            eprintln!("cece: {e}");
            ExitCode::FAILURE
        }
    }
}
