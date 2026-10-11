# TurboText (Rust rewrite) — Design

Date: 2026-10-12

## Intent

Tiny, instant-launch, single-file editor/viewer for quick edits: `.env`, config files, JSON viewing, formatted Markdown view/edit. Not a VS Code replacement. Replaces the archived Python/Tk prototype.

**Out of scope:** tabs, file explorer, regex find, themes beyond egui light/dark, single-instance IPC, settings UI, remote telemetry, installer.

**Platform:** Windows only. One file per window/process.

## Stack

`eframe`/`egui`, `egui_commonmark` (MD render), `serde_json` (JSON tree + telemetry), `rfd` (dialogs), `toml` + `serde` (settings), `winreg` (file association).
Release profile: `opt-level="z"`, `lto=true`, `strip=true`, `panic="abort"`, `codegen-units=1`.

## Repo / archive

1. Commit the current uncommitted Python changes.
2. Tag `python-final`, then remove the Python sources (`main.py`, `turbo_text/`, `TurboText.spec`, `requirements.txt`, `run.ps1`, `build/`, `dist/`) from the tree. History keeps them.
3. Rust project at repo root: `Cargo.toml`, `src/`. Keep `LICENSE`, icons.

## Modules

| File | Responsibility |
|---|---|
| `main.rs` | CLI parsing (`<file>`, `--associate`, `--unassociate`, `--stats`, `--stats --clear`), start eframe |
| `app.rs` | State (path, buffer, dirty, mode, banners), top-level UI, hotkeys, menu |
| `format.rs` | Detect `Json`, `Markdown`, `Env`, `Config` (ini/toml/yaml/conf), `Plain` from extension |
| `highlight.rs` | One tokenizer per format → egui `LayoutJob`; cached by buffer revision |
| `editor.rs` | Line-number gutter + `TextEdit` in one `ScrollArea`; wrap on/off |
| `views.rs` | JSON collapsible tree; Markdown rendered view |
| `find.rs` | Bottom find/replace bar, match list, highlight spans |
| `io.rs` | Read/write, UTF-8 check, CRLF/LF preservation, atomic save |
| `settings.rs` | Load/save `settings.toml` |
| `assoc.rs` | HKCU file-association registration |
| `telemetry.rs` | Local usage log, background writer, rotation, `--stats` |

## Behavior

**Modes.** One buffer (`String`). `.md`/`.json` open in view mode; everything else in edit mode. `Ctrl+E` toggles view/edit for all formats. JSON view = collapsible tree, parse cached per buffer revision; invalid JSON shows error with line:col, edit mode still works. MD view = `egui_commonmark`.

**Hotkeys.** `Ctrl+S` save, `Ctrl+O` open, `Ctrl+E` toggle view/edit, `Ctrl+F` find, `Ctrl+H` replace, `Alt+Z` wrap toggle.

**Line numbers.** Gutter drawn beside `TextEdit` in the same scroll area. In wrap mode, numbers align by counting visual rows per logical line from the text galley.

**Highlighting.** Hand-written tokenizers via `TextEdit::layouter`: JSON (keys, strings, numbers, bool/null), MD (headings, emphasis, code spans/fences, links, list markers), env/INI/TOML (sections, keys, values, comments). Disabled for files >5 MB.

**Find/replace.** Plain text, case-sensitive toggle, next/prev, replace, replace all, match count, matches highlighted. No regex.

**Open paths.** CLI arg, "Open with", drag-drop onto window (replaces current file; confirm if dirty), `Ctrl+O` dialog. No arg → empty untitled buffer.

**Dirty/close.** Dirty marker in title; confirm on close/replace.

## Errors

- Non-UTF-8: open read-only with lossy decode + banner; save blocked.
- Read/write failure: inline banner with OS error; buffer never discarded.
- Save: write temp file beside target, then rename (atomic).
- File >5 MB: banner, highlighting off, still editable.

## Settings

`%APPDATA%\turbo-text\settings.toml`. Missing/corrupt → defaults, never crash. Keys: `wrap`, `theme` (system|light|dark), `font_size`, `show_line_numbers`, `telemetry` (default true), `telemetry_max_kb` (default 2048), `telemetry_max_days` (default 90). No settings UI; menu item opens the file in the app.

## File association

Same exe. `turbo-text --associate .env .json .md .toml` / `--unassociate ...`; also menu "File associations…" with checkbox per extension. Writes under `HKCU\Software\Classes` (no admin). Windows protects per-extension default-app choice, so after registering, open the Windows "Default apps" page for extensions needing user confirmation.

## Telemetry (local only)

- File `%APPDATA%\turbo-text\usage.jsonl`, one JSON event per line, never leaves the machine, no network code.
- Events: session (startup→first frame, open→ready per file, duration, open source CLI/drag/dialog); files (format, extension, size bucket, initial mode); features (view/edit and wrap toggles, find/replace opens, replace-all, saves, save failures, hotkey vs menu); time split (seconds in view vs edit per format); problems (parse errors, non-UTF-8, large-file warnings, panics with truncated message).
- Never logged: file contents, keystrokes, cursor/scroll. High-frequency data aggregated in memory, flushed as one `session_summary` on exit.
- Paths: extension + salted hash only. Salt generated once, stored in settings dir.
- Limits: cap 2 MB; on startup if over cap rename to `usage.1.jsonl` (replacing older) and start fresh; on rotation drop events older than 90 days; lines truncated to ~300 bytes. Worst case ~4 MB on disk. Caps configurable.
- Fail-safe: writes on a background thread via channel; all I/O errors swallowed; torn last line skipped on read.
- `--stats` prints summary (sessions, avg startup, formats opened, view/edit split, feature counts, errors). `--stats --clear` and a menu item wipe both files.

## Testing

Unit tests for pure logic only: format detection, each tokenizer (input → expected spans), line-ending preservation, atomic save, JSON error position, find match spans, settings defaults on corrupt input, telemetry rotation/age/truncation, stats aggregation. UI verified manually by launching.
