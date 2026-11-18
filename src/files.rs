//! Real filesystem access for the editor: reading/writing documents,
//! workspace listing, recent-file persistence and an in-app file picker.
//!
//! Everything here is dependency-free (std only) so the app keeps building
//! on macOS, Linux and Windows without native toolkit requirements.

use std::path::{Path, PathBuf};

pub const MARKDOWN_EXTENSIONS: &[&str] = &["md", "markdown", "mdown", "qmd", "txt"];

const MAX_OPEN_BYTES: u64 = 32 * 1024 * 1024;

/// Reads a UTF-8 text document, tolerating a UTF-8 BOM and CRLF line endings
/// (which are normalised to `\n` so the editor and spellchecker stay consistent).
pub fn read_text(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("No se pudo abrir {}: {}", path.display(), e))?;
    if meta.len() > MAX_OPEN_BYTES {
        return Err(format!(
            "El archivo {} es demasiado grande ({:.1} MB, límite {} MB)",
            path.display(),
            meta.len() as f64 / (1024.0 * 1024.0),
            MAX_OPEN_BYTES / (1024 * 1024)
        ));
    }
    if meta.is_dir() {
        return Err(format!("{} es una carpeta, no un archivo", path.display()));
    }

    let bytes = std::fs::read(path).map_err(|e| format!("No se pudo leer {}: {}", path.display(), e))?;

    // UTF-16 BOMs are common for files saved by Notepad/Word; decode them too.
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(decode_utf16(&bytes[2..], true).replace("\r\n", "\n"));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(decode_utf16(&bytes[2..], false).replace("\r\n", "\n"));
    }

    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Writes through a temporary file + rename so a crash or full disk never
/// truncates the user's document (the old content survives until the rename).
pub fn write_text_atomic(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("No se pudo crear la carpeta {}: {}", parent.display(), e))?;
        }
    }

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".to_string());
    let tmp = path.with_file_name(format!(".{}.tmp-{}", file_name, std::process::id()));

    std::fs::write(&tmp, content.as_bytes())
        .map_err(|e| format!("No se pudo escribir {}: {}", tmp.display(), e))?;

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(first_error) => {
            // Windows refuses to rename over an existing file. Remove the
            // destination and retry once; POSIX platforms never reach here.
            let retry = std::fs::remove_file(path).and_then(|_| std::fs::rename(&tmp, path));
            match retry {
                Ok(()) => Ok(()),
                Err(_) => {
                    let _ = std::fs::remove_file(&tmp);
                    Err(format!("No se pudo guardar {}: {}", path.display(), first_error))
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

impl WorkspaceEntry {
    pub fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| MARKDOWN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
    }
}

pub fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MARKDOWN_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.') || name == "target" || name == "node_modules"
}

/// Lists a directory for the explorer: sub-folders first, then Markdown/Quarto
/// documents, alphabetically and case-insensitively.
pub fn scan_dir(dir: &Path) -> Vec<WorkspaceEntry> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_hidden(&name) {
            continue;
        }
        let path = entry.path();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir && !is_markdown_path(&path) {
            continue;
        }
        out.push(WorkspaceEntry { path, name, is_dir });
    }
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return Some(PathBuf::from(appdata).join("Tigorra"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = home_dir() {
        return Some(home.join("Library/Application Support/Tigorra"));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("tigorra"));
    }
    home_dir().map(|h| h.join(".config/tigorra"))
}

/// Recursively lists Markdown/Quarto documents under `dir` for quick-open.
/// Hidden folders, build outputs and dependency trees are skipped.
pub fn scan_tree(dir: &Path, max_entries: usize, max_depth: usize) -> Vec<WorkspaceEntry> {
    fn walk(dir: &Path, out: &mut Vec<WorkspaceEntry>, max: usize, depth: usize, max_depth: usize) {
        if depth > max_depth || out.len() >= max {
            return;
        }
        for entry in scan_dir(dir) {
            if out.len() >= max {
                return;
            }
            if entry.is_dir {
                walk(&entry.path, out, max, depth + 1, max_depth);
            } else {
                out.push(entry);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out, max_entries, 0, max_depth);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Filters quick-open entries. Exact filename prefix matches rank first, then
/// filename substrings, then path matches. Empty query lists everything.
pub fn filter_by_query<'a>(entries: &'a [WorkspaceEntry], query: &str) -> Vec<&'a WorkspaceEntry> {
    let q = query.trim().to_lowercase();
    let mut matches: Vec<(u8, &WorkspaceEntry)> = entries
        .iter()
        .filter_map(|entry| {
            if q.is_empty() {
                return Some((3, entry));
            }
            let name = entry.name.to_lowercase();
            let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(&name);
            let path = entry.path.to_string_lossy().to_lowercase();
            if name == q || stem == q {
                Some((0, entry))
            } else if name.starts_with(&q) {
                Some((1, entry))
            } else if name.contains(&q) {
                Some((2, entry))
            } else if path.contains(&q) {
                Some((3, entry))
            } else {
                None
            }
        })
        .collect();
    matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    matches.into_iter().map(|(_, e)| e).collect()
}

/// Where documents should be looked for on first launch (the folder the binary
/// was started from, when it actually contains Markdown).
pub fn default_workspace() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        if !scan_dir(&cwd).is_empty() {
            return cwd;
        }
    }
    home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn prefs_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("prefs.txt"))
}

#[derive(Clone, Debug, Default)]
pub struct Prefs {
    pub recent_files: Vec<PathBuf>,
    pub workspace: Option<PathBuf>,
    pub last_theme: Option<String>,
}

impl Prefs {
    pub fn load() -> Self {
        let mut prefs = Prefs::default();
        let Some(path) = prefs_path() else { return prefs };
        let Ok(text) = std::fs::read_to_string(path) else {
            return prefs;
        };
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("recent=") {
                if prefs.recent_files.len() < 12 {
                    prefs.recent_files.push(PathBuf::from(rest));
                }
            } else if let Some(rest) = line.strip_prefix("workspace=") {
                prefs.workspace = Some(PathBuf::from(rest));
            } else if let Some(rest) = line.strip_prefix("theme=") {
                prefs.last_theme = Some(rest.to_string());
            }
        }
        prefs.recent_files.retain(|p| p.exists());
        if let Some(ref ws) = prefs.workspace {
            if !ws.is_dir() {
                prefs.workspace = None;
            }
        }
        prefs
    }

    pub fn save(&self) {
        let Some(path) = prefs_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut out = String::new();
        for f in self.recent_files.iter().take(12) {
            out.push_str(&format!("recent={}\n", f.display()));
        }
        if let Some(ref ws) = self.workspace {
            out.push_str(&format!("workspace={}\n", ws.display()));
        }
        if let Some(ref t) = self.last_theme {
            out.push_str(&format!("theme={}\n", t));
        }
        let _ = write_text_atomic(&path, &out);
    }

    pub fn remember_file(&mut self, path: &Path) {
        let path = normalize_path(path);
        self.recent_files.retain(|p| *p != path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(12);
    }
}

pub fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

pub fn file_title(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Sin título.md".to_string())
}

/* ---------------------------------------------------------------- picker */

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerMode {
    OpenFile,
    OpenFolder,
    SaveFile,
}

#[derive(Clone, Debug)]
pub enum PickerOutcome {
    Chosen(PathBuf),
    Cancelled,
}

pub struct FilePicker {
    pub visible: bool,
    pub mode: PickerMode,
    pub dir: PathBuf,
    pub filename: String,
    pub show_all: bool,
    pub entries: Vec<WorkspaceEntry>,
    pub error: Option<String>,
    pub overwrite_confirmed: bool,
    scroll_to_top: bool,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePicker {
    pub fn new() -> Self {
        let dir = default_workspace();
        let entries = scan_dir(&dir);
        Self {
            visible: false,
            mode: PickerMode::OpenFile,
            dir,
            filename: String::new(),
            show_all: false,
            entries,
            error: None,
            overwrite_confirmed: false,
            scroll_to_top: false,
        }
    }

    pub fn open(&mut self, mode: PickerMode, start_dir: Option<&Path>, suggested_name: &str) {
        if let Some(dir) = start_dir {
            self.dir = if dir.is_dir() {
                dir.to_path_buf()
            } else {
                dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(default_workspace)
            };
        }
        self.mode = mode;
        self.visible = true;
        self.error = None;
        self.overwrite_confirmed = false;
        self.filename = if mode == PickerMode::SaveFile {
            suggested_name.to_string()
        } else {
            String::new()
        };
        self.refresh();
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.error = None;
        self.overwrite_confirmed = false;
    }

    pub fn refresh(&mut self) {
        self.entries = scan_dir(&self.dir);
        self.entries.retain(|e| {
            e.is_dir
                || self.show_all
                || e.path
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| MARKDOWN_EXTENSIONS.contains(&x.to_ascii_lowercase().as_str()))
                    .unwrap_or(false)
        });
        self.scroll_to_top = true;
    }

    pub fn navigate(&mut self, dir: PathBuf) {
        if dir.is_dir() {
            self.dir = dir;
            self.error = None;
            self.overwrite_confirmed = false;
            self.refresh();
        } else {
            self.error = Some(format!("No existe la carpeta {}", dir.display()));
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.dir.parent() {
            self.navigate(parent.to_path_buf());
        }
    }

    fn target(&self) -> Result<PathBuf, String> {
        if self.filename.trim().is_empty() {
            return Err("Escribe un nombre de archivo".to_string());
        }
        let name = self.filename.trim();
        let candidate = if Path::new(name).is_absolute() {
            PathBuf::from(name)
        } else {
            self.dir.join(name)
        };
        Ok(candidate)
    }

    pub fn show(&mut self, ctx: &egui::Context, theme: &crate::theme::ColorPalette) -> Option<PickerOutcome> {
        if !self.visible {
            return None;
        }
        let mut outcome = None;
        let title = match self.mode {
            PickerMode::OpenFile => "📂 Abrir archivo Markdown / Quarto",
            PickerMode::OpenFolder => "📁 Abrir carpeta de trabajo",
            PickerMode::SaveFile => "💾 Guardar documento como…",
        };
        let mut is_open = true;
        egui::Window::new(title)
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_size([620.0, 460.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(theme.sidebar_bg)
                    .stroke(egui::Stroke::new(1.5_f32, theme.accent))
                    .inner_margin(egui::Margin::same(14.0_f32)),
            )
            .show(ctx, |ui| {
                // Path bar -------------------------------------------------
                ui.horizontal(|ui| {
                    if ui.button("⬆").on_hover_text("Carpeta superior").clicked() {
                        self.go_up();
                    }
                    if ui.button("🏠").on_hover_text("Carpeta personal").clicked() {
                        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
                            self.navigate(PathBuf::from(home));
                        }
                    }
                    let mut dir_text = self.dir.display().to_string();
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut dir_text)
                            .desired_width(ui.available_width() - 60.0)
                            .font(egui::TextStyle::Monospace),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.navigate(PathBuf::from(dir_text.trim()));
                    }
                    if ui.button("Ir").clicked() {
                        self.navigate(PathBuf::from(dir_text.trim()));
                    }
                });

                if let Some(err) = &self.error {
                    ui.colored_label(egui::Color32::from_rgb(0xFF, 0x5F, 0x56), format!("⚠ {}", err));
                }

                ui.add_space(4.0);
                ui.separator();

                // Entries --------------------------------------------------
                let list_height = (ui.available_height() - 108.0).max(160.0);
                egui::ScrollArea::vertical()
                    .id_salt("file_picker_scroll")
                    .auto_shrink([false, false])
                    .max_height(list_height)
                    .show(ui, |ui| {
                        if self.scroll_to_top {
                            ui.scroll_to_rect(ui.min_rect(), Some(egui::Align::TOP));
                            self.scroll_to_top = false;
                        }
                        if self.mode != PickerMode::OpenFolder && self.entries.is_empty() {
                            ui.label(
                                egui::RichText::new("(carpeta vacía: no hay documentos .md/.qmd)")
                                    .italics()
                                    .color(theme.text_muted),
                            );
                        }
                        let mut clicked: Option<WorkspaceEntry> = None;
                        for entry in &self.entries {
                            let icon = if entry.is_dir { "📁" } else { "📄" };
                            let selected = !entry.is_dir
                                && !self.filename.trim().is_empty()
                                && self.dir.join(self.filename.trim()) == entry.path;
                            let label = format!("{} {}", icon, entry.name);
                            let mut button = egui::Button::new(
                                egui::RichText::new(label)
                                    .size(12.5)
                                    .color(if selected { theme.accent } else { theme.text_body }),
                            )
                            .fill(if selected { theme.selection_bg } else { egui::Color32::TRANSPARENT })
                            .min_size(egui::Vec2::new(ui.available_width(), 22.0));
                            if selected {
                                button = button.stroke(egui::Stroke::new(1.0_f32, theme.accent));
                            }
                            if ui.add(button).clicked() {
                                clicked = Some(entry.clone());
                            }
                        }
                        if let Some(entry) = clicked {
                            if entry.is_dir {
                                self.navigate(entry.path);
                            } else if self.mode == PickerMode::OpenFile {
                                outcome = Some(PickerOutcome::Chosen(entry.path));
                            } else {
                                self.filename = entry.name.clone();
                            }
                        }
                    });

                ui.separator();
                ui.add_space(4.0);

                // Footer: name field (save) / selection (open) -------------
                match self.mode {
                    PickerMode::SaveFile => {
                        ui.horizontal(|ui| {
                            ui.label("Nombre:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.filename)
                                    .desired_width(ui.available_width() - 120.0),
                            );
                            if ui.button("Guardar").clicked() {
                                match self.target() {
                                    Ok(path) => {
                                        let final_path = with_default_extension(&path, &self.filename);
                                        if final_path.exists() && !self.overwrite_confirmed {
                                            self.overwrite_confirmed = true;
                                            self.error = Some(format!(
                                                "{} ya existe. Pulsa «Sobrescribir» para confirmar.",
                                                final_path.display()
                                            ));
                                        } else {
                                            outcome = Some(PickerOutcome::Chosen(final_path));
                                        }
                                    }
                                    Err(e) => self.error = Some(e),
                                }
                            }
                            if ui.button("Cancelar").clicked() {
                                outcome = Some(PickerOutcome::Cancelled);
                            }
                        });
                        if self.overwrite_confirmed {
                            if ui.button("Sobrescribir").clicked() {
                                if let Ok(path) = self.target() {
                                    outcome = Some(PickerOutcome::Chosen(with_default_extension(&path, &self.filename)));
                                }
                            }
                        }
                    }
                    PickerMode::OpenFile => {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Haz clic en un documento para abrirlo")
                                    .size(11.5)
                                    .color(theme.text_muted),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Cancelar").clicked() {
                                    outcome = Some(PickerOutcome::Cancelled);
                                }
                                if ui.checkbox(&mut self.show_all, "Mostrar todos los archivos").changed() {
                                    self.refresh();
                                }
                            });
                        });
                    }
                    PickerMode::OpenFolder => {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("Carpeta actual: {}", self.dir.display()))
                                    .size(11.5)
                                    .color(theme.text_muted),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Cancelar").clicked() {
                                    outcome = Some(PickerOutcome::Cancelled);
                                }
                                if ui.button("Abrir esta carpeta").clicked() {
                                    outcome = Some(PickerOutcome::Chosen(self.dir.clone()));
                                }
                            });
                        });
                    }
                }
            });

        if !is_open || matches!(outcome, Some(PickerOutcome::Cancelled)) {
            self.close();
            return None;
        }
        if outcome.is_some() {
            self.close();
        }
        outcome
    }
}

fn with_default_extension(path: &Path, typed: &str) -> PathBuf {
    if path.extension().is_some() || typed.contains('.') {
        path.to_path_buf()
    } else {
        path.with_extension("md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tigorra_test_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_and_normalises_line_endings() {
        let dir = tmp_dir("read");
        let path = dir.join("a.md");
        std::fs::write(&path, b"\xef\xbb\xbf# Hola\r\nmundo\r\n").unwrap();
        let text = read_text(&path).unwrap();
        assert_eq!(text, "# Hola\nmundo\n");
    }

    #[test]
    fn decodes_utf16_files() {
        let dir = tmp_dir("utf16");
        let path = dir.join("b.md");
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "# Título".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_text(&path).unwrap(), "# Título");
    }

    #[test]
    fn atomic_write_creates_parents() {
        let dir = tmp_dir("write");
        let path = dir.join("sub/dir/doc.md");
        write_text_atomic(&path, "# Contenido\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Contenido\n");
        // no temporary files left behind
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn scan_dir_sorts_folders_first_and_filters() {
        let dir = tmp_dir("scan");
        std::fs::create_dir_all(dir.join("notas")).unwrap();
        std::fs::write(dir.join("z.md"), "# z").unwrap();
        std::fs::write(dir.join("a.qmd"), "---").unwrap();
        std::fs::write(dir.join("image.png"), "x").unwrap();
        std::fs::write(dir.join(".hidden.md"), "x").unwrap();

        let entries = scan_dir(&dir);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["notas", "a.qmd", "z.md"]);
    }

    #[test]
    fn prefs_recent_files_are_unique_and_ordered() {
        let mut prefs = Prefs::default();
        prefs.remember_file(Path::new("/tmp/a.md"));
        prefs.remember_file(Path::new("/tmp/b.md"));
        prefs.remember_file(Path::new("/tmp/a.md"));
        let names: Vec<_> = prefs
            .recent_files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.md", "b.md"]);
    }

    #[test]
    fn quick_open_filters_and_ranks() {
        let entries = vec![
            WorkspaceEntry {
                path: PathBuf::from("/w/notas/acta.md"),
                name: "acta.md".to_string(),
                is_dir: false,
            },
            WorkspaceEntry {
                path: PathBuf::from("/w/notas.md"),
                name: "notas.md".to_string(),
                is_dir: false,
            },
            WorkspaceEntry {
                path: PathBuf::from("/w/archivo/notas-viejas.qmd"),
                name: "notas-viejas.qmd".to_string(),
                is_dir: false,
            },
        ];
        let ranked = filter_by_query(&entries, "notas");
        let names: Vec<&str> = ranked.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["notas.md", "notas-viejas.qmd", "acta.md"]);
        assert_eq!(filter_by_query(&entries, "ZZZ").len(), 0);
        assert_eq!(filter_by_query(&entries, "").len(), 3);
    }

    #[test]
    fn scan_tree_respects_depth_and_limit() {
        let dir = tmp_dir("tree");
        std::fs::create_dir_all(dir.join("a/b/c")).unwrap();
        std::fs::write(dir.join("root.md"), "#").unwrap();
        std::fs::write(dir.join("a/one.md"), "#").unwrap();
        std::fs::write(dir.join("a/b/two.qmd"), "---").unwrap();
        std::fs::write(dir.join("a/b/c/three.md"), "#").unwrap();

        let depth_one = scan_tree(&dir, 100, 1);
        let names: Vec<_> = depth_one.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"root.md"));
        assert!(names.contains(&"one.md"));
        assert!(!names.contains(&"two.qmd"));
        assert_eq!(scan_tree(&dir, 2, 5).len(), 2);
    }

    #[test]
    fn markdown_detection_handles_case() {
        assert!(is_markdown_path(Path::new("Notas.MD")));
        assert!(is_markdown_path(Path::new("informe.qmd")));
        assert!(!is_markdown_path(Path::new("informe.pdf")));
    }
}
