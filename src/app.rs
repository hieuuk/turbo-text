use crate::editor::{self, HlCache};
use crate::find;
use crate::format::{self, Format};
use crate::io::{self, Eol};
use crate::settings::{self, Settings};
use crate::settings_ui::{AssocRequest, SettingsUi};
use crate::telemetry::{self, Telemetry};
use crate::{assoc, fonts, theme, views};
use eframe::egui::{self, FontFamily, Key, Modifiers, RichText, TextEdit};
use egui_commonmark::CommonMarkCache;
use serde_json::{Value, json};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// How often the open file's modification time is checked for outside changes.
const DISK_POLL: Duration = Duration::from_secs(1);

/// Settings are written this long after the last change (and on exit), not on every frame.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(400);

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

#[derive(Default)]
struct Goto {
    open: bool,
    focus: bool,
    input: String,
}

pub struct App {
    /// Resolved (editor, Markdown view) font families, and whether system fonts are loaded.
    families: (FontFamily, FontFamily),
    fonts_custom: bool,
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
    goto: Goto,
    about_open: bool,
    /// Modification time of `path` when it was last loaded or saved by this app.
    disk_mtime: Option<SystemTime>,
    last_poll: Instant,
    /// The file changed on disk while there are unsaved edits; the banner offers Reload / Keep mine.
    conflict: bool,
    mode_since: Instant,
    started_logged: bool,
    /// ms from `main` until the window and GL context exist (`App::new` runs).
    init_ms: u64,
    last_title: String,
    allow_close: bool,
    select: Option<(usize, usize)>,
    settings_ui: SettingsUi,
    /// Last settings written to disk / last settings applied to egui.
    saved: Settings,
    applied: Settings,
    settings_changed_at: Option<Instant>,
    /// Last known logical window size, stored on exit when `remember_window` is on.
    win_size: Option<(f32, f32)>,
}

fn hash_of<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// True once the settings have been unchanged for the debounce period.
fn save_due(since_last_change: Option<Duration>) -> bool {
    since_last_change.is_some_and(|d| d >= SAVE_DEBOUNCE)
}

/// Loads the chosen system fonts (if any) into egui and returns the (editor, view) families.
/// Reads font files only when one is selected, or once more to drop them again.
fn apply_fonts(
    ctx: &egui::Context,
    s: &Settings,
    was_custom: bool,
) -> ((FontFamily, FontFamily), bool) {
    let wanted = [s.editor_font.as_str(), s.view_font.as_str()];
    let custom = wanted
        .iter()
        .any(|w| !matches!(*w, "monospace" | "proportional"));
    let mut loaded = Vec::new();
    if custom || was_custom {
        let (defs, ids) = fonts::definitions(&fonts::system_dir(), &wanted);
        ctx.set_fonts(defs);
        loaded = ids;
    }
    let fam = (
        fonts::family(wanted[0], &loaded),
        fonts::family(wanted[1], &loaded),
    );
    (fam, custom)
}

fn apply_visuals(ctx: &egui::Context, s: &Settings) {
    // reset both slots so switching away from a named theme restores egui's own look
    ctx.set_visuals_of(egui::Theme::Dark, egui::Visuals::dark());
    ctx.set_visuals_of(egui::Theme::Light, egui::Visuals::light());
    if s.theme == "system" {
        ctx.set_theme(egui::ThemePreference::System);
    } else {
        let pal = theme::resolve(&s.theme, true);
        let slot = if pal.dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_visuals_of(slot, pal.visuals());
        ctx.set_theme(slot);
    }
    ctx.set_zoom_factor(s.ui_scale);
}

/// Status-bar text for the save state; `None` for a blank buffer that was never saved.
fn save_label(dirty: bool, has_path: bool) -> Option<&'static str> {
    match (dirty, has_path) {
        (true, _) => Some("Unsaved changes"),
        (false, true) => Some("Saved"),
        (false, false) => None,
    }
}

/// Starts another TurboText window, on `path` or blank.
pub fn spawn_self(path: Option<&Path>) {
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        if let Some(p) = path {
            cmd.arg(p);
        }
        let _ = cmd.spawn();
    }
}

/// 1-based line and column (in characters) of the character index `at`.
fn line_col(text: &str, at: usize) -> (usize, usize) {
    text.chars().take(at).fold((1, 1), |(line, col), c| {
        if c == '\n' {
            (line + 1, 1)
        } else {
            (line, col + 1)
        }
    })
}

/// Byte offset of the start of 1-based `line`, clamped to the last line.
fn line_start(text: &str, line: usize) -> usize {
    let mut start = 0;
    for _ in 1..line.max(1) {
        match text[start..].find('\n') {
            Some(i) => start += i + 1,
            None => break,
        }
    }
    start
}

/// True when the file now has a different modification time than we last saw. A file that can no
/// longer be read (`now` is `None`) is not reported, so a vanished file never nags.
fn changed_on_disk(seen: Option<SystemTime>, now: Option<SystemTime>) -> bool {
    now.is_some() && now != seen
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let settings = settings::load();
        // Ctrl +/-/0 change the font size (saved), not egui's unsaved UI zoom
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
        apply_visuals(&cc.egui_ctx, &settings);
        let (families, fonts_custom) = apply_fonts(&cc.egui_ctx, &settings, false);
        let tel = telemetry::init(&settings);
        let init_ms = tel.since_start_ms();
        let mut app = App {
            families,
            fonts_custom,
            path: None,
            text: String::new(),
            eol: Eol::Crlf,
            lossy: false,
            dirty: false,
            fmt: Format::Plain,
            view: false,
            banner: None,
            saved: settings.clone(),
            applied: settings.clone(),
            settings_changed_at: None,
            win_size: None,
            settings,
            tel,
            hl: HlCache::default(),
            json: None,
            md: CommonMarkCache::default(),
            find: Find::default(),
            goto: Goto::default(),
            about_open: false,
            disk_mtime: None,
            last_poll: Instant::now(),
            conflict: false,
            mode_since: Instant::now(),
            started_logged: false,
            init_ms,
            last_title: String::new(),
            allow_close: false,
            select: None,
            settings_ui: SettingsUi::default(),
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
        self.disk_mtime = mtime(path);
        self.conflict = false;
        self.fmt = format::detect(path);
        self.view = self.settings.start_in_view(self.fmt) && !self.text.trim().is_empty();
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
        } else if size > self.settings.large_file_mb * 1024 * 1024 {
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
        let path = self
            .path
            .clone()
            .or_else(|| rfd::FileDialog::new().save_file());
        if let Some(p) = path {
            self.save_to(p);
        }
    }

    fn save_as(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        if let Some(name) = self.path.as_ref().and_then(|p| p.file_name()) {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        if let Some(p) = dialog.save_file() {
            self.save_to(p);
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        if self.lossy {
            self.banner = Some("Read-only: file is not valid UTF-8, cannot save.".into());
            return;
        }
        match io::save(&path, &self.text, self.eol) {
            Ok(()) => {
                self.fmt = format::detect(&path);
                self.disk_mtime = mtime(&path);
                self.path = Some(path);
                self.dirty = false;
                self.conflict = false;
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

    /// Checks once a second whether the open file changed on disk. A clean buffer reloads
    /// silently; a buffer with edits is never overwritten, the banner asks instead.
    fn poll_disk(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else {
            return;
        };
        ctx.request_repaint_after(DISK_POLL);
        if self.conflict || self.last_poll.elapsed() < DISK_POLL {
            return;
        }
        self.last_poll = Instant::now();
        if !changed_on_disk(self.disk_mtime, mtime(&path)) {
            return;
        }
        if self.dirty {
            self.conflict = true;
        } else {
            self.reload(&path);
        }
    }

    fn reload(&mut self, path: &Path) {
        let view = self.view;
        self.load(path, "reload");
        self.view = view;
        self.conflict = false;
        self.banner = Some("File changed on disk: reloaded.".into());
    }

    fn zoom(&mut self, step: f32) {
        self.settings.font_size = if step == 0.0 {
            Settings::default().font_size
        } else {
            (self.settings.font_size + step).clamp(6.0, 72.0)
        };
    }

    fn open_goto(&mut self) {
        if self.view && matches!(self.fmt, Format::Json | Format::Markdown) {
            self.toggle_mode("goto");
        }
        self.goto.open = true;
        self.goto.focus = true;
    }

    fn jump_to_line(&mut self) {
        if let Ok(n) = self.goto.input.trim().parse::<usize>() {
            let at = line_start(&self.text, n);
            self.select = Some((at, at));
        }
    }

    fn toggle_mode(&mut self, via: &str) {
        self.flush_mode_time();
        self.view = !self.view;
        self.tel.event("toggle_mode", json!({"to": if self.view { "view" } else { "edit" }, "via": via, "format": self.fmt.name()}));
    }

    fn toggle_wrap(&mut self, via: &str) {
        self.settings.wrap = !self.settings.wrap;
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
        let (save_as, new_win, goto, zoom) = ctx.input_mut(|i| {
            let wheel = i.zoom_delta();
            let step = if i.consume_key(Modifiers::COMMAND, Key::Num0) {
                Some(0.0)
            } else if i.consume_key(Modifiers::COMMAND, Key::Minus) {
                Some(-1.0)
            } else if i.consume_key(Modifiers::COMMAND, Key::Plus)
                || i.consume_key(Modifiers::COMMAND, Key::Equals)
            {
                Some(1.0)
            } else if wheel != 1.0 {
                Some(if wheel > 1.0 { 1.0 } else { -1.0 })
            } else {
                None
            };
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::N),
                i.consume_key(Modifiers::COMMAND, Key::G),
                step,
            )
        });
        if save_as {
            self.save_as();
        }
        if new_win {
            spawn_self(None);
        }
        if goto {
            self.open_goto();
        }
        if let Some(step) = zoom {
            self.zoom(step);
        }
        let (save, open, toggle, find, replace, wrap, settings_key, esc) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::S),
                i.consume_key(Modifiers::COMMAND, Key::O),
                i.consume_key(Modifiers::COMMAND, Key::E),
                i.consume_key(Modifiers::COMMAND, Key::F),
                i.consume_key(Modifiers::COMMAND, Key::H),
                i.consume_key(Modifiers::ALT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Comma),
                (self.find.open || self.goto.open || self.settings_ui.open || self.about_open)
                    && i.consume_key(Modifiers::NONE, Key::Escape),
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
        if settings_key {
            self.settings_ui.open();
        }
        if esc {
            // Esc closes dialogs first (About, Settings), then the go-to bar, then the find bar
            if self.about_open {
                self.about_open = false;
            } else if self.settings_ui.open {
                self.settings_ui.open = false;
            } else if self.goto.open {
                self.goto.open = false;
            } else {
                self.find.open = false;
            }
        }
        if let Some(p) = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
            && self.confirm_discard()
        {
            self.load(&p, "drop");
        }
    }

    /// Writes settings without clobbering hand-edits: overlays only this session's changes on the
    /// file as it is now (see `settings::merge_for_save`).
    fn save_settings(&mut self) -> std::io::Result<()> {
        let merged = settings::merge_for_save(&settings::load(), &self.saved, &self.settings);
        settings::save(&merged)?;
        self.saved = self.settings.clone();
        Ok(())
    }

    /// Applies settings changes the frame they happen, debounces the file write, tracks window size.
    fn track_settings(&mut self, ctx: &egui::Context) {
        if let Some(r) = ctx.input(|i| i.viewport().inner_rect) {
            // inner_rect is in zoomed points; store the logical size the OS window actually has
            let z = ctx.zoom_factor();
            let (w, h) = ((r.width() * z).round(), (r.height() * z).round());
            if (300.0..=10000.0).contains(&w) && (300.0..=10000.0).contains(&h) {
                self.win_size = Some((w, h));
            }
        }
        if self.settings != self.applied {
            apply_visuals(ctx, &self.settings);
            if self.settings.editor_font != self.applied.editor_font
                || self.settings.view_font != self.applied.view_font
            {
                (self.families, self.fonts_custom) =
                    apply_fonts(ctx, &self.settings, self.fonts_custom);
            }
            self.applied = self.settings.clone();
            self.settings_changed_at = Some(Instant::now());
        }
        if self.settings_changed_at.is_some() {
            if save_due(self.settings_changed_at.map(|t| t.elapsed())) {
                if let Err(e) = self.save_settings() {
                    self.banner = Some(format!("Could not save settings: {e}"));
                }
                self.settings_changed_at = None;
            } else {
                ctx.request_repaint_after(SAVE_DEBOUNCE);
            }
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
                if ui.button("Save as…    Ctrl+Shift+S").clicked() {
                    ui.close();
                    self.save_as();
                }
                if ui.button("New window    Ctrl+N").clicked() {
                    ui.close();
                    spawn_self(None);
                }
                ui.separator();
                if ui.button("Open settings file").clicked() {
                    ui.close();
                    let _ = self.save_settings();
                    spawn_self(Some(&settings::path()));
                }
                if ui.button("Settings…    Ctrl+,").clicked() {
                    ui.close();
                    self.settings_ui.open();
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
                if ui.button("Go to line…    Ctrl+G").clicked() {
                    ui.close();
                    self.open_goto();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About and shortcuts").clicked() {
                    ui.close();
                    self.about_open = true;
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
            if ui.button("Prev").clicked() {
                self.step_match(false);
            }
            if ui.button("Next").clicked() || enter {
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
            if ui.button("Close").clicked() {
                self.find.open = false;
            }
        });
    }

    fn goto_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Go to line");
            let r = ui.add(TextEdit::singleline(&mut self.goto.input).desired_width(80.0));
            if self.goto.focus {
                r.request_focus();
                self.goto.focus = false;
            }
            let lines = self.text.bytes().filter(|&b| b == b'\n').count() + 1;
            ui.label(RichText::new(format!("of {lines}")).weak());
            if (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                || ui.button("Go").clicked()
            {
                self.jump_to_line();
                r.request_focus();
            }
            if ui.button("Close").clicked() {
                self.goto.open = false;
            }
        });
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.about_open;
        egui::Window::new("About TurboText")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(format!("TurboText {}", env!("CARGO_PKG_VERSION")));
                ui.label("Tiny, fast text editor and viewer for Windows.");
                ui.label("Free software, GPL-3.0-only. No network access.");
                ui.hyperlink_to(
                    "github.com/hieuuk/turbo-text",
                    "https://github.com/hieuuk/turbo-text",
                );
                ui.separator();
                egui::Grid::new("shortcuts").num_columns(2).show(ui, |ui| {
                    for (key, what) in [
                        ("Ctrl+O / Ctrl+S / Ctrl+Shift+S", "Open / Save / Save as"),
                        ("Ctrl+N", "New window"),
                        ("Ctrl+E", "Toggle view / edit"),
                        ("Ctrl+F / Ctrl+H", "Find / Replace"),
                        ("Ctrl+G", "Go to line"),
                        (
                            "Ctrl + / Ctrl - / Ctrl 0",
                            "Zoom in / out / reset (also Ctrl+wheel)",
                        ),
                        ("Alt+Z", "Word wrap"),
                        ("Ctrl+,", "Settings"),
                    ] {
                        ui.label(RichText::new(key).monospace());
                        ui.label(what);
                        ui.end_row();
                    }
                });
            });
        self.about_open = open;
    }

    fn apply_assoc(&mut self, req: AssocRequest) {
        let result = assoc::associate(&req.on).and_then(|_| assoc::unassociate(&req.off));
        self.banner = Some(match result {
            Ok(()) => {
                "Associations updated. Confirm the default app in Windows Settings if asked.".into()
            }
            Err(e) => format!("Association failed: {e}"),
        });
        if !req.on.is_empty() {
            assoc::open_default_apps();
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let pal = theme::resolve(&self.settings.theme, ui.visuals().dark_mode);
        if self.view && self.fmt == Format::Markdown {
            views::markdown(
                ui,
                &mut self.md,
                &self.text,
                &self.families.1,
                self.settings.font_size,
                self.settings.line_height,
            );
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
                    views::json_tree(ui, v, pal);
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
            highlight: self.settings.highlight
                && (self.text.len() as u64) <= self.settings.large_file_mb * 1024 * 1024,
            family: self.families.0.clone(),
            tab_spaces: self.settings.tab_spaces as usize,
            pal,
            line_height: self.settings.line_height,
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
        self.poll_disk(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if !self.settings.confirm_close || self.confirm_discard() {
                self.allow_close = true;
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.track_settings(ui.ctx());
        if !self.started_logged {
            self.started_logged = true;
            self.tel.event(
                "session_start",
                json!({"startup_ms": self.tel.since_start_ms(), "init_ms": self.init_ms}),
            );
            // Debug builds only: lets screenshots of find/scroll/settings run without synthetic keystrokes.
            #[cfg(debug_assertions)]
            if std::env::var("TT_DEBUG_ABOUT").is_ok() {
                self.about_open = true;
            }
            #[cfg(debug_assertions)]
            if let Ok(n) = std::env::var("TT_DEBUG_GOTO") {
                self.open_goto();
                self.goto.input = n;
                self.jump_to_line();
            }
            #[cfg(debug_assertions)]
            if std::env::var("TT_DEBUG_SETTINGS").is_ok() {
                self.settings_ui.open();
            }
            #[cfg(debug_assertions)]
            if let Ok(needle) = std::env::var("TT_DEBUG_FIND") {
                self.find.open = true;
                self.find.needle = needle;
                self.step_match(true);
                if let Ok(with) = std::env::var("TT_DEBUG_REPLACE") {
                    self.find.with = with;
                    self.replace_everything();
                }
            }
        }
        let ctx = ui.ctx().clone();
        egui::Panel::top("menu").show(ui, |ui| self.menu(ui));
        if self.conflict {
            egui::Panel::top("conflict").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("This file changed on disk and you have unsaved edits.")
                            .color(egui::Color32::from_rgb(220, 160, 40)),
                    );
                    if ui.button("Reload (lose my edits)").clicked()
                        && let Some(p) = self.path.clone()
                    {
                        self.reload(&p);
                    }
                    if ui.button("Keep mine").clicked() {
                        self.disk_mtime = self.path.as_deref().and_then(mtime);
                        self.conflict = false;
                    }
                });
            });
        }
        if let Some(b) = self.banner.clone() {
            egui::Panel::top("banner").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(b).color(egui::Color32::from_rgb(220, 160, 40)));
                    if ui.small_button("x").clicked() {
                        self.banner = None;
                    }
                });
            });
        }
        if self.goto.open {
            egui::Panel::bottom("goto").show(ui, |ui| self.goto_bar(ui));
        }
        if self.find.open {
            egui::Panel::bottom("find").show(ui, |ui| self.find_bar(ui));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(label) = save_label(self.dirty, self.path.is_some()) {
                    let text = RichText::new(label).small();
                    ui.label(if self.dirty {
                        text.strong().color(egui::Color32::from_rgb(230, 160, 40))
                    } else {
                        text.weak()
                    });
                    ui.separator();
                }
                let eol = if self.eol == Eol::Crlf { "CRLF" } else { "LF" };
                let mode = if self.lossy {
                    "read-only"
                } else if self.view {
                    "view"
                } else {
                    "edit"
                };
                let pos = (!self.view)
                    .then(|| editor::cursor_char(&ctx).unwrap_or(0))
                    .map(|at| line_col(&self.text, at))
                    .map_or(String::new(), |(l, c)| format!("Ln {l}, Col {c}  |  "));
                ui.label(
                    RichText::new(format!(
                        "{pos}{}  |  {}  |  {}  |  {} chars",
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
        self.about_window(&ctx);
        if let Some(req) = self.settings_ui.show(&ctx, &mut self.settings) {
            self.apply_assoc(req);
        }
        self.refresh_matches();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if self.settings.remember_window
            && let Some((w, h)) = self.win_size
        {
            self.settings.window_w = w;
            self.settings.window_h = h;
        }
        if self.settings != self.saved {
            let _ = self.save_settings();
        }
        self.flush_mode_time();
        self.tel.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::{SAVE_DEBOUNCE, changed_on_disk, line_col, line_start, save_due, save_label};
    use std::time::{Duration, SystemTime};

    #[test]
    fn line_col_counts_from_one_in_characters() {
        assert_eq!(line_col("", 0), (1, 1));
        assert_eq!(line_col("ab\ncd", 4), (2, 2));
        assert_eq!(line_col("ab\n", 3), (2, 1));
        assert_eq!(line_col("é\nüx", 3), (2, 2));
        assert_eq!(line_col("ab", 99), (1, 3));
    }

    #[test]
    fn line_start_clamps_to_the_text() {
        let t = "a\nbb\nccc";
        assert_eq!(line_start(t, 1), 0);
        assert_eq!(line_start(t, 2), 2);
        assert_eq!(line_start(t, 3), 5);
        assert_eq!(line_start(t, 99), 5);
        assert_eq!(line_start(t, 0), 0);
    }

    #[test]
    fn disk_change_needs_a_readable_newer_stamp() {
        let t0 = SystemTime::UNIX_EPOCH;
        let t1 = t0 + Duration::from_secs(5);
        assert!(!changed_on_disk(Some(t0), Some(t0)));
        assert!(changed_on_disk(Some(t0), Some(t1)));
        assert!(changed_on_disk(None, Some(t1)));
        assert!(!changed_on_disk(Some(t0), None));
    }

    #[test]
    fn settings_save_waits_for_quiet_period() {
        assert!(!save_due(None));
        assert!(!save_due(Some(Duration::from_millis(0))));
        assert!(!save_due(Some(SAVE_DEBOUNCE - Duration::from_millis(1))));
        assert!(save_due(Some(SAVE_DEBOUNCE)));
        assert!(save_due(Some(Duration::from_secs(60))));
    }

    #[test]
    fn unsaved_changes_are_called_out() {
        assert_eq!(save_label(true, true), Some("Unsaved changes"));
        assert_eq!(save_label(true, false), Some("Unsaved changes"));
    }

    #[test]
    fn clean_file_says_saved_and_blank_buffer_says_nothing() {
        assert_eq!(save_label(false, true), Some("Saved"));
        assert_eq!(save_label(false, false), None);
    }
}
