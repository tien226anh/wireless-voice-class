$ErrorActionPreference = "Stop"
cargo build --release
Write-Host "Built: target\\release\\wireless-pa.exe"
