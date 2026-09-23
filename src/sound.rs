//! Sound effects, played on a background thread that owns the audio device.

use std::sync::mpsc::{self, Sender};

use crate::cat::Sound;

/// The audio device is opened lazily, on the first sound played while not
/// muted, so a quiet cat never touches it.
pub struct Audio {
    tx: Option<Sender<Sound>>,
    pub muted: bool,
}

impl Audio {
    pub fn new(muted: bool) -> Self {
        Audio { tx: None, muted }
    }

    pub fn play(&mut self, sound: Sound) {
        if self.muted {
            return;
        }
        if self.tx.is_none() {
            self.tx = spawn();
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(sound);
        }
    }
}

#[cfg(feature = "sound")]
fn spawn() -> Option<Sender<Sound>> {
    use std::io::Cursor;

    static AWAKE: &[u8] = include_bytes!("../assets/awake.wav");
    static YAWN: &[u8] = include_bytes!("../assets/idle3.wav");
    static SLEEP: &[u8] = include_bytes!("../assets/sleep.wav");

    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("sound".into())
        .spawn(move || {
            let mut sink = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(sink) => sink,
                Err(e) => {
                    eprintln!("cece: no audio ({e}); continuing quietly");
                    return;
                }
            };
            sink.log_on_drop(false);
            let mut current: Option<rodio::Player> = None;
            for sound in rx {
                let bytes = match sound {
                    Sound::Awake => AWAKE,
                    Sound::Yawn => YAWN,
                    Sound::Sleep => SLEEP,
                };
                // Like the original, a new sound cuts off the previous one.
                if let Some(player) = current.take() {
                    player.stop();
                }
                match rodio::play(sink.mixer(), Cursor::new(bytes)) {
                    Ok(player) => {
                        player.set_volume(0.3);
                        current = Some(player);
                    }
                    Err(e) => eprintln!("cece: sound: {e}"),
                }
            }
        })
        .ok()?;
    Some(tx)
}

#[cfg(not(feature = "sound"))]
fn spawn() -> Option<Sender<Sound>> {
    let _ = mpsc::channel::<Sound>;
    None
}
