# Guía para llevar y ejecutar esta aplicación en Windows

Esta aplicación está desarrollada en **Rust** con interfaz nativa **egui/eframe**. Todas las fuentes matemáticas STIX, las hojas de estilo de Typora y los diccionarios ortográficos de Hunspell (Español, Gallego e Inglés) están **incrustados estáticamente dentro del ejecutable**, lo que significa que en Windows no necesitas instalar librerías externas ni navegadores web.

---

## Opción 1: Trasladar el proyecto y usar el lanzador automático (Recomendado)

1. **Copia la carpeta `app/`** a una memoria USB, o comprímela en un archivo `.zip` y envíatela por correo o súbela a Google Drive / OneDrive.
2. **Pégala en tu ordenador con Windows**.
3. Haz doble clic sobre el archivo **`run-windows.bat`**:
   - Si Windows aún no tiene instalado Rust, el archivo abrirá automáticamente la web oficial [rustup.rs](https://rustup.rs/) para instalarlo con un solo clic.
   - Si Rust ya está instalado, compilará y abrirá la ventana de la aplicación de forma nativa e inmediata.

---

## Opción 2: Generar un archivo `.exe` único e independiente

1. En el ordenador con Windows, abre la carpeta y haz doble clic sobre **`build-windows.bat`**.
2. Al terminar, encontrarás un archivo único llamado:
   `dist-windows\Tigorra.exe`
3. **Ese único archivo `.exe` es totalmente portable e independiente**:
   - Pesa muy poco.
   - No necesita instalador.
   - Puedes llevar solo ese archivo `.exe` a cualquier otro ordenador con Windows y funcionará al hacer doble clic.

---

## Opción 3: Ejecución manual desde la consola de Windows (CMD o PowerShell)

Abre la consola en la carpeta de la aplicación y ejecuta:
```cmd
cargo run --release
```
O para compilar el ejecutable final:
```cmd
cargo build --release
```
El ejecutable resultante estará en `target\release\tigorra.exe`.
