# TurboText Rust Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (native, chosen) to implement this plan task-by-task. Steps use checkbox syntax.

**Goal:** Tiny, fast, single-file Windows editor/viewer for .env/config/JSON/Markdown, built on egui.

**Architecture:** Pure-logic modules (format, highlight tokenizers, io, find, settings, telemetry, json error pos) are unit-tested without UI. `app.rs` glues them into an eframe app. One binary also serves `--associate/--unassociate/--stats` subcommands.

**Tech Stack:** Rust stable, eframe/egui, egui_commonmark, serde_json, serde, toml, rfd, winreg, std `DefaultHasher` (salted) for path tokens.

**Spec:** `docs/superpowers/specs/2026-10-12-turbo-text-rust-design.md`

## Global Constraints

- Windows only; one file per process; no tabs, no explorer, no regex find.
- Release profile: `opt-level="z"`, `lto=true`, `strip=true`, `panic="abort"`, `codegen-units=1`.
- Large-file threshold 5 MB (highlighting off). Telemetry cap 2048 KB, age 90 days, line <= 300 bytes.
- Settings: `%APPDATA%\turbo-text\settings.toml`; telemetry: `%APPDATA%\turbo-text\usage.jsonl` (+ `usage.1.jsonl`).
- Telemetry never records contents, keystrokes, or full paths; I/O errors swallowed.
- Pin crate versions via `cargo add` at implementation time; adapt to current egui API.

## Review Focus

- Empty file / only newlines: opens, saves, line numbers show 1.
- CRLF file edited and saved stays CRLF; LF stays LF; mixed keeps majority.
- Invalid UTF-8 file: read-only banner, save blocked, no data loss.
- Invalid / empty / huge JSON: error with line:col, no panic, edit mode works.
- Corrupt settings.toml or unwritable %APPDATA%: defaults, no crash.
- Corrupt/torn usage.jsonl line: skipped by `--stats`.
- Save failure (read-only file): buffer kept, banner shown, temp file cleaned.
- Find with empty needle, needle at buffer end, overlapping matches: no infinite loop.

---

### Task 1: Archive Python, scaffold Rust crate

**Files:** Create `Cargo.toml`, `src/main.rs`; add `/target` to `.gitignore`; remove Python sources.

- [ ] Commit current Python state: `git add -A -- . ':!docs'` then commit "Snapshot Python prototype before archive".
- [ ] `git tag python-final`.
- [ ] `git rm -r main.py turbo_text TurboText.spec requirements.txt run.ps1`; delete `build/`, `dist/` dirs.
- [ ] `cargo init --name turbo-text`; `cargo add eframe egui_commonmark serde_json rfd toml winreg`; `cargo add serde --features derive`.
- [ ] Add release profile from Global Constraints. `cargo build` passes. Commit.

### Task 2: format.rs

**Produces:** `enum Format{Json,Markdown,Env,Config,Plain}`; `fn detect(path:&Path)->Format`; `Format::default_view(&self)->bool` (true for Json, Markdown).

- [ ] Tests: `.json`, `.md/.markdown`, `.env`, `.env.local`, `prod.env`, `.ini/.toml/.yaml/.yml/.conf/.cfg/.config/.properties`, `.txt`/none -> Plain; case-insensitive.
- [ ] Implement, pass, commit.

### Task 3: io.rs

**Produces:**
```rust
pub enum Eol{Lf,Crlf}
pub struct Loaded{pub text:String,pub eol:Eol,pub lossy:bool,pub size:u64}
pub fn load(path:&Path)->io::Result<Loaded>   // CRLF->LF in text; lossy if invalid UTF-8; BOM stripped
pub fn save(path:&Path,text:&str,eol:Eol)->io::Result<()> // temp file same dir + rename
```
- [ ] Tests: empty file; CRLF roundtrip; LF roundtrip; mixed -> majority; invalid UTF-8 sets lossy; no `.tmp` left on success; failed save keeps original and leaves no temp.
- [ ] Implement, pass, commit.

### Task 4: settings.rs

**Produces:** `Settings{wrap,theme,font_size,show_line_numbers,telemetry,telemetry_max_kb,telemetry_max_days}` (serde, `#[serde(default)]`); `dir()->PathBuf`; `load()->Settings`; `save(&Settings)->io::Result<()>`; `parse(&str)->Settings`.
Defaults: wrap false, theme "system", font_size 14.0, line numbers true, telemetry true, 2048, 90.
- [ ] Tests: empty -> defaults; partial keeps given; garbage -> defaults; roundtrip.
- [ ] Implement, commit.

### Task 5: highlight.rs

**Produces:** `Span{start,end,kind}` (byte offsets, char boundaries); `Kind{Key,String,Number,Keyword,Comment,Heading,Emph,Code,Link,Marker,Section}`; `tokenize(fmt,&str)->Vec<Span>` (sorted, non-overlapping); `layout_job(text,&[Span],FontId,dark)->LayoutJob`.
- [ ] Tests: JSON keys/strings/numbers/true/false/null, escaped quote; env `KEY=val # c`, `# c`, `export K="v"`; INI `[sec]`, `k = v`, `; c`; TOML `# c`; MD heading, bold, italic, code span, fenced block, link, list marker; unterminated string/fence no panic; multibyte offsets valid.
- [ ] Implement line-based tokenizers, pass, commit.

### Task 6: find.rs + json_error

**Produces:** `find_all(hay,needle,case)->Vec<(usize,usize)>`; `replace_one(&mut String,(usize,usize),&str)`; `replace_all(text,needle,with,case)->(String,usize)`; `json_error(text)->Option<(usize,usize,String)>` (line, col, msg; empty text is an error).
- [ ] Tests: case on/off; overlap `aaa`/`aa` = 1; unicode; empty needle -> none; needle at end; replacement containing needle terminates; json valid/invalid/multiline/empty.
- [ ] Implement, pass, commit.

### Task 7: telemetry.rs

**Produces:** `init(&Settings)->Telemetry` (background writer thread, no-op if disabled); `Telemetry::event(&self,&str,serde_json::Value)`; `add_mode_time(&self,fmt,view,secs)`; `flush_summary(&self)`; `path_token(&Path,salt)->String` ("ext:hash"); `rotate(dir,max_kb,max_days,now)`; `stats(dir)->String`; `clear(dir)`; `trunc_str(&str,max)->String`.
- [ ] Tests (unique temp dir): rotate over cap -> `usage.1.jsonl`, replaces older; under cap untouched; old events dropped; stats skips garbage lines; trunc_str boundary-safe; path_token has no filename and differs per salt; disabled writes nothing; write failure no panic.
- [ ] Implement, commit.

### Task 8: assoc.rs

**Produces:** `normalize_ext(&str)->String`; `associate(&[String])->Result<(),String>`; `unassociate`; `is_associated(&str)->bool`; `open_default_apps()`.
- [ ] Tests: normalize_ext cases; key path builders.
- [ ] Implement with winreg under `HKCU\Software\Classes` (ProgID `TurboText.File`, `shell\open\command` = `"<exe>" "%1"`, `OpenWithProgids` per ext); `open_default_apps` runs `cmd /c start ms-settings:defaultapps`. Commit.

### Task 9: editor + app skeleton

**Files:** `src/app.rs`, `src/editor.rs`, `src/main.rs`.
- [ ] `main.rs`: args (`--associate`, `--unassociate`, `--stats [--clear]`, or file path); eframe window 900x650.
- [ ] `App` state: path, text, eol, lossy, dirty, mode, fmt, banner, settings, telemetry, caches. Load via `io::load`, mode via `default_view`.
- [ ] Editor: line-number gutter + TextEdit with highlight layouter (cached by revision); no-wrap scroll; wrap uses galley row counts; highlighting off >5 MB; read-only when lossy.
- [ ] Manual verify, commit.

### Task 10: views + mode toggle

- [ ] JSON tree with CollapsingHeader, parse cached per revision, error banner `line:col msg` and fall back to editor.
- [ ] Markdown view via egui_commonmark with cache.
- [ ] Ctrl+E toggle. Manual verify with valid/invalid/empty json and md; commit.

### Task 11: save/open/dirty/drag-drop/close

- [ ] Ctrl+S (blocked when lossy; Err -> banner, buffer kept), Ctrl+O via rfd, Alt+Z wrap.
- [ ] Dirty title marker; confirm on close/replace.
- [ ] Drag-drop replaces file. Large-file and lossy banners. Manual verify, commit.

### Task 12: find/replace bar

- [ ] Ctrl+F / Ctrl+H bottom bar; Esc closes; case toggle; Next/Prev/Replace/Replace all; "n of m".
- [ ] Matches cached by needle/case/revision; highlighted; Next scrolls to match. Manual verify, commit.

### Task 13: settings, menu, telemetry wiring

- [ ] Apply settings; menus File/View/Telemetry; persist wrap and line-number toggles.
- [ ] Events: session_start, file_open, toggle_mode, toggle_wrap, find_open, replace_all, save, save_fail, problem, session_summary; panic hook.
- [ ] `--stats`, `--stats --clear`. Manual verify, commit.

### Task 14: association UI/CLI, release, docs

- [ ] `--associate/--unassociate` CLI and menu dialog with checkboxes.
- [ ] `cargo build --release`, record exe size; README; `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt`; commit.

## Self-Review

Spec coverage: modes/hotkeys (9-11), highlight (5), line numbers+wrap (9), find (6,12), io+errors (3,11), settings (4,13), association (8,14), telemetry (7,13), archive (1), tests (2-8). Types consistent across tasks.
