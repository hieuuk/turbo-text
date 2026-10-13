use crate::format::Format;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;

/// Extensions offered for file association out of the box (editable in the settings dialog).
pub const DEFAULT_ASSOC_EXTS: [&str; 17] = [
    ".env",
    ".json",
    ".md",
    ".toml",
    ".ini",
    ".yaml",
    ".yml",
    ".txt",
    ".log",
    ".conf",
    ".cfg",
    ".config",
    ".properties",
    ".gitignore",
    ".editorconfig",
    ".csv",
    ".xml",
];

const MODES: [&str; 2] = ["view", "edit"];
const TAB_SPACES: [u8; 4] = [0, 2, 4, 8];
const WINDOW_RANGE: std::ops::RangeInclusive<f32> = 300.0..=10000.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    // editor
    pub font_size: f32,
    /// Row height as a multiple of the font's natural row height.
    pub line_height: f32,
    pub wrap: bool,
    pub show_line_numbers: bool,
    pub tab_spaces: u8,
    pub highlight: bool,
    pub large_file_mb: u64,
    pub editor_font: String,
    /// Body font of the rendered Markdown view.
    pub view_font: String,
    // appearance
    pub theme: String,
    pub ui_scale: f32,
    // behavior
    pub open_json: String,
    pub open_markdown: String,
    pub open_other: String,
    pub confirm_close: bool,
    pub remember_window: bool,
    pub window_w: f32,
    pub window_h: f32,
    // files and privacy
    pub assoc_exts: Vec<String>,
    pub telemetry: bool,
    pub telemetry_max_kb: u64,
    pub telemetry_max_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            font_size: 14.0,
            line_height: 1.0,
            wrap: false,
            show_line_numbers: true,
            tab_spaces: 0,
            highlight: true,
            large_file_mb: 5,
            editor_font: "monospace".into(),
            view_font: "proportional".into(),
            theme: "system".into(),
            ui_scale: 1.0,
            open_json: "view".into(),
            open_markdown: "view".into(),
            open_other: "edit".into(),
            confirm_close: true,
            remember_window: true,
            window_w: 900.0,
            window_h: 650.0,
            assoc_exts: DEFAULT_ASSOC_EXTS.iter().map(|e| e.to_string()).collect(),
            telemetry: false,
            telemetry_max_kb: 2048,
            telemetry_max_days: 90,
        }
    }
}

impl Settings {
    /// Whether a file of this format should open in view mode (before the empty-file rule).
    pub fn start_in_view(&self, fmt: Format) -> bool {
        let mode = match fmt {
            Format::Json => &self.open_json,
            Format::Markdown => &self.open_markdown,
            _ => &self.open_other,
        };
        mode == "view"
    }

    /// Window size to start with, or `None` to use the built-in default.
    pub fn window_size(&self) -> Option<(f32, f32)> {
        (self.remember_window
            && WINDOW_RANGE.contains(&self.window_w)
            && WINDOW_RANGE.contains(&self.window_h))
        .then_some((self.window_w, self.window_h))
    }

    pub fn reset_editor(&mut self) {
        let d = Settings::default();
        self.font_size = d.font_size;
        self.line_height = d.line_height;
        self.wrap = d.wrap;
        self.show_line_numbers = d.show_line_numbers;
        self.tab_spaces = d.tab_spaces;
        self.highlight = d.highlight;
        self.large_file_mb = d.large_file_mb;
        self.editor_font = d.editor_font;
    }

    pub fn reset_appearance(&mut self) {
        let d = Settings::default();
        self.theme = d.theme;
        self.ui_scale = d.ui_scale;
        self.view_font = d.view_font;
    }

    pub fn reset_behavior(&mut self) {
        let d = Settings::default();
        self.open_json = d.open_json;
        self.open_markdown = d.open_markdown;
        self.open_other = d.open_other;
        self.confirm_close = d.confirm_close;
        self.remember_window = d.remember_window;
        self.window_w = d.window_w;
        self.window_h = d.window_h;
    }

    pub fn reset_all(&mut self) {
        self.reset_editor();
        self.reset_appearance();
        self.reset_behavior();
        // keeps `assoc_exts`: it mirrors registry state, so wiping it would orphan registered types
        let exts = std::mem::take(&mut self.assoc_exts);
        self.reset_files_privacy();
        self.assoc_exts = exts;
    }

    pub fn reset_files_privacy(&mut self) {
        let d = Settings::default();
        self.assoc_exts = d.assoc_exts;
        self.telemetry = d.telemetry;
        self.telemetry_max_kb = d.telemetry_max_kb;
        self.telemetry_max_days = d.telemetry_max_days;
    }
}

/// Normalizes each entry with `assoc::normalize_ext`, drops invalid ones and duplicates, keeps order.
pub fn normalize_ext_list(input: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in input {
        let n = crate::assoc::normalize_ext(e);
        if !n.is_empty() && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// Per-field parsing: an invalid or missing value falls back to its own default (clamped where a
/// range exists) and never discards its neighbours (a bad `theme` can't undo `telemetry = true`).
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
    let float = |k: &str| {
        t.get(k)
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
            .map(|f| f as f32)
            .filter(|f| f.is_finite())
    };
    let choice = |k: &str, allowed: &[&str], def: &str| {
        t.get(k)
            .and_then(|v| v.as_str())
            .filter(|v| allowed.contains(v))
            .unwrap_or(def)
            .to_owned()
    };
    let assoc_exts = match t.get("assoc_exts").and_then(|v| v.as_array()) {
        Some(items) => {
            let strings: Vec<String> = items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            normalize_ext_list(&strings)
        }
        None => d.assoc_exts.clone(),
    };
    let window = |k: &str, def: f32| float(k).filter(|f| WINDOW_RANGE.contains(f)).unwrap_or(def);
    Settings {
        font_size: float("font_size").map_or(d.font_size, |f| f.clamp(6.0, 72.0)),
        line_height: float("line_height").map_or(d.line_height, |f| f.clamp(0.8, 2.0)),
        wrap: flag("wrap", d.wrap),
        show_line_numbers: flag("show_line_numbers", d.show_line_numbers),
        tab_spaces: uint("tab_spaces")
            .and_then(|n| u8::try_from(n).ok())
            .filter(|n| TAB_SPACES.contains(n))
            .unwrap_or(d.tab_spaces),
        highlight: flag("highlight", d.highlight),
        large_file_mb: uint("large_file_mb").map_or(d.large_file_mb, |n| n.clamp(1, 100)),
        editor_font: choice("editor_font", &crate::fonts::IDS, &d.editor_font),
        view_font: choice("view_font", &crate::fonts::IDS, &d.view_font),
        theme: choice("theme", &crate::theme::IDS, &d.theme),
        ui_scale: float("ui_scale").map_or(d.ui_scale, |f| f.clamp(0.75, 2.0)),
        open_json: choice("open_json", &MODES, &d.open_json),
        open_markdown: choice("open_markdown", &MODES, &d.open_markdown),
        open_other: choice("open_other", &MODES, &d.open_other),
        confirm_close: flag("confirm_close", d.confirm_close),
        remember_window: flag("remember_window", d.remember_window),
        window_w: window("window_w", d.window_w),
        window_h: window("window_h", d.window_h),
        assoc_exts,
        telemetry: flag("telemetry", d.telemetry),
        telemetry_max_kb: uint("telemetry_max_kb").unwrap_or(d.telemetry_max_kb),
        telemetry_max_days: uint("telemetry_max_days")
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(d.telemetry_max_days),
    }
}

/// What to write when saving: the file as it is on disk now, overlaid with only the keys this
/// session changed (`current` vs `saved`). Keeps hand-edits made in another window or editor.
pub fn merge_for_save(disk: &Settings, saved: &Settings, current: &Settings) -> Settings {
    let table = |s: &Settings| toml::Table::try_from(s).unwrap_or_default();
    let (mut out, saved_t) = (table(disk), table(saved));
    for (k, v) in table(current) {
        if saved_t.get(&k) != Some(&v) {
            out.insert(k, v);
        }
    }
    parse(&toml::to_string(&out).unwrap_or_default())
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
        assert!(!s.wrap && s.show_line_numbers && !s.telemetry);
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
        assert!(!s.telemetry);
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
        let s = parse("telemetry = true\ntheme = 5\nwrap = true\nfont_size = \"x\"\n");
        assert!(s.telemetry, "telemetry opt-in must survive a bad neighbour");
        assert!(s.wrap);
        assert_eq!(s.theme, "system");
        assert_eq!(s.font_size, 14.0);
    }

    #[test]
    fn reset_all_restores_every_group_but_keeps_nothing_else() {
        let mut s = parse(
            "wrap = true
theme = \"nord\"
line_height = 1.6
confirm_close = false
telemetry = false
assoc_exts = [\".x\"]
",
        );
        let exts = s.assoc_exts.clone();
        s.reset_all();
        // the extension list mirrors the registry, so wiping it would orphan registered types
        assert_eq!(
            s,
            Settings {
                assoc_exts: exts,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn fonts_accept_every_listed_id_and_fall_back_per_key() {
        for id in crate::fonts::IDS {
            let s = parse(&format!(
                "editor_font = \"{id}\"
view_font = \"{id}\""
            ));
            assert_eq!((s.editor_font.as_str(), s.view_font.as_str()), (id, id));
        }
        let s = parse(
            "editor_font = \"comic-sans\"
view_font = 3
",
        );
        assert_eq!(
            (s.editor_font.as_str(), s.view_font.as_str()),
            ("monospace", "proportional")
        );
        let mut s = parse(
            "view_font = \"georgia\"
editor_font = \"arial\"",
        );
        s.reset_editor();
        assert_eq!(s.editor_font, "monospace");
        assert_eq!(
            s.view_font, "georgia",
            "reset_editor leaves the Appearance-page font"
        );
        s.reset_appearance();
        assert_eq!(
            s.view_font, "proportional",
            "the view font lives on the Appearance page"
        );
    }

    #[test]
    fn line_height_defaults_and_clamps() {
        assert_eq!(parse("").line_height, 1.0);
        assert_eq!(parse("line_height = 1.5").line_height, 1.5);
        assert_eq!(parse("line_height = 5.0").line_height, 2.0);
        assert_eq!(parse("line_height = 0.1").line_height, 0.8);
        for bad in ["\"tall\"", "nan", "inf"] {
            assert_eq!(
                parse(&format!("line_height = {bad}")).line_height,
                1.0,
                "{bad}"
            );
        }
        let mut s = parse("line_height = 1.7");
        s.reset_editor();
        assert_eq!(s.line_height, 1.0);
    }

    #[test]
    fn every_named_theme_is_a_valid_setting_and_junk_falls_back() {
        for id in crate::theme::IDS {
            assert_eq!(parse(&format!("theme = \"{id}\"")).theme, id);
        }
        assert_eq!(parse("theme = \"hotdog\"").theme, "system");
    }

    #[test]
    fn new_keys_have_documented_defaults() {
        let s = parse("");
        assert_eq!(s.tab_spaces, 0);
        assert!(s.highlight);
        assert_eq!(s.large_file_mb, 5);
        assert_eq!(s.editor_font, "monospace");
        assert_eq!(s.ui_scale, 1.0);
        assert_eq!(
            (
                s.open_json.as_str(),
                s.open_markdown.as_str(),
                s.open_other.as_str()
            ),
            ("view", "view", "edit")
        );
        assert!(s.confirm_close && s.remember_window);
        assert_eq!((s.window_w, s.window_h), (900.0, 650.0));
        assert_eq!(s.assoc_exts.len(), 17);
        assert!(s.assoc_exts.contains(&".gitignore".to_string()));
        assert!(s.assoc_exts.contains(&".env".to_string()));
    }

    #[test]
    fn out_of_range_and_wrong_values_fall_back_per_field() {
        let s = parse(
            "tab_spaces = 3\nui_scale = 9.0\nlarge_file_mb = 0\neditor_font = \"comic\"\n\
             theme = \"blue\"\nopen_json = \"x\"\nwindow_w = 5\nwindow_h = 99999\nconfirm_close = 1\n",
        );
        assert_eq!(s.tab_spaces, 0);
        assert_eq!(s.ui_scale, 2.0);
        assert_eq!(s.large_file_mb, 1);
        assert_eq!(s.editor_font, "monospace");
        assert_eq!(s.theme, "system");
        assert_eq!(s.open_json, "view");
        assert_eq!(s.window_w, 900.0);
        assert_eq!(s.window_h, 650.0);
        assert!(s.confirm_close, "wrong type keeps default");
        assert_eq!(parse("ui_scale = 0.1").ui_scale, 0.75);
        assert_eq!(parse("tab_spaces = 4").tab_spaces, 4);
        assert_eq!(parse("tab_spaces = 8").tab_spaces, 8);
        assert_eq!(parse("large_file_mb = 1000").large_file_mb, 100);
        for bad in ["nan", "inf", "-inf", "\"2\""] {
            let s = parse(&format!("ui_scale = {bad}"));
            assert!((0.75..=2.0).contains(&s.ui_scale), "{bad}");
        }
    }

    #[test]
    fn good_values_survive_a_bad_neighbour() {
        let s = parse("tab_spaces = 4\nui_scale = \"big\"\nhighlight = false\ntheme = \"dark\"\n");
        assert_eq!(s.tab_spaces, 4);
        assert!(!s.highlight);
        assert_eq!(s.theme, "dark");
        assert_eq!(s.ui_scale, 1.0);
    }

    #[test]
    fn assoc_exts_are_normalized_or_defaulted() {
        let s = parse("assoc_exts = [\"JSON\", \"json\", \".Md\", \"a b\", \"\", \".env\"]");
        assert_eq!(s.assoc_exts, vec![".json", ".md", ".env"]);
        assert_eq!(parse("assoc_exts = []").assoc_exts, Vec::<String>::new());
        assert_eq!(parse("assoc_exts = \"x\"").assoc_exts.len(), 17);
        assert_eq!(
            parse("assoc_exts = [1, 2]").assoc_exts,
            Vec::<String>::new()
        );
    }

    #[test]
    fn normalize_ext_list_dedupes_and_keeps_order() {
        let input: Vec<String> = ["JSON", "json", ".Md", "a b", "", ".env", "md"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(normalize_ext_list(&input), vec![".json", ".md", ".env"]);
        assert!(normalize_ext_list(&[]).is_empty());
    }

    #[test]
    fn start_in_view_follows_open_settings() {
        use crate::format::Format;
        let d = Settings::default();
        assert!(d.start_in_view(Format::Json));
        assert!(d.start_in_view(Format::Markdown));
        assert!(!d.start_in_view(Format::Env));
        assert!(!d.start_in_view(Format::Config));
        assert!(!d.start_in_view(Format::Plain));
        let s = parse("open_json = \"edit\"\nopen_markdown = \"edit\"\nopen_other = \"view\"");
        assert!(!s.start_in_view(Format::Json));
        assert!(!s.start_in_view(Format::Markdown));
        assert!(s.start_in_view(Format::Env));
        assert!(s.start_in_view(Format::Plain));
    }

    #[test]
    fn window_size_only_when_remembered_and_sane() {
        let mut s = Settings::default();
        assert_eq!(s.window_size(), Some((900.0, 650.0)));
        s.remember_window = false;
        assert_eq!(s.window_size(), None);
        s.remember_window = true;
        s.window_w = 1200.0;
        s.window_h = 800.0;
        assert_eq!(s.window_size(), Some((1200.0, 800.0)));
    }

    #[test]
    fn group_resets_touch_only_their_group() {
        let mut s = Settings {
            font_size: 30.0,
            theme: "dark".into(),
            telemetry: true,
            confirm_close: false,
            ..Default::default()
        };
        s.reset_editor();
        assert_eq!(s.font_size, 14.0);
        assert_eq!(s.theme, "dark");
        assert!(s.telemetry);
        s.reset_appearance();
        assert_eq!(s.theme, "system");
        assert!(!s.confirm_close);
        s.reset_behavior();
        assert!(s.confirm_close);
        assert!(s.telemetry);
        s.reset_files_privacy();
        assert!(!s.telemetry);
    }

    #[test]
    fn merge_keeps_external_edits_to_fields_this_session_did_not_change() {
        let saved = Settings::default();
        let disk = Settings {
            theme: "dark".into(),
            tab_spaces: 4,
            ..Default::default()
        };
        let current = Settings {
            wrap: true,
            ..Default::default()
        };
        let m = merge_for_save(&disk, &saved, &current);
        assert_eq!(m.theme, "dark", "hand-edit on disk survives");
        assert_eq!(m.tab_spaces, 4);
        assert!(m.wrap, "this session's change is kept");
    }

    #[test]
    fn merge_prefers_this_sessions_change_when_both_changed_the_same_key() {
        let saved = Settings::default();
        let disk = Settings {
            theme: "dark".into(),
            ..Default::default()
        };
        let current = Settings {
            theme: "light".into(),
            ..Default::default()
        };
        assert_eq!(merge_for_save(&disk, &saved, &current).theme, "light");
    }

    #[test]
    fn merge_with_no_session_changes_is_just_the_disk_state() {
        let saved = Settings::default();
        let disk = Settings {
            font_size: 20.0,
            assoc_exts: vec![".env".into()],
            ..Default::default()
        };
        assert_eq!(merge_for_save(&disk, &saved, &saved.clone()), disk);
        let current = Settings {
            assoc_exts: vec![".log".into()],
            ..Default::default()
        };
        assert_eq!(
            merge_for_save(&disk, &saved, &current).assoc_exts,
            vec![".log".to_string()]
        );
    }

    #[test]
    fn full_roundtrip_including_new_keys() {
        let s = Settings {
            tab_spaces: 4,
            highlight: false,
            editor_font: "proportional".into(),
            ui_scale: 1.25,
            open_other: "view".into(),
            window_w: 1000.0,
            assoc_exts: vec![".env".into(), ".log".into()],
            ..Default::default()
        };
        assert_eq!(parse(&toml::to_string(&s).unwrap()), s);
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
