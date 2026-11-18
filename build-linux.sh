#!/usr/bin/env bash
# Generador de ejecutable independiente para Linux
set -e

echo "========================================================"
echo "  Generando ejecutable Tigorra para Linux"
echo "========================================================"
echo ""

if ! command -v cargo &> /dev/null; then
    echo "[ERROR] No se encontró Rust/Cargo instalado."
    echo "Instala Rust con: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "Compilando binario optimizado para Linux (Release)..."
cargo build --release

mkdir -p dist-linux
cp target/release/tigorra dist-linux/tigorra
chmod +x dist-linux/tigorra

if [ -f tigorra.desktop ]; then
    cp tigorra.desktop dist-linux/
fi

echo ""
echo "========================================================"
echo "  [ÉXITO] Ejecutable generado correctamente en:"
echo "  dist-linux/tigorra"
echo ""
echo "  Este binario es totalmente autónomo y portable."
echo "  Puedes copiarlo y ejecutarlo en cualquier Linux!"
echo "========================================================"
