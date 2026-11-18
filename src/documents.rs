use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Document {
    #[allow(dead_code)]
    pub id: usize,
    pub title: String,
    pub content: String,
    /// On-disk location, `None` for an unsaved scratch document.
    pub file_path: Option<PathBuf>,
    /// Snapshot of the content at the last successful load/save. Dirty state is
    /// derived from it instead of being a separate flag that can drift.
    saved_content: String,
}

impl Document {
    pub fn new(id: usize, title: impl Into<String>, content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            id,
            title: title.into(),
            saved_content: content.clone(),
            content,
            file_path: None,
        }
    }

    pub fn from_file(id: usize, path: &Path) -> Result<Self, String> {
        let content = crate::files::read_text(path)?;
        Ok(Self {
            id,
            title: crate::files::file_title(path),
            saved_content: content.clone(),
            content,
            file_path: Some(crate::files::normalize_path(path)),
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.content != self.saved_content
    }

    pub fn mark_saved(&mut self, path: PathBuf) {
        self.title = crate::files::file_title(&path);
        self.file_path = Some(path);
        self.saved_content = self.content.clone();
    }

    pub fn revert(&mut self) {
        self.content = self.saved_content.clone();
    }

    pub fn path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    /// Folder used to resolve relative image paths and Python working directory.
    pub fn directory(&self) -> Option<PathBuf> {
        self.file_path
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    }

    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }

    pub fn char_count(&self) -> usize {
        self.content.chars().count()
    }

    pub fn line_count(&self) -> usize {
        self.content.lines().count().max(1)
    }

    #[allow(dead_code)]
    pub fn reading_time_mins(&self) -> usize {
        let words = self.word_count();
        (words / 200).max(1)
    }

    pub fn is_quarto(&self) -> bool {
        let by_extension = self.title.to_lowercase().ends_with(".qmd")
            || self
                .file_path
                .as_ref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("qmd"));
        if by_extension {
            return true;
        }
        // Plain Markdown files often carry YAML frontmatter (Hugo, Jekyll);
        // only treat it as Quarto when the frontmatter actually mentions Quarto.
        self.content
            .lines()
            .take(24)
            .any(|line| line.trim_start().starts_with("quarto:") || line.contains("quarto-"))
    }
}

pub struct DocumentManager {
    pub documents: Vec<Document>,
    pub active_index: usize,
    pub next_id: usize,
}

impl DocumentManager {
    pub fn with_samples() -> Self {
        let doc1 = Document::new(
            1,
            "Bienvenida a Typora.md",
            r#"# Positron / Typora — Editor de Markdown con KaTeX

Positron y Typora ofrecen una experiencia fluida tanto para **lectores** como para **autores**, combinando la potencia de un IDE moderno con la elegancia de un editor sin distracciones.

---

## ∑ Soporte Matemático Completo con KaTeX

Ahora puedes escribir y previsualizar fórmulas matemáticas en tiempo real tanto en línea como en bloque con la máxima fidelidad tipográfica:

### 1. Fórmulas en Línea
La famosa equivalencia entre masa y energía de Einstein se expresa como $E = mc^2$.
El teorema de Pitágoras define la relación $a^2 + b^2 = c^2$, y el radio de Euler satisface $e^{i\pi} + 1 = 0$.

### 2. Fórmulas en Bloque (Display Math)

La fórmula cuadrática general para resolver $ax^2 + bx + c = 0$:

$$
x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
$$

La integral gaussiana de Poisson fundamental en cálculo y estadística:

$$
\int_{-\infty}^{\infty} e^{-x^2} \, dx = \sqrt{\pi}
$$

La función de densidad de probabilidad de la distribución normal estándar:

$$
f(x) = \frac{1}{\sigma \sqrt{2\pi}} e^{-\frac{1}{2}\left(\frac{x - \mu}{\sigma}\right)^2}
$$

---

## Características de la Interfaz

- **Selector de Temas**: Cambia al instante entre *Tokyo Night*, *Solarized Dark*, *Solarized Light*, *VS Code Dark+* y *Catppuccin*.
- **Barra de Actividades**: Explorador de archivos, buscador y esquema de títulos interactivo.
- **Editor con Líneas y Vista Dividida**: Visualiza el código fuente con números de línea a la izquierda y la vista previa renderizada a la derecha.

---

### Ejemplo de Código en Rust

```rust
fn main() {
    println!("¡KaTeX funcionando a la perfección en Rust!");
}
```

---

### Tareas del Proyecto

- [x] Motor de fórmulas matemáticas KaTeX integrado
- [x] Renderizado de fracciones, raíces e integrales
- [x] Selector de temas de color (Solarized, Tokyo Night, etc.)
- [ ] Exportación directa a documentos PDF

---

| Notación | Ejemplo KaTeX | Descripción |
| :--- | :--- | :--- |
| En línea | `$E = mc^2$` | Se integra en el flujo del texto |
| Bloque | `$$\int_0^\infty f(x)dx$$` | Centrado con mayor resolución |
"#,
        );

        let doc2 = Document::new(
            2,
            "Fórmulas y Teoremas.md",
            r#"# Cuaderno de Matemáticas y Física

Este cuaderno reúne identidades matemáticas fundamentales renderizadas con KaTeX.

## 1. Identidad de Euler y Series
$$
e^{ix} = \cos(x) + i\sin(x)
$$

Serie de Taylor para la función exponencial:
$$
e^x = \sum_{n=0}^{\infty} \frac{x^n}{n!} = 1 + x + \frac{x^2}{2!} + \frac{x^3}{3!} + \cdots
$$

## 2. Ecuaciones de Maxwell en el Vacío
$$
\nabla \cdot \mathbf{E} = 0
$$
$$
\nabla \times \mathbf{E} = -\frac{\partial \mathbf{B}}{\partial t}
$$

## 3. Matrices y Determinantes
$$
A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}, \quad \det(A) = ad - bc
$$
"#,
        );

        let doc3 = Document::new(
            3,
            "Notas de Proyecto.md",
            r#"# Notas de Proyecto — Rediseño Positron

## Objetivos Cumplidos
1. Integración de motor KaTeX de alta precisión con fuentes matemáticas STIX Two.
2. Soporte para fórmulas en línea `$ ... $` y en bloque `$$ ... $$`.
3. Selector dinámico de temas de colores (Tokyo Night, Solarized, Catppuccin, Gruvbox).

## Tareas
- [x] KaTeX implementado en app/src/math.rs.
- [x] Renderizado nítido adaptado a la paleta de colores activa.
- [x] Botón de acceso rápido para insertar fórmulas matemáticas.
"#,
        );

        let doc4 = Document::new(
            4,
            "Química con mhchem y Ortografía.md",
            r#"# Laboratorio de Química — KaTeX, mhchem y Ortografía

Este documento demuestra la integración del **corrector ortográfico Hunspell** en vivo junto a la renderización de reacciones químicas con **mhchem** de KaTeX.

---

## 1. Corrector Ortográfico Hunspell en Vivo

El corrector Hunspell examina en tiempo real el texto del documento para garantizar una redacción impecable en **Español (es-ES)** e **Inglés (en-US)**.
Las palabras con posibles errores aparecen **subrayadas en rojo** directamente en el editor. Puedes hacer clic derecho sobre cualquier palabra o usar los botones de la barra superior para corregirla al instante con un solo clic:

### Ejemplos interactivos de prueba:
- Esta frase contiene una correcion que Hunspell detecta y subraya en rojo.
- Este texto tiene la palabra huspell que se sugiere cambiar a hunspell.
- Aquí probamos una expresión con typo dividida como corre ión para restaurar su ortografía.
- La heramienta de laboratorio es precisa y de alta resolución.

---

## 2. Reacciones Químicas Fundamentales con mhchem

### Síntesis del Agua
Combustión elemental de hidrógeno y oxígeno gaseoso:

$$
\ce{2 H2 + O2 -> 2 H2O}
$$

### Reacción de Precipitación
Formación de precipitado insoluble de sulfato de bario:

$$
\ce{SO4^2- + Ba^2+ -> BaSO4 v}
$$

### Descomposición Térmica y Liberación de Gas
Descomposición de carbonato de calcio con desprendimiento de dióxido de carbono:

$$
\ce{CaCO3 -> CaO + CO2 ^}
$$

### Fotosíntesis
Conversión de dióxido de carbono y agua en glucosa y oxígeno:

$$
\ce{6 CO2 + 6 H2O -> C6H12O6 + 6 O2}
$$

### Equilibrio Químico (Proceso Haber-Bosch)
Síntesis industrial de amoníaco a partir de nitrógeno e hidrógeno:

$$
\ce{N2 + 3 H2 <=> 2 NH3}
$$

### Hidratos y Complejos
Sulfato de cobre pentahidratado:

$$
\ce{CuSO4 . 5 H2O}
$$

Fórmulas en línea: agua $\ce{H2O}$, ion hidronio $\ce{H3O+}$, metano $\ce{CH4}$ y ácido sulfúrico $\ce{H2SO4}$.
"#,
        );

        let doc5 = Document::new(
            5,
            "Documento en Galego.md",
            r#"# Lingua Galega — Typora con Hunspell GL

Benvido a Typora co soporte completo para a **lingua galega (gl-ES)** e fórmulas KaTeX integradas.

---

## 1. Corrector Ortográfico en Galego

O corrector Hunspell examina en tempo real a ortografía en **Galego (gl-ES)**, **Español (es-ES)** e **Inglés (en-US)**.
As palabras con posibles erros amósanse **subliñadas en vermello** no editor, con menú contextual e botóns de corrección inmediata.

### Exemplos en Galego:
- Palabras correctas: *galego*, *lingua*, *Galicia*, *tamén*, *grazas*, *coidado*, *mañá*, *roxo*.
- Exemplo de proba con falta: o termo *idoma* sinálase en vermello e recoméndase substituír por **idioma**.
- Exemplo con falta tipográfica: *crrección* recoméndase corrixir como **corrección**.
- Fórmulas matemáticas: o teorema de Pitágoras $a^2 + b^2 = c^2$ e a famosa equivalencia $E = mc^2$.
"#,
        );

        let doc6 = Document::new(
            6,
            "Guía para Windows.md",
            r#"# 🪟 Cómo llevar y usar esta aplicación en Windows

Esta aplicación es 100% compatible con **Windows 10 y Windows 11**. Al estar construida en Rust nativo, todos los estilos tipográficos, fórmulas KaTeX y diccionarios Hunspell (Español, Gallego e Inglés) están incluidos dentro del propio programa.

---

## 🚀 Pasos para usarla en otro PC con Windows

### 1. Copiar la carpeta
Copia la carpeta del proyecto a un pendrive USB, disco externo o envíala mediante Google Drive, OneDrive o correo electrónico a tu otro ordenador.

### 2. Ejecutar con un clic (run-windows.bat)
En Windows, abre la carpeta y haz doble clic sobre el archivo **`run-windows.bat`**:
- Si el ordenador no tiene Rust instalado, el script te abrirá automáticamente la web oficial de [rustup.rs](https://rustup.rs) para instalarlo.
- Una vez instalado, compilará y abrirá la ventana de la aplicación de inmediato.

### 3. Crear un archivo .exe portable e independiente (build-windows.bat)
Si quieres un único archivo ejecutable `.exe` para llevar en un pendrive sin código:
- Haz doble clic en **`build-windows.bat`**.
- Creará el archivo **`Tigorra.exe`** en la carpeta `dist-windows/`.
- ¡Ese único archivo `.exe` funciona de forma autónoma en cualquier PC con Windows haciendo doble clic!

---

## ⌨️ Comandos manuales en Windows (CMD / PowerShell)
Para compilar y ejecutar en modo optimizado:
```cmd
cargo run --release
```
Para generar el `.exe`:
```cmd
cargo build --release
```
El archivo ejecutable estará ubicado en `target\release\tigorra.exe`.
"#,
        );

        let doc7 = Document::new(
            7,
            "Guía para Linux.md",
            r#"# 🐧 Cómo llevar y usar esta aplicación en Linux

Esta aplicación es 100% compatible con **Linux** (Ubuntu, Debian, Fedora, Arch, Manjaro, Linux Mint, etc.), tanto en entornos **Wayland** como **X11**. Todos los estilos, fórmulas KaTeX y diccionarios Hunspell (Español, Gallego e Inglés) están integrados dentro del ejecutable sin depender de Electron ni navegadores web.

---

## 🚀 Pasos para usarla en otro PC con Linux

### 1. Copiar la carpeta
Copia la carpeta del proyecto a una memoria USB o descárgala en tu ordenador con Linux.

### 2. Ejecutar con el script automático (run-linux.sh)
Abre la terminal en la carpeta y ejecuta:
```bash
./run-linux.sh
```
- Si tu equipo no tiene Rust instalado, el script lo instalará automáticamente con `rustup`.
- La aplicación compilará y se ejecutará de forma nativa.

### 3. Crear un binario independiente portable (build-linux.sh)
Para generar un binario único que puedas mover a cualquier sitio:
```bash
./build-linux.sh
```
- Creará el ejecutable **`Tigorra`** en la carpeta `dist-linux/`.
- ¡Ese archivo funciona de forma autónoma con solo hacer doble clic o ejecutar `./Tigorra`!

---

## ⌨️ Comandos manuales en Linux
Para compilar y ejecutar en modo optimizado:
```bash
cargo run --release
```
Para generar el binario release:
```bash
cargo build --release
```
El archivo ejecutable estará ubicado en `target/release/tigorra`.
"#,
        );

        let doc8 = Document::new(
            8,
            "Caderno Científico Quarto.qmd",
            r##"---
title: "Caderno Quarto con Python Interactivo"
subtitle: "Execución de celas en tempo real como nos Jupyter Notebooks"
author: "Investigador Quarto"
date: "2026-11-16"
format:
  html:
    toc: true
    code-fold: show
    theme: cosmo
---

# 1. Introdución a Python en Quarto

Este documento funciona como un **caderno científico reproducible** (ao estilo dos **Jupyter Notebooks**).
Podes premer no botón **▶ Executar cela** en cada bloque de código ou no botón **▶ Executar Python** da barra superior para correr o código con Python 3 en tempo real.

---

## 2. Estatística e Cálculos con NumPy

```{python}
#| label: fig-calculo
#| echo: true

import numpy as np

# Xeración de datos de mostra
np.random.seed(42)
datos = np.random.normal(loc=25.0, scale=4.5, size=500)

print(f"Total de mostras analizadas: {len(datos)}")
print(f"Media estimada: {np.mean(datos):.3f}")
print(f"Desviación estándar: {np.std(datos):.3f}")
print(f"Percentil 95%: {np.percentile(datos, 95):.3f}")
```

---

## 3. Xeración de Gráficos con Matplotlib

Ao premer **▶ Executar cela**, Matplotlib xera o gráfico e este móstrase de inmediato debaixo do código como nos Jupyter Notebooks:

```{python}
#| label: fig-grafico-seno
#| fig-cap: "Onda sinusoidal xerada en tempo real con Matplotlib"
#| echo: true

import matplotlib.pyplot as plt
import numpy as np

x = np.linspace(0, 4 * np.pi, 200)
y1 = np.sin(x)
y2 = np.cos(x)

plt.figure(figsize=(7, 3.2), dpi=100)
plt.plot(x, y1, label="Seno: sin(x)", color="#409EFF", linewidth=2.2)
plt.plot(x, y2, label="Coseno: cos(x)", color="#27C93F", linewidth=2.2, linestyle="--")
plt.title("Ondas Harmónicas — Tigorra Quarto", fontsize=12, fontweight="bold")
plt.xlabel("Tempo (t)", fontsize=10)
plt.ylabel("Amplitude", fontsize=10)
plt.grid(True, linestyle=":", alpha=0.6)
plt.legend(loc="upper right")
plt.tight_layout()
plt.show()
```

---

## 4. Fórmulas Matemáticas con KaTeX e Ecuacións Químicas con mhchem

As ecuacións teóricas relaciónanse cos cálculos numéricos anteriores:

$$
y(t) = A \sin(\omega t + \phi) = A \sin(2\pi f t + \phi)
$$

E a función de densidade gaussiana:

$$
f(x) = \frac{1}{\sigma \sqrt{2\pi}} \exp\left( -\frac{(x - \mu)^2}{2\sigma^2} \right)
$$

Reaccións químicas fundamentais con mhchem ($\ce{H2O}$ e fotosíntese):

$$
\ce{6 CO2 + 6 H2O -> C6H12O6 + 6 O2}
$$

E a combustión completa do metano:

$$
\ce{CH4 + 2 O2 -> CO2 + 2 H2O}
$$

---

## 5. Caixas de Aviso (Callouts de Quarto)

::: {.callout-tip}
### Execución Interactiva como en Jupyter
Podes premer en calquera momento o botón **▶ Executar cela** na cabeceira de cada bloque ou usar **▶ Executar Python** para correr todas as celas á vez.
:::

::: {.callout-note}
### Renderizado e Exportación
O botón **Quarto Render** na barra superior compila o informe HTML completo incrustando os gráficos de Matplotlib e as saídas das túas celas.
:::
"##,
        );

        Self {
            documents: vec![doc1, doc2, doc3, doc4, doc5, doc6, doc7, doc8],
            active_index: 7,
            next_id: 9,
        }
    }

    /// Starts with a single empty scratch document (used when a workspace is
    /// available, so the demo tabs do not bury the user's real files).
    pub fn with_blank_document() -> Self {
        let doc = Document::new(
            1,
            "Sin título.md",
            "# Nuevo documento\n\nEscribe **Markdown** o fórmulas como $E = mc^2$.\n",
        );
        Self {
            documents: vec![doc],
            active_index: 0,
            next_id: 2,
        }
    }

    pub fn active(&self) -> &Document {
        &self.documents[self.active_index]
    }

    pub fn active_mut(&mut self) -> &mut Document {
        &mut self.documents[self.active_index]
    }

    pub fn add_new(&mut self) {
        let title = format!("Sin título {}.md", self.next_id);
        let doc = Document::new(self.next_id, title, "# Nuevo Documento\n\nEscribe texto y fórmulas matemáticas como $E = mc^2$...\n");
        self.documents.push(doc);
        self.active_index = self.documents.len() - 1;
        self.next_id += 1;
    }

    pub fn add_new_quarto(&mut self) {
        let title = format!("Documento {}.qmd", self.next_id);
        let content = r#"---
title: "Novo Documento Quarto"
author: "Autor"
date: "today"
format:
  html:
    toc: true
    code-fold: show
---

# Introdución a Quarto

Escribe aquí o contido do teu documento Quarto (.qmd).

## 📊 Bloque de Código Executable

```{python}
#| label: fig-exemplo
#| echo: true
print("Execución de código con Quarto")
```

## 💡 Caixa de Aviso

::: {.callout-note}
### Nota
Documento reproducible xerado con Tigorra.
:::
"#;
        let doc = Document::new(self.next_id, title, content);
        self.documents.push(doc);
        self.active_index = self.documents.len() - 1;
        self.next_id += 1;
    }

    /// Opens a document from disk. If it is already open, its tab is focused
    /// instead of duplicating it (and the caller's unsaved edits are kept).
    pub fn open_path(&mut self, path: &Path) -> Result<usize, String> {
        let normalized = crate::files::normalize_path(path);
        if let Some(idx) = self
            .documents
            .iter()
            .position(|d| d.file_path.as_deref() == Some(normalized.as_path()))
        {
            self.active_index = idx;
            return Ok(idx);
        }
        let id = self.next_id;
        let doc = Document::from_file(id, &normalized)?;
        self.next_id += 1;

        // A pristine scratch tab is replaced instead of left behind, so opening
        // a file at launch does not leave an empty "Sin título" tab.
        let replace_pristine = self.documents.len() == 1
            && self.documents[0].file_path.is_none()
            && !self.documents[0].is_dirty();
        if replace_pristine {
            self.documents[0] = doc;
            self.active_index = 0;
            return Ok(0);
        }

        self.documents.push(doc);
        self.active_index = self.documents.len() - 1;
        Ok(self.active_index)
    }

    pub fn select(&mut self, index: usize) {
        if index < self.documents.len() {
            self.active_index = index;
        }
    }

    /// Removes a tab. Always keeps at least one document open.
    pub fn remove(&mut self, index: usize) {
        if self.documents.len() > 1 && index < self.documents.len() {
            self.documents.remove(index);
            if self.active_index >= self.documents.len() {
                self.active_index = self.documents.len() - 1;
            }
        }
    }

    pub fn dirty_indices(&self) -> Vec<usize> {
        self.documents
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_dirty())
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spellcheck::{detect_language, SpellLanguage};

    #[test]
    fn sample_documents_get_the_right_dictionary() {
        let manager = DocumentManager::with_samples();
        let galician = manager
            .documents
            .iter()
            .find(|d| d.title.contains("Caderno"))
            .expect("quarto sample");
        assert_eq!(
            detect_language(&galician.content, &galician.title),
            SpellLanguage::Galician,
            "the Quarto sample is written in Galician"
        );

        let spanish = manager
            .documents
            .iter()
            .find(|d| d.title.contains("Bienvenida"))
            .expect("welcome sample");
        assert_eq!(
            detect_language(&spanish.content, &spanish.title),
            SpellLanguage::Spanish,
            "the welcome sample is written in Spanish"
        );
    }

    #[test]
    fn dirty_scratch_tabs_are_never_silently_replaced() {
        let dir = std::env::temp_dir().join(format!("tigorra_docs_dirty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nota.md");
        std::fs::write(&path, "# Nota\n").unwrap();

        let mut manager = DocumentManager::with_blank_document();
        manager.active_mut().content.push_str("cambios sin guardar\n");
        assert!(manager.active().is_dirty());
        manager.open_path(&path).unwrap();
        assert_eq!(manager.documents.len(), 2, "the edited scratch tab is kept");
        assert_eq!(manager.active().content, "# Nota\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarto_detection_prefers_extension_over_frontmatter() {
        let mut manager = DocumentManager::with_blank_document();
        let doc = manager.active_mut();
        doc.content = "---\ntitle: Blog\n---\n\nTexto\n".to_string();
        doc.title = "post.md".to_string();
        assert!(!doc.is_quarto(), "plain Markdown with YAML is not Quarto");

        doc.title = "informe.qmd".to_string();
        assert!(doc.is_quarto());

        doc.title = "informe.md".to_string();
        doc.content = "---\ntitle: X\nquarto: true\n---\n".to_string();
        assert!(doc.is_quarto(), "frontmatter that mentions quarto is Quarto");
    }

    #[test]
    fn dirty_state_is_derived_from_the_saved_snapshot() {
        let mut doc = Document::new(1, "a.md", "# hola\n");
        assert!(!doc.is_dirty());
        doc.content.push_str("más texto\n");
        assert!(doc.is_dirty());
        doc.mark_saved(std::path::PathBuf::from("/tmp/a.md"));
        assert!(!doc.is_dirty());
        doc.revert();
        assert_eq!(doc.content, "# hola\nmás texto\n");
    }

    #[test]
    fn open_path_reuses_an_existing_tab() {
        let dir = std::env::temp_dir().join(format!("tigorra_docs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nota.md");
        std::fs::write(&path, "# Nota\n").unwrap();

        let mut manager = DocumentManager::with_blank_document();
        let first = manager.open_path(&path).unwrap();
        // A pristine scratch tab is replaced by the opened file.
        assert_eq!(manager.documents.len(), 1);
        let second = manager.open_path(&path).unwrap();
        assert_eq!(first, second);
        assert_eq!(manager.documents.len(), 1);
        assert_eq!(manager.active().content, "# Nota\n");
        assert_eq!(manager.active().title, "nota.md");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
