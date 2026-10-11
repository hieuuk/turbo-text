# TurboText

Tiny, fast, single-file editor/viewer for Windows. Built for quick edits to `.env` and config files, viewing JSON, and viewing/editing Markdown. No tabs, no explorer, no plugins: use VS Code for that.

Rust + [egui](https://github.com/emilk/egui). Release exe is about 6 MB and the window opens in well under 100 ms.

The previous Python/Tk prototype is archived under the git tag `python-final`.

## Usage

```
turbo-text.exe [file]
turbo-text.exe --associate .env .json .md     # register as "Open with" handler (current user, no admin)
turbo-text.exe --unassociate .env
turbo-text.exe --stats                        # summarize the local usage log
turbo-text.exe --stats --clear                # wipe the usage log
```

One file per window. Open another file with `Ctrl+O` or drag it onto the window. Opening a path that doesn't exist starts a new file that is created on save.

| Key | Action |
|---|---|
| `Ctrl+S` | Save (atomic: temp file + rename; keeps CRLF/LF) |
| `Ctrl+O` | Open |
| `Ctrl+E` | Toggle view / edit |
| `Ctrl+F` / `Ctrl+H` | Find / find and replace (plain text, optional match case) |
| `Alt+Z` | Toggle word wrap |

`.json` and `.md` open in view mode (collapsible tree / rendered Markdown). Everything else opens in the editor with syntax highlighting and line numbers. Invalid JSON shows the error with line and column. Files that are not valid UTF-8 open read-only so they can't be corrupted. Files over 5 MB open without highlighting.

## Settings

`%APPDATA%\turbo-text\settings.toml` (File > Open settings file). Missing or invalid values fall back to defaults.

```toml
wrap = false
theme = "system"          # system | light | dark
font_size = 14.0
show_line_numbers = true
telemetry = true
telemetry_max_kb = 2048
telemetry_max_days = 90
```

## Usage log (local only)

With `telemetry = true`, TurboText appends small JSON events to `%APPDATA%\turbo-text\usage.jsonl`: startup time, formats and size buckets opened, feature use, view vs edit time per format, and problems. It never records file contents, keystrokes, or file names (paths are stored as extension plus a salted hash), and nothing leaves your machine. The log rotates at `telemetry_max_kb` (old events beyond `telemetry_max_days` are dropped), so it stays under about twice that size. Use `--stats` to read it, or the "Usage log" menu to open or clear it.

## File associations

`--associate` (or File > File associations) registers TurboText under `HKCU\Software\Classes` and sets itself as an extension's default only when none exists. Windows keeps the default-app choice behind a confirmation, so the Default apps settings page opens afterwards.

## Build

```
cargo build --release
cargo test
```
