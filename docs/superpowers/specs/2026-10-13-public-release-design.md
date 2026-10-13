# Public release readiness (v1.0.0)

Goal: TurboText stays fast and minimal; ready for public GitHub (zip release + build from source). Repo stays private until the owner flips it.

## App
Save As (Ctrl+Shift+S), Ln/Col in status bar, Go to line (Ctrl+G), zoom (Ctrl +/-/0, Ctrl+wheel -> font_size), New window (Ctrl+N), reload on external change (mtime poll; clean = silent reload, dirty = banner Reload / Keep mine), --version/--help, --install/--uninstall (copy to %LOCALAPPDATA%\Programs\TurboText, associations, HKCU uninstall entry), Usage log menu moved into Settings, Help > About, telemetry default off.

## Repo
Delete designs/, desktop.ini, folder.*, .superpowers, docs/superpowers, docs/DEPLOY.md. Rust .gitignore. Cargo metadata, version 1.0.0. CHANGELOG, SECURITY, CONTRIBUTING, .gitattributes, issue templates. README with screenshots, install, SmartScreen note.

## CI/release
ci.yml (fmt, clippy, test, build on windows-latest); release.yml on tag v* (zip + sha256 + GitHub Release). No code signing in v1.

## Out of scope
Tabs, recent files, regex find, auto-save, plugins, signing, winget.
