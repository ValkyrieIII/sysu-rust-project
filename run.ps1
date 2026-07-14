# Build and run the Rust OS in QEMU
# Usage: .\run.ps1

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

Write-Host "Building kernel + disk image..." -ForegroundColor Cyan
cargo build 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit $LASTEXITCODE
}

Write-Host "Launching QEMU..." -ForegroundColor Green
cargo run
