//! Settings: defaults, then ~/.config/cece/config, then command-line flags.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub speed: f64,
    pub scale: f64,
    pub quiet: bool,
    pub skin: String,
}

impl Default for Config {
    fn default() -> Self {
        Config { speed: 2.0, scale: 2.0, quiet: false, skin: "burmilla".into() }
    }
}

pub const USAGE: &str = "\
cece - a Burmilla cat that chases your cursor (Hyprland)

Usage: cece [options]

Options:
  --speed <n>     running speed (default 2.0)
  --scale <n>     size; 1.0 is the classic 32px cat (default 2.0)
  --skin <name>   burmilla, classic, or a path to a sprite sheet PNG
  --quiet         no sounds
  -h, --help      show this help
  -V, --version   show the version

Config file: $XDG_CONFIG_HOME/cece/config (~/.config/cece/config), e.g.
  speed = 2.5
  scale = 2
  quiet = true
  skin = classic

Signals: SIGUSR1 tells the cat to stay put (or to follow again),
SIGUSR2 mutes (or unmutes) it.";

pub fn path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("cece").join("config"))
}

pub enum Action {
    Run(Config),
    Help,
    Version,
}

impl Config {
    fn set(&mut self, key: &str, value: &str) -> Result<(), String> {
        let number = |v: &str| {
            v.parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0)
                .ok_or_else(|| format!("{key}: expected a positive number, got {v:?}"))
        };
        match key {
            "speed" => self.speed = number(value)?,
            "scale" => self.scale = number(value)?.clamp(0.5, 8.0),
            "quiet" => {
                self.quiet = match value {
                    "true" | "yes" | "on" | "1" => true,
                    "false" | "no" | "off" | "0" => false,
                    _ => return Err(format!("quiet: expected true or false, got {value:?}")),
                }
            }
            "skin" => self.skin = value.to_string(),
            _ => return Err(format!("unknown setting {key:?}")),
        }
        Ok(())
    }

    /// Parse `key = value` lines; `#` starts a comment.
    pub fn apply_file(&mut self, text: &str) -> Result<(), String> {
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("line {}: expected key = value", n + 1))?;
            let value = value.trim().trim_matches('"');
            self.set(key.trim(), value).map_err(|e| format!("line {}: {e}", n + 1))?;
        }
        Ok(())
    }

    pub fn apply_args(mut self, args: impl IntoIterator<Item = String>) -> Result<Action, String> {
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) if arg.starts_with("--") => (f.to_string(), Some(v.to_string())),
                _ => (arg.clone(), None),
            };
            match flag.as_str() {
                "-h" | "--help" => return Ok(Action::Help),
                "-V" | "--version" => return Ok(Action::Version),
                "--quiet" => self.quiet = true,
                "--speed" | "--scale" | "--skin" => {
                    let value = inline
                        .or_else(|| args.next())
                        .ok_or_else(|| format!("{flag} needs a value"))?;
                    self.set(&flag[2..], &value)?;
                }
                _ => return Err(format!("unknown option {arg:?} (see --help)")),
            }
        }
        Ok(Action::Run(self))
    }
}

pub fn load(args: impl IntoIterator<Item = String>) -> Result<Action, String> {
    let mut cfg = Config::default();
    if let Some(path) = path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            cfg.apply_file(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    cfg.apply_args(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn file_then_flags() {
        let mut cfg = Config::default();
        cfg.apply_file("# comment\nspeed = 3\nskin = \"classic\"\nquiet = yes\n").unwrap();
        let Action::Run(cfg) = cfg.apply_args(args(&["--speed=4", "--scale", "3"])).unwrap() else {
            panic!("expected run");
        };
        assert_eq!(cfg, Config { speed: 4.0, scale: 3.0, quiet: true, skin: "classic".into() });
    }

    #[test]
    fn rejects_nonsense() {
        assert!(Config::default().apply_file("speed = fast").is_err());
        assert!(Config::default().apply_file("colour = red").is_err());
        assert!(Config::default().apply_args(args(&["--bogus"])).is_err());
        assert!(Config::default().apply_args(args(&["--speed"])).is_err());
    }
}
