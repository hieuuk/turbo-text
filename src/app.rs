use crate::editor::{self, HlCache};
use crate::find;
use crate::format::{self, Format};
use crate::io::{self, Eol};
use crate::settings::{self, Settings};
use crate::telemetry::{self, Telemetry};
use crate::{assoc, views};
use eframe::egui::{self, Key, Modifiers, RichText, TextEdit};
use egui_commonmark::CommonMarkCache;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Files above this size open without syntax highlighting.
const LARGE_FILE: u64 = 5 * 1024 * 1024;
const ASSOC_EXTS: [&str; 6] = [".env", ".json", ".md", ".toml", ".ini", ".yaml"];

type JsonResult = Result<Value, (usize, usize, String)>;

#[derive(Default)]
struct Find {
    open: bool,
    replace: bool,
    focus: bool,
    needle: String,
    with: String,
    case: bool,
    matches: Vec<(usize, usize)>,
    cur: Option<usize>,
    key: u64,
}

pub struct App {
    path: Option<PathBuf>,
    text: String,
    eol: Eol,
    lossy: bool,
    dirty: bool,
    fmt: Format,
    view: bool,
    banner: Option<String>,
    settings: Settings,
    tel: Telemetry,
    hl: HlCache,
    json: Option<(u64, JsonResult)>,
    md: CommonMarkCache,
    find: Find,
    mode_since: Instant,
    started_logged: bool,
    last_title: String,
    allow_close: bool,
    select: Option<(usize, usize)>,
    assoc_open: bool,
    assoc_sel: Vec<(String, bool)>,
}

fn hash_of<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

fn spawn_self(path: &Path) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).arg(path).spawn();
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let settings = settings::load();
        cc.egui_ctx.set_theme(match settings.theme.as_str() {
            "dark" => egui::ThemePreference::Dark,
            "light" => egui::ThemePreference::Light,
            _ => egui::ThemePreference::System,
        });
        let tel = telemetry::init(&settings);
        let mut app = App {
            path: None,
            text: String::new(),
            eol: Eol::Crlf,
            lossy: false,
            dirty: false,
            fmt: Format::Plain,
            view: false,
            banner: None,
            settings,
            tel,
            hl: HlCache::default(),
            json: None,
            md: CommonMarkCache::default(),
            find: Find::default(),
            mode_since: Instant::now(),
            started_logged: false,
            last_title: String::new(),
            allow_close: false,
            select: None,
            assoc_open: false,
            assoc_sel: Vec::new(),
        };
        if let Some(p) = path {
            app.load(&p, "cli");
        }
        app
    }

    fn load(&mut self, path: &Path, source: &str) {
        let t0 = Instant::now();
        self.flush_mode_time();
        match io::load(path) {
            Ok(l) => {
                self.size_banner(l.size, l.lossy);
                self.text = l.text;
                self.eol = l.eol;
                self.lossy = l.lossy;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.text.clear();
                self.eol = Eol::Crlf;
                self.lossy = false;
                self.banner = Some("New file: it will be created on save.".into());
            }
            Err(e) => {
                self.banner = Some(format!("Cannot open {}: {e}", path.display()));
                self.tel.event(
                    "problem",
                    json!({"kind": "open_failed", "err": e.kind().to_string()}),
                );
                return;
            }
        }
        self.path = Some(path.to_path_buf());
        self.fmt = format::detect(path);
        self.view = self.fmt.default_view() && !self.text.trim().is_empty();
        self.dirty = false;
        self.json = None;
        self.find.matches.clear();
        self.find.key = 0;
        self.mode_since = Instant::now();
        self.tel.event(
            "file_open",
            json!({
                "format": self.fmt.name(),
                "ext": self.tel.token(path),
                "size": telemetry::size_bucket(self.text.len() as u64),
                "source": source,
                "view": self.view,
                "ms": t0.elapsed().as_millis() as u64,
            }),
        );
    }

    fn size_banner(&mut self, size: u64, lossy: bool) {
        self.banner = if lossy {
            self.tel.event("problem", json!({"kind": "non_utf8"}));
            Some("File is not valid UTF-8: opened read-only, saving is disabled.".into())
        } else if size > LARGE_FILE {
            self.tel.event("problem", json!({"kind": "large_file"}));
            Some("Large file: syntax highlighting is off.".into())
        } else {
            None
        };
    }

    fn flush_mode_time(&mut self) {
        let secs = self.mode_since.elapsed().as_secs_f64();
        if self.path.is_some() {
            self.tel.add_mode_time(self.fmt.name(), self.view, secs);
        }
        self.mode_since = Instant::now();
    }

    fn confirm_discard(&self) -> bool {
        !self.dirty
            || rfd::MessageDialog::new()
                .set_title("Unsaved changes")
                .set_description("Discard unsaved changes?")
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                == rfd::MessageDialogResult::Yes
    }

    fn open_dialog(&mut self) {
        if !self.confirm_discard() {
            return;
        }
        if let Some(p) = rfd::FileDialog::new().pick_file() {
            self.load(&p, "dialog");
        }
    }

    fn save(&mut self) {
        if self.lossy {
            self.banner = Some("Read-only: file is not valid UTF-8, cannot save.".into());
            return;
        }
        let path = match self
            .path
            .clone()
            .or_else(|| rfd::FileDialog::new().save_file())
        {
            Some(p) => p,
            None => return,
        };
        match io::save(&path, &self.text, self.eol) {
            Ok(()) => {
                if self.path.is_none() {
                    self.fmt = format::detect(&path);
                    self.path = Some(path);
                }
                self.dirty = false;
                self.banner = None;
                self.tel.event("save", json!({"format": self.fmt.name()}));
            }
            Err(e) => {
                self.banner = Some(format!(
                    "Save failed: {e}. Your text is still in the editor."
                ));
                self.tel
                    .event("save_fail", json!({"err": e.kind().to_string()}));
            }
        }
    }

    fn toggle_mode(&mut self, via: &str) {
        self.flush_mode_time();
        self.view = !self.view;
        self.tel.event("toggle_mode", json!({"to": if self.view { "view" } else { "edit" }, "via": via, "format": self.fmt.name()}));
    }

    fn toggle_wrap(&mut self, via: &str) {
        self.settings.wrap = !self.settings.wrap;
        let _ = settings::save(&self.settings);
        self.tel
            .event("toggle_wrap", json!({"to": self.settings.wrap, "via": via}));
    }

    fn open_find(&mut self, replace: bool) {
        if self.view && matches!(self.fmt, Format::Json | Format::Markdown) {
            self.toggle_mode("find");
        }
        self.find.open = true;
        self.find.replace = replace;
        self.find.focus = true;
        self.tel.event("find_open", json!({"replace": replace}));
    }

    fn refresh_matches(&mut self) {
        if !self.find.open {
            return;
        }
        let key = hash_of(&(&self.text, &self.find.needle, self.find.case));
        if key == self.find.key {
            return;
        }
        self.find.key = key;
        self.find.matches = find::find_all(&self.text, &self.find.needle, self.find.case);
        self.find.cur = match self.find.cur {
            _ if self.find.matches.is_empty() => None,
            Some(c) => Some(c.min(self.find.matches.len() - 1)),
            None => None,
        };
    }

    fn step_match(&mut self, forward: bool) {
        self.refresh_matches();
        let n = self.find.matches.len();
        if n == 0 {
            return;
        }
        let next = match (self.find.cur, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(c), true) => (c + 1) % n,
            (Some(c), false) => (c + n - 1) % n,
        };
        self.find.cur = Some(next);
        self.select = Some(self.find.matches[next]);
    }

    fn replace_current(&mut self) {
        self.refresh_matches();
        let Some(c) = self.find.cur else {
            return self.step_match(true);
        };
        let range = self.find.matches[c];
        find::replace_one(&mut self.text, range, &self.find.with.clone());
        self.dirty = true;
        self.find.key = 0;
        self.refresh_matches();
        self.find.cur = if self.find.matches.is_empty() {
            None
        } else {
            Some(c.min(self.find.matches.len() - 1))
        };
        if let Some(c) = self.find.cur {
            self.select = Some(self.find.matches[c]);
        }
    }

    fn replace_everything(&mut self) {
        let (text, n) = find::replace_all(
            &self.text,
            &self.find.needle,
            &self.find.with,
            self.find.case,
        );
        if n > 0 {
            self.text = text;
            self.dirty = true;
            self.find.key = 0;
            self.find.cur = None;
        }
        self.banner = Some(format!("Replaced {n} occurrence(s)."));
        self.tel.event("replace_all", json!({"count": n}));
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        let (save, open, toggle, find, replace, wrap, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::E),
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::COMMAND, Key::H),
                i.consume_key(Modifiers::ALT, Key::Z),
                self.find.open && i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if save {
            self.save();
        }
        if open {
            self.open_dialog();
        }
        if toggle {
            self.toggle_mode("hotkey");
        }
        if find {
            self.open_find(false);
        }
        if replace {
            self.open_find(true);
        }
        if wrap {
            self.toggle_wrap("hotkey");
        }
        if esc {
            self.find.open = false;
        }
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
            && self.confirm_discard()
        {
            self.load(&p, "drop");
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        let title = format!(
            "{}{} - TurboText",
            if self.dirty { "* " } else { "" },
            name.as_deref().unwrap_or("Untitled")
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open…    Ctrl+O").clicked() {
                    ui.close();
                    self.open_dialog();
                }
                if ui.button("Save    Ctrl+S").clicked() {
                    ui.close();
                    self.save();
                }
                ui.separator();
                if ui.button("Open settings file").clicked() {
                    ui.close();
                    let _ = settings::save(&self.settings);
                    spawn_self(&settings::path());
                }
                if ui.button("File associations…").clicked() {
                    ui.close();
                    self.assoc_sel = ASSOC_EXTS
                        .iter()
                        .map(|e| (e.to_string(), assoc::is_associated(e)))
                        .collect();
                    self.assoc_open = true;
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("View", |ui| {
                let label = if self.view {
                    "Edit    Ctrl+E"
                } else {
                    "View    Ctrl+E"
                };
                if ui.button(label).clicked() {
                    ui.close();
                    self.toggle_mode("menu");
                }
                if ui
                    .checkbox(&mut self.settings.wrap.clone(), "Word wrap    Alt+Z")
                    .clicked()
                {
                    ui.close();
                    self.toggle_wrap("menu");
                }
                if ui
                    .checkbox(&mut self.settings.show_line_numbers.clone(), "Line numbers")
                    .clicked()
                {
                    ui.close();
                    self.settings.show_line_numbers = !self.settings.show_line_numbers;
                    let _ = settings::save(&self.settings);
                }
            });
            ui.menu_button("Search", |ui| {
                if ui.button("Find    Ctrl+F").clicked() {
                    ui.close();
                    self.open_find(false);
                }
                if ui.button("Replace    Ctrl+H").clicked() {
                    ui.close();
                    self.open_find(true);
                }
            });
            ui.menu_button("Usage log", |ui| {
                if ui.button("Open log").clicked() {
                    ui.close();
                    spawn_self(&settings::dir().join("usage.jsonl"));
                }
                if ui.button("Clear log").clicked() {
                    ui.close();
                    telemetry::clear(&settings::dir());
                    self.banner = Some("Usage log cleared.".into());
                }
            });
        });
    }

    fn find_bar(&mut self, ui: &mut egui::Ui) {
        let can_replace = !self.view && !self.lossy;
        ui.horizontal(|ui| {
            ui.label("Find");
            let r = ui.add(TextEdit::singleline(&mut self.find.needle).desired_width(220.0));
            if self.find.focus {
                r.request_focus();
                self.find.focus = false;
            }
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            ui.toggle_value(&mut self.find.case, "Aa")
                .on_hover_text("Match case");
            self.refresh_matches();
            if ui.button("▲").clicked() {
                self.step_match(false);
            }
            if ui.button("▼").clicked() || enter {
                self.step_match(true);
                r.request_focus();
            }
            let n = self.find.matches.len();
            let label = match self.find.cur {
                Some(c) => format!("{}/{}", c + 1, n),
                None if n > 0 => format!("{n} matches"),
                None if self.find.needle.is_empty() => String::new(),
                None => "no matches".into(),
            };
            ui.label(label);
            if self.find.replace {
                ui.separator();
                ui.label("Replace");
                ui.add(TextEdit::singleline(&mut self.find.with).desired_width(160.0));
                if ui
                    .add_enabled(can_replace, egui::Button::new("Replace"))
                    .clicked()
                {
                    self.replace_current();
                }
                if ui
                    .add_enabled(can_replace, egui::Button::new("All"))
                    .clicked()
                {
                    self.replace_everything();
                }
            }
            if ui.button("✕").clicked() {
                self.find.open = false;
            }
        });
    }

    fn assoc_window(&mut self, ctx: &egui::Context) {
        let mut open = self.assoc_open;
        let mut apply = false;
        egui::Window::new("File associations")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    "Register TurboText as an \"Open with\" handler (current user, no admin).",
                );
                for (ext, on) in self.assoc_sel.iter_mut() {
                    ui.checkbox(on, ext.as_str());
                }
                if ui.button("Apply").clicked() {
                    apply = true;
                }
            });
        if apply {
            let on: Vec<String> = self
                .assoc_sel
                .iter()
                .filter(|(_, c)| *c)
                .map(|(e, _)| e.clone())
                .collect();
            let off: Vec<String> = self
                .assoc_sel
                .iter()
                .filter(|(_, c)| !*c)
                .map(|(e, _)| e.clone())
                .collect();
            let result = assoc::associate(&on).and_then(|_| assoc::unassociate(&off));
            self.banner = Some(match result {
                Ok(()) => {
                    "Associations updated. Confirm the default app in Windows Settings if asked."
                        .into()
                }
                Err(e) => format!("Association failed: {e}"),
            });
            if !on.is_empty() {
                assoc::open_default_apps();
            }
            open = false;
        }
        self.assoc_open = open;
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        if self.view && self.fmt == Format::Markdown {
            views::markdown(ui, &mut self.md, &self.text);
            return;
        }
        if self.view && self.fmt == Format::Json {
            let key = hash_of(&self.text);
            if self.json.as_ref().map(|j| j.0) != Some(key) {
                let parsed = match views::json_error(&self.text) {
                    Some(e) => Err(e),
                    None => Ok(serde_json::from_str(&self.text).unwrap_or(Value::Null)),
                };
                if let Err((l, c, _)) = &parsed {
                    self.tel.event(
                        "problem",
                        json!({"kind": "json_parse_error", "line": l, "col": c}),
                    );
                }
                self.json = Some((key, parsed));
            }
            match &self.json {
                Some((_, Ok(v))) => {
                    views::json_tree(ui, v);
                    return;
                }
                Some((_, Err((l, c, m)))) => {
                    ui.label(
                        RichText::new(format!(
                            "Invalid JSON at line {l}, column {c}: {m}   (Ctrl+E to edit)"
                        ))
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                    );
                }
                None => {}
            }
        }
        let marks: &[(usize, usize)] = if self.find.open {
            &self.find.matches
        } else {
            &[]
        };
        let opts = editor::Opts {
            fmt: self.fmt,
            wrap: self.settings.wrap,
            line_numbers: self.settings.show_line_numbers,
            font_size: self.settings.font_size,
            read_only: self.lossy || self.view,
            highlight: (self.text.len() as u64) <= LARGE_FILE,
            marks,
            current: self.find.cur,
            select: self.select.take(),
        };
        if editor::show(ui, &mut self.text, &opts, &mut self.hl) {
            self.dirty = true;
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);
        self.update_title(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if self.confirm_discard() {
                self.allow_close = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if !self.started_logged {
            self.started_logged = true;
            self.tel.event(
                "session_start",
                json!({"startup_ms": self.tel.since_start_ms()}),
            );
        }
        let ctx = ui.ctx().clone();
        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        if let Some(b) = self.banner.clone() {
            egui::Panel::top("banner").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(b).color(egui::Color32::from_rgb(220, 160, 40)));
                    if ui.small_button("✕").clicked() {
                        self.banner = None;
                    }
                });
            });
        }
        if self.find.open {
            egui::Panel::bottom("find").show(ui, |ui| self.find_bar(ui));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let eol = if self.eol == Eol::Crlf { "CRLF" } else { "LF" };
                let mode = if self.lossy {
                    "read-only"
                } else if self.view {
                    "view"
                } else {
                    "edit"
                };
                ui.label(
                    RichText::new(format!(
                        "{}  |  {}  |  {}  |  {} chars",
                        self.fmt.name(),
                        eol,
                        mode,
                        self.text.chars().count()
                    ))
                    .small(),
                );
            });
        });
        egui::CentralPanel::default().show(ui, |ui| self.body(ui));
        self.assoc_window(&ctx);
        self.refresh_matches();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_mode_time();
        self.tel.shutdown();
    }
}
