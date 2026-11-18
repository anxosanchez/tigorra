# Guía para llevar y ejecutar esta aplicación en Linux

Esta aplicación está desarrollada en **Rust** con el motor gráfico nativo **egui/eframe**, compatible tanto con servidores gráficos **Wayland** como **X11**. Todos los diccionarios ortográficos de Hunspell (Español, Gallego e Inglés), fuentes matemáticas STIX y estilos visuales están **incrustados dentro del ejecutable**, sin necesidad de navegadores web internos ni dependencias pesadas.

---

## Opción 1: Trasladar la carpeta y usar el script de inicio (Más sencillo)

1. **Copia la carpeta `app/`** a una memoria USB o descárgala en tu ordenador con Linux.
2. Abre una terminal en esa carpeta y ejecuta:
   ```bash
   ./run-linux.sh
   ```
   - Si tu equipo no tiene Rust instalado, el script lo instalará automáticamente de forma segura mediante `rustup`.
   - La aplicación compilará y se abrirá de inmediato.

---

## Opción 2: Generar un archivo ejecutable independiente (`dist-linux/Tigorra`)

1. En tu ordenador con Linux, ejecuta en la terminal:
   ```bash
   ./build-linux.sh
   ```
2. Al finalizar, tendrás un archivo ejecutable único en:
   `dist-linux/Tigorra`
3. Puedes copiar solo ese archivo a `/usr/local/bin` o a tu carpeta personal y abrirlo con doble clic.

---

## Opción 3: Ejecución manual estándar

En cualquier distribución Linux (Ubuntu, Debian, Fedora, Arch, Manjaro, Mint, etc.):
```bash
cargo run --release
```

---

## Librerías del sistema recomendadas (si tu sistema es una instalación mínima)

La mayoría de distribuciones de escritorio ya las incluyen. En caso de una instalación mínima:
- **Ubuntu / Debian / Linux Mint**:
  `sudo apt install build-essential libx11-dev libwayland-dev libxkbcommon-dev`
- **Fedora / RHEL**:
  `sudo dnf install gcc libX11-devel wayland-devel libxkbcommon-devel`
- **Arch Linux / Manjaro**:
  `sudo pacman -S base-devel libx11 wayland libxkbcommon`
