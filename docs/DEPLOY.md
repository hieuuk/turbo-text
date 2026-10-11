# TurboText deploy plan

"Live" = a build you actually use day to day, installed at a path that doesn't move, with Windows pointing at it. Windows only, per-user, no admin.

## Why this exists

The Python version registered its handlers against `dist\TurboText.exe` and `main.py` inside the repo. Deleting or moving those files silently broke every double-click. Rule for the new app: **file associations must never point into the repo or a build directory.**

## Install location

`%LOCALAPPDATA%\Programs\TurboText\turbo-text.exe` (plus `turbo-text.exe.prev` for rollback). Settings and the usage log stay in `%APPDATA%\turbo-text\` and survive upgrades.

## Stage 1: personal install (done, scripted)

Run `scripts\deploy-local.ps1` from the repo:

1. `cargo test` must pass, else it aborts.
2. `cargo build --release`.
3. Keep the old exe as `.prev`, stop any running instance, copy the new exe.
4. First install (or with `-Associate`): `turbo-text.exe --associate` with the 17 default extensions from `scripts\deploy-local.ps1` (idempotent; the same list is editable in Settings > Files and privacy). Later deploys skip it so the Windows Default apps page does not reopen.

Rollback: copy `turbo-text.exe.prev` over `turbo-text.exe`.

Old Python leftovers were re-pointed on 2026-10-12; the backup of the previous registry values is `old-python-associations-backup.reg` in the install folder. Dead machine-wide classes `TurboTextMD` / `TurboTextINI` remain in HKLM (harmless; removing them needs an admin prompt).

## Stage 2: sharing with other people (not done, decide when needed)

| Step | Choice | Notes |
|---|---|---|
| Versioning | SemVer in `Cargo.toml`, tag `vX.Y.Z` | `--version` flag is not implemented yet (today `--version` opens a file named that) |
| Artifact | GitHub Release with `turbo-text-vX.Y.Z-x86_64.zip` + SHA256 | built by a GitHub Actions `windows-latest` job running tests, then `cargo build --release` |
| Signing | none initially | unsigned exes trigger SmartScreen "unknown publisher"; a code-signing cert (~$100-300/yr) removes it; antivirus false positives are common for unsigned Rust GUI binaries, so submit each release to Microsoft for analysis |
| Install | zip + `turbo-text.exe --install` (to build: copies itself to the install path and runs `--associate`) and `--uninstall` (removes associations and the exe) | not implemented; add before sharing so nobody hand-edits the registry |
| Updates | manual download | no auto-update; no network code in the app by design |
| Distribution later | winget manifest or Scoop bucket | only worthwhile with signing and a stable release cadence |

## Release checklist

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all clean.
2. Bump the version, update README if behavior changed, tag.
3. Build release; confirm the exe launches, opens a `.env`, a `.json`, a `.md`.
4. Run `scripts\deploy-local.ps1`; double-click a `.env` in Explorer.
5. Check `turbo-text.exe --stats` shows the launch (telemetry sanity).
6. (Stage 2) Publish release, attach zip and checksum.

## Known gaps / risks

- No Windows-side uninstall entry (Settings > Apps); `--uninstall` is the planned fix.
- `--associate` cannot change a default Windows has "locked" (UserChoice); the Default apps page opens for confirmation.
- Associations written by a different program's ProgID (for example `JSON`, `TOML`, `YAML` currently opening Cursor) are left alone by design; pick TurboText in Windows' "Open with > Always" if you want it as the default for those.
