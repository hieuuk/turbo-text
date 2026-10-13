# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project uses [Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-10-14

### Added
- Single-file editor and viewer for .env, JSON, Markdown and config files.
- Atomic saves that keep the file's CRLF or LF line endings.
- Find and replace, Go to line, zoom, and Ln/Col display in the status bar.
- Save As and New window.
- Automatic reload when the file changes on disk.
- Settings dialog with 12 themes and Windows font choices for the editor and the Markdown view.
- File associations, managed with `--install` and `--uninstall`.
- `--version` and `--help` command-line options.
- Optional usage log. It is off by default and stays on the local machine.
