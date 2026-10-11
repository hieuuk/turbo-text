# Build the release exe, install it to a stable per-user path, and (re)register file associations.
# Safe to re-run; use it after every release build. No admin needed.
param(
    [string[]]$Exts = @('.env', '.json', '.md', '.toml', '.ini', '.yaml', '.yml', '.txt', '.log', '.conf', '.cfg', '.config', '.properties', '.gitignore', '.editorconfig', '.csv', '.xml'),
    [switch]$Associate   # re-register associations (always done on first install)
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$dest = Join-Path $env:LOCALAPPDATA 'Programs\TurboText'
$exe = Join-Path $dest 'turbo-text.exe'

Push-Location $root
try {
    cargo test --quiet
    if ($LASTEXITCODE) { throw 'tests failed, not deploying' }
    cargo build --release --quiet
    if ($LASTEXITCODE) { throw 'build failed' }
} finally { Pop-Location }

New-Item -ItemType Directory -Force $dest | Out-Null
$firstInstall = -not (Test-Path $exe)
# keep the previous build for rollback; a running exe can't be overwritten, so stop it first
if (Test-Path $exe) { Copy-Item $exe "$exe.prev" -Force }
Get-Process turbo-text -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process -Force
Copy-Item (Join-Path $root 'target\release\turbo-text.exe') $exe -Force
if ($firstInstall -or $Associate) { & $exe --associate @Exts }
"installed $exe ($((Get-Item $exe).Length) bytes). Rollback: copy $exe.prev over $exe"
