//! The settings dialog: an in-window egui `Window` with a section sidebar, drawn only while open.

use crate::assoc;
use crate::settings::Settings;
use crate::theme::{self, PALETTES};
use eframe::egui::{
    self, Align, Align2, Button, ComboBox, DragValue, Layout, RichText, ScrollArea, Sense, Ui, Vec2,
};

/// What the user asked to (un)register in the Windows registry when pressing Apply.
pub struct AssocRequest {
    pub on: Vec<String>,
    pub off: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Editor,
    Appearance,
    Behavior,
    Files,
}

const PAGES: [(Page, &str); 4] = [
    (Page::Editor, "Editor"),
    (Page::Appearance, "Appearance"),
    (Page::Behavior, "Behavior"),
    (Page::Files, "Files and privacy"),
];

const SIDEBAR_W: f32 = 180.0;
const ROW_H: f32 = 36.0;
const CONTENT_W: f32 = 520.0;
const BODY_H: f32 = 440.0;
const GRID_SPACING: [f32; 2] = [28.0, 16.0];

#[derive(Default)]
pub struct SettingsUi {
    pub open: bool,
    page: Page,
    /// Checkbox state per extension, kept in the same order as `Settings::assoc_exts`.
    assoc: Vec<(String, bool)>,
    custom: String,
    custom_error: Option<&'static str>,
}

/// Adds a user-typed extension to the list. Returns the normalized extension on success.
pub fn add_ext(list: &mut Vec<String>, input: &str) -> Result<String, &'static str> {
    let ext = assoc::normalize_ext(input);
    if ext.is_empty() {
        return Err("Enter an extension like .log");
    }
    if list.contains(&ext) {
        return Err("Already in the list");
    }
    list.push(ext.clone());
    Ok(ext)
}

/// "Follow the system light / dark setting": on stores "system"; off pins the theme in use now.
pub fn set_follow_system(theme: &mut String, follow: bool, system_dark: bool) {
    if follow {
        *theme = "system".into();
    } else if theme == "system" {
        *theme = theme::resolve("system", system_dark).id.into();
    }
}

const SCALES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

impl SettingsUi {
    pub fn open(&mut self) {
        self.open = true;
        // Debug builds only: pick the page for screenshots (TT_DEBUG_SETTINGS=appearance|behavior|files).
        #[cfg(debug_assertions)]
        if let Ok(v) = std::env::var("TT_DEBUG_SETTINGS") {
            self.page = match v.as_str() {
                "appearance" => Page::Appearance,
                "behavior" => Page::Behavior,
                "files" => Page::Files,
                _ => Page::Editor,
            };
        }
        self.assoc.clear();
        self.custom_error = None;
    }

    /// Keeps checkbox state aligned with the extension list; new entries read the registry.
    fn sync(&mut self, exts: &[String]) {
        let old = std::mem::take(&mut self.assoc);
        self.assoc = exts
            .iter()
            .map(|e| {
                let on = old
                    .iter()
                    .find(|(o, _)| o == e)
                    .map_or_else(|| assoc::is_associated(e), |(_, on)| *on);
                (e.clone(), on)
            })
            .collect();
    }

    pub fn show(&mut self, ctx: &egui::Context, s: &mut Settings) -> Option<AssocRequest> {
        if !self.open {
            return None;
        }
        self.sync(&s.assoc_exts);
        let mut open = true;
        let mut request = None;
        let frame = egui::Frame::window(&ctx.global_style()).inner_margin(20.0);
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .frame(frame)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size([760.0, 520.0])
            .max_size(ctx.content_rect().size() * 0.95)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.sidebar(ui, s);
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(CONTENT_W);
                        ui.set_height(BODY_H);
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.add_space(2.0);
                                request = self.page_content(ui, s);
                            });
                    });
                });
            });
        self.open = open;
        request
    }

    fn sidebar(&mut self, ui: &mut Ui, s: &mut Settings) {
        ui.allocate_ui_with_layout(
            Vec2::new(SIDEBAR_W, BODY_H),
            Layout::top_down(Align::LEFT),
            |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                for (page, label) in PAGES {
                    let b = Button::selectable(self.page == page, label);
                    if ui.add_sized([SIDEBAR_W, ROW_H], b).clicked() {
                        self.page = page;
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    let b = Button::new("Reset all to defaults");
                    if ui.add_sized([SIDEBAR_W, ROW_H], b).clicked() {
                        s.reset_all();
                    }
                });
            },
        );
    }

    fn page_content(&mut self, ui: &mut Ui, s: &mut Settings) -> Option<AssocRequest> {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 12.0);
        ui.spacing_mut().interact_size.y = 26.0;
        let title = PAGES.iter().find(|(p, _)| *p == self.page).map(|(_, l)| *l);
        ui.label(RichText::new(title.unwrap_or("")).size(20.0).strong());
        ui.add_space(8.0);
        let mut request = None;
        match self.page {
            Page::Editor => editor_page(ui, s),
            Page::Appearance => appearance_page(ui, s),
            Page::Behavior => behavior_page(ui, s),
            Page::Files => request = self.files_page(ui, s),
        }
        request
    }

    fn files_page(&mut self, ui: &mut Ui, s: &mut Settings) -> Option<AssocRequest> {
        let mut request = None;
        ui.label("Open with TurboText (current user, no admin):");
        let mut remove = None;
        ScrollArea::vertical()
            .id_salt("assoc_list")
            .max_height(214.0)
            .show(ui, |ui| {
                egui::Grid::new("assoc_grid")
                    .num_columns(2)
                    .spacing([24.0, 10.0])
                    .show(ui, |ui| {
                        for (i, (ext, on)) in self.assoc.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.checkbox(on, ext.as_str());
                                if ui
                                    .small_button("x")
                                    .on_hover_text("Remove from list")
                                    .clicked()
                                {
                                    remove = Some(i);
                                }
                            });
                            if i % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
            });
        if let Some(i) = remove {
            s.assoc_exts.remove(i);
            self.assoc.remove(i);
        }
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.custom)
                    .hint_text(".ext")
                    .desired_width(110.0),
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Add").clicked() || enter {
                match add_ext(&mut s.assoc_exts, &self.custom) {
                    Ok(_) => {
                        self.custom.clear();
                        self.custom_error = None;
                    }
                    Err(e) => self.custom_error = Some(e),
                }
            }
            if let Some(e) = self.custom_error {
                ui.colored_label(egui::Color32::from_rgb(220, 80, 80), e);
            }
        });
        if ui.button("Apply associations").clicked() {
            request = Some(AssocRequest {
                on: self
                    .assoc
                    .iter()
                    .filter(|(_, c)| *c)
                    .map(|(e, _)| e.clone())
                    .collect(),
                off: self
                    .assoc
                    .iter()
                    .filter(|(_, c)| !*c)
                    .map(|(e, _)| e.clone())
                    .collect(),
            });
        }
        hint(
            ui,
            "Windows may ask you to confirm the default app in Settings.",
        );
        ui.separator();
        ui.checkbox(
            &mut s.telemetry,
            "Keep a local usage log (never leaves this PC)",
        );
        egui::Grid::new("telemetry_grid")
            .num_columns(2)
            .spacing(GRID_SPACING)
            .show(ui, |ui| {
                ui.label("Log size limit");
                ui.add(
                    DragValue::new(&mut s.telemetry_max_kb)
                        .range(64..=102_400)
                        .suffix(" KB"),
                );
                ui.end_row();
                ui.label("Keep for");
                ui.add(
                    DragValue::new(&mut s.telemetry_max_days)
                        .range(1..=3650)
                        .suffix(" days"),
                );
                ui.end_row();
            });
        hint(ui, "Usage log settings apply next launch.");
        reset_button(ui, || s.reset_files_privacy());
        request
    }
}

fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).small().weak());
}

fn reset_button(ui: &mut Ui, reset: impl FnOnce()) {
    ui.add_space(8.0);
    if ui.button("Reset this page to defaults").clicked() {
        reset();
    }
}

fn grid(id: &str) -> egui::Grid {
    egui::Grid::new(id).num_columns(2).spacing(GRID_SPACING)
}

fn editor_page(ui: &mut Ui, s: &mut Settings) {
    grid("editor_grid").show(ui, |ui| {
        ui.label("Font size");
        ui.add(egui::Slider::new(&mut s.font_size, 6.0..=72.0).step_by(1.0));
        ui.end_row();

        ui.label("Font");
        choice(
            ui,
            "editor_font",
            &mut s.editor_font,
            &[("monospace", "Monospace"), ("proportional", "Proportional")],
        );
        ui.end_row();

        ui.label("Line height");
        ui.add(
            egui::Slider::new(&mut s.line_height, 0.8..=2.0)
                .step_by(0.05)
                .fixed_decimals(2),
        );
        ui.end_row();

        ui.label("Tab key inserts");
        let label = match s.tab_spaces {
            0 => "a tab character".to_string(),
            n => format!("{n} spaces"),
        };
        ComboBox::from_id_salt("tab_spaces")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut s.tab_spaces, 0, "a tab character");
                for n in [2u8, 4, 8] {
                    ui.selectable_value(&mut s.tab_spaces, n, format!("{n} spaces"));
                }
            });
        ui.end_row();

        ui.label("Word wrap");
        ui.checkbox(&mut s.wrap, "");
        ui.end_row();

        ui.label("Line numbers");
        ui.checkbox(&mut s.show_line_numbers, "");
        ui.end_row();

        ui.label("Syntax highlighting");
        ui.checkbox(&mut s.highlight, "");
        ui.end_row();

        ui.label("Highlighting off above");
        ui.add(
            DragValue::new(&mut s.large_file_mb)
                .range(1..=100)
                .suffix(" MB"),
        );
        ui.end_row();
    });
    reset_button(ui, || s.reset_editor());
}

fn appearance_page(ui: &mut Ui, s: &mut Settings) {
    let mut follow = s.theme == "system";
    if ui
        .checkbox(&mut follow, "Follow the system light / dark setting")
        .changed()
    {
        let system_dark = ui.visuals().dark_mode;
        set_follow_system(&mut s.theme, follow, system_dark);
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Colour theme").strong());
    ui.add_enabled_ui(!follow, |ui| {
        let gap = 14.0;
        let w = ((ui.available_width() - gap) / 2.0).floor().max(120.0);
        egui::Grid::new("theme_grid")
            .num_columns(2)
            .spacing([gap, gap])
            .show(ui, |ui| {
                for (i, pal) in PALETTES.iter().enumerate() {
                    if theme_card(ui, pal, s.theme == pal.id, w) {
                        s.theme = pal.id.into();
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    });
    ui.add_space(6.0);
    grid("appearance_grid").show(ui, |ui| {
        ui.label("Interface zoom");
        let label = format!("{:.0}%", s.ui_scale * 100.0);
        ComboBox::from_id_salt("ui_scale")
            .selected_text(label)
            .show_ui(ui, |ui| {
                for v in SCALES {
                    ui.selectable_value(&mut s.ui_scale, v, format!("{:.0}%", v * 100.0));
                }
            });
        ui.end_row();
    });
    reset_button(ui, || s.reset_appearance());
}

/// A clickable card: a strip of the theme's colours above its name. Returns true when clicked.
fn theme_card(ui: &mut Ui, pal: &theme::Palette, selected: bool, w: f32) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 64.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let fill = if resp.hovered() {
            v.widgets.hovered.bg_fill
        } else {
            v.widgets.inactive.bg_fill
        };
        let border = if selected {
            egui::Stroke::new(2.0, v.hyperlink_color)
        } else {
            egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color)
        };
        let p = ui.painter();
        p.rect(rect, 6.0, fill, border, egui::StrokeKind::Inside);
        let strip =
            egui::Rect::from_min_size(rect.min + Vec2::new(10.0, 10.0), Vec2::new(w - 20.0, 18.0));
        let sw = pal.swatch();
        let seg = strip.width() / sw.len() as f32;
        for (i, c) in sw.iter().enumerate() {
            let r = egui::Rect::from_min_size(
                strip.min + Vec2::new(seg * i as f32, 0.0),
                Vec2::new(seg, strip.height()),
            );
            p.rect_filled(r, 0.0, *c);
        }
        p.rect_stroke(
            strip,
            2.0,
            egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Outside,
        );
        p.text(
            rect.left_bottom() + Vec2::new(10.0, -9.0),
            Align2::LEFT_BOTTOM,
            pal.name,
            egui::FontId::proportional(14.0),
            v.text_color(),
        );
    }
    resp.clicked()
}

fn behavior_page(ui: &mut Ui, s: &mut Settings) {
    let modes = [("view", "View"), ("edit", "Edit")];
    ui.label("Open files in (applies to the next file opened):");
    grid("open_grid").show(ui, |ui| {
        ui.label("JSON");
        choice(ui, "open_json", &mut s.open_json, &modes);
        ui.end_row();
        ui.label("Markdown");
        choice(ui, "open_markdown", &mut s.open_markdown, &modes);
        ui.end_row();
        ui.label("Everything else");
        choice(ui, "open_other", &mut s.open_other, &modes);
        ui.end_row();
    });
    ui.separator();
    ui.checkbox(
        &mut s.confirm_close,
        "Ask before closing with unsaved changes",
    );
    if !s.confirm_close {
        ui.colored_label(
            egui::Color32::from_rgb(220, 160, 40),
            "Off: closing discards unsaved edits without asking.",
        );
    }
    ui.checkbox(
        &mut s.remember_window,
        "Remember window size (applies next launch)",
    );
    reset_button(ui, || s.reset_behavior());
}

/// A combo box over `(stored value, label)` pairs, storing the value string.
fn choice(ui: &mut Ui, id: &str, value: &mut String, options: &[(&str, &str)]) {
    let label = options
        .iter()
        .find(|(v, _)| v == value)
        .map_or(value.as_str(), |(_, l)| l)
        .to_owned();
    ComboBox::from_id_salt(id)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (v, l) in options {
                ui.selectable_value(value, v.to_string(), *l);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn add_normalizes_and_appends() {
        let mut l = list(&[".env"]);
        assert_eq!(add_ext(&mut l, "  LOG "), Ok(".log".to_string()));
        assert_eq!(add_ext(&mut l, ".Toml"), Ok(".toml".to_string()));
        assert_eq!(l, list(&[".env", ".log", ".toml"]));
    }

    #[test]
    fn follow_system_toggles_between_system_and_the_theme_in_use() {
        let mut t = "nord".to_string();
        set_follow_system(&mut t, true, true);
        assert_eq!(t, "system");
        set_follow_system(&mut t, false, true);
        assert_eq!(t, "dark");
        set_follow_system(&mut t, true, false);
        set_follow_system(&mut t, false, false);
        assert_eq!(t, "light");
        let mut named = "dracula".to_string();
        set_follow_system(&mut named, false, false);
        assert_eq!(named, "dracula", "unticking keeps an explicit theme");
    }

    #[test]
    fn add_rejects_invalid_and_duplicates_without_changing_the_list() {
        let mut l = list(&[".env"]);
        assert_eq!(add_ext(&mut l, ""), Err("Enter an extension like .log"));
        assert_eq!(add_ext(&mut l, "a b"), Err("Enter an extension like .log"));
        assert_eq!(add_ext(&mut l, "ENV"), Err("Already in the list"));
        assert_eq!(add_ext(&mut l, ".env"), Err("Already in the list"));
        assert_eq!(l, list(&[".env"]));
    }
}

#[cfg(test)]
mod glyph_tests {
    /// egui's bundled font has no glyph for these; they render as empty boxes in buttons and labels.
    const MISSING: [char; 5] = ['✕', '▲', '▼', '●', '✓'];

    #[test]
    fn ui_source_avoids_glyphs_missing_from_the_default_font() {
        for (name, src) in [
            ("app.rs", include_str!("app.rs")),
            ("settings_ui.rs", include_str!("settings_ui.rs")),
            ("editor.rs", include_str!("editor.rs")),
            ("views.rs", include_str!("views.rs")),
        ] {
            // the list above is the only place these characters may appear
            let body = src.split("const MISSING").next().unwrap_or(src);
            for g in MISSING {
                assert!(
                    !body.contains(g),
                    "{name} contains {g:?}, which renders as a box"
                );
            }
        }
    }
}
