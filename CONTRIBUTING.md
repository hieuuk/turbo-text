# Contributing

## Philosophy

TurboText is a fast, simple, minimal editor. There are no tabs, plugins or file explorer, and that is intentional. A new feature has to justify its cost in binary size and startup time.

## Build and test

```
cargo build --release
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

To install a local build, run `scripts\deploy-local.ps1`.

## Safe testing

Never test against your real `%APPDATA%\turbo-text`. Point `APPDATA` at a scratch folder before launching:

```
$env:APPDATA = "$env:TEMP\turbotext-scratch"
```

## Pull requests

Keep them small and include tests.
