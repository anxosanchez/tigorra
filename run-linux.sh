#!/usr/bin/env bash
# Tigorra - Linux Launcher
set -e

echo "========================================================"
echo "  Iniciando Tigorra en Linux"
echo "========================================================"
echo ""

# Comprobar si Rust/Cargo está instalado
if ! command -v cargo &> /dev/null; then
    echo "[AVISO] Rust y Cargo no están instalados en este equipo."
    echo "Instalando Rust mediante rustup oficial..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "Compilando y ejecutando la aplicación de forma nativa..."
echo ""
cargo run --release
