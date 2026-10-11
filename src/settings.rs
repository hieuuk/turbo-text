use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub wrap: bool,
    pub theme: String,
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub telemetry: bool,
    pub telemetry_max_kb: u64,
    pub telemetry_max_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wrap: false,
            theme: "system".into(),
            font_size: 14.0,
            show_line_numbers: true,
            telemetry: true,
            telemetry_max_kb: 2048,
            telemetry_max_days: 90,
        }
    }
}

pub fn parse(s: &str) -> Settings {
    toml::from_str(s).unwrap_or_default()
}

pub fn dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("turbo-text")
}

pub fn path() -> PathBuf {
    dir().join("settings.toml")
}

pub fn load() -> Settings {
    std::fs::read_to_string(path())
        .map(|s| parse(&s))
        .unwrap_or_default()
}

pub fn save(s: &Settings) -> io::Result<()> {
    std::fs::create_dir_all(dir())?;
    let text = toml::to_string(s).map_err(io::Error::other)?;
    std::fs::write(path(), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_defaults() {
        let s = parse("");
        assert!(!s.wrap && s.show_line_numbers && s.telemetry);
        assert_eq!(s.theme, "system");
        assert_eq!(s.font_size, 14.0);
        assert_eq!(s.telemetry_max_kb, 2048);
        assert_eq!(s.telemetry_max_days, 90);
    }

    #[test]
    fn partial_keeps_given() {
        let s = parse("wrap = true\nfont_size = 18.0\n");
        assert!(s.wrap);
        assert_eq!(s.font_size, 18.0);
        assert!(s.telemetry);
    }

    #[test]
    fn garbage_is_defaults() {
        assert_eq!(parse("{{{ not toml \u{0}"), parse(""));
        assert_eq!(parse("wrap = \"maybe\""), parse(""));
    }

    #[test]
    fn roundtrip() {
        let s = Settings {
            wrap: true,
            theme: "dark".into(),
            ..Default::default()
        };
        let t = toml::to_string(&s).unwrap();
        assert_eq!(parse(&t), s);
    }
}
