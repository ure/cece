//! Global cursor position from Hyprland's IPC socket.
//!
//! Wayland never tells a client where the pointer is unless it is over one of
//! the client's own surfaces, and our surface is click-through. Hyprland will
//! answer `cursorpos` on its request socket in global layout coordinates, so a
//! background thread polls that and publishes the latest position.

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct Cursor(Arc<Mutex<Option<(f64, f64)>>>);

impl Cursor {
    pub fn spawn() -> Result<Self, String> {
        let socket = socket_path()?;
        query(&socket).map_err(|e| format!("Hyprland IPC at {}: {e}", socket.display()))?;

        let cursor = Cursor(Arc::new(Mutex::new(None)));
        let shared = cursor.clone();
        thread::Builder::new()
            .name("cursor".into())
            .spawn(move || {
                loop {
                    let pos = query(&socket).ok();
                    *shared.0.lock().unwrap() = pos;
                    thread::sleep(Duration::from_millis(12));
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(cursor)
    }

    pub fn get(&self) -> Option<(f64, f64)> {
        *self.0.lock().unwrap()
    }
}

fn socket_path() -> Result<PathBuf, String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE is not set; cece needs Hyprland")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let path = PathBuf::from(runtime).join("hypr").join(&sig).join(".socket.sock");
    if path.exists() {
        return Ok(path);
    }
    // Older Hyprland releases kept the socket under /tmp.
    Ok(PathBuf::from("/tmp/hypr").join(sig).join(".socket.sock"))
}

fn query(socket: &PathBuf) -> std::io::Result<(f64, f64)> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.write_all(b"cursorpos")?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply)?;
    parse(&reply).ok_or_else(|| std::io::Error::other(format!("unexpected reply {reply:?}")))
}

fn parse(reply: &str) -> Option<(f64, f64)> {
    let (x, y) = reply.trim().split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_cursorpos() {
        assert_eq!(super::parse("1254, 1079\n"), Some((1254.0, 1079.0)));
        assert_eq!(super::parse("-3, 7"), Some((-3.0, 7.0)));
        assert_eq!(super::parse("unknown request"), None);
    }
}
