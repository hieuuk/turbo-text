$venvPath = "$PSScriptRoot\.venv"

if (-not (Test-Path "$venvPath\Scripts\Activate.ps1")) {
    Write-Host "Creating virtual environment..." -ForegroundColor Cyan
    python -m venv $venvPath
    & "$venvPath\Scripts\Activate.ps1"
    Write-Host "Installing requirements..." -ForegroundColor Cyan
    pip install -r "$PSScriptRoot\requirements.txt"
} else {
    & "$venvPath\Scripts\Activate.ps1"
}

python "$PSScriptRoot\main.py"
