@echo off
title Generador de ejecutable para Windows (.exe)
echo ========================================================
echo   Generando ejecutable Tigorra para Windows
echo ========================================================
echo.

where cargo >nul 2>nul
if %ERRORLEVEL% NEQ 0 (
    echo [ERROR] No se encontro Cargo/Rust instalado.
    echo Por favor descarga e instala Rust desde https://rustup.rs/
    start https://rustup.rs/
    pause
    exit /b 1
)

echo Compilando binario optimizado para Windows (Release)...
cargo build --release

if %ERRORLEVEL% EQU 0 (
    if not exist "dist-windows" mkdir "dist-windows"
    copy /y "target\release\tigorra.exe" "dist-windows\Tigorra.exe" >nul
    echo.
    echo ========================================================
    echo   [EXITO] Ejecutable generado correctamente:
    echo   dist-windows\Tigorra.exe
    echo.
    echo   Este archivo .exe es completamente autonomo y portable.
    echo   Tiene todos los diccionarios, estilos y formulas incluidos.
    echo   Puedes copiar solo Tigorra.exe y abrirlo en cualquier PC!
    echo ========================================================
) else (
    echo.
    echo [ERROR] Fallo la compilacion. Revisa los mensajes de error.
)

pause
