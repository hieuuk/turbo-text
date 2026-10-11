//! The settings dialog: an in-window egui `Window`, drawn only while open.

use crate::assoc;
use crate::settings::Settings;
use eframe::egui::{self, Align2, ComboBox, DragValue, RichText, ScrollArea, Ui};

/// What the user asked to (un)register in the Windows registry when pressing Apply.
pub struct AssocRequest {
    pub on: Vec<String>,
    pub off: Vec<String>,
}

#[derive(Default)]
pub struct SettingsUi {
    pub open: bool,
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

const SCALES: [f32; 8] = [0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

impl SettingsUi {
    pub fn open(&mut self) {
        self.open = true;
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
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size([460.0, 600.0])
            .max_width(520.0)
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        editor_group(ui, s);
                        appearance_group(ui, s);
                        behavior_group(ui, s);
                        request = self.files_group(ui, s);
                    });
            });
        self.open = open;
        request
    }

    fn files_group(&mut self, ui: &mut Ui, s: &mut Settings) -> Option<AssocRequest> {
        let mut request = None;
        egui::CollapsingHeader::new(RichText::new("Files and privacy").strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Open with TurboText (current user, no admin):");
                let mut remove = None;
                ScrollArea::vertical()
                    .id_salt("assoc_list")
                    .max_height(150.0)
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
                        }
                    });
                if let Some(i) = remove {
                    s.assoc_exts.remove(i);
                    self.assoc.remove(i);
                }
                ui.horizontal(|ui| {
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut self.custom)
                            .hint_text(".ext")
                            .desired_width(100.0),
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
                ui.label(
                    RichText::new("Windows may ask you to confirm the default app in Settings.")
                        .small()
                        .weak(),
                );
                ui.separator();
                ui.checkbox(
                    &mut s.telemetry,
                    "Keep a local usage log (never leaves this PC)",
                );
                ui.horizontal(|ui| {
                    ui.label("Log size limit (KB)");
                    ui.add(DragValue::new(&mut s.telemetry_max_kb).range(64..=102_400));
                    ui.label("keep days");
                    ui.add(DragValue::new(&mut s.telemetry_max_days).range(1..=3650));
                });
                ui.label(
                    RichText::new("Usage log settings apply next launch.")
                        .small()
                        .weak(),
                );
                if ui.button("Reset this group to defaults").clicked() {
                    s.reset_files_privacy();
                }
            });
        request
    }
}

fn editor_group(ui: &mut Ui, s: &mut Settings) {
    egui::CollapsingHeader::new(RichText::new("Editor").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.add(
                egui::Slider::new(&mut s.font_size, 6.0..=72.0)
                    .step_by(1.0)
                    .text("Font size"),
            );
            ui.horizontal(|ui| {
                ui.label("Font");
                choice(
                    ui,
                    "editor_font",
                    &mut s.editor_font,
                    &[("monospace", "Monospace"), ("proportional", "Proportional")],
                );
            });
            ui.checkbox(&mut s.wrap, "Word wrap");
            ui.checkbox(&mut s.show_line_numbers, "Line numbers");
            ui.checkbox(&mut s.highlight, "Syntax highlighting");
            ui.horizontal(|ui| {
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
            });
            ui.horizontal(|ui| {
                ui.label("Turn highlighting off above");
                ui.add(
                    DragValue::new(&mut s.large_file_mb)
                        .range(1..=100)
                        .suffix(" MB"),
                );
            });
            if ui.button("Reset this group to defaults").clicked() {
                s.reset_editor();
            }
        });
}

fn appearance_group(ui: &mut Ui, s: &mut Settings) {
    egui::CollapsingHeader::new(RichText::new("Appearance").strong())
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Theme");
                choice(
                    ui,
                    "theme",
                    &mut s.theme,
                    &[
                        ("system", "Follow system"),
                        ("light", "Light"),
                        ("dark", "Dark"),
                    ],
                );
            });
            ui.horizontal(|ui| {
                ui.label("Interface zoom");
                let label = format!("{:.0}%", s.ui_scale * 100.0);
                ComboBox::from_id_salt("ui_scale")
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        for v in SCALES {
                            ui.selectable_value(&mut s.ui_scale, v, format!("{:.0}%", v * 100.0));
                        }
                    });
            });
            if ui.button("Reset this group to defaults").clicked() {
                s.reset_appearance();
            }
        });
}

fn behavior_group(ui: &mut Ui, s: &mut Settings) {
    egui::CollapsingHeader::new(RichText::new("Behavior").strong())
        .default_open(true)
        .show(ui, |ui| {
            let modes = [("view", "View"), ("edit", "Edit")];
            ui.label("Open files in (applies to the next file opened):");
            ui.horizontal(|ui| {
                ui.label("JSON");
                choice(ui, "open_json", &mut s.open_json, &modes);
                ui.label("Markdown");
                choice(ui, "open_markdown", &mut s.open_markdown, &modes);
                ui.label("Other");
                choice(ui, "open_other", &mut s.open_other, &modes);
            });
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
            if ui.button("Reset this group to defaults").clicked() {
                s.reset_behavior();
            }
        });
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
