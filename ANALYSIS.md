# Análisis y mejora de Tigorra

**Qué es:** una recreación de Typora escrita en Rust + egui (`app/src`, ~12.100 líneas con tests).
Edita Markdown y documentos Quarto (`.qmd`), renderiza KaTeX/mhchem, tiene corrector Hunspell
español/gallego/inglés, exportación a HTML/PDF y ejecución de celdas Python.

**Cómo se analizó:** lectura completa de los 7 módulos, compilación, ejecución de la suite de tests,
capturas a 2× de DPI, y ejercitación real con archivos Markdown/Quarto en disco y con el kernel de Python.

---

## 1. Veredicto

La parte *visual* era muy superior a la parte *funcional*. La app parecía un editor completo,
pero le faltaba la base que define a un editor de documentos: **abrir y guardar archivos reales**.
El botón «Guardar» solo cambiaba un flag en memoria; no existía «Abrir»; cada celda de Python se
ejecutaba en un proceso nuevo (perdiendo todas las variables); y la vista previa recortaba el texto
en vez de ajustarlo al ancho disponible.

Esas carencias se corrigieron. Lo que sigue es el análisis que las motivó, lo implementado y la
hoja de ruta que queda pendiente.

---

## 2. Hallazgos críticos (corregidos)

| # | Hallazgo | Impacto | Dónde | Estado |
|---|---|---|---|---|
| C1 | **«Guardar» no guardaba nada.** Solo ponía `is_modified = false`; no había escritura a disco ni diálogo de archivo. | Un editor que pierde el trabajo del usuario. | `main.rs` (menú Archivo, pre-mejora) | ✅ Guardado real, atómico y con diálogo |
| C2 | **No se podía abrir un archivo.** Todos los documentos estaban codificados en `documents.rs`. | Solo servía de demo. | `documents.rs` | ✅ Abrir archivo/carpeta, recientes, CLI, pestañas |
| C3 | **El kernel de Python perdía el estado entre celdas.** Cada celda se ejecutaba en un `python3` nuevo con un script envoltorio. | `import numpy` en la celda 1 no existía en la celda 2: el «como Jupyter» era falso. | `quarto.rs` (`PythonKernel::execute_cell`) | ✅ Sesión persistente con protocolo por stdin |
| C4 | **La ejecución bloqueaba la interfaz.** `child.wait_with_output()` en el hilo de UI, sin timeout. | Un `while True` colgaba la app para siempre. | `quarto.rs` | ✅ Ejecución en segundo plano, `poll()` no bloqueante, timeout de 180 s |
| C5 | **`has_quarto_cli()` lanzaba un subproceso en cada frame** mientras se dibujaba el panel Quarto o la barra de estado. | Decenas de procesos por segundo. | `quarto.rs` | ✅ Detección cacheada + botón «re-detectar» |
| C6 | **El texto de la vista previa se recortaba.** El ancho del panel crecía por la fila de pestañas y los párrafos no tenían ancho de ajuste. | Previas ilegibles en ventana normal, especialmente en vista dividida. | `main.rs` (fila de pestañas), `markdown.rs` | ✅ Pestañas con scroll, párrafos con `wrap_width`, columnas con padding |
| C7 | **Corrector ortográfico peligroso.** El idioma se deducía del *título* de los documentos de ejemplo, y «Corregir todo» aplicaba la primera sugerencia incluso en palabras correctas. En la captura inicial proponía `title → tite`, `Caderno → Quaderno`, `Quarto → Quarte`. | Podía destruir el documento con un clic. | `spellcheck.rs`, `main.rs` | ✅ Detección por contenido + `fix_all_errors` conservador + sugerencias plausibles |
| C8 | **El PDF perdía los acentos** (`Título → Titulo`) y exportaba los marcadores Markdown (`**negrita**`) literalmente. | Export inutilizable en español/gallego. | `export.rs` (`pdf_escape`) | ✅ `WinAnsiEncoding` + `plain_markdown` + frontmatter omitido |
| C9 | **Insertar formato añadía texto al final del documento**, no en el cursor, e insertaba la palabra «texto». | Botones B/I/KaTeX inservibles para escribir. | `main.rs` (`insert_markup_at_cursor`) | ✅ Inserción en el cursor, envolviendo la selección |
| C10 | **Sin atajos de teclado.** Ni ⌘S, ni ⌘O, ni ⌘F, ni undo explícito. | Flujo de trabajo roto para cualquier usuario. | `main.rs` | ✅ ⌘S/⇧⌘S/⌘O/⌘N/⇧⌘N/⌘F/⌘B/⌘I/⌘W y hoja de atajos |
| C11 | **El resultado de `quarto render` se descartaba.** Se escribía el `.qmd` temporal, se lanzaba el CLI, se ignoraba su salida y siempre se abría la previa nativa. | El usuario creía haber renderizado con Quarto cuando no era así. | `quarto.rs` (`export_and_preview_quarto`) | ✅ Se abre el HTML del CLI si tiene éxito y se informa del error si falla |
| C12 | **`is_quarto()` clasificaba como Quarto cualquier documento que empezara por `---`.** | Un Markdown con frontmatter YAML aparecía como `.qmd`, y una regla horizontal inicial se comía el documento como frontmatter. | `documents.rs`, `quarto.rs` | ✅ Detección por extensión/`quarto:` y frontmatter validado |
| C13 | **Sin protección de cambios sin guardar.** Cerrar una pestaña o la ventana descartaba el trabajo. | Pérdida silenciosa de datos. | `main.rs` | ✅ Diálogo Guardar / Descartar / Cancelar al cerrar pestaña o app |
| C14 | **La barra de estado mentía:** `Ln {líneas}, Col 1` fijo, avisos que nunca caducaban. | Información incorrecta y ruido permanente. | `main.rs` | ✅ Ln/Col reales, avisos con caducidad de 7 s |
| C15 | **El frontmatter YAML se revisaba ortográficamente** y generaba avisos como `lang → Lang`. | Ruido en cada documento Quarto. | `spellcheck.rs` (`extract_prose_words`) | ✅ Frontmatter excluido del corrector |
| C16 | **La previa ignoraba imágenes, enlaces, listas anidadas, alineación de tablas y casillas de tareas.** | La vista previa no representaba el documento. | `markdown.rs` | ✅ Imágenes locales, enlaces clicables, listas anidadas, alineación, tareas interactivas |
| C17 | **El detector de Python elegía el primer `python3` del PATH**, que en macOS/Homebrew suele no tener NumPy ni Matplotlib. | «Motor científico» roto en la máquina del usuario. | `quarto.rs` (`detect_python`) | ✅ Se recorren todos los intérpretes y se prefiere el que tiene NumPy + Matplotlib |
| C18 | **El gutter se desalineaba** al ajustar líneas y el estado de scroll se filtraba entre documentos. | Números de línea incorrectos y scroll «pegado» al cambiar de pestaña. | `main.rs` (`render_editor_with_gutter`) | ✅ Gutter solo sin ajuste, scroll por documento, opción «Ajuste» |

---

## 3. Lo que ya estaba bien (y se conservó)

- **Diseño visual**: barra de actividad, panel lateral, pestañas, barra de estado, 7 paletas y 5 hojas de estilo Typora. Muy cercano al original.
- **KaTeX nativo** con `latex-rust` y caché de texturas: fórmulas en línea y en bloque, incluidas fracciones, integrales y matrices.
- **mhchem**: `\ce{...}` se expande a LaTeX estándar (`quarto.rs`/`math.rs`), con etiqueta y copia del original.
- **Callouts de Quarto** (`::: {.callout-*}`) con iconos, colores por tipo y borde lateral.
- **Diccionarios Hunspell empaquetados** (`include_str!`): sin dependencias externas, con lista blanca técnica y diccionario personal.
- **Exportación HTML autocontenida** con KaTeX desde CDN, portada y hoja de estilo.
- **Tests de la lógica pura** (Quarto, export, corrector): la base sobre la que se construyó lo nuevo.

---

## 4. Qué se implementó

### 4.1 Archivos reales (`app/src/files.rs`, nuevo)

- `read_text`: BOM UTF-8, detección de UTF-16 LE/BE, normalización CRLF→LF, límite de 32 MB.
- `write_text_atomic`: escribe un temporal y renombra; reintento borrando el destino en Windows (donde `rename` no sobrescribe).
- `scan_dir`: carpetas primero, filtro de extensiones (`.md/.markdown/.mdown/.qmd/.txt`), oculta `.git/target/node_modules`.
- `Prefs`: archivos recientes, última carpeta y último tema en `~/Library/Application Support/Tigorra` (macOS), `%APPDATA%` (Windows) o `~/.config/tigorra` (Linux).
- `FilePicker`: diálogo propio (sin GTK ni dependencias nativas) con navegación, ruta editable, filtro de extensiones, guardado con confirmación de sobrescritura.

### 4.2 Modelo de documento y pestañas (`documents.rs`)

- `Document` guarda `file_path` y una **instantánea del contenido guardado**; `is_dirty()` se deriva de ella (ya no hay un flag que pueda desincronizarse).
- `open_path` reutiliza una pestaña ya abierta y **sustituye la pestaña en blanco** si está intacta, sin pisar nunca una pestaña con cambios.
- `mark_saved`, `revert`, `directory()` (para resolver imágenes y el `cwd` de Python).

### 4.3 Editor (`main.rs`)

- Guardar / Guardar como / Guardar todo / Abrir / Abrir carpeta / Recientes / Cerrar pestaña / Salir.
- Inserción real en el cursor (`insert_text_at_cursor`) y envoltura de la selección.
- Atajos ⌘/Ctrl + S, ⇧S, O, N, ⇧N, F, B, I, W, y ventana «Atajos de teclado».
- Diálogo de cambios sin guardar al cerrar pestaña o ventana.
- Gutter alineado 1:1 (sin ajuste) y conmutador **Ajuste** para activar el ajuste de línea (que oculta el gutter para no mentir con los números).
- Ln/Col reales, indicador de guardado, avisos caducables, y limpieza de estado de scroll por documento.
- Buscador con contador de coincidencias por línea, salto a la línea en la previa, mayúsculas/minúsculas y reemplazo con recuento.
- Esquema clicable: al pulsar un encabezado, la previa se desplaza a esa línea.

### 4.4 Vista previa (`markdown.rs`)

- **Ajuste de párrafos** al ancho de la columna (el bug que recortaba el texto).
- Tokenizador en línea propio: `$...$` realista (no confunde `$5 y $10`), `**negrita**`, `*cursiva*`, `~~tachado~~`, `` `código` ``, imágenes y enlaces.
- **Imágenes locales** con caché de texturas invalidada por mtime, resolución relativa al documento y `%20` decodificado; placeholder explicativo si falta la ruta.
- **Enlaces clicables** (externos en el navegador, `.md/.qmd` locales abiertos como pestaña).
- **Listas anidadas**, listas de tareas con **casilla interactiva** que modifica el `[ ]/[x]` del origen, tablas con alineación `:---:`/`---:`, y citas con formato en línea.
- Extracción de encabezados con soporte de bloques de código y matemáticas.

### 4.5 Quarto y Python (`quarto.rs`)

- Detección del CLI cacheada y «re-detectar» en el panel.
- `export_and_preview_quarto` genera la previa nativa **y** ejecuta `quarto render --to html --output-dir`, abriendo el resultado real del CLI y reportando su error si falla. Se eliminó el `.qmd` huérfano en favor de un flujo reproducible.
- **Kernel persistente**: un único proceso Python con un driver que lee peticiones por `stdin`, ejecuta en un espacio de nombres compartido, captura `stdout/stderr` en archivos y guarda las figuras de Matplotlib.
- `poll()` drena resultados con un canal y un hilo lector: la UI nunca se bloquea; se repinta mientras hay trabajo; timeout que reinicia el kernel.
- Cola de celdas («Ejecutar Python» encola todas las celdas) con estado «⏳ Executando…» por celda.
- Directorio de trabajo por celda (rutas relativas correctas y cambio de documento).
- Detección de intérpretes que prefiere NumPy + Matplotlib y muestra la ruta elegida.
- Filtro del aviso benigno de Matplotlib en modo Agg.

### 4.6 Corrector ortográfico (`spellcheck.rs`)

- `detect_language`: frontmatter `lang:`, título y vocabulario característico; empate → español (idioma de la UI).
- `fix_all_errors` ahora devuelve `FixReport { fixed, skipped }` y **omite** palabras válidas en otro diccionario instalado y sugerencias que no cambian la palabra.
- `plausible_suggestion`: rechaza sugerencias con espacios, exige misma inicial, máximo 2 ediciones y solo 1 para palabras capitalizadas (evita `Jupyter → Júpiter`, `Quarto → Cuarto`).
- Lista blanca técnica ampliada (Jupyter, Quarto, NumPy, Matplotlib, YAML, chunk, kernel…).
- Frontmatter excluido del análisis, y el editor ya no ajusta líneas sin querer (gutter alineado).

### 4.7 Exportación (`export.rs`)

- PDF con `WinAnsiEncoding` (acentos, `ñ`, `¿`, `¡`, comillas tipográficas, `€`, `–`, `—`) y escapado octal.
- El texto del PDF se limpia de marcadores Markdown, se omiten frontmatter y reglas de código, y se añaden listas numeradas y casillas.
- Ancho de ajuste calculado a partir del ancho útil de la página.

---

## 5. Segunda ronda: deshacer, interrupción, CLI y tipografía

Después de la primera ronda quedaron huecos que no eran «bugs» sino funciones ausentes. Esta ronda los
cierra y documenta una regresión que los tests nuevos detectaron.

| # | Hallazgo | Impacto | Solución |
|---|---|---|---|
| R1 | Las acciones de formato, reemplazo global y corrección ortográfica **no entraban en la pila de deshacer**. ⌘Z solo deshacía lo tecleado. | Una negrita o un «Corregir todo» era irreversible salvo cerrando sin guardar. | `push_editor_undo`: inserta la instantánea previa en el `Undoer` interno de egui (`TextEditState::undoer` + `set_undoer`). Tests `undo_tests`. |
| R2 | **El kernel no se podía interrumpir.** Una celda en bucle bloqueaba la app hasta el timeout de 180 s, que además reiniciaba el intérprete y perdía las variables. | Mala experiencia en notebooks. | `PythonKernel::interrupt` envía SIGINT (`kill -INT` / `taskkill`); el driver captura `KeyboardInterrupt` y la sesión sigue viva. Botón «⏹ Interrumpir» en la celda y test de interrupción. |
| R3 | El driver importaba Matplotlib **al arrancar**; un SIGINT temprano mataba el proceso y la UI quedaba «pensando» para siempre. | Kernel fantasma sin diagnóstico. | Import perezoso de Matplotlib (Rust ya fija `MPLBACKEND=Agg`) y detección de muerte del proceso con error visible. |
| R4 | **Regresión hallada por el test de interrupción:** la comprobación de «proceso muerto» hacía `self.active.take()` incondicionalmente, por lo que **cada `poll` descartaba la celda activa**. | Las celdas nunca terminaban (habría roto todo el Python). | `active` se toma solo si el proceso terminó de verdad; el test de interrupción queda como regresión. |
| R5 | No había forma rápida de saltar entre archivos de la carpeta. | Navegación incómoda en proyectos grandes. | **Abrir rápido (⌘P)**: escaneo recursivo (600 archivos, 4 niveles), ranking por nombre y filtro; `files::filter_by_query` + test. |
| R6 | El CLI de Quarto solo se usaba para la previa HTML; no había PDF ni DOCX. | Quarto a medias. | `render_with_quarto_cli` con HTML/PDF/DOCX, renderizado del **documento real** (respeta `_quarto.yml`) o copia en `exports/` si no está guardado, parseo de «Output created:» y búsqueda del archivo generado. Botones en el panel y menú Archivo. |
| R7 | Muchos símbolos del UI (✓ ✗ ● ○ ◉ ⚠ ⌘ ⇧ ⏱ ⏹ ⟳ ↺ ⬆) salían como **cuadros** porque las fuentes de epaint no incluyen dingbats. | Aspecto roto en barras, paneles y botones. | Fuentes **DejaVuSans + NotoSansSymbols2** empaquetadas (con licencias) y cargadas como fallback en `theme::install_fonts`. Hoja de prueba `TIPPORA_UI_TEST=glyphs`. |
| R8 | El test del CLI de Quarto falla en el sandbox del agente porque Quarto escribe su caché en `~/Library/Caches`. | Falso negativo, no bug de la app. | El test omite con mensaje si el error es de caché/permisos; verificado con acceso completo (render HTML real, 2,8 s). |

**Lección:** los tests de concurrencia/tiempo (kernel) encontraron un fallo que ninguna captura de
pantalla habría mostrado. Merece la pena mantener el kernel bajo pruebas con procesos reales.

---

## 6. Verificación

```bash
cd app
cargo test          # 51 tests, 0 fallos
cargo build --release
./target/release/tigorra --help
```

Cobertura nueva (además de los 16 tests previos): lectura con BOM/UTF-16, escritura atómica, escaneo de
carpetas, recientes, modelo sucio/guardado, apertura sin duplicar pestañas, detección de idioma por
documento, sugerencias plausibles, tokenizador en línea, rutas de imagen, separadores de tabla,
**estado compartido entre celdas**, **supervivencia a una celda que lanza excepción**, **captura de
figuras Matplotlib**, **interrupción con SIGINT sin reiniciar el kernel**, **exportación Quarto a
archivo**, **detección/parseo de salidas del CLI**, **filtro y ranking de Abrir rápido**, **deshacer de
acciones programáticas** y **fuentes empaquetadas válidas**.

Capturas (generadas con `--screenshot` y los hooks descritos abajo):

| Archivo | Qué muestra |
|---|---|
| `shots/improved_split.png` | Vista combinada con pestañas contenidas, gutter alineado, previa ajustada y símbolos correctos |
| `shots/improved_preview.png` | Previa a ancho de lectura, párrafos ajustados y celdas Quarto |
| `shots/improved_tables.png` | Tabla alineada, tareas interactivas, listas anidadas y cita con KaTeX |
| `shots/improved_python.png` | Kernel persistente: `In [1]`, `Out [1]` y figura de Matplotlib inline |
| `shots/improved_quickopen.png` | Abrir rápido (⌘P) con ranking de archivos |
| `shots/improved_quarto_panel.png` | Panel Quarto con exportación CLI HTML/PDF/DOCX |
| `shots/improved_picker.png` | Selector de archivos propio |

También se verificó el ciclo completo de guardado: abrir `guardado.md`, modificarlo desde la app y
comprobar que el archivo en disco contiene el cambio. El render real del CLI se verificó con
`quarto 1.8.25` (test `quarto_cli_renders_html_when_available`).

### Hooks de prueba (sin efecto en uso normal)

- `--screenshot <png>`: dibuja 120 frames, captura y sale.
- `--view source|split|preview`: vista inicial.
- `--folder <dir>` / `archivo.md`: espacio de trabajo y documento inicial.
- `TIPPORA_UI_TEST=picker|picker-save|confirm|shortcuts|quick-open|quarto-panel|run-python|edit-save|glyphs`:
  abre un diálogo o ejecuta una acción en el primer frame (capturas y smoke tests).

---

## 7. Hoja de ruta recomendada (pendiente)

Ordenada por relación impacto/esfuerzo. Nada de esto está implementado todavía.

1. **Historial de deshacer a nivel de documento.** egui ya da undo dentro del `TextEdit`, pero las
   modificaciones programáticas (negrita, reemplazo global, corrección masiva, casillas) no entran en
   esa pila. Guardar instantáneas del contenido por documento antes de cada edición programática.
2. **WYSIWYG real (estilo Typora).** Hoy hay editor + previa. El salto natural es renderizar bloques
   editables in situ; requiere un mapeo estable entre offsets del buffer y bloques del layout.
3. **Parser CommonMark completo.** El renderizador es propio y por líneas: no soporta tablas sin
   cabecera, listas de definición, notas al pie, HTML inline, atributos `{#id}` ni referencias.
   Adoptar `pulldown-cmark` (o `comrak`) para el AST y conservar el renderizador egui.
4. **Exportación PDF de calidad tipográfica.** El PDF nativo sigue siendo un generador propio con
   `Type1` y matemáticas como texto. Ya existe la vía `quarto render --to pdf|docx` (requiere Quarto,
   y LaTeX para PDF); el siguiente paso es usarla por defecto cuando esté disponible e integrar
   Typst/WeasyPrint para el modo sin Quarto.
5. **Imágenes pegadas y remotas.** Pegar desde el portapapeles y guardar en `assets/`; descargar
   imágenes `http(s)` con caché en disco y aviso de red.
6. **Quarto de proyecto.** El CLI ya renderiza el documento real (por lo que `_quarto.yml` y las
   extensiones se aplican) y exporta HTML/PDF/DOCX. Falta `quarto preview` como servidor, perfiles
   (`--profile`), `freeze`/caché de ejecución y comprobación previa de dependencias (LaTeX para PDF).
7. ~~**Kernel Python de verdad (interrupción).**~~ **Hecho:** SIGINT sin matar el proceso y botón de
   interrupción. Queda pendiente el streaming de salida en vivo por celda, un selector de intérprete en
   la UI y soporte de R cuando el CLI de Quarto exista.
8. **Autoguardado y recuperación.** Autosave cada N segundos en un archivo lateral, detección de
   cambios externos (mtime/hash) y restauración de sesión.
9. **Corrector por documento.** Recordar el idioma elegido por archivo, diccionarios de usuario en
   disco (ahora solo memoria), `ignore all` por documento y sugerencias con distancia mínima visible.
10. **Accesibilidad y rendimiento.** Navegación por teclado en paneles y diálogos, etiquetas ARIA
    equivalentes (egui no las expone), contraste configurable; y en rendimiento: evitar el clonado
    del contenido y de la lista de errores en cada frame, cachear el `LayoutJob` del editor.
11. **Empaquetado.** CI en matriz macOS/Linux/Windows, firma y notarización, icono `.icns/.ico`,
    actualizador. Los scripts actuales compilan, pero no hay verificación automática de los binarios.
12. **Tests de interfaz.** El hook `TIPPORA_UI_TEST` + `--screenshot` permite diffs visuales
    (golden screenshots) en CI; hoy se verifican a mano.

### Riesgos y notas

- **Ejecutar Python es ejecutar código arbitrario.** Conviene un aviso explícito y, si se comparte en
  equipo, una opción de «modo seguro» que desactive la ejecución.
- **El parser por líneas no es CommonMark.** Las mejoras cubren lo cotidiano, pero documentos complejos
  seguirán mostrándose de forma aproximada hasta adoptar un parser real.
- **`latex-rust` cubre una parte de KaTeX.** Fórmulas avanzadas (`\begin{align}`, macros, `\ce` con
  opciones) pueden fallar silenciosamente; hay que mostrar un error visible de fórmula.
- **La UI está en español/gallego mezclado** (menús en español, panel Quarto en gallego). Recomendación:
  unificar en un solo idioma o añadir i18n.
- **Fuentes empaquetadas:** `assets/fonts` añade ~1,9 MB al repositorio y al binario; DejaVuSans es de
  licencia libre y NotoSansSymbols2 usa OFL (ambas licencias incluidas). Es el precio de que los
  símbolos se vean igual en macOS, Linux y Windows. No hay fuente CJK: chino/japonés/coreano se verán
  como cuadros hasta añadir una (p. ej. Noto Sans CJK, varios MB).
- **PDF/DOCX vía Quarto** requieren el CLI; el PDF requiere además LaTeX. Se verificó en esta máquina
  con Quarto 1.8.25 (`t.docx` 10,6 KB y `t.pdf` 5,9 KB generados correctamente).
- **No hay `rustfmt` ni `clippy` instalados en este entorno**, así que no se aplicó formato automático
  ni lint; conviene añadirlos al CI (`rustup component add rustfmt clippy`).

---

## 8. Mapa del código

| Módulo | Responsabilidad |
|---|---|
| `main.rs` | Estado de la app, paneles, menús, diálogos, atajos y editor |
| `documents.rs` | Documento (ruta, contenido, instantánea), gestor de pestañas |
| `files.rs` | Lectura/escritura, escaneo de carpetas, preferencias, selector y Abrir rápido |
| `markdown.rs` | Renderizador de vista previa y tokenizador en línea; caché de imágenes |
| `quarto.rs` | Frontmatter, callouts, chunks, kernel Python, render HTML/PDF/DOCX y previa |
| `math.rs` | Renderizado KaTeX/mhchem a texturas egui |
| `export.rs` | HTML autocontenido, PDF 1.4, hojas de estilo Typora |
| `spellcheck.rs` | Hunspell, detección de idioma, sugerencias, layouter del editor |
| `theme.rs` | Paletas de color y fuentes de símbolos |
| `assets/fonts/` | DejaVuSans + NotoSansSymbols2 y sus licencias |
