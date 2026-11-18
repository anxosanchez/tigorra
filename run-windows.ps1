# Tigorra - Windows PowerShell Launcher
Write-Host "========================================================" -ForegroundColor Cyan
Write-Host "  Iniciando Tigorra en Windows (PowerShell)" -ForegroundColor Cyan
Write-Host "========================================================" -ForegroundColor Cyan

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Warning "Cargo/Rust no está instalado en este sistema."
    Write-Host "Abriendo instalador oficial de Rust (https://rustup.rs)..." -ForegroundColor Yellow
    Start-Process "https://rustup.rs/"
    Read-Host "Presiona Enter para salir..."
    exit 1
}

Write-Host "Compilando y ejecutando la aplicación..." -ForegroundColor Green
cargo run --release
