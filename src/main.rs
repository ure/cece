//! cece: a cat that chases the cursor, as a Hyprland layer-shell overlay.
//!
//! The cat lives on a small click-through overlay surface that follows it
//! around by updating the surface's margins. The cursor position comes from
//! Hyprland's IPC (see cursor.rs).

mod cat;
mod config;
mod cursor;
mod render;
mod sound;
mod sprites;

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
            Connection, QueueHandle,
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
};

use cat::{Cat, Pose, Rect};

const TICK: Duration = Duration::from_millis(16);

struct Monitor {
    output: wl_output::WlOutput,
    rect: Rect,
    scale: i32,
}

struct CatSurface {
    layer: LayerSurface,
    output: wl_output::WlOutput,
    configured: bool,
}

/// What was last presented; a new buffer is only committed when this changes.
#[derive(PartialEq)]
struct Presented {
    pose: Pose,
    margin: (i32, i32),
    sub: (i64, i64),
    scale: i32,
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    qh: QueueHandle<Self>,

    surface: Option<CatSurface>,
    presented: Option<Presented>,
    frame_requested: Option<Instant>,
    last_tick: Instant,

    cat: Cat,
    sheet: sprites::Sheet,
    cursor: cursor::Cursor,
    audio: sound::Audio,
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

    fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        let monitors = self.monitors();
        let cursor = self.cursor.get();
        let screen = cursor
            .and_then(|(x, y)| monitors.iter().find(|m| m.rect.contains(x, y)))
            .map(|m| m.rect);
        self.cat.update(dt, cursor, screen);
        for sound in self.cat.sounds.drain(..) {
            self.audio.play(sound);
        }

        // The surface belongs to one output: the one under the cat's centre.
        // Between monitors, stay on the current one.
        let (cx, cy) = self.cat.center();
        let home = monitors
            .iter()
            .find(|m| m.rect.contains(cx, cy))
            .or_else(|| {
                let current = self.surface.as_ref()?;
                monitors.iter().find(|m| m.output == current.output)
            })
            .or(monitors.first());
        let Some(home) = home else {
            self.surface = None;
            return;
        };
        if self.surface.as_ref().is_none_or(|s| s.output != home.output) {
            self.surface = Some(self.create_surface(qh, &home.output));
            self.presented = None;
        }
        let (rect, scale) = (home.rect, home.scale);
        self.draw(qh, rect, scale);
    }

    fn create_surface(&self, qh: &QueueHandle<Self>, output: &wl_output::WlOutput) -> CatSurface {
        let surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("cece"),
            Some(output),
        );
        let side = (self.cat.size() + 2.0 * self.pad()) as u32;
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
        CatSurface { layer, output: output.clone(), configured: false }
    }

    fn draw(&mut self, qh: &QueueHandle<Self>, rect: Rect, scale: i32) {
        let Some(surface) = &self.surface else { return };
        if !surface.configured {
            return;
        }
        let pad = self.pad();
        let size = self.cat.size();
        let side = (size + 2.0 * pad) as i32;

        // Whole logical pixels go into the margin; the remainder becomes a
        // sub-pixel offset in the (denser) buffer, so HiDPI motion is smooth.
        let (fx, fy) = (self.cat.x - pad - rect.x, self.cat.y - pad - rect.y);
        let margin = (fx.floor() as i32, fy.floor() as i32);
        let sf = scale as f64;
        let sub = (((fx - fx.floor()) * sf).round() as i64, ((fy - fy.floor()) * sf).round() as i64);

        let mut pose = self.cat.pose();
        // The Zzz animate at 15 fps; no need to repaint a sleeping cat faster.
        pose.asleep = pose.asleep.map(|t| (t * 15.0).floor() / 15.0);
        let presented = Presented { pose, margin, sub, scale };
        if self.presented.as_ref() == Some(&presented) {
            return;
        }

        let width = side * scale;
        let (buffer, canvas) = match self.pool.create_buffer(
            width,
            width,
            width * 4,
            wl_shm::Format::Argb8888,
        ) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("cece: buffer: {e}");
                return;
            }
        };
        // wl_shm's ARGB8888 is native-endian 32-bit pixels.
        let pixels: &mut [u32] = bytemuck_cast(canvas);
        let mut target = render::Canvas { pixels, width: width as usize, height: width as usize };
        let origin = (pad * sf + sub.0 as f64, pad * sf + sub.1 as f64);
        render::draw(&mut target, self.sheet.get(pose.frame), &pose, origin, size * sf, sf);

        let layer = &surface.layer;
        let wl = layer.wl_surface();
        layer.set_margin(margin.1, 0, 0, margin.0);
        let _ = layer.set_buffer_scale(scale as u32);
        wl.damage_buffer(0, 0, width, width);
        wl.frame(qh, FrameCallbackData(wl.clone()));
        if let Err(e) = buffer.attach_to(wl) {
            eprintln!("cece: attach: {e}");
            return;
        }
        layer.commit();
        self.frame_requested = Some(Instant::now());
        self.presented = Some(presented);
    }
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
        self.presented = None;
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
        self.presented = None;
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.surface.as_ref().is_some_and(|s| s.output == output) {
            self.surface = None;
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.surface.as_ref().is_some_and(|s| &s.layer == layer) {
            self.surface = None;
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
        if let Some(surface) = self.surface.as_mut().filter(|s| &s.layer == layer) {
            let first = !surface.configured;
            surface.configured = true;
            if first {
                self.presented = None;
                self.tick(qh);
            }
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
        presented: None,
        frame_requested: None,
        last_tick: Instant::now(),
        cat: Cat::new(0.0, 0.0, size, cfg.speed, seed),
        sheet,
        cursor,
        audio: sound::Audio::new(cfg.quiet),
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
