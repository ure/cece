//! Sprite sheets. Each sheet is a grid of 128x128 frames, 8 per row, in the
//! order of `FRAMES`; tools/burmilla.py generates them and must list frames in
//! the same order.

use std::{fs::File, io::Read, path::Path};

pub const FRAME: usize = 128;
const COLUMNS: usize = 8;

pub const FRAMES: &[&str] = &[
    "awake", "awake_blink", "scratch1", "scratch2", "wash", "yawn1", "yawn1_blink", "yawn2",
    "sleep1", "sleep2", //
    "up1", "up2", "upright1", "upright2", "right1", "right2", "downright1", "downright2",
    "down1", "down2", "downleft1", "downleft2", "left1", "left2", "upleft1", "upleft2", //
    "upclaw1", "upclaw2", "rightclaw1", "rightclaw2", "downclaw1", "downclaw2", "leftclaw1",
    "leftclaw2",
];

static BURMILLA: &[u8] = include_bytes!("../assets/burmilla.png");
static CLASSIC: &[u8] = include_bytes!("../assets/classic.png");

/// Premultiplied ARGB frames, `FRAME * FRAME` pixels each.
pub struct Sheet {
    frames: Vec<Vec<u32>>,
}

impl Sheet {
    /// `skin` is a built-in name ("burmilla", "classic") or a path to a PNG
    /// laid out like the built-in sheets.
    pub fn load(skin: &str) -> Result<Self, String> {
        let bytes = match skin {
            "burmilla" | "" => BURMILLA.to_vec(),
            "classic" => CLASSIC.to_vec(),
            path => {
                let mut buf = Vec::new();
                File::open(Path::new(path))
                    .and_then(|mut f| f.read_to_end(&mut buf))
                    .map_err(|e| format!("read skin {path}: {e}"))?;
                buf
            }
        };
        Self::decode(&bytes).map_err(|e| format!("skin {skin}: {e}"))
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let (w, h) = (info.width as usize, info.height as usize);
        let channels = match info.color_type {
            png::ColorType::Rgba => 4,
            png::ColorType::Rgb => 3,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Grayscale => 1,
            png::ColorType::Indexed => return Err("unexpected indexed PNG".into()),
        };
        let rows = FRAMES.len().div_ceil(COLUMNS);
        if w < COLUMNS * FRAME || h < rows * FRAME {
            return Err(format!(
                "sheet is {w}x{h}, need at least {}x{}",
                COLUMNS * FRAME,
                rows * FRAME
            ));
        }

        let pixel = |x: usize, y: usize| -> u32 {
            let i = (y * w + x) * channels;
            let (r, g, b, a) = match channels {
                4 => (buf[i], buf[i + 1], buf[i + 2], buf[i + 3]),
                3 => (buf[i], buf[i + 1], buf[i + 2], 255),
                2 => (buf[i], buf[i], buf[i], buf[i + 1]),
                _ => (buf[i], buf[i], buf[i], 255),
            };
            let pm = |c: u8| (c as u32 * a as u32 + 127) / 255;
            (a as u32) << 24 | pm(r) << 16 | pm(g) << 8 | pm(b)
        };

        let frames = (0..FRAMES.len())
            .map(|i| {
                let (ox, oy) = ((i % COLUMNS) * FRAME, (i / COLUMNS) * FRAME);
                (0..FRAME * FRAME)
                    .map(|p| pixel(ox + p % FRAME, oy + p / FRAME))
                    .collect()
            })
            .collect();
        Ok(Self { frames })
    }

    pub fn get(&self, name: &str) -> &[u32] {
        let index = FRAMES
            .iter()
            .position(|f| *f == name)
            .unwrap_or_else(|| panic!("unknown frame {name}"));
        &self.frames[index]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn built_in_skins_have_every_frame() {
        for skin in ["burmilla", "classic"] {
            let sheet = super::Sheet::load(skin).unwrap();
            for name in super::FRAMES {
                let frame = sheet.get(name);
                assert_eq!(frame.len(), super::FRAME * super::FRAME);
                assert!(frame.iter().any(|p| p >> 24 == 255), "{skin}/{name} is empty");
            }
        }
    }
}
