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

/// Per-field parsing: an invalid or missing value falls back to its own default and never
/// discards its neighbours (so a bad `theme` can't silently undo `telemetry = false`).
pub fn parse(s: &str) -> Settings {
    let d = Settings::default();
    let Ok(t) = s.parse::<toml::Table>() else {
        return d;
    };
    let flag = |k: &str, def: bool| t.get(k).and_then(|v| v.as_bool()).unwrap_or(def);
    let uint = |k: &str| {
        t.get(k)
            .and_then(|v| v.as_integer())
            .and_then(|i| u64::try_from(i).ok())
    };
    let font_size = t
        .get("font_size")
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .map(|f| f as f32)
        .filter(|f| f.is_finite())
        .map_or(d.font_size, |f| f.clamp(6.0, 72.0));
    Settings {
        wrap: flag("wrap", d.wrap),
        theme: t
            .get("theme")
            .and_then(|v| v.as_str())
            .map_or(d.theme, str::to_owned),
        font_size,
        show_line_numbers: flag("show_line_numbers", d.show_line_numbers),
        telemetry: flag("telemetry", d.telemetry),
        telemetry_max_kb: uint("telemetry_max_kb").unwrap_or(d.telemetry_max_kb),
        telemetry_max_days: uint("telemetry_max_days")
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(d.telemetry_max_days),
    }
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
    fn unusable_font_sizes_never_reach_the_ui() {
        for bad in ["0.0", "-5.0", "nan", "inf", "100000.0", "\"16\""] {
            let s = parse(&format!("font_size = {bad}"));
            assert!(
                s.font_size.is_finite() && (6.0..=72.0).contains(&s.font_size),
                "{bad} -> {}",
                s.font_size
            );
        }
        assert_eq!(parse("font_size = 16").font_size, 16.0);
        assert_eq!(parse("font_size = 1.0").font_size, 6.0);
    }

    #[test]
    fn one_bad_value_does_not_reset_the_rest() {
        let s = parse("telemetry = false\ntheme = 5\nwrap = true\nfont_size = \"x\"\n");
        assert!(
            !s.telemetry,
            "telemetry opt-out must survive a bad neighbour"
        );
        assert!(s.wrap);
        assert_eq!(s.theme, "system");
        assert_eq!(s.font_size, 14.0);
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
