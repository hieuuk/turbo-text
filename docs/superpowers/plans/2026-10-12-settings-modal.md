# Settings Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (inline) to implement this plan task-by-task. Steps use checkbox syntax.

**Goal:** In-app settings dialog (editor, appearance, behavior, files/privacy) plus a larger default extension list for file association.

**Architecture:** Extend `Settings` with per-field validated parsing; new `settings_ui.rs` egui window; `app.rs` applies changes live and debounces saves; pure helpers (`start_in_view`, `normalize_ext_list`, `indent_insert`) are unit-tested.

**Tech Stack:** existing (eframe/egui 0.36, serde, toml). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-12-settings-modal-design.md`

## Global Constraints

- No new crates; binary stays about 6 MB; window-up time stays about 30-80 ms (measure before/after).
- Every new setting parsed per field: invalid type/range falls back to that field's default (clamped where a range exists); never resets neighbours, never panics.
- Position of window is NOT restored (size only). Settings file writes debounced about 400 ms and flushed on exit.
- Windows only; Stage 1 deploy only (`scripts\deploy-local.ps1`).

## Review Focus

- `settings.toml` with wrong types / out-of-range / unknown values for each new key: app starts, other keys intact.
- `tab_spaces` Tab insertion mid-line, at line start, with a multibyte char before the cursor, with a selection.
- `ui_scale` / `font_size` extremes cannot produce an unusable or crashing UI.
- Association list: duplicates, invalid entries, empty list, extension already registered by another program.
- `confirm_close = false` with unsaved edits: closing must still not corrupt the file on disk (just discards the buffer).
- Closing the settings dialog with Esc must not also close the find bar or the app.
- Saving settings when the directory is unwritable: session still works, banner shown.

---

### Task 1: Settings model (validated fields, helpers)

**Files:** Modify `src/settings.rs`.

**Produces:**
```rust
pub const DEFAULT_ASSOC_EXTS: [&str; 17] = [".env",".json",".md",".toml",".ini",".yaml",".yml",".txt",".log",".conf",".cfg",".config",".properties",".gitignore",".editorconfig",".csv",".xml"];
pub struct Settings { /* existing + */
  pub tab_spaces: u8, pub highlight: bool, pub large_file_mb: u64, pub editor_font: String,
  pub ui_scale: f32, pub open_json: String, pub open_markdown: String, pub open_other: String,
  pub confirm_close: bool, pub remember_window: bool, pub window_w: f32, pub window_h: f32,
  pub assoc_exts: Vec<String> }
impl Settings { pub fn start_in_view(&self, fmt: Format) -> bool; pub fn window_size(&self) -> Option<(f32,f32)>; }
pub fn normalize_ext_list(input: &[String]) -> Vec<String>
```
Group reset helpers: `Settings::reset_editor(&mut self)`, `reset_appearance`, `reset_behavior`, `reset_files_privacy` (each copies that group's defaults).

- [ ] Write failing tests: defaults for every new key; each key's wrong-type and out-of-range fallback (`tab_spaces = 3` -> 0, `ui_scale = 9.0` -> 2.0, `ui_scale = 0.1` -> 0.75, `large_file_mb = 0` -> 1, `editor_font = "comic"` -> "monospace", `theme = "blue"` -> "system", `open_json = "x"` -> "view", `window_w = 5` -> default 900, nan/inf for floats); one bad value keeps neighbours; `assoc_exts` non-array -> default list, entries normalized/deduped/invalid dropped; `start_in_view` (json view, md view, env edit, plain edit, overridable); `normalize_ext_list` (`["JSON","json",".Md","a b","",".env"]` -> `[".json",".md",".env"]`); roundtrip through `toml::to_string` + `parse`.
- [ ] Run (FAIL), implement, run (PASS), `cargo test`, commit `feat: validated settings model`.

### Task 2: Editor honors settings; tab-stop indent

**Files:** Modify `src/editor.rs`, `src/app.rs` (Opts construction only).

**Produces:** `pub fn indent_insert(text: &mut String, sel: (usize, usize), spaces: usize) -> usize` (char indices; replaces the selection with spaces up to the next tab stop based on the column of `sel.0`; returns new cursor char index); `Opts` gains `font_proportional: bool`, `tab_spaces: usize`.

- [ ] Failing tests: col 0 with 4 -> 4 spaces; col 3 with 4 -> 1 space; col 4 -> 4; after multibyte `"é"` col counts chars; selection replaced; `spaces = 0` -> inserts `\t`; after a newline column resets.
- [ ] Implement; in `editor::show`, when `tab_spaces > 0` and the editor has focus, consume `Key::Tab` before the TextEdit, read `TextEditState` cursor range, call `indent_insert`, write text + cursor back, mark changed. Use `FontId::proportional` when `font_proportional`.
- [ ] `cargo test`, commit `feat: tab-stop spaces and editor font option`.

### Task 3: App applies settings live

**Files:** Modify `src/app.rs`, `src/main.rs`.

- [ ] Track `applied: Settings` snapshot; each frame if `self.settings != self.applied`: set theme (`set_theme`), `ctx.set_zoom_factor(ui_scale)`, start debounce timer; save via `settings::save` when 400 ms have passed since last change and in `on_exit`; failure -> banner "Could not save settings".
- [ ] Use `highlight` and `large_file_mb` instead of the `LARGE_FILE` const; `start_in_view` in `load`; `confirm_close` in `logic`; remember window size: capture `viewport().inner_rect` each frame, store `window_w/h` on exit when `remember_window`.
- [ ] `main.rs`: initial size from `settings.window_size()` when `remember_window` else 900x650.
- [ ] `cargo test`, clippy, commit `feat: apply settings live, window size, open modes`.

### Task 4: Settings dialog

**Files:** Create `src/settings_ui.rs`; modify `src/app.rs`, `src/main.rs` (`mod settings_ui;`).

**Produces:** `pub struct SettingsUi { pub open: bool, assoc: Vec<(String,bool)>, custom: String }`; `SettingsUi::show(&mut self, ctx, &mut Settings) -> Option<AssocRequest>` where `AssocRequest { on: Vec<String>, off: Vec<String> }`; `SettingsUi::refresh_assoc(&mut self, exts: &[String])` (reads `assoc::is_associated`).

- [ ] Window with four groups (Editor, Appearance, Behavior, Files and privacy), each with controls from the spec table, a "Reset to defaults" button per group, "applies next launch" labels, the confirm-close warning, association checkboxes + Add/Remove + Apply.
- [ ] `Ctrl+,` and File > Settings… open it; Esc closes it only when it is open and focused (check before the find-bar Esc handling); remove the old `assoc_window` and menu item.
- [ ] Debug-only (`cfg(debug_assertions)`) hook `TT_DEBUG_SETTINGS=1` opens the dialog at startup.
- [ ] Launch, screenshot only the app window (never full screen), check each group renders and a toggle (wrap/theme) applies live. Commit `feat: settings dialog`.

### Task 5: Docs, measurements, deploy

- [ ] README settings table + keys; `docs/DEPLOY.md` mention; `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
- [ ] Measure window-up time (3 runs) and exe size vs. the previous 6.28 MB / 30-80 ms; record in the commit message.
- [ ] Final whole-branch review by a fresh reviewer; fix Critical/Important with tests first.
- [ ] `scripts\deploy-local.ps1 -Associate` once to register the larger extension set (reopens Default apps page); verify an installed-exe launch.
- [ ] Commit `docs: settings modal`.

## Self-Review

Spec coverage: settings table (T1, T3), tab spaces/editor font/highlight (T2, T3), dialog + reset + associations (T4), window size (T3), errors/banner (T3), tests (T1, T2), measurements and deploy (T5). Types consistent: `Settings` fields, `Opts`, `AssocRequest`.
