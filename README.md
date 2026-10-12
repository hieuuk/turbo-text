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

`.json` and `.md` open in view mode (collapsible tree / rendered Markdown). Everything else opens in the editor with syntax highlighting and line numbers. Invalid JSON shows the error with line and column. Files that are not valid UTF-8 open read-only so they can't be corrupted. Files over `large_file_mb` (5 by default) open without highlighting.

## Settings

Open **File > Settings…** (`Ctrl+,`). A sidebar switches between Editor, Appearance (colour themes), Behavior and Files and privacy. Changes apply instantly and are saved automatically; each page has "Reset this page to defaults" and the sidebar has "Reset all to defaults". The file is `%APPDATA%\turbo-text\settings.toml` (File > Open settings file) and can also be edited by hand: every key is validated on its own, so an invalid or out-of-range value falls back to that key's default and never affects the others.

| Key | Values (default) | Applies |
|---|---|---|
| `font_size` | 6-72 (14) | live |
| `line_height` | 0.8-2.0 (1.0): row height as a multiple of the font's natural row height | live |
| `editor_font` | `monospace` / `proportional` | live |
| `wrap`, `show_line_numbers`, `highlight` | bool (false, true, true) | live |
| `tab_spaces` | 0 = tab character, 2, 4, 8 (0) | live |
| `large_file_mb` | 1-100 (5): highlighting turns off above this size | live |
| `theme` | `system` (follow Windows), `dark`, `light`, `dracula`, `tokyo-night`, `catppuccin-mocha`, `nord`, `gruvbox-dark`, `solarized-dark`, `one-dark`, `monokai`, `solarized-light`, `catppuccin-latte` (system) | live |
| `ui_scale` | 0.75-2.0 (1.0) | live |
| `open_json`, `open_markdown`, `open_other` | `view` / `edit` (view, view, edit) | next file opened |
| `confirm_close` | bool (true): ask before closing with unsaved changes | live |
| `remember_window`, `window_w`, `window_h` | bool (true), size in 300-10000 | next launch |
| `assoc_exts` | list of extensions offered for association | on Apply |
| `telemetry`, `telemetry_max_kb`, `telemetry_max_days` | true, 2048, 90 | next launch |

Default association list: `.env .json .md .toml .ini .yaml .yml .txt .log .conf .cfg .config .properties .gitignore .editorconfig .csv .xml`. Add or remove entries in the settings dialog (Files and privacy), tick the ones you want and press "Apply associations".

## Usage log (local only)

With `telemetry = true`, TurboText appends small JSON events to `%APPDATA%\turbo-text\usage.jsonl`: startup time, formats and size buckets opened, feature use, view vs edit time per format, and problems. It never records file contents, keystrokes, or file names (paths are stored as extension plus a salted hash), and nothing leaves your machine. The log rotates at `telemetry_max_kb` (old events beyond `telemetry_max_days` are dropped), so it stays under about twice that size. Use `--stats` to read it, or the "Usage log" menu to open or clear it.

## File associations

`--associate` (or Settings > Files and privacy > Apply associations) registers TurboText under `HKCU\Software\Classes` and sets itself as an extension's default only when none exists. Windows keeps the default-app choice behind a confirmation, so the Default apps settings page opens afterwards.

## Build

```
cargo build --release
cargo test
```
