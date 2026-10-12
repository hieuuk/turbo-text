//! Colour themes: const palettes, applied to egui once on change (no per-frame cost).

use crate::highlight::Kind;
use eframe::egui::{Color32, Visuals};

/// Number of `Kind` variants; `Palette::syn` is indexed by `Kind as usize`.
const KINDS: usize = 11;

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub struct Palette {
    pub id: &'static str,
    pub name: &'static str,
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub fg: Color32,
    pub accent: Color32,
    syn: [Color32; KINDS],
}

impl Palette {
    pub fn kind(&self, k: Kind) -> Color32 {
        self.syn[k as usize]
    }

    /// Line numbers: the text colour faded towards the background.
    pub fn gutter(&self) -> Color32 {
        self.fg.lerp_to_gamma(self.bg, 0.55)
    }

    /// egui visuals for this palette. The two stock themes keep egui's own look.
    pub fn visuals(&self) -> Visuals {
        let mut v = if self.dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        if self.id == "dark" || self.id == "light" {
            return v;
        }
        v.panel_fill = self.bg;
        // between bg and surface, so dialogs stand out from the editor and widgets stand out from dialogs
        v.window_fill = self.bg.lerp_to_gamma(self.surface, 0.5);
        v.extreme_bg_color = self.bg;
        v.faint_bg_color = self.surface;
        v.hyperlink_color = self.accent;
        v.selection.bg_fill = self.accent.gamma_multiply(0.45);
        v.selection.stroke.color = self.fg;
        v.window_stroke.color = self.fg.lerp_to_gamma(self.bg, 0.8);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = self.bg;
        w.noninteractive.bg_stroke.color = self.fg.lerp_to_gamma(self.bg, 0.85);
        w.noninteractive.fg_stroke.color = self.fg;
        for s in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
            s.fg_stroke.color = self.fg;
        }
        w.inactive.bg_fill = self.surface;
        w.inactive.weak_bg_fill = self.surface;
        w.inactive.bg_stroke.color = self.fg.lerp_to_gamma(self.bg, 0.8);
        w.hovered.bg_fill = self.surface.lerp_to_gamma(self.accent, 0.25);
        w.hovered.weak_bg_fill = w.hovered.bg_fill;
        w.active.bg_fill = self.surface.lerp_to_gamma(self.accent, 0.45);
        w.active.weak_bg_fill = w.active.bg_fill;
        w.open.bg_fill = self.surface;
        w.open.weak_bg_fill = self.surface;
        v
    }

    /// Swatch colours for the theme card, in display order.
    pub fn swatch(&self) -> [Color32; 6] {
        [
            self.bg,
            self.fg,
            self.kind(Kind::Key),
            self.kind(Kind::String),
            self.kind(Kind::Keyword),
            self.kind(Kind::Number),
        ]
    }
}

// Kind order: Key, String, Number, Keyword, Comment, Heading, Emph, Code, Link, Marker, Section.
const fn syn(c: [u32; KINDS]) -> [Color32; KINDS] {
    let mut out = [Color32::BLACK; KINDS];
    let mut i = 0;
    while i < KINDS {
        out[i] = hex(c[i]);
        i += 1;
    }
    out
}

macro_rules! palette {
    ($id:literal, $name:literal, $dark:literal, $bg:literal, $surface:literal, $fg:literal, $accent:literal, $syn:expr) => {
        Palette {
            id: $id,
            name: $name,
            dark: $dark,
            bg: hex($bg),
            surface: hex($surface),
            fg: hex($fg),
            accent: hex($accent),
            syn: syn($syn),
        }
    };
}

/// Stored in `Settings::theme`; "system" follows the OS between the first two palettes.
pub const IDS: [&str; 13] = [
    "system",
    "dark",
    "light",
    "dracula",
    "tokyo-night",
    "catppuccin-mocha",
    "nord",
    "gruvbox-dark",
    "solarized-dark",
    "one-dark",
    "monokai",
    "solarized-light",
    "catppuccin-latte",
];

pub static PALETTES: [Palette; 12] = [
    palette!(
        "dark",
        "Default Dark",
        true,
        0x1b1b1b,
        0x2b2b2b,
        0xd2d2d2,
        0x4e9bd6,
        [
            0x9cdcfe, 0xce9178, 0xb5cea8, 0x569cd6, 0x6a9955, 0x569cd6, 0xdcdcaa, 0xce9178,
            0x4ec9b0, 0xc586c0, 0xdcdcaa
        ]
    ),
    palette!(
        "light",
        "Default Light",
        false,
        0xf8f8f8,
        0xe9e9e9,
        0x282828,
        0x0078d4,
        [
            0x00468c, 0xa31515, 0x098658, 0x0000ff, 0x008000, 0x0000a0, 0x795e26, 0xa31515,
            0x007082, 0xaf00db, 0x795e26
        ]
    ),
    palette!(
        "dracula",
        "Dracula",
        true,
        0x282a36,
        0x343746,
        0xf8f8f2,
        0xbd93f9,
        [
            0x8be9fd, 0xf1fa8c, 0xbd93f9, 0xff79c6, 0x7f8dc0, 0xbd93f9, 0xffb86c, 0xf1fa8c,
            0x8be9fd, 0xff79c6, 0x50fa7b
        ]
    ),
    palette!(
        "tokyo-night",
        "Tokyo Night",
        true,
        0x1a1b26,
        0x24283b,
        0xc0caf5,
        0x7aa2f7,
        [
            0x7dcfff, 0x9ece6a, 0xff9e64, 0xbb9af7, 0x7a83ac, 0x7aa2f7, 0xe0af68, 0x9ece6a,
            0x73daca, 0xbb9af7, 0xe0af68
        ]
    ),
    palette!(
        "catppuccin-mocha",
        "Catppuccin Mocha",
        true,
        0x1e1e2e,
        0x313244,
        0xcdd6f4,
        0xcba6f7,
        [
            0x89dceb, 0xa6e3a1, 0xfab387, 0xcba6f7, 0x8a90a8, 0x89b4fa, 0xf9e2af, 0xa6e3a1,
            0x94e2d5, 0xf5c2e7, 0xf9e2af
        ]
    ),
    palette!(
        "nord",
        "Nord",
        true,
        0x2e3440,
        0x3b4252,
        0xd8dee9,
        0x88c0d0,
        [
            0x8fbcbb, 0xa3be8c, 0xb48ead, 0x81a1c1, 0x8a96ae, 0x88c0d0, 0xebcb8b, 0xa3be8c,
            0x88c0d0, 0xb48ead, 0xebcb8b
        ]
    ),
    palette!(
        "gruvbox-dark",
        "Gruvbox Dark",
        true,
        0x282828,
        0x3c3836,
        0xebdbb2,
        0xfabd2f,
        [
            0x83a598, 0xb8bb26, 0xd3869b, 0xfb4934, 0x928374, 0xfabd2f, 0xfe8019, 0xb8bb26,
            0x8ec07c, 0xd3869b, 0xfabd2f
        ]
    ),
    palette!(
        "solarized-dark",
        "Solarized Dark",
        true,
        0x002b36,
        0x073642,
        0x93a1a1,
        0x268bd2,
        [
            0x4a9fe0, 0x2aa198, 0xe0529a, 0x859900, 0x6c8a92, 0x4a9fe0, 0xb58900, 0x2aa198,
            0x2aa198, 0x8a8fe0, 0xe0653a
        ]
    ),
    palette!(
        "one-dark",
        "One Dark",
        true,
        0x282c34,
        0x2c313a,
        0xabb2bf,
        0x61afef,
        [
            0xe06c75, 0x98c379, 0xd19a66, 0xc678dd, 0x7f8798, 0x61afef, 0xe5c07b, 0x98c379,
            0x56b6c2, 0xc678dd, 0xe5c07b
        ]
    ),
    palette!(
        "monokai",
        "Monokai",
        true,
        0x272822,
        0x3e3d32,
        0xf8f8f2,
        0xa6e22e,
        [
            0xa6e22e, 0xe6db74, 0xae81ff, 0xf92672, 0x8f8f7a, 0x66d9ef, 0xfd971f, 0xe6db74,
            0x66d9ef, 0xf92672, 0xfd971f
        ]
    ),
    palette!(
        "solarized-light",
        "Solarized Light",
        false,
        0xfdf6e3,
        0xeee8d5,
        0x586e75,
        0x268bd2,
        [
            0x1f6fb0, 0x1e8078, 0xc02a78, 0x6f7d00, 0x6b7f84, 0x1f6fb0, 0x96700a, 0x1e8078,
            0x1e8078, 0x5b60b5, 0xb3420f
        ]
    ),
    palette!(
        "catppuccin-latte",
        "Catppuccin Latte",
        false,
        0xeff1f5,
        0xdce0e8,
        0x4c4f69,
        0x8839ef,
        [
            0x1a7fb0, 0x2f7d1f, 0xd04e00, 0x8839ef, 0x6c6f85, 0x1e66f5, 0xa8660f, 0x2f7d1f,
            0x179299, 0xc2479f, 0xa8660f
        ]
    ),
];

/// The palette for a stored theme id. "system" (and anything unknown) follows the OS.
pub fn resolve(id: &str, system_dark: bool) -> &'static Palette {
    let by_id = |id: &str| PALETTES.iter().find(|p| p.id == id);
    match id {
        "system" => by_id(if system_dark { "dark" } else { "light" }),
        _ => by_id(id),
    }
    .unwrap_or(&PALETTES[usize::from(!system_dark)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lum(c: Color32) -> f32 {
        let f = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    const ALL_KINDS: [Kind; KINDS] = [
        Kind::Key,
        Kind::String,
        Kind::Number,
        Kind::Keyword,
        Kind::Comment,
        Kind::Heading,
        Kind::Emph,
        Kind::Code,
        Kind::Link,
        Kind::Marker,
        Kind::Section,
    ];

    #[test]
    fn kind_order_matches_the_syn_array() {
        for (i, k) in ALL_KINDS.iter().enumerate() {
            assert_eq!(*k as usize, i, "{k:?}");
        }
    }

    #[test]
    fn ids_and_palettes_line_up() {
        let mut seen: Vec<&str> = PALETTES.iter().map(|p| p.id).collect();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n, "palette ids unique");
        assert_eq!(IDS[0], "system");
        for p in &PALETTES {
            assert!(IDS.contains(&p.id), "{} missing from IDS", p.id);
        }
        for id in &IDS[1..] {
            assert!(PALETTES.iter().any(|p| p.id == *id), "{id} has no palette");
        }
    }

    #[test]
    fn resolve_follows_system_and_falls_back() {
        assert_eq!(resolve("system", true).id, "dark");
        assert_eq!(resolve("system", false).id, "light");
        assert_eq!(resolve("nord", false).id, "nord");
        assert_eq!(resolve("nonsense", true).id, "dark");
        assert_eq!(resolve("nonsense", false).id, "light");
    }

    #[test]
    fn text_is_readable_on_every_background() {
        for p in &PALETTES {
            assert!(contrast(p.fg, p.bg) >= 4.5, "{} plain text", p.id);
            for k in ALL_KINDS {
                let c = contrast(p.kind(k), p.bg);
                assert!(c >= 3.0, "{} {k:?} contrast {c:.2}", p.id);
            }
        }
    }

    #[test]
    fn dark_flag_matches_background() {
        for p in &PALETTES {
            assert_eq!(p.dark, lum(p.bg) < 0.5, "{}", p.id);
        }
    }

    #[test]
    fn stock_palettes_keep_the_previous_colours() {
        let d = resolve("dark", true);
        assert_eq!(d.kind(Kind::Key), Color32::from_rgb(156, 220, 254));
        assert_eq!(d.fg, Color32::from_gray(210));
        let l = resolve("light", false);
        assert_eq!(l.kind(Kind::String), Color32::from_rgb(163, 21, 21));
    }
}
