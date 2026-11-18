@echo off
title Tigorra - Windows Launcher
echo ========================================================
echo   Iniciando Tigorra en Windows
echo ========================================================
echo.

where cargo >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [AVISO] Rust y Cargo no estan instalados en este equipo.
    echo Abriendo la pagina oficial de descarga de Rust (https://rustup.rs)...
    start https://rustup.rs/
    echo.
    echo Descarga e instala Rust ejecutando 'rustup-init.exe' y vuelve a ejecutar este archivo.
    pause
    exit /b 1
)

echo Compilando y ejecutando la aplicacion en Windows...
echo.
cargo run --release
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Hubo un problema con la version release. Reintentando en modo normal...
    cargo run
)
pause
