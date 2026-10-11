# Settings modal + more file extensions — Design

Date: 2026-10-12. Builds on `2026-10-12-turbo-text-rust-design.md`.

## Intent

Let the user customize the app from inside it instead of hand-editing `settings.toml`, and associate TurboText with many more text-file extensions. Success: every setting below can be changed from a dialog, takes effect as described, survives restart, and a bad value in `settings.toml` can never crash the app or reset neighbouring settings. The app must not get measurably slower (window up in about 30-80 ms, binary about 6 MB, no new dependencies).

**Out of scope:** accent-colour pickers, system-font loading, "open last file", restoring window position (size only), a separate OS window for settings, regex find, any network code.

## Approach

One new module `settings_ui.rs`: an in-window egui `Window` ("Settings", centered, scrollable) with one small function per group. Hand-wired fields, no schema layer. Zero per-frame cost when closed (egui draws nothing for a closed window).

## Settings (all in `settings.toml`, all validated per field)

| Group | Key | Type / range | Default | Takes effect |
|---|---|---|---|---|
| Editor | `font_size` | 6-72 | 14 | live |
| | `wrap` | bool | false | live |
| | `show_line_numbers` | bool | true | live |
| | `tab_spaces` | 0, 2, 4, 8 (0 = real tab) | 0 | live |
| | `highlight` | bool | true | live |
| | `large_file_mb` | 1-100 | 5 | live |
| | `editor_font` | "monospace" or "proportional" (egui built-in fonts) | "monospace" | live |
| Appearance | `theme` | "system", "light", "dark" | "system" | live |
| | `ui_scale` | 0.75-2.0 | 1.0 | live |
| Behavior | `open_json`, `open_markdown`, `open_other` | "view" or "edit" | view, view, edit | next file opened |
| | `confirm_close` | bool | true | live |
| | `remember_window` (size only) + `window_w`, `window_h` | bool; 300-10000 | true | next launch |
| Files and privacy | `assoc_exts` | list of extensions | config-and-docs set | on Apply |
| | `telemetry`, `telemetry_max_kb`, `telemetry_max_days` | existing | true, 2048, 90 | next launch |

Default extension list: `.env .json .md .toml .ini .yaml .yml .txt .log .conf .cfg .config .properties .gitignore .editorconfig .csv .xml`.

Invalid, missing or out-of-range values fall back to that field's own default (clamped where a range exists); a field never discards its neighbours. An empty file always opens in edit mode regardless of `open_*`.

## Behavior

- **Opening:** File > Settings… or `Ctrl+,`. `Esc` or the window's close button closes it. It never blocks editing the file behind it.
- **Live apply:** every control change applies on the same frame. Writing `settings.toml` is debounced (about 400 ms after the last change) and flushed on exit, so dragging a slider does not write the file every frame.
- **Reset:** each group has "Reset to defaults" (resets only that group's keys).
- **Tab key:** when `tab_spaces` > 0, Tab in the editor inserts spaces up to the next tab stop (replacing any selection) instead of a tab character.
- **Associations:** the list shows a checkbox per extension reflecting the registry; text field + Add for custom extensions (validated with the existing `assoc::normalize_ext`, de-duplicated, order kept); a Remove button per entry; **Apply** registers the checked and unregisters the unchecked, then opens the Windows Default apps page if anything was registered. This replaces the old "File associations…" window.
- **Unsaved changes:** with `confirm_close` off, closing discards unsaved edits without asking; the dialog says so next to the checkbox.
- **Window size:** on exit, if `remember_window` is on, the last inner size is stored; at startup it is used if within 300-10000 px per side, else 900x650. Position is deliberately not restored (an off-screen window after a monitor change would be unrecoverable).
- **Telemetry settings** are labelled "applies next launch".

## Units

- `settings.rs`: the `Settings` struct, per-field parse/validate, defaults, `start_in_view(Format)`, default extension list, `normalize_ext_list`.
- `settings_ui.rs`: the dialog; pure UI over `&mut Settings` plus association state; returns what changed.
- `editor.rs`: honors `highlight`, `editor_font`, `large_file_mb`, `tab_spaces` (pure `indent_insert`).
- `app.rs`: owns dialog state, applies theme/zoom when settings change, debounced save, confirm-close, open-mode, window size capture.
- `main.rs`: initial window size from settings.
- `assoc.rs`: unchanged API; the dialog calls `associate`/`unassociate`/`is_associated`.

## Errors

Unwritable settings directory: the in-memory settings still apply for the session and a banner says "Could not save settings". Corrupt `settings.toml`: defaults, as before.

## Testing

Unit tests (pure logic): every new field's parse, default, clamp and invalid-type fallback; one bad value does not reset the others; `start_in_view` per format; default extension list contents; `normalize_ext_list` (dedupe, order, invalid dropped, case); `indent_insert` (tab stops, mid-line, selection replacement, multibyte); save/parse roundtrip including the new fields; window size validation. UI verified manually: launch with a debug-only hook (`TT_DEBUG_SETTINGS=1`) that opens the dialog, screenshot only the app window. Startup time compared before/after.
