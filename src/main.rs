mod documents;
mod export;
mod files;
mod markdown;
mod math;
mod quarto;
mod spellcheck;
mod theme;

use std::path::{Path, PathBuf};
use egui::{
    Align, Button, Color32, FontId, Frame, Layout, Margin, RichText, Rounding, ScrollArea,
    Stroke, TextEdit, Ui, Vec2,
};
use documents::DocumentManager;
use export::{clean_filename, export_to_html, export_to_pdf, TyporaStylesheet};
use files::{FilePicker, PickerMode, PickerOutcome, Prefs};
use math::MathRenderer;
use quarto::{has_quarto_cli, PythonKernel, QuartoFormat};
use spellcheck::{HunspellManager, SpellLanguage};
use theme::{ColorPalette, ThemeId};

#[derive(PartialEq, Clone, Copy, Debug)]
enum ActivityTab {
    Explorer,
    Search,
    Outline,
    SpellCheck,
    Quarto,
    Themes,
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum ViewMode {
    Live,
    Source,
    Split,
}

#[derive(Default, Clone)]
struct LaunchOptions {
    screenshot: Option<PathBuf>,
    open_file: Option<PathBuf>,
    folder: Option<PathBuf>,
    /// Force a blank document instead of the built-in demo documents.
    blank: bool,
    /// Initial view mode: source, split or preview.
    view: Option<ViewMode>,
}

impl LaunchOptions {
    /// Supports `tigorra notes.md`, `--folder <dir>`, `--screenshot <png>`
    /// and a plain path argument (`tigorra ~/notas`).
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut opts = LaunchOptions::default();

        let mut i = 1;
        while i < args.len() {
            let arg = args[i].as_str();
            if arg == "--screenshot" && i + 1 < args.len() {
                opts.screenshot = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            } else if let Some(p) = arg.strip_prefix("--screenshot=") {
                opts.screenshot = Some(PathBuf::from(p));
                i += 1;
            } else if arg == "--folder" && i + 1 < args.len() {
                opts.folder = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            } else if let Some(p) = arg.strip_prefix("--folder=") {
                opts.folder = Some(PathBuf::from(p));
                i += 1;
            } else if arg == "--new" {
                opts.blank = true;
                i += 1;
            } else if arg == "--view" && i + 1 < args.len() {
                opts.view = match args[i + 1].to_lowercase().as_str() {
                    "src" | "source" | "codigo" => Some(ViewMode::Source),
                    "preview" | "live" | "previa" => Some(ViewMode::Live),
                    "split" | "combinada" => Some(ViewMode::Split),
                    other => {
                        eprintln!("Modo de vista descoñecido: {} (usa source|split|preview)", other);
                        None
                    }
                };
                i += 2;
            } else if arg == "--help" || arg == "-h" {
                println!(
                    "Tigorra\n\nUso: tigorra [ARCHIVO.md|ARCHIVO.qmd] [opciones]\n\nOpciones:\n  --folder <dir>       Abre una carpeta como espacio de trabajo\n  --new                Empieza con un documento vacío\n  --view <modo>        Vista inicial: source | split | preview\n  --screenshot <png>   Captura la ventana y sale (pruebas)\n  -h, --help           Muestra esta ayuda"
                );
                std::process::exit(0);
            } else if !arg.starts_with('-') {
                opts.open_file = Some(PathBuf::from(arg));
                i += 1;
            } else {
                eprintln!("Opción desconocida: {} (usa --help)", arg);
                i += 1;
            }
        }
        opts
    }
}

fn main() {
    let launch = LaunchOptions::parse();
    let is_screenshot = launch.screenshot.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1220.0, 820.0])
        .with_min_inner_size([800.0, 540.0])
        .with_title("Tigorra — Editor de Markdown y Quarto");

    if is_screenshot {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::viewport::WindowLevel::AlwaysOnBottom);
        }
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Tigorra — Editor de Markdown y Quarto",
        native_options,
        Box::new(move |cc| {
            let prefs = Prefs::load();
            let theme_id = prefs
                .last_theme
                .as_deref()
                .and_then(ThemeId::from_name)
                .unwrap_or(ThemeId::TokyoNight);
            theme::install_fonts(&cc.egui_ctx);
            let initial_palette = ColorPalette::get(theme_id);
            initial_palette.apply(&cc.egui_ctx);
            Ok(Box::new(TigorraApp::new(launch, initial_palette, prefs)))
        }),
    );
}

/// A destructive action that needs a "save first?" confirmation.
#[derive(Clone, Debug)]
enum PendingAction {
    CloseTab(usize),
    OpenFile(PathBuf),
    Quit,
}

struct ConfirmState {
    message: String,
    action: PendingAction,
}

struct TigorraApp {
    doc_manager: DocumentManager,
    math_renderer: MathRenderer,
    preview_assets: markdown::PreviewAssets,
    current_theme: ColorPalette,
    current_stylesheet: TyporaStylesheet,
    activity_tab: ActivityTab,
    sidebar_visible: bool,
    view_mode: ViewMode,

    search_query: String,
    replace_query: String,
    search_case_sensitive: bool,

    status_notification: Option<(String, f64)>,

    // Files / workspace
    prefs: Prefs,
    workspace_root: Option<PathBuf>,
    workspace_entries: Vec<files::WorkspaceEntry>,
    picker: FilePicker,
    save_as_index: usize,
    pending_confirm: Option<ConfirmState>,
    after_save_action: Option<PendingAction>,
    quick_open: bool,
    quick_query: String,
    quick_entries: Vec<files::WorkspaceEntry>,
    quick_selected: usize,
    quick_focus: bool,
    quit_requested: bool,
    shortcuts_open: bool,

    // Typora Export
    export_dialog_open: bool,
    selected_export_style: TyporaStylesheet,
    export_format_pdf: bool,

    // Hunspell Spellchecker
    hunspell: HunspellManager,
    spell_highlight_enabled: bool,

    // Quarto / Python Jupyter Kernel
    quarto_kernel: PythonKernel,

    // Editor state
    editor_id: egui::Id,
    /// Character range (start, end) of the current editor selection.
    editor_selection: Option<(usize, usize)>,
    pending_caret: Option<usize>,
    word_wrap: bool,
    scroll_preview_to_line: Option<usize>,

    screenshot_path: Option<PathBuf>,
    frame_count: u32,
    screenshot_requested: bool,
    /// One-shot hook used by the screenshot harness (`TIPPORA_UI_TEST`).
    test_hook_done: bool,
    test_hook_started: Option<f64>,
}

impl TigorraApp {
    fn new(launch: LaunchOptions, theme: ColorPalette, prefs: Prefs) -> Self {
        let mut hunspell = HunspellManager::new();
        hunspell.set_language(SpellLanguage::Spanish);

        let has_workspace = launch.folder.is_some() || launch.open_file.is_some() || launch.blank;
        let doc_manager = if has_workspace {
            DocumentManager::with_blank_document()
        } else {
            DocumentManager::with_samples()
        };

        let workspace_root = launch
            .folder
            .clone()
            .or_else(|| prefs.workspace.clone())
            .or_else(|| Some(files::default_workspace()));
        let workspace_entries = workspace_root
            .as_deref()
            .map(files::scan_dir)
            .unwrap_or_default();

        let mut app = Self {
            doc_manager,
            math_renderer: MathRenderer::new(),
            preview_assets: markdown::PreviewAssets::new(),
            current_theme: theme,
            current_stylesheet: TyporaStylesheet::GitHub,
            activity_tab: ActivityTab::Explorer,
            sidebar_visible: true,
            view_mode: launch.view.unwrap_or(ViewMode::Split),
            search_query: String::new(),
            replace_query: String::new(),
            search_case_sensitive: false,
            status_notification: None,

            prefs,
            workspace_root,
            workspace_entries,
            picker: FilePicker::new(),
            save_as_index: 0,
            pending_confirm: None,
            after_save_action: None,
            quick_open: false,
            quick_query: String::new(),
            quick_entries: Vec::new(),
            quick_selected: 0,
            quick_focus: false,
            quit_requested: false,
            shortcuts_open: false,

            export_dialog_open: false,
            selected_export_style: TyporaStylesheet::GitHub,
            export_format_pdf: true,

            hunspell,
            spell_highlight_enabled: true,

            quarto_kernel: PythonKernel::new(),

            editor_id: egui::Id::new("main_markdown_editor"),
            editor_selection: None,
            pending_caret: None,
            word_wrap: false,
            scroll_preview_to_line: None,

            screenshot_path: launch.screenshot.clone(),
            frame_count: 0,
            screenshot_requested: false,
            test_hook_done: false,
            test_hook_started: None,
        };

        app.sync_spellcheck_language();

        if let Some(path) = launch.open_file.clone() {
            match app.doc_manager.open_path(&path) {
                Ok(_) => {
                    app.prefs.remember_file(&files::normalize_path(&path));
                    app.prefs.save();
                    app.status_notification = Some((
                        format!("Abierto {}", files::file_title(&path)),
                        0.0,
                    ));
                }
                Err(e) => {
                    app.status_notification = Some((format!("No se pudo abrir: {}", e), 0.0));
                }
            }
        }

        app.sync_spellcheck_language();
        app
    }

    /* ------------------------------------------------------- file actions */

    fn notify_error(&mut self, msg: impl Into<String>, ctx: &egui::Context) {
        let msg = msg.into();
        eprintln!("[error] {}", msg);
        self.notify(msg, ctx);
    }

    fn reload_workspace(&mut self) {
        self.workspace_entries = self
            .workspace_root
            .as_deref()
            .map(files::scan_dir)
            .unwrap_or_default();
    }

    fn open_file_path(&mut self, path: &Path, ctx: &egui::Context) {
        let path = files::normalize_path(path);
        if let Some(idx) = self
            .doc_manager
            .documents
            .iter()
            .position(|d| d.file_path.as_deref() == Some(path.as_path()))
        {
            self.select_document(idx);
            return;
        }

        // Do not discard unsaved work in a pristine scratch tab.
        if let Some(active) = self.doc_manager.documents.get(self.doc_manager.active_index) {
            if active.is_dirty() && active.file_path.is_none() {
                self.pending_confirm = Some(ConfirmState {
                    message: format!(
                        "«{}» tiene cambios sin guardar. ¿Guardarlos antes de abrir {}?",
                        active.title,
                        files::file_title(&path)
                    ),
                    action: PendingAction::OpenFile(path),
                });
                return;
            }
        }

        match self.doc_manager.open_path(&path) {
            Ok(_) => {
                self.prefs.remember_file(&path);
                self.prefs.save();
                self.reload_workspace();
                self.activity_tab = ActivityTab::Explorer;
                let title = self.doc_manager.active().title.clone();
                self.notify(format!("Abierto {}", title), ctx);
                self.sync_spellcheck_language();
            }
            Err(e) => self.notify_error(format!("No se pudo abrir: {}", e), ctx),
        }
    }

    fn save_active(&mut self, ctx: &egui::Context, force_dialog: bool) {
        let Some(doc) = self.doc_manager.documents.get(self.doc_manager.active_index) else {
            return;
        };
        let existing = doc.file_path.clone();
        if !force_dialog {
            if let Some(path) = existing {
                self.write_active_to(path, ctx);
                return;
            }
        }
        self.save_as_index = self.doc_manager.active_index;
        let suggestion = doc.title.clone();
        let start_dir = self
            .workspace_root
            .clone()
            .or_else(|| self.doc_manager.active().directory());
        self.picker.open(PickerMode::SaveFile, start_dir.as_deref(), &suggestion);
        self.picker.show_all = true;
    }

    fn write_active_to(&mut self, path: PathBuf, ctx: &egui::Context) {
        let idx = self.doc_manager.active_index;
        let content = self.doc_manager.active().content.clone();
        match files::write_text_atomic(&path, &content) {
            Ok(()) => {
                if let Some(doc) = self.doc_manager.documents.get_mut(idx) {
                    doc.mark_saved(path.clone());
                }
                self.prefs.remember_file(&path);
                self.prefs.save();
                if let Some(parent) = path.parent() {
                    if self.workspace_root.is_none() {
                        self.workspace_root = Some(parent.to_path_buf());
                    }
                }
                self.reload_workspace();
                self.notify(
                    format!("Guardado en {}", path.display()),
                    ctx,
                );
                if let Some(action) = self.after_save_action.take() {
                    if self.doc_manager.dirty_indices().is_empty() {
                        self.perform_pending(action, ctx);
                    } else {
                        // Other tabs still have unsaved work: never quit or
                        // close on their behalf.
                        self.notify(
                            "Hay otros documentos con cambios sin guardar; guárdalos y repite la acción",
                            ctx,
                        );
                    }
                }
            }
            Err(e) => self.notify_error(format!("No se pudo guardar: {}", e), ctx),
        }
    }

    fn new_document(&mut self, quarto: bool, ctx: &egui::Context) {
        if quarto {
            self.doc_manager.add_new_quarto();
        } else {
            self.doc_manager.add_new();
        }
        self.notify(
            if quarto {
                "Nuevo documento Quarto (.qmd)"
            } else {
                "Nuevo documento Markdown (.md)"
            },
            ctx,
        );
    }

    fn close_tab(&mut self, idx: usize, ctx: &egui::Context) {
        let Some(doc) = self.doc_manager.documents.get(idx) else {
            return;
        };
        if doc.is_dirty() {
            self.pending_confirm = Some(ConfirmState {
                message: format!("«{}» tiene cambios sin guardar.", doc.title),
                action: PendingAction::CloseTab(idx),
            });
            return;
        }
        self.remove_tab(idx, ctx);
    }

    fn remove_tab(&mut self, idx: usize, ctx: &egui::Context) {
        if self.doc_manager.documents.len() <= 1 {
            self.doc_manager.active_mut().content.clear();
            self.notify("Documento vaciado (no se puede cerrar la última pestaña)", ctx);
            return;
        }
        self.doc_manager.remove(idx);
        self.notify("Documento cerrado", ctx);
    }

    fn perform_pending(&mut self, action: PendingAction, ctx: &egui::Context) {
        match action {
            PendingAction::CloseTab(idx) => self.remove_tab(idx, ctx),
            PendingAction::OpenFile(path) => {
                // The dirty scratch tab is replaced, not saved: drop it after
                // loading so no data is silently overwritten.
                match self.doc_manager.open_path(&path) {
                    Ok(_) => {
                        self.prefs.remember_file(&path);
                        self.prefs.save();
                        self.reload_workspace();
                        self.sync_spellcheck_language();
                        self.notify(format!("Abierto {}", files::file_title(&path)), ctx);
                    }
                    Err(e) => self.notify_error(format!("No se pudo abrir: {}", e), ctx),
                }
            }
            PendingAction::Quit => {
                self.quit_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn save_all(&mut self, ctx: &egui::Context) -> bool {
        let dirty: Vec<usize> = self.doc_manager.dirty_indices();
        let mut all_ok = true;
        for idx in dirty {
            let Some(doc) = self.doc_manager.documents.get(idx) else {
                continue;
            };
            match doc.file_path.clone() {
                Some(path) => {
                    let content = doc.content.clone();
                    match files::write_text_atomic(&path, &content) {
                        Ok(()) => {
                            if let Some(d) = self.doc_manager.documents.get_mut(idx) {
                                d.mark_saved(path.clone());
                            }
                            self.prefs.remember_file(&path);
                        }
                        Err(e) => {
                            all_ok = false;
                            self.notify_error(format!("No se pudo guardar {}: {}", path.display(), e), ctx);
                        }
                    }
                }
                None => {
                    all_ok = false;
                    self.save_as_index = idx;
                    self.picker.open(PickerMode::SaveFile, self.workspace_root.as_deref(), &doc.title.clone());
                    self.picker.show_all = true;
                }
            }
        }
        self.prefs.save();
        if all_ok {
            self.notify("Todos los documentos guardados", ctx);
        }
        all_ok
    }

    fn sync_spellcheck_language(&mut self) {
        let doc = self.doc_manager.active();
        let lang = detect_document_language(&doc.content, &doc.title);
        if self.hunspell.active_language() != lang {
            self.hunspell.set_language(lang);
        }
    }

    fn exports_dir(&self) -> PathBuf {
        match &self.workspace_root {
            Some(root) => root.join("exports"),
            None => PathBuf::from("exports"),
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let has_command = ctx.input(|i| i.modifiers.command);
        if !has_command {
            return;
        }
        let pressed = |ctx: &egui::Context, key: egui::Key| {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, key))
        };

        if pressed(ctx, egui::Key::S) {
            let shift = ctx.input(|i| i.modifiers.shift);
            self.save_active(ctx, shift);
        }
        if pressed(ctx, egui::Key::O) {
            self.picker.open(PickerMode::OpenFile, self.workspace_root.as_deref(), "");
        }
        if pressed(ctx, egui::Key::P) {
            self.open_quick_open();
        }
        if pressed(ctx, egui::Key::N) {
            let shift = ctx.input(|i| i.modifiers.shift);
            self.new_document(shift, ctx);
        }
        if pressed(ctx, egui::Key::F) {
            self.activity_tab = ActivityTab::Search;
            self.sidebar_visible = true;
        }
        if pressed(ctx, egui::Key::B) {
            self.insert_markup_at_cursor("**", "**", ctx);
        }
        if pressed(ctx, egui::Key::I) {
            self.insert_markup_at_cursor("*", "*", ctx);
        }
        if pressed(ctx, egui::Key::W) {
            let idx = self.doc_manager.active_index;
            self.close_tab(idx, ctx);
        }
    }

    /// Content snapshot taken before a programmatic edit (toolbar, replace-all,
    /// spelling fix, task checkbox…). Restored by Ctrl/Cmd+Z like typed input.
    fn begin_programmatic_edit(&self) -> String {
        self.doc_manager.active().content.clone()
    }

    /// Registers the pre-edit snapshot in egui's per-widget undo stack, so the
    /// built-in Cmd/Ctrl+Z works for edits that did not come from typing.
    fn end_programmatic_edit(&self, ctx: &egui::Context, previous: String) {
        if self.doc_manager.active().content == previous {
            return;
        }
        push_editor_undo(ctx, self.editor_id, self.editor_selection, previous);
    }

    /// Inserts text at the caret (or replaces the current selection) and leaves
    /// the caret at `caret_offset` characters into the inserted text.
    fn insert_text_at_cursor(&mut self, text: &str, caret_offset: usize, ctx: &egui::Context) {
        let previous = self.begin_programmatic_edit();
        let result = self.insert_text_at_cursor_inner(text, caret_offset, ctx);
        self.end_programmatic_edit(ctx, previous);
        result
    }

    fn insert_text_at_cursor_inner(&mut self, text: &str, caret_offset: usize, ctx: &egui::Context) {
        let Some((start, end)) = self.editor_selection else {
            // No editor interaction yet: append at the end of the document.
            let idx = self.doc_manager.active_index;
            if let Some(doc) = self.doc_manager.documents.get_mut(idx) {
                if !doc.content.is_empty() && !doc.content.ends_with('\n') {
                    doc.content.push('\n');
                }
                let base = doc.content.chars().count();
                doc.content.push_str(text);
                self.editor_selection = Some((base + caret_offset, base + caret_offset));
            }
            return;
        };

        let idx = self.doc_manager.active_index;
        let Some(doc) = self.doc_manager.documents.get_mut(idx) else {
            return;
        };
        let (start, end) = (start.min(end), start.max(end));
        let start_byte = doc
            .content
            .char_indices()
            .nth(start)
            .map(|(b, _)| b)
            .unwrap_or(doc.content.len());
        let end_byte = doc
            .content
            .char_indices()
            .nth(end)
            .map(|(b, _)| b)
            .unwrap_or(doc.content.len());

        doc.content.replace_range(start_byte..end_byte, text);

        let caret = start + caret_offset;
        self.editor_selection = Some((caret, caret));
        self.pending_caret = Some(caret);

        // Push the caret into the TextEdit state so the widget shows it there
        // on this very frame (the state is normally saved by the widget).
        if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, self.editor_id) {
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(caret))));
            state.store(ctx, self.editor_id);
        }
    }

    /// Wraps the selection (or inserts a placeholder) with Markdown markup.
    fn insert_markup_at_cursor(&mut self, before: &str, after: &str, ctx: &egui::Context) {
        let placeholder = if after.is_empty() { "" } else { "texto" };
        let text = format!("{}{}{}", before, placeholder, after);
        let caret = before.chars().count() + placeholder.chars().count();
        self.insert_text_at_cursor(&text, caret, ctx);
    }

    fn select_document(&mut self, idx: usize) {
        self.doc_manager.select(idx);
        self.preview_assets.clear();
        self.editor_selection = None;
        self.sync_spellcheck_language();
    }

    fn set_theme(&mut self, theme_id: ThemeId, ctx: &egui::Context) {
        self.current_theme = ColorPalette::get(theme_id);
        self.current_theme.apply(ctx);
        self.prefs.last_theme = Some(theme_id.key().to_string());
        self.prefs.save();
        self.status_notification = Some((
            format!("Tema cambiado a {}", self.current_theme.name),
            ctx.input(|i| i.time),
        ));
    }

    fn notify(&mut self, msg: impl Into<String>, ctx: &egui::Context) {
        self.status_notification = Some((msg.into(), ctx.input(|i| i.time)));
    }

    fn execute_export(&mut self, ctx: &egui::Context) {
        let doc = self.doc_manager.active();
        let safe_name = clean_filename(&doc.title);
        let exports = self.exports_dir();
        let _ = std::fs::create_dir_all(&exports);

        if self.export_format_pdf {
            let path = exports.join(format!("{}.pdf", safe_name));
            let out_str = path.display().to_string();
            match export_to_pdf(&path, &doc.title, &doc.content, self.selected_export_style) {
                Ok(_) => {
                    self.notify(format!("Exportado con éxito a {}", out_str), ctx);
                }
                Err(e) => {
                    self.notify(format!("Error al exportar PDF: {}", e), ctx);
                }
            }
        } else {
            let path = exports.join(format!("{}.html", safe_name));
            let out_str = path.display().to_string();
            let html = export_to_html(&doc.title, &doc.content, self.selected_export_style);
            match std::fs::write(&path, html) {
                Ok(_) => {
                    self.notify(format!("Exportado con éxito a {}", out_str), ctx);
                }
                Err(e) => {
                    self.notify(format!("Error al exportar HTML: {}", e), ctx);
                }
            }
        }
    }

    fn execute_quarto_cli_export(&mut self, format: QuartoFormat, ctx: &egui::Context) {
        let doc = self.doc_manager.active();
        let title = doc.title.clone();
        let content = doc.content.clone();
        let path = doc.path().map(|p| p.to_path_buf());
        let exports = self.exports_dir();

        match quarto::render_with_quarto_cli(
            &title,
            &content,
            path.as_deref(),
            &exports,
            format,
            true,
        ) {
            Ok(outcome) => {
                let name = outcome
                    .output
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| outcome.output.display().to_string());
                self.notify(format!("Quarto {}: {}", outcome.format.label(), name), ctx);
            }
            Err(e) => {
                self.notify_error(format!("Quarto {} fallou: {}", format.label(), e), ctx);
            }
        }
    }

    fn execute_quarto_render_preview(&mut self, ctx: &egui::Context) {
        let doc = self.doc_manager.active();
        let title = doc.title.clone();
        let content = doc.content.clone();
        let exports = self.exports_dir();

        match quarto::export_and_preview_quarto(
            &title,
            &content,
            self.current_stylesheet,
            Some(&self.quarto_kernel),
            &exports,
            true,
        ) {
            Ok(outcome) => {
                let shown = outcome
                    .cli_output
                    .as_deref()
                    .unwrap_or(&outcome.preview_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if outcome.used_cli {
                    self.notify(format!("Quarto CLI: renderizado e aberto ({})", shown), ctx);
                } else if let Some(err) = outcome.cli_error {
                    self.notify(
                        format!("Vista previa nativa aberta ({}); quarto CLI fallou: {}", shown, err),
                        ctx,
                    );
                } else {
                    self.notify(
                        format!("Vista previa Quarto xerada e aberta ({})", shown),
                        ctx,
                    );
                }
            }
            Err(e) => {
                self.notify_error(format!("Erro ao renderizar Quarto: {}", e), ctx);
            }
        }
    }

    fn render_export_modal(&mut self, ctx: &egui::Context) {
        let mut is_open = self.export_dialog_open;
        let mut do_export = false;
        let mut cancel = false;

        egui::Window::new("Exportar documento con hoja de estilo")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .default_width(540.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                Frame::window(&ctx.style())
                    .fill(self.current_theme.sidebar_bg)
                    .stroke(Stroke::new(1.5_f32, self.current_theme.accent))
                    .rounding(Rounding::same(8.0_f32))
                    .inner_margin(Margin::same(16.0_f32)),
            )
            .show(ctx, |ui| {
                let doc_title = self.doc_manager.active().title.clone();
                ui.label(
                    RichText::new(format!("Documento activo: {}", doc_title))
                        .size(15.0)
                        .strong()
                        .color(self.current_theme.text_primary),
                );
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(10.0);

                // Format selector
                ui.label(
                    RichText::new("1. Formato de Salida:")
                        .size(13.0)
                        .strong()
                        .color(self.current_theme.accent),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.export_format_pdf, "📄 Documento PDF (.pdf)").clicked() {
                        self.export_format_pdf = true;
                    }
                    if ui.selectable_label(!self.export_format_pdf, "🌐 Página Web HTML (.html)").clicked() {
                        self.export_format_pdf = false;
                    }
                });

                ui.add_space(12.0);

                // Stylesheet selector
                ui.label(
                    RichText::new("2. Hoja de Estilo Typora:")
                        .size(13.0)
                        .strong()
                        .color(self.current_theme.accent),
                );
                ui.add_space(4.0);

                for style in TyporaStylesheet::all() {
                    let is_sel = self.selected_export_style == style;
                    let desc = match style {
                        TyporaStylesheet::GitHub => "GitHub — Limpio, estándar web, tablas y código elegantes",
                        TyporaStylesheet::Night => "Night — Modo oscuro oficial Typora de alto contraste",
                        TyporaStylesheet::Newsprint => "Newsprint — Tipografía serif editorial (Georgia/Times)",
                        TyporaStylesheet::Gothic => "Gothic — Tipografía limpia y encabezados modernos",
                        TyporaStylesheet::Whitey => "Whitey — Estética minimalista con acento esmeralda",
                    };

                    if ui.selectable_label(is_sel, desc).clicked() {
                        self.selected_export_style = style;
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Target filename preview
                let ext = if self.export_format_pdf { "pdf" } else { "html" };
                let safe_name = clean_filename(&doc_title);
                let target_file = format!("exports/{}.{}", safe_name, ext);
                ui.label(
                    RichText::new(format!("Ruta de guardado: {}", target_file))
                        .size(11.5)
                        .color(self.current_theme.text_muted),
                );

                ui.add_space(14.0);

                // Actions
                ui.horizontal(|ui| {
                    if ui.add(
                        Button::new(
                            RichText::new("  ✓ Exportar Ahora  ")
                                .size(13.0)
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(self.current_theme.accent)
                        .rounding(Rounding::same(4.0_f32)),
                    ).clicked() {
                        do_export = true;
                    }

                    if ui.button("Cancelar").clicked() {
                        cancel = true;
                    }
                });
            });

        if cancel {
            is_open = false;
        }

        self.export_dialog_open = is_open;

        if do_export {
            self.execute_export(ctx);
            self.export_dialog_open = false;
        }
    }

    fn apply_preview_actions(&mut self, actions: Vec<markdown::PreviewAction>, ctx: &egui::Context) {
        let previous = self.begin_programmatic_edit();
        for action in actions {
            match action {
                markdown::PreviewAction::ToggleTask(line_no) => {
                    let doc = self.doc_manager.active_mut();
                    let mut lines: Vec<String> = doc.content.lines().map(|s| s.to_string()).collect();
                    if let Some(line) = lines.get_mut(line_no.saturating_sub(1)) {
                        let next = if line.contains("[x]") || line.contains("[X]") {
                            line.replacen("[x]", "[ ]", 1).replacen("[X]", "[ ]", 1)
                        } else {
                            line.replacen("[ ]", "[x]", 1)
                        };
                        *line = next;
                        doc.content = lines.join("\n");
                    }
                }
                markdown::PreviewAction::OpenFile(path) => {
                    self.open_file_path(&path, ctx);
                }
            }
        }
        self.end_programmatic_edit(ctx, previous);
    }

    /// 1-based (line, column) of the caret for the status bar.
    fn cursor_line_col(&self) -> (usize, usize) {
        let doc = self.doc_manager.active();
        let Some((start, _)) = self.editor_selection else {
            return (1, 1);
        };
        let mut line = 1usize;
        let mut col = 1usize;
        for (i, ch) in doc.content.chars().enumerate() {
            if i >= start {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn set_workspace(&mut self, dir: PathBuf, ctx: &egui::Context) {
        if !dir.is_dir() {
            self.notify_error(format!("No es una carpeta: {}", dir.display()), ctx);
            return;
        }
        self.workspace_root = Some(dir.clone());
        self.prefs.workspace = Some(dir.clone());
        self.prefs.save();
        self.reload_workspace();
        self.notify(format!("Carpeta de trabajo: {}", dir.display()), ctx);
    }

    fn open_quick_open(&mut self) {
        let root = self
            .workspace_root
            .clone()
            .or_else(|| self.doc_manager.active().directory())
            .unwrap_or_else(files::default_workspace);
        self.quick_entries = files::scan_tree(&root, 600, 4);
        self.quick_query.clear();
        self.quick_selected = 0;
        self.quick_open = true;
        self.quick_focus = true;
    }

    fn render_quick_open(&mut self, ctx: &egui::Context) {
        if !self.quick_open {
            return;
        }

        let mut close = false;
        let mut chosen: Option<PathBuf> = None;
        let mut query = std::mem::take(&mut self.quick_query);
        let entries = self.quick_entries.clone();
        let root = self.workspace_root.clone();
        let theme = self.current_theme.clone();
        let mut selected = self.quick_selected;
        let focus = self.quick_focus;
        let matches = files::filter_by_query(&entries, &query);
        if selected >= matches.len() {
            selected = 0;
        }

        let down = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
        let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
        let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));

        if down && !matches.is_empty() {
            selected = (selected + 1).min(matches.len() - 1);
        }
        if up {
            selected = selected.saturating_sub(1);
        }
        if enter {
            if let Some(entry) = matches.get(selected) {
                chosen = Some(entry.path.clone());
            }
            close = true;
        }
        if escape {
            close = true;
        }

        egui::Window::new("quick_open")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(600.0)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 72.0])
            .frame(
                Frame::window(&ctx.style())
                    .fill(theme.sidebar_bg)
                    .stroke(Stroke::new(1.5_f32, theme.accent))
                    .rounding(Rounding::same(8.0_f32))
                    .inner_margin(Margin::same(12.0_f32)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔎").size(14.0));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut query)
                            .hint_text("Abrir rápido: escribe parte del nombre del archivo…")
                            .desired_width(520.0),
                    );
                    if focus {
                        resp.request_focus();
                    }
                    if resp.changed() {
                        selected = 0;
                    }
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(2.0);

                if matches.is_empty() {
                    ui.label(
                        RichText::new("Sin archivos que coincidan en la carpeta de trabajo")
                            .size(11.5)
                            .italics()
                            .color(theme.text_muted),
                    );
                }

                egui::ScrollArea::vertical()
                    .id_salt("quick_open_scroll")
                    .max_height(300.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, entry) in matches.iter().enumerate().take(200) {
                            let relative = root
                                .as_ref()
                                .and_then(|r| entry.path.strip_prefix(r).ok())
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| entry.path.display().to_string());
                            let is_selected = i == selected;
                            let button = Button::new(
                                RichText::new(&relative)
                                    .size(12.0)
                                    .color(if is_selected { theme.accent } else { theme.text_body }),
                            )
                            .fill(if is_selected { theme.selection_bg } else { Color32::TRANSPARENT })
                            .min_size(Vec2::new(ui.available_width(), 22.0));
                            let resp = ui.add(button);
                            if resp.hovered() {
                                selected = i;
                            }
                            if resp.clicked() {
                                chosen = Some(entry.path.clone());
                            }
                            if is_selected {
                                resp.scroll_to_me(Some(Align::Center));
                            }
                        }
                    });

                ui.add_space(2.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} archivo(s) · flechas para navegar · Enter abrir · Esc cerrar",
                            matches.len()
                        ))
                        .size(10.5)
                        .color(theme.text_muted),
                    );
                });
            });

        self.quick_query = query;
        self.quick_selected = selected;
        self.quick_focus = false;

        if let Some(path) = chosen {
            self.quick_open = false;
            self.quick_query.clear();
            self.open_file_path(&path, ctx);
        } else if close {
            self.quick_open = false;
            self.quick_query.clear();
        }

    }

    fn render_file_picker(&mut self, ctx: &egui::Context) {
        let theme = self.current_theme.clone();
        let Some(outcome) = self.picker.show(ctx, &theme) else {
            return;
        };
        let mode = self.picker.mode;
        match outcome {
            PickerOutcome::Cancelled => {}
            PickerOutcome::Chosen(path) => match mode {
                PickerMode::OpenFile => self.open_file_path(&path, ctx),
                PickerMode::OpenFolder => self.set_workspace(path, ctx),
                PickerMode::SaveFile => {
                    let idx = self.save_as_index;
                    if idx < self.doc_manager.documents.len() {
                        self.doc_manager.select(idx);
                        self.write_active_to(path, ctx);
                    }
                }
            },
        }
    }

    fn render_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.pending_confirm.as_ref() else {
            return;
        };
        let message = confirm.message.clone();
        let action = confirm.action.clone();
        let mut decision: Option<u8> = None;

        egui::Window::new("Cambios sin guardar")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(430.0)
            .frame(
                Frame::window(&ctx.style())
                    .fill(self.current_theme.sidebar_bg)
                    .stroke(Stroke::new(1.5_f32, Color32::from_rgb(0xFF, 0xBD, 0x2E)))
                    .rounding(Rounding::same(8.0_f32))
                    .inner_margin(Margin::same(16.0_f32)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("⚠ Hay cambios sin guardar")
                        .size(15.0)
                        .strong()
                        .color(Color32::from_rgb(0xFF, 0xBD, 0x2E)),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(&message).size(12.5).color(self.current_theme.text_primary));
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Guardar y continuar").clicked() {
                        decision = Some(1);
                    }
                    if ui.button("Descartar cambios").clicked() {
                        decision = Some(2);
                    }
                    if ui.button("Cancelar").clicked() {
                        decision = Some(3);
                    }
                });
            });

        match decision {
            Some(1) => {
                self.pending_confirm = None;
                self.after_save_action = Some(action);
                let all_saved = self.save_all(ctx);
                // If everything was already on disk there is no picker to wait
                // for, so continue immediately; otherwise the action is resumed
                // after a successful Save As.
                if all_saved && !self.picker.visible {
                    if let Some(action) = self.after_save_action.take() {
                        self.perform_pending(action, ctx);
                    }
                } else if !self.picker.visible {
                    self.after_save_action = None;
                    self.notify(
                        "No se guardaron todos los documentos; la acción se canceló",
                        ctx,
                    );
                }
            }
            Some(2) => {
                self.pending_confirm = None;
                match &action {
                    PendingAction::CloseTab(idx) => {
                        if let Some(doc) = self.doc_manager.documents.get_mut(*idx) {
                            doc.revert();
                        }
                    }
                    PendingAction::Quit => {
                        self.quit_requested = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        return;
                    }
                    PendingAction::OpenFile(_) => {}
                }
                self.perform_pending(action, ctx);
            }
            Some(3) => self.pending_confirm = None,
            _ => {}
        }
    }

    fn render_shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            return;
        }
        let mut open = self.shortcuts_open;
        egui::Window::new("Atajos de teclado")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(430.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                let rows = [
                    ("⌘/Ctrl + S", "Guardar"),
                    ("⌘/Ctrl + Shift + S", "Guardar como…"),
                    ("⌘/Ctrl + O", "Abrir archivo…"),
                    ("⌘/Ctrl + P", "Abrir rápido (buscar por nombre)"),
                    ("⌘/Ctrl + N", "Nuevo Markdown"),
                    ("⌘/Ctrl + Shift + N", "Nuevo Quarto (.qmd)"),
                    ("⌘/Ctrl + F", "Buscar y reemplazar"),
                    ("⌘/Ctrl + B / I", "Negrita / cursiva en la selección"),
                    ("⌘/Ctrl + W", "Cerrar pestaña"),
                ];
                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing([18.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for (keys, what) in rows {
                            ui.label(RichText::new(keys).monospace().strong().color(self.current_theme.accent));
                            ui.label(RichText::new(what).size(12.5).color(self.current_theme.text_body));
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Dentro del editor, ⌘/Ctrl+Z y ⌘/Ctrl+Shift+Z deshacen y rehacen.")
                        .size(11.0)
                        .color(self.current_theme.text_muted),
                );
            });
        self.shortcuts_open = open;
    }
}

impl eframe::App for TigorraApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keyboard shortcuts (Cmd/Ctrl+S, +O, +N, +F, +B, +I, +W)
        self.handle_shortcuts(ctx);

        // Ask before closing the window with unsaved documents.
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit_requested {
            if !self.doc_manager.dirty_indices().is_empty() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.pending_confirm = Some(ConfirmState {
                    message: format!(
                        "Hay {} documento(s) con cambios sin guardar. ¿Guardarlos antes de salir?",
                        self.doc_manager.dirty_indices().len()
                    ),
                    action: PendingAction::Quit,
                });
            }
        }

        // One-shot UI hooks for automated screenshots / smoke tests.
        if !self.test_hook_done {
            self.test_hook_done = true;
            self.test_hook_started = Some(ctx.input(|i| i.time));
            if let Ok(mode) = std::env::var("TIPPORA_UI_TEST") {
                match mode.as_str() {
                    "picker" => {
                        self.picker.open(PickerMode::OpenFile, self.workspace_root.as_deref(), "");
                    }
                    "picker-save" => {
                        self.save_active(ctx, true);
                    }
                    "confirm" => {
                        self.pending_confirm = Some(ConfirmState {
                            message: "«Caderno Científico Quarto.qmd» tiene cambios sin guardar.".to_string(),
                            action: PendingAction::Quit,
                        });
                    }
                    "shortcuts" => self.shortcuts_open = true,
                    "quick-open" => self.open_quick_open(),
                    "glyphs" => {
                        self.doc_manager.active_mut().content = "# Glifos\n\n☑ ✓ ✔ ✗ ✘ ● ○ ◉ ⏹ ⏱ ⌘ ⇧ ⚠ ▶ ▸ ⟳ ↺ ⬆ ← → ± ≠ ≈ ≤ ≥ ∞ ∑ ∫ √ 😀 🔎 🏠 📁 📊 ⚙ 🗑 💾 📤\n".to_string();
                        self.view_mode = ViewMode::Live;
                    }
                    "quarto-panel" => {
                        self.activity_tab = ActivityTab::Quarto;
                        self.sidebar_visible = true;
                    }
                    "edit-save" => {
                        self.doc_manager
                            .active_mut()
                            .content
                            .push_str("\n<!-- guardado desde a app -->\n");
                        self.save_active(ctx, false);
                    }
                    "run-python" => {
                        let content = self.doc_manager.active().content.clone();
                        let dir = self.doc_manager.active().directory();
                        self.quarto_kernel.execute_all_cells(&content, dir.as_deref());
                        self.view_mode = ViewMode::Live;
                    }
                    other => eprintln!("TIPPORA_UI_TEST desconocido: {}", other),
                }
            }
        }

        // Screenshot support for automated testing
        if let Some(target_path) = &self.screenshot_path {
            self.frame_count += 1;
            ctx.request_repaint();

            for event in &ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let mut bytes = Vec::with_capacity(image.width() * image.height() * 4);
                    for pixel in &image.pixels {
                        bytes.extend_from_slice(&pixel.to_array());
                    }
                    if let Some(parent) = target_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = image::save_buffer(
                        target_path,
                        &bytes,
                        image.width() as u32,
                        image.height() as u32,
                        image::ColorType::Rgba8,
                    );
                    std::process::exit(0);
                }
            }

            // When a test hook runs Python, wait (by wall-clock time) for the
            // kernel to go idle so the screenshot includes real outputs.
            let python_hook = std::env::var("TIPPORA_UI_TEST").is_ok_and(|v| v == "run-python");
            let elapsed = ctx.input(|i| i.time);
            let started = self.test_hook_started.unwrap_or(elapsed);
            let waiting_for_python = python_hook
                && self.quarto_kernel.is_busy()
                && elapsed - started < 45.0;
            let frame_limit = if python_hook { 120 } else { 120 };
            if self.frame_count >= frame_limit && !waiting_for_python && !self.screenshot_requested {
                self.screenshot_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }

        // Advanced queued Python cells and collect finished outputs.
        self.quarto_kernel.poll(Some(ctx));

        // Live spelling check on the active document (cached by content hash).
        let active_content = self.doc_manager.active().content.clone();
        self.hunspell.check_document(&active_content);

        // Toasts expire after a few seconds instead of lingering forever.
        if let Some((_, stamp)) = &self.status_notification {
            if *stamp > 0.0 && ctx.input(|i| i.time) - stamp > 7.0 {
                self.status_notification = None;
            }
        }

        // 1. Top Menu / Linux Headerbar
        egui::TopBottomPanel::top("top_header")
            .frame(
                Frame::none()
                    .fill(self.current_theme.header_bg)
                    .stroke(Stroke::new(1.0_f32, self.current_theme.border))
                    .inner_margin(Margin::symmetric(8.0_f32, 4.0_f32)),
            )
            .show(ctx, |ui| {
                self.render_top_menu_bar(ui, ctx);
            });

        // 2. Bottom Status Bar (VS Code style colored bar)
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                Frame::none()
                    .fill(self.current_theme.status_bg)
                    .inner_margin(Margin::symmetric(10.0_f32, 3.0_f32)),
            )
            .show(ctx, |ui| {
                self.render_vscode_status_bar(ui, ctx);
            });

        // 3. Activity Bar (Narrow vertical strip on far left, ~48px)
        egui::SidePanel::left("activity_bar")
            .exact_width(46.0)
            .resizable(false)
            .frame(
                Frame::none()
                    .fill(self.current_theme.activity_bar_bg)
                    .stroke(Stroke::new(1.0_f32, self.current_theme.border))
                    .inner_margin(Margin::symmetric(4.0_f32, 8.0_f32)),
            )
            .show(ctx, |ui| {
                self.render_activity_bar(ui, ctx);
            });

        // 4. Primary Sidebar (Collapsible: Explorer, Search, Outline, Themes)
        if self.sidebar_visible {
            egui::SidePanel::left("primary_sidebar")
                .default_width(260.0)
                .min_width(200.0)
                .max_width(420.0)
                .resizable(true)
                .frame(
                    Frame::none()
                        .fill(self.current_theme.sidebar_bg)
                        .stroke(Stroke::new(1.0_f32, self.current_theme.border))
                        .inner_margin(Margin::same(8.0_f32)),
                )
                .show(ctx, |ui| {
                    self.render_primary_sidebar(ui, ctx);
                });
        }

        // 5. Central Editor & Workspace
        egui::CentralPanel::default()
            .frame(Frame::none().fill(self.current_theme.bg_color))
            .show(ctx, |ui| {
                self.render_editor_workspace(ui, ctx);
            });

        // 6. Typora Export Dialog Modal
        if self.export_dialog_open {
            self.render_export_modal(ctx);
        }

        // 7. File picker, unsaved-changes confirmation and shortcuts sheet
        self.render_quick_open(ctx);
        self.render_file_picker(ctx);
        self.render_confirm_modal(ctx);
        self.render_shortcuts_window(ctx);
    }
}

impl TigorraApp {
    fn render_top_menu_bar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            // macOS style window control dots (red, yellow, green)
            ui.spacing_mut().item_spacing = Vec2::new(7.0, 0.0);
            ui.add_space(2.0);

            let close_dot = ui.add(
                Button::new("")
                    .fill(Color32::from_rgb(0xFF, 0x5F, 0x56))
                    .stroke(Stroke::new(0.75_f32, Color32::from_rgb(0xE0, 0x44, 0x3E)))
                    .min_size(Vec2::new(12.0, 12.0))
                    .rounding(Rounding::same(6.0_f32)),
            ).on_hover_text("Cerrar");
            if close_dot.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            let min_dot = ui.add(
                Button::new("")
                    .fill(Color32::from_rgb(0xFF, 0xBD, 0x2E))
                    .stroke(Stroke::new(0.75_f32, Color32::from_rgb(0xDE, 0xA1, 0x23)))
                    .min_size(Vec2::new(12.0, 12.0))
                    .rounding(Rounding::same(6.0_f32)),
            ).on_hover_text("Minimizar");
            if min_dot.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }

            let max_dot = ui.add(
                Button::new("")
                    .fill(Color32::from_rgb(0x27, 0xC9, 0x3F))
                    .stroke(Stroke::new(0.75_f32, Color32::from_rgb(0x1A, 0xAB, 0x29)))
                    .min_size(Vec2::new(12.0, 12.0))
                    .rounding(Rounding::same(6.0_f32)),
            ).on_hover_text("Maximizar");
            if max_dot.clicked() {
                let is_max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_max));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);

            // VS Code / Positron Menus
            ui.menu_button("Archivo", |ui| {
                if ui.button("Nuevo documento Markdown (.md)     ⌘N").clicked() {
                    self.new_document(false, ctx);
                    ui.close_menu();
                }
                if ui.button("Nuevo documento Quarto (.qmd)     ⇧⌘N").clicked() {
                    self.new_document(true, ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Abrir archivo…     ⌘O").clicked() {
                    self.picker.open(PickerMode::OpenFile, self.workspace_root.as_deref(), "");
                    ui.close_menu();
                }
                if ui.button("Abrir rápido…     ⌘P").clicked() {
                    self.open_quick_open();
                    ui.close_menu();
                }
                if ui.button("Abrir carpeta de trabajo…").clicked() {
                    self.picker.open(PickerMode::OpenFolder, self.workspace_root.as_deref(), "");
                    ui.close_menu();
                }
                if !self.prefs.recent_files.is_empty() {
                    ui.menu_button("Abrir reciente", |ui| {
                        let recents = self.prefs.recent_files.clone();
                        for path in recents {
                            let label = format!(
                                "{}  ({})",
                                files::file_title(&path),
                                path.parent().map(|p| p.display().to_string()).unwrap_or_default()
                            );
                            if ui.button(label).clicked() {
                                self.open_file_path(&path, ctx);
                                ui.close_menu();
                            }
                        }
                    });
                }
                ui.separator();
                if ui.button("Guardar     ⌘S").clicked() {
                    self.save_active(ctx, false);
                    ui.close_menu();
                }
                if ui.button("Guardar como…     ⇧⌘S").clicked() {
                    self.save_active(ctx, true);
                    ui.close_menu();
                }
                if ui.button("Guardar todos").clicked() {
                    self.save_all(ctx);
                    ui.close_menu();
                }
                if ui.button("Cerrar pestaña     ⌘W").clicked() {
                    let idx = self.doc_manager.active_index;
                    self.close_tab(idx, ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📊 Renderizar / Previsualizar Quarto (.qmd)").clicked() {
                    self.execute_quarto_render_preview(ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("📤 Exportar con Hojas de Estilo...").clicked() {
                    self.selected_export_style = self.current_stylesheet;
                    self.export_dialog_open = true;
                    ui.close_menu();
                }
                if ui.button("📄 Exportación Rápida a PDF").clicked() {
                    self.selected_export_style = self.current_stylesheet;
                    self.export_format_pdf = true;
                    self.execute_export(ctx);
                    ui.close_menu();
                }
                if ui.button("🌐 Exportación Rápida a HTML").clicked() {
                    self.selected_export_style = self.current_stylesheet;
                    self.export_format_pdf = false;
                    self.execute_export(ctx);
                    ui.close_menu();
                }
                ui.menu_button("Quarto CLI (documento real)", |ui| {
                    let cli = has_quarto_cli();
                    if !cli {
                        ui.label(
                            RichText::new("Instala Quarto para exportar con CLI")
                                .size(11.0)
                                .italics()
                                .color(self.current_theme.text_muted),
                        );
                    }
                    for format in QuartoFormat::all() {
                        let button = ui.add_enabled(
                            cli,
                            Button::new(RichText::new(format!("→ {}", format.label())).size(12.0)),
                        );
                        if button.clicked() {
                            self.execute_quarto_cli_export(format, ctx);
                            ui.close_menu();
                        }
                    }
                });
                ui.separator();
                if ui.button("Salir").clicked() {
                    if self.doc_manager.dirty_indices().is_empty() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.pending_confirm = Some(ConfirmState {
                            message: "Hay documentos con cambios sin guardar.".to_string(),
                            action: PendingAction::Quit,
                        });
                    }
                    ui.close_menu();
                }
            });

            ui.menu_button("Editar", |ui| {
                if ui.button("Buscar y Reemplazar     ⌘F").clicked() {
                    self.activity_tab = ActivityTab::Search;
                    self.sidebar_visible = true;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Negrita (envolver selección)     ⌘B").clicked() {
                    self.insert_markup_at_cursor("**", "**", ctx);
                    ui.close_menu();
                }
                if ui.button("Cursiva (envolver selección)     ⌘I").clicked() {
                    self.insert_markup_at_cursor("*", "*", ctx);
                    ui.close_menu();
                }
                if ui.button("Código en línea").clicked() {
                    self.insert_markup_at_cursor("`", "`", ctx);
                    ui.close_menu();
                }
                if ui.button("Enlace Markdown").clicked() {
                    let url = "https://";
                    self.insert_text_at_cursor(&format!("[texto]({})", url), 1, ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Ordenar líneas (A→Z)").clicked() {
                    let previous = self.begin_programmatic_edit();
                    let doc = self.doc_manager.active_mut();
                    let mut lines: Vec<String> = doc.content.lines().map(|s| s.to_string()).collect();
                    lines.sort_by_key(|l| l.to_lowercase());
                    doc.content = lines.join("\n");
                    self.end_programmatic_edit(ctx, previous);
                    ui.close_menu();
                }
            });

            ui.menu_button("Ver", |ui| {
                if ui.button("Alternar Barra Lateral").clicked() {
                    self.sidebar_visible = !self.sidebar_visible;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("1. 📝 Modo Sólo Editor (Código)").clicked() {
                    self.view_mode = ViewMode::Source;
                    ui.close_menu();
                }
                if ui.button("2. 📖 Modo Sólo Vista Previa").clicked() {
                    self.view_mode = ViewMode::Live;
                    ui.close_menu();
                }
                if ui.button("3. ◫ Modo Combinado (Editor + Previa)").clicked() {
                    self.view_mode = ViewMode::Split;
                    ui.close_menu();
                }
            });

            ui.menu_button("Estilos", |ui| {
                for s in TyporaStylesheet::all() {
                    let is_active = s == self.current_stylesheet;
                    let prefix = if is_active { "● " } else { "  " };
                    if ui.button(format!("{}{}", prefix, s.name())).clicked() {
                        self.current_stylesheet = s;
                        self.notify(format!("Hoja de estilo Typora: {}", s.name()), ctx);
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button("Tema", |ui| {
                for t in ColorPalette::all() {
                    let is_active = t.id == self.current_theme.id;
                    let prefix = if is_active { "● " } else { "  " };
                    if ui.button(format!("{}{}", prefix, t.name)).clicked() {
                        self.set_theme(t.id, ctx);
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button("Ortografía", |ui| {
                if ui.button("🔤 Abrir Panel de Corrección").clicked() {
                    self.activity_tab = ActivityTab::SpellCheck;
                    self.sidebar_visible = true;
                    ui.close_menu();
                }
                let sub_prefix = if self.spell_highlight_enabled { "✓ " } else { "  " };
                if ui.button(format!("{}Subrayar faltas en rojo en editor", sub_prefix)).clicked() {
                    self.spell_highlight_enabled = !self.spell_highlight_enabled;
                    self.notify(
                        if self.spell_highlight_enabled { "Subrayado ortográfico activado" } else { "Subrayado ortográfico desactivado" },
                        ctx,
                    );
                    ui.close_menu();
                }
                if self.hunspell.error_count() > 0 {
                    if ui.button("✨ Corregir todos los errores").clicked() {
                        let previous = self.begin_programmatic_edit();
                        let doc = self.doc_manager.active_mut();
                        let report = self.hunspell.fix_all_errors(&mut doc.content);
                        self.end_programmatic_edit(ctx, previous);
                        self.notify(
                            format!("Corregidas {} palabra(s); {} omitida(s) por seguridad", report.fixed, report.skipped),
                            ctx,
                        );
                        ui.close_menu();
                    }
                }
                ui.separator();
                for lang in SpellLanguage::all() {
                    let is_active = self.hunspell.active_language() == lang;
                    let prefix = if is_active { "● " } else { "  " };
                    if ui.button(format!("{}{} {}", prefix, lang.flag(), lang.name())).clicked() {
                        self.hunspell.set_language(lang);
                        let content = self.doc_manager.active().content.clone();
                        self.hunspell.check_document(&content);
                        self.notify(format!("Diccionario Hunspell: {}", lang.name()), ctx);
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button("Ayuda", |ui| {
                if ui.button("🪟 Cómo llevar esta app a Windows").clicked() {
                    for (idx, doc) in self.doc_manager.documents.iter().enumerate() {
                        if doc.title.contains("Windows") {
                            self.select_document(idx);
                            break;
                        }
                    }
                    self.notify("Guía para Windows abierta", ctx);
                    ui.close_menu();
                }
                if ui.button("🐧 Cómo llevar esta app a Linux").clicked() {
                    for (idx, doc) in self.doc_manager.documents.iter().enumerate() {
                        if doc.title.contains("Linux") {
                            self.select_document(idx);
                            break;
                        }
                    }
                    self.notify("Guía para Linux abierta", ctx);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("⌨ Atajos de teclado").clicked() {
                    self.shortcuts_open = true;
                    ui.close_menu();
                }
                if ui.button("ℹ Acerca de Tigorra").clicked() {
                    self.notify("Tigorra — Compatible con Mac, Windows y Linux", ctx);
                    ui.close_menu();
                }
            });

            // Right: Quick Theme selector pill, Export button & View switcher
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let export_btn = ui.add(
                    Button::new(
                        RichText::new("📤")
                            .size(12.0)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(self.current_theme.accent)
                    .rounding(Rounding::same(4.0_f32)),
                ).on_hover_text("Exportar con hoja de estilo Typora");
                if export_btn.clicked() {
                    self.selected_export_style = self.current_stylesheet;
                    self.export_dialog_open = true;
                }

                let quarto_btn = ui.add(
                    Button::new(
                        RichText::new("📊 Quarto")
                            .size(11.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0x1E, 0x88, 0xE5))
                    .rounding(Rounding::same(4.0_f32)),
                ).on_hover_text("Renderizar documento Quarto (.qmd) e previsualizar en navegador");
                if quarto_btn.clicked() {
                    self.execute_quarto_render_preview(ctx);
                }

                let run_py_btn = ui.add(
                    Button::new(
                        RichText::new("▶ Python")
                            .size(11.5)
                            .strong()
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(0x27, 0xC9, 0x3F))
                    .rounding(Rounding::same(4.0_f32)),
                ).on_hover_text("Executar todas as celas de Python como en Jupyter Notebooks");
                if run_py_btn.clicked() {
                    let content = self.doc_manager.active().content.clone();
                    let doc_dir = self.doc_manager.active().directory();
                    let count = self.quarto_kernel.execute_all_cells(&content, doc_dir.as_deref());
                    self.notify(
                        if count == 0 {
                            "Non hai celas Python neste documento".to_string()
                        } else {
                            format!("{} cela(s) Python en cola (execución en segundo plano)", count)
                        },
                        ctx,
                    );
                }

                ui.separator();

                // Theme picker button with current theme name
                let theme_btn = ui.add(
                    Button::new(
                        RichText::new(format!("🎨 {}", self.current_theme.name))
                            .size(11.5)
                            .color(self.current_theme.accent),
                    )
                    .rounding(Rounding::same(4.0_f32)),
                );
                if theme_btn.clicked() {
                    self.activity_tab = ActivityTab::Themes;
                    self.sidebar_visible = true;
                }

                ui.separator();

                // Logical order from left to right: [ 📝 Código ] [ 📖 Previa ] [ ◫ Combinada ]
                // Added right-to-left: Combinada (right), Previa (middle), Código (left)
                let split_sel = self.view_mode == ViewMode::Split;
                if ui.add(Button::new("◫").selected(split_sel)).on_hover_text("Vista combinada (editor + previa)").clicked() {
                    self.view_mode = ViewMode::Split;
                }

                let live_sel = self.view_mode == ViewMode::Live;
                if ui.add(Button::new("📖").selected(live_sel)).on_hover_text("Sólo vista previa").clicked() {
                    self.view_mode = ViewMode::Live;
                }

                let src_sel = self.view_mode == ViewMode::Source;
                if ui.add(Button::new("📝").selected(src_sel)).on_hover_text("Sólo editor (código)").clicked() {
                    self.view_mode = ViewMode::Source;
                }
            });
        });
    }

    fn render_activity_bar(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 10.0);

            // Explorer button
            let exp_active = self.activity_tab == ActivityTab::Explorer && self.sidebar_visible;
            let btn_exp = ui.add(
                Button::new(
                    RichText::new("📁")
                        .size(18.0)
                        .color(if exp_active { self.current_theme.accent } else { self.current_theme.text_muted }),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            );
            if btn_exp.clicked() {
                if self.activity_tab == ActivityTab::Explorer {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::Explorer;
                    self.sidebar_visible = true;
                }
            }

            // Search button
            let search_active = self.activity_tab == ActivityTab::Search && self.sidebar_visible;
            let btn_search = ui.add(
                Button::new(
                    RichText::new("🔍")
                        .size(18.0)
                        .color(if search_active { self.current_theme.accent } else { self.current_theme.text_muted }),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            );
            if btn_search.clicked() {
                if self.activity_tab == ActivityTab::Search {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::Search;
                    self.sidebar_visible = true;
                }
            }

            // Outline button
            let outline_active = self.activity_tab == ActivityTab::Outline && self.sidebar_visible;
            let btn_outline = ui.add(
                Button::new(
                    RichText::new("📑")
                        .size(18.0)
                        .color(if outline_active { self.current_theme.accent } else { self.current_theme.text_muted }),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            );
            if btn_outline.clicked() {
                if self.activity_tab == ActivityTab::Outline {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::Outline;
                    self.sidebar_visible = true;
                }
            }

            // Hunspell Spellcheck button
            let spell_active = self.activity_tab == ActivityTab::SpellCheck && self.sidebar_visible;
            let err_cnt = self.hunspell.error_count();
            let spell_color = if spell_active {
                self.current_theme.accent
            } else if err_cnt > 0 {
                Color32::from_rgb(0xFF, 0xBD, 0x2E)
            } else {
                self.current_theme.text_muted
            };
            let btn_spell = ui.add(
                Button::new(
                    RichText::new("🔤")
                        .size(18.0)
                        .color(spell_color),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            ).on_hover_text(format!("Corrector Hunspell ({} palabras dudosas)", err_cnt));
            if btn_spell.clicked() {
                if self.activity_tab == ActivityTab::SpellCheck {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::SpellCheck;
                    self.sidebar_visible = true;
                }
            }

            // Quarto button
            let quarto_active = self.activity_tab == ActivityTab::Quarto && self.sidebar_visible;
            let is_qmd = self.doc_manager.active().is_quarto();
            let quarto_color = if quarto_active {
                Color32::from_rgb(0x40, 0x9E, 0xFF)
            } else if is_qmd {
                Color32::from_rgb(0x60, 0xBF, 0xFF)
            } else {
                self.current_theme.text_muted
            };
            let btn_quarto = ui.add(
                Button::new(
                    RichText::new("📊")
                        .size(18.0)
                        .color(quarto_color),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            ).on_hover_text("Estudio Quarto (.qmd): Crear, Renderizar e Previsualizar");
            if btn_quarto.clicked() {
                if self.activity_tab == ActivityTab::Quarto {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::Quarto;
                    self.sidebar_visible = true;
                }
            }

            // Themes selector button
            let themes_active = self.activity_tab == ActivityTab::Themes && self.sidebar_visible;
            let btn_themes = ui.add(
                Button::new(
                    RichText::new("🎨")
                        .size(18.0)
                        .color(if themes_active { self.current_theme.accent } else { self.current_theme.text_muted }),
                )
                .fill(Color32::TRANSPARENT)
                .min_size(Vec2::new(36.0, 36.0)),
            );
            if btn_themes.clicked() {
                if self.activity_tab == ActivityTab::Themes {
                    self.sidebar_visible = !self.sidebar_visible;
                } else {
                    self.activity_tab = ActivityTab::Themes;
                    self.sidebar_visible = true;
                }
            }

            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.add(Button::new(RichText::new("⚙").size(18.0).color(self.current_theme.text_muted)).fill(Color32::TRANSPARENT)).clicked() {
                    self.activity_tab = ActivityTab::Themes;
                    self.sidebar_visible = true;
                }
            });
        });
    }

    fn render_primary_sidebar(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        match self.activity_tab {
            ActivityTab::Explorer => {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("EXPLORADOR")
                            .size(11.0)
                            .strong()
                            .color(self.current_theme.text_muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("+ Qmd").size(10.5).strong().color(Color32::from_rgb(0x40, 0x9E, 0xFF))).on_hover_text("Nuevo documento Quarto (.qmd)").clicked() {
                            self.new_document(true, ctx);
                        }
                        if ui.button(RichText::new("+ Md").size(10.5).strong()).on_hover_text("Nuevo documento Markdown (.md)").clicked() {
                            self.new_document(false, ctx);
                        }
                        if ui.button(RichText::new("📂").size(11.0)).on_hover_text("Abrir archivo…").clicked() {
                            self.picker.open(PickerMode::OpenFile, self.workspace_root.as_deref(), "");
                        }
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                // Open editors section
                ui.label(
                    RichText::new("▾ EDITORES ABIERTOS")
                        .size(10.5)
                        .color(self.current_theme.text_muted)
                        .strong(),
                );
                ui.add_space(3.0);

                let mut to_select = None;
                let mut to_close = None;

                for (idx, doc) in self.doc_manager.documents.iter().enumerate() {
                    let is_active = idx == self.doc_manager.active_index;
                    let star = if doc.is_dirty() { " ●" } else { "" };
                    let icon = if doc.is_quarto() { "📊" } else { "📄" };
                    let label = format!("{} {}{}", icon, doc.title, star);

                    ui.horizontal(|ui| {
                        let btn = ui.add(
                            Button::new(
                                RichText::new(label).size(12.0).color(
                                    if is_active {
                                        self.current_theme.accent
                                    } else {
                                        self.current_theme.text_body
                                    },
                                ),
                            )
                            .fill(if is_active { self.current_theme.selection_bg } else { Color32::TRANSPARENT }),
                        );

                        if btn.clicked() {
                            to_select = Some(idx);
                        }

                        if ui.button(RichText::new("×").size(11.0)).on_hover_text("Cerrar pestaña").clicked() {
                            to_close = Some(idx);
                        }
                    });
                }

                if let Some(idx) = to_select {
                    self.select_document(idx);
                }
                if let Some(idx) = to_close {
                    self.close_tab(idx, ctx);
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(6.0);

                // Real workspace folder section
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("▾ CARPETA DE TRABAJO")
                            .size(10.5)
                            .color(self.current_theme.text_muted)
                            .strong(),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button(RichText::new("⟳").size(11.0)).on_hover_text("Actualizar").clicked() {
                            self.reload_workspace();
                        }
                        if ui.button(RichText::new("…").size(11.0)).on_hover_text("Cambiar carpeta").clicked() {
                            self.picker.open(PickerMode::OpenFolder, self.workspace_root.as_deref(), "");
                        }
                    });
                });
                ui.add_space(2.0);

                match self.workspace_root.clone() {
                    Some(root) => {
                        ui.label(
                            RichText::new(root.display().to_string())
                                .size(10.0)
                                .color(self.current_theme.text_muted)
                                .monospace(),
                        );
                        ui.add_space(4.0);

                        let mut open_workspace_file: Option<PathBuf> = None;
                        let mut enter_dir: Option<PathBuf> = None;
                        ScrollArea::vertical()
                            .id_salt("workspace_scroll")
                            .max_height(ui.available_height() - 8.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut dirs: Vec<files::WorkspaceEntry> = Vec::new();
                                let mut docs: Vec<files::WorkspaceEntry> = Vec::new();
                                for entry in &self.workspace_entries {
                                    if entry.is_dir {
                                        dirs.push(entry.clone());
                                    } else {
                                        docs.push(entry.clone());
                                    }
                                }
                                for entry in dirs {
                                    let label = format!("📁 {}/", entry.name);
                                    if ui.add(
                                        Button::new(RichText::new(label).size(11.5).color(self.current_theme.text_body))
                                            .fill(Color32::TRANSPARENT),
                                    ).on_hover_text(entry.path.display().to_string()).clicked() {
                                        enter_dir = Some(entry.path);
                                    }
                                }
                                if docs.is_empty() && dirs_count(&self.workspace_entries) == 0 {
                                    ui.label(
                                        RichText::new("No hay documentos .md/.qmd aquí")
                                            .italics()
                                            .size(11.0)
                                            .color(self.current_theme.text_muted),
                                    );
                                }
                                for entry in docs {
                                    let is_open = self.doc_manager.documents.iter().any(|d| {
                                        d.file_path.as_deref() == Some(entry.path.as_path())
                                    });
                                    let icon = if entry.is_markdown() && entry.path.extension().and_then(|e| e.to_str()) == Some("qmd") { "📊" } else { "📄" };
                                    let color = if is_open { self.current_theme.accent } else { self.current_theme.text_body };
                                    if ui.add(
                                        Button::new(RichText::new(format!("  {} {}", icon, entry.name)).size(11.5).color(color))
                                            .fill(Color32::TRANSPARENT),
                                    ).on_hover_text(entry.path.display().to_string()).clicked() {
                                        open_workspace_file = Some(entry.path);
                                    }
                                }
                            });

                        if let Some(dir) = enter_dir {
                            self.set_workspace(dir, ctx);
                        }
                        if let Some(path) = open_workspace_file {
                            self.open_file_path(&path, ctx);
                        }
                    }
                    None => {
                        ui.label(RichText::new("Sin carpeta seleccionada").italics().color(self.current_theme.text_muted));
                        if ui.button("Elegir carpeta de trabajo…").clicked() {
                            self.picker.open(PickerMode::OpenFolder, None, "");
                        }
                    }
                }
            }
            ActivityTab::Themes => {
                ui.label(
                    RichText::new("SELECTOR DE TEMAS DE COLOR")
                        .size(11.0)
                        .strong()
                        .color(self.current_theme.text_muted),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(8.0);

                ScrollArea::vertical().id_salt("themes_sidebar_scroll").show(ui, |ui| {
                    for t in ColorPalette::all() {
                        let is_active = t.id == self.current_theme.id;
                        let frame = Frame::none()
                            .fill(if is_active { self.current_theme.selection_bg } else { self.current_theme.code_bg })
                            .stroke(Stroke::new(
                                if is_active { 1.5_f32 } else { 1.0_f32 },
                                if is_active { self.current_theme.accent } else { self.current_theme.border },
                            ))
                            .rounding(Rounding::same(6.0_f32))
                            .inner_margin(Margin::same(8.0_f32));

                        frame.show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Color swatches
                                let (r1, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                                ui.painter().rect_filled(r1, Rounding::same(2.0_f32), t.bg_color);

                                let (r2, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                                ui.painter().rect_filled(r2, Rounding::same(2.0_f32), t.sidebar_bg);

                                let (r3, _) = ui.allocate_exact_size(Vec2::new(12.0, 12.0), egui::Sense::hover());
                                ui.painter().rect_filled(r3, Rounding::same(2.0_f32), t.accent);

                                ui.add_space(4.0);
                                if ui.add(
                                    Button::new(
                                        RichText::new(t.name)
                                            .size(12.5)
                                            .strong()
                                            .color(if is_active { self.current_theme.accent } else { self.current_theme.text_primary }),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                ).clicked() {
                                    self.set_theme(t.id, ctx);
                                }
                            });
                        });
                        ui.add_space(5.0);
                    }
                });
            }
            ActivityTab::Search => {
                ui.label(
                    RichText::new("BUSCAR Y REEMPLAZAR")
                        .size(11.0)
                        .strong()
                        .color(self.current_theme.text_muted),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                ui.label(RichText::new("Buscar:").size(11.5));
                let search_resp = ui.add(
                    TextEdit::singleline(&mut self.search_query)
                        .desired_width(ui.available_width())
                        .hint_text("texto a buscar"),
                );
                ui.add_space(4.0);
                ui.label(RichText::new("Reemplazar por:").size(11.5));
                ui.add(
                    TextEdit::singleline(&mut self.replace_query)
                        .desired_width(ui.available_width())
                        .hint_text("texto de reemplazo"),
                );
                ui.add_space(4.0);
                ui.checkbox(&mut self.search_case_sensitive, "Distinguir mayúsculas");

                // Live match list
                let matches = search_matches(
                    &self.doc_manager.active().content,
                    &self.search_query,
                    self.search_case_sensitive,
                );
                let total: usize = matches.iter().map(|(_, n)| *n).sum();
                ui.add_space(6.0);
                ui.label(
                    RichText::new(if self.search_query.is_empty() {
                        "Escribe para buscar".to_string()
                    } else if total == 0 {
                        "Sin coincidencias".to_string()
                    } else {
                        format!("{} coincidencia(s) en {} línea(s)", total, matches.len())
                    })
                    .size(11.0)
                    .color(if total == 0 { self.current_theme.text_muted } else { self.current_theme.accent }),
                );

                let mut jump_to: Option<usize> = None;
                let mut do_replace_all = false;
                ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .max_height((ui.available_height() - 70.0).max(120.0))
                    .show(ui, |ui| {
                        for (line_no, count) in matches.iter().take(200) {
                            let text = self
                                .doc_manager
                                .active()
                                .content
                                .lines()
                                .nth(line_no.saturating_sub(1))
                                .unwrap_or("")
                                .trim()
                                .to_string();
                            let label = format!("{}  {}", line_no, truncate_chars(&text, 46));
                            if ui
                                .add(
                                    Button::new(
                                        RichText::new(label)
                                            .size(11.0)
                                            .monospace()
                                            .color(self.current_theme.text_body),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                )
                                .on_hover_text(format!("{} coincidencia(s) — ir a la vista previa", count))
                                .clicked()
                            {
                                jump_to = Some(*line_no);
                            }
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reemplazar todo").clicked() {
                        do_replace_all = true;
                    }
                    if ui.button("Limpiar").clicked() {
                        self.search_query.clear();
                        self.replace_query.clear();
                    }
                });

                if let Some(line) = jump_to {
                    self.scroll_preview_to_line = Some(line);
                    self.view_mode = ViewMode::Split;
                }

                if do_replace_all && !self.search_query.is_empty() {
                    let previous = self.begin_programmatic_edit();
                    let count = replace_all_matches(
                        &mut self.doc_manager.active_mut().content,
                        &self.search_query,
                        &self.replace_query,
                        self.search_case_sensitive,
                    );
                    self.end_programmatic_edit(ctx, previous);
                    self.notify(format!("{} reemplazo(s) aplicados", count), ctx);
                }

                if search_resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if let Some((line, _)) = matches.first() {
                        self.scroll_preview_to_line = Some(*line);
                    }
                }
            }
            ActivityTab::Outline => {
                ui.label(
                    RichText::new("ESQUEMA DEL DOCUMENTO")
                        .size(11.0)
                        .strong()
                        .color(self.current_theme.text_muted),
                );
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                let content = &self.doc_manager.active().content;
                let headings = markdown::extract_headings(content);
                let mut jump_to: Option<usize> = None;

                ScrollArea::vertical().id_salt("outline_sidebar_scroll").show(ui, |ui| {
                    if headings.is_empty() {
                        ui.label(RichText::new("Sin encabezados").italics().color(self.current_theme.text_muted));
                    } else {
                        for h in headings {
                            let indent = (h.level.saturating_sub(1) as f32) * 12.0;
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                let badge = match h.level {
                                    1 => "H1",
                                    2 => "H2",
                                    3 => "H3",
                                    _ => "H",
                                };
                                let label = ui.add(
                                    Button::new(
                                        RichText::new(format!("{}  {}", badge, h.title))
                                            .size(11.5)
                                            .color(self.current_theme.text_body),
                                    )
                                    .fill(Color32::TRANSPARENT),
                                ).on_hover_text(format!("Ir al encabezado (línea {})", h.line_number));
                                if label.clicked() {
                                    jump_to = Some(h.line_number);
                                }
                            });
                        }
                    }
                });

                if let Some(line) = jump_to {
                    self.scroll_preview_to_line = Some(line);
                    self.view_mode = if self.view_mode == ViewMode::Source { ViewMode::Split } else { self.view_mode };
                }
            }
            ActivityTab::SpellCheck => {
                self.render_spellcheck_panel(ui, ctx);
            }
            ActivityTab::Quarto => {
                self.render_quarto_panel(ui, ctx);
            }
        }
    }

    fn render_spellcheck_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("CORRECTOR HUNSPELL")
                    .size(11.0)
                    .strong()
                    .color(self.current_theme.text_muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(RichText::new("🔄 Re-examinar").size(10.0)).clicked() {
                    let content = self.doc_manager.active().content.clone();
                    self.hunspell.check_document(&content);
                    self.notify("Documento re-examinado con Hunspell", ctx);
                }
            });
        });
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // Language Selector
        ui.horizontal(|ui| {
            ui.label(RichText::new("Idioma:").size(12.0).color(self.current_theme.text_body));
            for lang in SpellLanguage::all() {
                let is_sel = self.hunspell.active_language() == lang;
                if ui.add(Button::new(RichText::new(lang.flag()).size(12.0)).selected(is_sel)).clicked() {
                    self.hunspell.set_language(lang);
                    let content = self.doc_manager.active().content.clone();
                    self.hunspell.check_document(&content);
                    self.notify(format!("Diccionario Hunspell: {}", lang.name()), ctx);
                }
            }
            ui.label(
                RichText::new(self.hunspell.active_language().name())
                    .size(11.5)
                    .color(self.current_theme.accent),
            );
        });

        ui.add_space(8.0);

        let err_count = self.hunspell.error_count();
        if err_count == 0 {
            Frame::none()
                .fill(self.current_theme.code_bg)
                .rounding(Rounding::same(6.0_f32))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x27, 0xC9, 0x3F)))
                .inner_margin(Margin::same(12.0_f32))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("✓").size(16.0).color(Color32::from_rgb(0x27, 0xC9, 0x3F)).strong());
                        ui.label(
                            RichText::new("Sin faltas ortográficas detectadas")
                                .size(12.5)
                                .color(self.current_theme.text_primary),
                        );
                    });
                });
        } else {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("⚠ {} palabra(s) dudosas:", err_count))
                        .size(12.0)
                        .color(Color32::from_rgb(0xFF, 0xBD, 0x2E))
                        .strong(),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("✨ Corregir Todo").size(10.5).strong().color(Color32::from_rgb(0x27, 0xC9, 0x3F))).clicked() {
                        let previous = self.begin_programmatic_edit();
                        let doc = self.doc_manager.active_mut();
                        let report = self.hunspell.fix_all_errors(&mut doc.content);
                        self.end_programmatic_edit(ctx, previous);
                        self.notify(
                            format!("Corregidas {} palabra(s); {} omitida(s) por seguridad", report.fixed, report.skipped),
                            ctx,
                        );
                    }
                });
            });
            ui.checkbox(&mut self.spell_highlight_enabled, "Subrayar faltas en rojo en editor");
            ui.add_space(6.0);

            let errors = self.hunspell.errors().to_vec();
            let mut to_replace = None;
            let mut to_add_dict = None;
            let mut to_ignore = None;

            ScrollArea::vertical().id_salt("spellcheck_sidebar_scroll").show(ui, |ui| {
                for err in &errors {
                    Frame::none()
                        .fill(self.current_theme.code_bg)
                        .rounding(Rounding::same(4.0_f32))
                        .stroke(Stroke::new(1.0_f32, self.current_theme.border))
                        .inner_margin(Margin::same(8.0_f32))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&err.word)
                                        .size(13.0)
                                        .strong()
                                        .color(Color32::from_rgb(0xFF, 0x5F, 0x56)),
                                );
                                ui.label(
                                    RichText::new(format!("(Línea {})", err.line))
                                        .size(10.5)
                                        .color(self.current_theme.text_muted),
                                );
                            });

                            ui.add_space(4.0);
                            if !err.suggestions.is_empty() {
                                ui.label(
                                    RichText::new("Sugerencias:")
                                        .size(11.0)
                                        .color(self.current_theme.text_body),
                                );
                                ui.horizontal_wrapped(|ui| {
                                    for sugg in &err.suggestions {
                                        if ui.button(RichText::new(sugg).size(11.0).color(self.current_theme.accent)).clicked() {
                                            to_replace = Some((err.clone(), sugg.clone()));
                                        }
                                    }
                                });
                            } else {
                                ui.label(
                                    RichText::new("Sin sugerencias directas")
                                        .size(10.5)
                                        .italics()
                                        .color(self.current_theme.text_muted),
                                );
                            }

                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new("+ Diccionario").size(10.0)).clicked() {
                                    to_add_dict = Some(err.word.clone());
                                }
                                if ui.button(RichText::new("Ignorar").size(10.0)).clicked() {
                                    to_ignore = Some(err.word.clone());
                                }
                            });
                        });
                    ui.add_space(6.0);
                }
            });

            if let Some((err, replacement)) = to_replace {
                let previous = self.begin_programmatic_edit();
                let doc = self.doc_manager.active_mut();
                self.hunspell.replace_word(&mut doc.content, &err, &replacement);
                self.end_programmatic_edit(ctx, previous);
                self.notify(format!("Reemplazado '{}' por '{}'", err.word, replacement), ctx);
            }
            if let Some(word) = to_add_dict {
                self.hunspell.add_to_personal_dict(&word);
                self.notify(format!("'{}' añadido al diccionario personal", word), ctx);
            }
            if let Some(word) = to_ignore {
                self.hunspell.ignore_word(&word);
                self.notify(format!("'{}' ignorado", word), ctx);
            }
        }
    }

    fn render_quarto_panel(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("ESTUDIO QUARTO (.QMD)")
                    .size(11.0)
                    .strong()
                    .color(Color32::from_rgb(0x40, 0x9E, 0xFF)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(RichText::new("➕ Novo .qmd").size(10.5).strong().color(Color32::from_rgb(0x40, 0x9E, 0xFF))).clicked() {
                    self.doc_manager.add_new_quarto();
                    self.notify("Novo ficheiro Quarto (.qmd) creado", ctx);
                }
            });
        });

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(8.0);

        let doc = self.doc_manager.active();
        let is_qmd = doc.is_quarto();

        // Document Status Card
        Frame::none()
            .fill(self.current_theme.code_bg)
            .rounding(Rounding::same(6.0_f32))
            .stroke(Stroke::new(1.0_f32, if is_qmd { Color32::from_rgb(0x40, 0x9E, 0xFF) } else { self.current_theme.border }))
            .inner_margin(Margin::same(10.0_f32))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if is_qmd { "📊 Ficheiro Quarto activo:" } else { "📄 Ficheiro estándar:" }).size(11.0).strong().color(self.current_theme.text_muted));
                });
                ui.add_space(3.0);
                ui.label(RichText::new(&doc.title).size(13.0).strong().color(self.current_theme.text_primary));
                ui.add_space(4.0);

                if let Some((meta, _)) = quarto::parse_quarto_frontmatter(&doc.content) {
                    ui.label(RichText::new(format!("Formato: {} | Palabras: {}", meta.format, doc.word_count())).size(11.0).color(self.current_theme.accent));
                } else {
                    ui.label(RichText::new(format!("Palabras: {} | Liñas: {}", doc.word_count(), doc.line_count())).size(11.0).color(self.current_theme.text_muted));
                }
            });

        ui.add_space(10.0);

        // Big Render & Preview Button
        let render_btn = ui.add(
            Button::new(
                RichText::new("▶ Renderizar e Previsualizar Quarto (HTML)")
                    .size(12.0)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(0x1E, 0x88, 0xE5))
            .rounding(Rounding::same(5.0_f32))
            .min_size(Vec2::new(ui.available_width(), 32.0)),
        );
        if render_btn.clicked() {
            self.execute_quarto_render_preview(ctx);
        }

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("CLI:").size(10.5).color(self.current_theme.text_muted));
            let cli = has_quarto_cli();
            for format in QuartoFormat::all() {
                let button = ui.add_enabled(
                    cli,
                    Button::new(RichText::new(format.label()).size(10.5)),
                ).on_hover_text(format!("Renderizar con Quarto CLI → {}", format.label()));
                if button.clicked() {
                    self.execute_quarto_cli_export(format, ctx);
                }
            }
            if !cli {
                ui.label(
                    RichText::new("(quarto non instalado)")
                        .size(10.0)
                        .italics()
                        .color(self.current_theme.text_muted),
                );
            }
        });

        let split_btn = ui.add(
            Button::new(
                RichText::new("👁 Activar Previsualización Dividida")
                    .size(11.5)
                    .color(self.current_theme.text_primary),
            )
            .rounding(Rounding::same(4.0_f32))
            .min_size(Vec2::new(ui.available_width(), 26.0)),
        );
        if split_btn.clicked() {
            self.view_mode = ViewMode::Split;
            self.notify("Vista previa dividida activada", ctx);
        }

        ui.add_space(10.0);

        // Python Jupyter Kernel Status & Execution
        ui.label(RichText::new("▾ KERNEL PYTHON (JUPYTER)").size(10.5).color(Color32::from_rgb(0x40, 0x9E, 0xFF)).strong());
        ui.add_space(4.0);

        Frame::none()
            .fill(self.current_theme.code_bg)
            .rounding(Rounding::same(6.0_f32))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x9E, 0xFF)))
            .inner_margin(Margin::same(10.0_f32))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let has_py = self.quarto_kernel.python_binary.is_some();
                    ui.label(RichText::new(if has_py { "●" } else { "○" }).color(if has_py { Color32::from_rgb(0x27, 0xC9, 0x3F) } else { Color32::from_rgb(0xFF, 0x5F, 0x56) }));
                    ui.label(RichText::new(&self.quarto_kernel.python_version).size(11.5).strong().color(self.current_theme.text_primary));
                });
                if let Some(bin) = &self.quarto_kernel.python_binary {
                    ui.label(RichText::new(bin).size(9.5).monospace().color(self.current_theme.text_muted));
                }
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    let np_badge = if self.quarto_kernel.has_numpy { "✓ NumPy" } else { "✗ NumPy" };
                    let plt_badge = if self.quarto_kernel.has_matplotlib { "✓ Matplotlib" } else { "✗ Matplotlib" };
                    ui.label(RichText::new(format!("{} | {}", np_badge, plt_badge)).size(10.5).color(self.current_theme.accent));
                });
                ui.add_space(2.0);
                ui.label(RichText::new(format!("Celas executadas: {} (Out [{}])", self.quarto_kernel.cell_outputs.len(), self.quarto_kernel.execution_counter)).size(10.5).color(self.current_theme.text_muted));
                if self.quarto_kernel.is_busy() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Executando cela en segundo plano…").size(10.5).color(Color32::from_rgb(0xFF, 0xBD, 0x2E)));
                    });
                }
                if let Some(err) = &self.quarto_kernel.last_session_error {
                    ui.label(RichText::new(format!("⚠ {}", err)).size(10.0).color(Color32::from_rgb(0xFF, 0x5F, 0x56)));
                }
            });

        ui.add_space(8.0);

        let run_all_btn = ui.add(
            Button::new(
                RichText::new("▶ Executar todas as celas Python")
                    .size(11.5)
                    .strong()
                    .color(Color32::WHITE),
            )
            .fill(Color32::from_rgb(0x27, 0xC9, 0x3F))
            .rounding(Rounding::same(4.0_f32))
            .min_size(Vec2::new(ui.available_width(), 28.0)),
        );
        if run_all_btn.clicked() {
            let content = self.doc_manager.active().content.clone();
            let doc_dir = self.doc_manager.active().directory();
            let count = self.quarto_kernel.execute_all_cells(&content, doc_dir.as_deref());
            self.notify(
                if count == 0 {
                    "Non hai celas Python neste documento".to_string()
                } else {
                    format!("{} cela(s) Python en cola (execución en segundo plano)", count)
                },
                ctx,
            );
        }

        ui.add_space(4.0);

        let clear_btn = ui.add(
            Button::new(
                RichText::new("↺ Reiniciar kernel / Limpar saídas")
                    .size(11.0)
                    .color(self.current_theme.text_muted),
            )
            .rounding(Rounding::same(4.0_f32))
            .min_size(Vec2::new(ui.available_width(), 24.0)),
        );
        if clear_btn.clicked() {
            self.quarto_kernel.clear_outputs();
            self.notify("Kernel de Python reiniciado e saídas limpadas", ctx);
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);

        // Quick Snippets
        ui.label(RichText::new("▾ INSERTAR ELEMENTOS QUARTO").size(10.5).color(self.current_theme.text_muted).strong());
        ui.add_space(4.0);

        if ui.button(RichText::new("➕ Bloque Executable Python {python}").size(11.5)).clicked() {
            self.insert_markup_at_cursor("```{python}\n#| label: fig-1\n#| echo: true\n", "\n```", ctx);
        }
        if ui.button(RichText::new("➕ Bloque Executable R {r}").size(11.5)).clicked() {
            self.insert_markup_at_cursor("```{r}\n#| label: analise-1\n#| echo: true\n", "\n```", ctx);
        }
        if ui.button(RichText::new("➕ Caixa de Aviso (Callout Note)").size(11.5)).clicked() {
            self.insert_markup_at_cursor("::: {.callout-note}\n### Nota\n", "\n:::", ctx);
        }
        if ui.button(RichText::new("➕ Caixa de Consello (Callout Tip)").size(11.5)).clicked() {
            self.insert_markup_at_cursor("::: {.callout-tip}\n### Consello\n", "\n:::", ctx);
        }
        if ui.button(RichText::new("➕ Caixa de Alerta (Callout Warning)").size(11.5)).clicked() {
            self.insert_markup_at_cursor("::: {.callout-warning}\n### Aviso\n", "\n:::", ctx);
        }
        if ui.button(RichText::new("➕ Cabeceira YAML de Quarto").size(11.5)).clicked() {
            if !self.doc_manager.active().content.starts_with("---") {
                let previous = self.begin_programmatic_edit();
                let clean_title = {
                    let title = self.doc_manager.active().title.clone();
                    title
                        .trim_end_matches(".qmd")
                        .trim_end_matches(".md")
                        .to_string()
                };
                let body = std::mem::take(&mut self.doc_manager.active_mut().content);
                self.doc_manager.active_mut().content = format!(
                    "---\ntitle: \"{}\"\nauthor: \"Autor\"\ndate: \"today\"\nformat:\n  html:\n    toc: true\n---\n\n{}",
                    clean_title, body
                );
                self.end_programmatic_edit(ctx, previous);
                self.notify("Cabeceira YAML de Quarto engadida", ctx);
            }
        }

        ui.add_space(14.0);
        ui.separator();
        ui.add_space(8.0);

        // Engine status
        ui.horizontal(|ui| {
            ui.label(RichText::new("▾ MOTOR DE RENDERIZADO").size(10.5).color(self.current_theme.text_muted).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(RichText::new("⟳").size(10.5)).on_hover_text("Volver a detectar o CLI de Quarto").clicked() {
                    let found = quarto::refresh_quarto_cli();
                    self.notify(
                        if found { "Quarto CLI detectado" } else { "Quarto CLI non está instalado" },
                        ctx,
                    );
                }
            });
        });
        ui.add_space(4.0);
        if has_quarto_cli() {
            Frame::none()
                .fill(self.current_theme.code_bg)
                .rounding(Rounding::same(4.0_f32))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x27, 0xC9, 0x3F)))
                .inner_margin(Margin::same(8.0_f32))
                .show(ui, |ui| {
                    ui.label(RichText::new("✓ Quarto CLI detectado no sistema").size(11.0).color(Color32::from_rgb(0x27, 0xC9, 0x3F)));
                });
        } else {
            Frame::none()
                .fill(self.current_theme.code_bg)
                .rounding(Rounding::same(4.0_f32))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0x40, 0x9E, 0xFF)))
                .inner_margin(Margin::same(8.0_f32))
                .show(ui, |ui| {
                    ui.label(RichText::new("ℹ Motor nativo integrado activo").size(11.0).color(Color32::from_rgb(0x40, 0x9E, 0xFF)));
                    ui.label(RichText::new("Xera e previsualiza HTML Quarto con fórmulas KaTeX e callouts sen depender de librarías externas.").size(10.5).color(self.current_theme.text_muted));
                });
        }
    }

    fn render_editor_workspace(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        // ------------------------------------------------------------------
        // Row 1: editor tabs (horizontally scrollable so many open documents
        // never widen the central panel past the window).
        // ------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(1.0, 0.0);

            let mut tab_select = None;
            let mut tab_close = None;

            egui::ScrollArea::horizontal()
                .id_salt("editor_tabs_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (idx, doc) in self.doc_manager.documents.iter().enumerate() {
                            let is_active = idx == self.doc_manager.active_index;
                            let bg = if is_active {
                                self.current_theme.tab_active_bg
                            } else {
                                self.current_theme.tab_inactive_bg
                            };

                            let dirty = doc.is_dirty();
                            let star = if dirty { " ●" } else { "" };
                            let icon = if doc.is_quarto() { "📊" } else { "📄" };
                            let title = format!("{} {}{}", icon, doc.title, star);

                            let frame = Frame::none()
                                .fill(bg)
                                .stroke(Stroke::new(1.0_f32, self.current_theme.border))
                                .inner_margin(Margin::symmetric(8.0_f32, 5.0_f32));

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let label = ui.add(
                                        Button::new(
                                            RichText::new(title).size(12.0).color(
                                                if is_active { self.current_theme.text_primary } else { self.current_theme.text_muted },
                                            ),
                                        )
                                        .fill(Color32::TRANSPARENT),
                                    );
                                    if label.clicked() {
                                        tab_select = Some(idx);
                                    }
                                    let close = ui.add(
                                        Button::new(RichText::new(if dirty { "●" } else { "×" }).size(10.0).color(
                                            if dirty { Color32::from_rgb(0xFF, 0xBD, 0x2E) } else { self.current_theme.text_muted },
                                        ))
                                        .fill(Color32::TRANSPARENT),
                                    ).on_hover_text(if dirty { "Cambios sin guardar — pulsa para cerrar" } else { "Cerrar pestaña" });
                                    if close.clicked() {
                                        tab_close = Some(idx);
                                    }
                                });
                            });
                        }
                    });
                });

            if let Some(idx) = tab_select {
                self.select_document(idx);
            }
            if let Some(idx) = tab_close {
                self.close_tab(idx, ctx);
            }
        });

        // ------------------------------------------------------------------
        // Row 2: quick formatting toolbar + Typora stylesheet selector.
        // ------------------------------------------------------------------
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

            if ui.button(RichText::new("B").strong().size(12.0)).on_hover_text("Negrita").clicked() {
                self.insert_markup_at_cursor("**", "**", ctx);
            }
            if ui.button(RichText::new("I").italics().size(12.0)).on_hover_text("Cursiva").clicked() {
                self.insert_markup_at_cursor("*", "*", ctx);
            }
            if ui.button(RichText::new("</>").monospace().size(11.0)).on_hover_text("Código en línea").clicked() {
                self.insert_markup_at_cursor("`", "`", ctx);
            }
            if ui.button(RichText::new("H1").size(11.0).strong()).on_hover_text("Encabezado de nivel 1").clicked() {
                self.insert_markup_at_cursor("# ", "", ctx);
            }
            if ui.button(RichText::new("∑").size(12.0).strong().color(self.current_theme.accent)).on_hover_text("Fórmula KaTeX en bloque").clicked() {
                self.insert_markup_at_cursor("$$\n", "\n$$", ctx);
            }
            if ui.button(RichText::new("{ } Quarto").size(11.0).color(Color32::from_rgb(0x40, 0x9E, 0xFF))).on_hover_text("Bloque executable Python {python}").clicked() {
                self.insert_markup_at_cursor("```{python}\n#| label: fig-1\n#| echo: true\n", "\n```", ctx);
            }
            if ui.button(RichText::new("::: Callout").size(11.0).color(Color32::from_rgb(0x40, 0x9E, 0xFF))).on_hover_text("Caixa de aviso de Quarto").clicked() {
                self.insert_markup_at_cursor("::: {.callout-note}\n### Nota\n", "\n:::", ctx);
            }
            ui.menu_button("+", |ui| {
                if ui.button("Tabla").clicked() {
                    self.insert_markup_at_cursor("| Col 1 | Col 2 |\n| :--- | :--- |\n| 1 | 2 |", "", ctx);
                    ui.close_menu();
                }
                if ui.button("Cita en bloque").clicked() {
                    self.insert_markup_at_cursor("> ", "", ctx);
                    ui.close_menu();
                }
                if ui.button("Lista de tareas").clicked() {
                    self.insert_markup_at_cursor("- [ ] ", "", ctx);
                    ui.close_menu();
                }
                if ui.button("Enlace").clicked() {
                    self.insert_text_at_cursor("[texto](https://)", 1, ctx);
                    ui.close_menu();
                }
                if ui.button("Imagen").clicked() {
                    self.insert_text_at_cursor("![descripción](imagen.png)", 2, ctx);
                    ui.close_menu();
                }
                if ui.button("Fórmula química (mhchem)").clicked() {
                    self.insert_markup_at_cursor("$$\n\\ce{", "}\n$$", ctx);
                    ui.close_menu();
                }
                if ui.button("Regla horizontal").clicked() {
                    self.insert_markup_at_cursor("\n---\n", "", ctx);
                    ui.close_menu();
                }
            }).response.on_hover_text("Más elementos Markdown / Quarto");

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                egui::ComboBox::from_id_salt("typora_stylesheet_combo")
                    .selected_text(RichText::new(format!("📋 {}", self.current_stylesheet.name())).size(11.0))
                    .width(130.0)
                    .show_ui(ui, |ui| {
                        for s in TyporaStylesheet::all() {
                            if ui.selectable_value(&mut self.current_stylesheet, s, s.name()).clicked() {
                                self.notify(format!("Hoja de estilo Typora: {}", s.name()), ctx);
                            }
                        }
                    });
                ui.label(
                    RichText::new(format!("{} palabras", self.doc_manager.active().word_count()))
                        .size(10.5)
                        .color(self.current_theme.text_muted),
                );
            });
        });

        // Breadcrumb bar
        Frame::none()
            .fill(self.current_theme.sidebar_bg)
            .stroke(Stroke::new(1.0_f32, self.current_theme.border))
            .inner_margin(Margin::symmetric(14.0_f32, 3.0_f32))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let doc = self.doc_manager.active();
                    let doc_type = if doc.is_quarto() { "Quarto (.qmd)" } else { "Markdown" };
                    let location = doc
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "sin guardar".to_string());
                    ui.label(
                        RichText::new(format!("{}  ›  {}  ›  {}  ›  Estilo [{}]", location, doc.title, doc_type, self.current_stylesheet.name()))
                            .size(11.0)
                            .color(self.current_theme.text_muted),
                    );
                });
            });

        // Quick-Fix Spellcheck Ribbon if errors exist
        if self.hunspell.error_count() > 0 && self.spell_highlight_enabled {
            let all_errors = self.hunspell.errors().to_vec();
            let mut errors = Vec::new();
            for err in all_errors {
                // Only offer one-click fixes that are plausible: the word must
                // not be valid in another installed dictionary, and the top
                // suggestion must be within one edit step ("Jupyter" is not a
                // typo for "Júpiter", but "correcion" is one for "corrección").
                let plausible = err
                    .suggestions
                    .first()
                    .is_some_and(|s| spellcheck::plausible_suggestion(&err.word, s));
                if plausible && !self.hunspell.word_valid_in_other_language(&err.word) {
                    errors.push(err);
                }
            }
            let err_count = errors.len();
            if err_count > 0 {

            Frame::none()
                .fill(Color32::from_rgb(0x2D, 0x18, 0x18))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(0xFF, 0x5F, 0x56)))
                .inner_margin(Margin::symmetric(14.0_f32, 4.0_f32))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("🔤 Hunspell ({} faltas):", err_count))
                                .size(11.5)
                                .strong()
                                .color(Color32::from_rgb(0xFF, 0xBD, 0x2E)),
                        );

                        let mut fix_item = None;
                        for err in errors.iter().take(3) {
                            if let Some(top_sugg) = err.suggestions.first() {
                                let label = format!("{} -> {}", err.word, top_sugg);
                                if ui.button(RichText::new(label).size(11.0).color(Color32::WHITE)).on_hover_text(format!("Reemplazar '{}' por '{}' en el documento", err.word, top_sugg)).clicked() {
                                    fix_item = Some((err.clone(), top_sugg.clone()));
                                }
                            }
                        }

                        if let Some((err, sugg)) = fix_item {
                            let previous = self.begin_programmatic_edit();
                            let doc = self.doc_manager.active_mut();
                            self.hunspell.replace_word(&mut doc.content, &err, &sugg);
                            self.end_programmatic_edit(ctx, previous);
                            self.notify(format!("Corregido '{}' por '{}'", err.word, sugg), ctx);
                        }

                        if errors.len() > 1 {
                            if ui.button(RichText::new("✨ Corregir Todo").size(11.0).strong().color(Color32::from_rgb(0x27, 0xC9, 0x3F))).on_hover_text("Aplica las mejores sugerencias a todas las faltas encontradas").clicked() {
                                let previous = self.begin_programmatic_edit();
                                let doc = self.doc_manager.active_mut();
                                let report = self.hunspell.fix_all_errors(&mut doc.content);
                                self.end_programmatic_edit(ctx, previous);
                                self.notify(format!("Corregidas {} palabra(s); {} omitida(s)", report.fixed, report.skipped), ctx);
                            }
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.button(RichText::new("Panel Completo »").size(10.5)).clicked() {
                                self.activity_tab = ActivityTab::SpellCheck;
                                self.sidebar_visible = true;
                            }
                        });
                    });
                });
            }
        }

        // Main Editor Area - Dynamically spans full available width and height (maximized or windowed)
        match self.view_mode {
            ViewMode::Live => {
                let doc_salt = self.doc_manager.active().id;
                ScrollArea::vertical()
                    .id_salt(("live_preview_scroll", doc_salt))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let total_w = ui.available_width();
                        let max_w = (total_w - 60.0).max(400.0);
                        let pad = ((total_w - max_w) / 2.0).max(20.0);

                        ui.horizontal(|ui| {
                            ui.add_space(pad);
                            ui.vertical(|ui| {
                                ui.set_max_width(max_w);
                                ui.set_min_width(max_w);
                                ui.add_space(16.0);
                                let content = self.doc_manager.active().content.clone();
                                let doc_dir = self.doc_manager.active().directory();
                                let is_quarto_doc = self.doc_manager.active().is_quarto();
                                let mut actions = Vec::new();
                                markdown::render_markdown_view(
                                    ui,
                                    &content,
                                    &self.current_theme,
                                    self.current_stylesheet,
                                    &mut self.math_renderer,
                                    &mut self.quarto_kernel,
                                    doc_dir.as_deref(),
                                    &mut self.preview_assets,
                                    &mut actions,
                                    &mut self.scroll_preview_to_line,
                                    is_quarto_doc,
                                );
                                self.apply_preview_actions(actions, ctx);
                                ui.add_space(60.0);
                            });
                            ui.add_space(pad);
                        });
                    });
            }
            ViewMode::Source => {
                let avail_w = ui.available_width();
                let avail_h = ui.available_height();
                self.render_editor_with_gutter(ui, avail_w, avail_h);
            }
            ViewMode::Split => {
                let avail_h = ui.available_height();

                ui.columns(2, |columns| {
                    // Left column: Source code editor with line numbers gutter filling full half
                    let left_ui = &mut columns[0];
                    left_ui.set_min_height(avail_h);
                    let left_w = left_ui.available_width();
                    self.render_editor_with_gutter(left_ui, left_w, avail_h);

                    // Crisp vertical divider line between editor and preview
                    let sep_x = left_ui.max_rect().right() + 4.0;
                    let top_y = left_ui.max_rect().top();
                    left_ui.painter().line_segment(
                        [egui::pos2(sep_x, top_y), egui::pos2(sep_x, top_y + avail_h)],
                        Stroke::new(1.0_f32, self.current_theme.border),
                    );

                    // Right column: rendered Markdown preview, padded inside
                    // the column so text never touches the window edge.
                    let right_ui = &mut columns[1];
                    right_ui.set_min_height(avail_h);
                    let right_w = right_ui.available_width();
                    let content_w = (right_w - 30.0).max(200.0);

                    let doc_salt = self.doc_manager.active().id;
                    ScrollArea::vertical()
                        .id_salt(("split_preview", doc_salt))
                        .auto_shrink([false, false])
                        .min_scrolled_height(avail_h)
                        .show(right_ui, |ui| {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(6.0);
                                ui.vertical(|ui| {
                                    ui.set_width(content_w);
                                    let content = self.doc_manager.active().content.clone();
                                    let doc_dir = self.doc_manager.active().directory();
                                    let is_quarto_doc = self.doc_manager.active().is_quarto();
                                    let mut actions = Vec::new();
                                    markdown::render_markdown_view(
                                        ui,
                                        &content,
                                        &self.current_theme,
                                        self.current_stylesheet,
                                        &mut self.math_renderer,
                                        &mut self.quarto_kernel,
                                        doc_dir.as_deref(),
                                        &mut self.preview_assets,
                                        &mut actions,
                                        &mut self.scroll_preview_to_line,
                                        is_quarto_doc,
                                    );
                                    self.apply_preview_actions(actions, ctx);
                                });
                                ui.add_space(8.0);
                            });
                            ui.add_space(50.0);
                        });
                });
            }
        }
    }

    fn render_editor_with_gutter(&mut self, ui: &mut Ui, width: f32, height: f32) {
        let doc_idx = self.doc_manager.active_index;
        let line_count = self.doc_manager.active().line_count();
        let theme = self.current_theme.clone();
        let flagged_words = self.hunspell.flagged_words_set();
        let highlight_spelling = self.spell_highlight_enabled;
        let errors = self.hunspell.errors().to_vec();
        let current_lang = self.hunspell.active_language();
        let editor_id = self.editor_id;
        let pending_caret = self.pending_caret.take();

        let min_lines = ((height / 19.5).ceil() as usize).max(40);
        let display_lines = line_count.max(min_lines);
        let mut gutter_text = String::new();
        for l in 1..=display_lines {
            gutter_text.push_str(&format!("{}\n", l));
        }

        let editor_min_h = height.max(400.0);
        let mut new_selection: Option<(usize, usize)> = None;
        let mut outcome = EditorContextOutcome::default();

        let doc_salt = self.doc_manager.active().id;
        ScrollArea::vertical()
            .id_salt(("editor_scroll", doc_salt))
            .auto_shrink([false, false])
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    // Line numbers gutter: it is only shown when wrapping is
                    // off, because then every visual row maps 1:1 to a source
                    // line (wrapped continuation rows would desync it).
                    if !self.word_wrap {
                    ui.vertical(|ui| {
                        ui.set_width(44.0);
                        ui.add_space(4.0);
                        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                            ui.add_space(6.0);
                            let gutter_color = if theme.is_dark {
                                theme.text_muted.linear_multiply(0.75)
                            } else {
                                theme.text_muted
                            };
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&gutter_text)
                                        .font(FontId::monospace(13.5))
                                        .color(gutter_color),
                                )
                                .selectable(false),
                            );
                        });
                    });

                    ui.separator();
                    }

                    // Horizontal scroll keeps long lines readable without
                    // breaking the 1:1 gutter mapping.
                    egui::ScrollArea::horizontal()
                        .id_salt(("editor_hscroll", doc_salt))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let content = &mut self.doc_manager.documents[doc_idx].content;
                            let longest_line_chars = content
                                .lines()
                                .map(|l| l.chars().count())
                                .max()
                                .unwrap_or(0);
                            let base_w = (width - 62.0).max(240.0);
                            let editor_w = if self.word_wrap {
                                ui.available_width().max(240.0)
                            } else {
                                base_w.max(longest_line_chars as f32 * 8.15 + 24.0)
                            };
                            let word_wrap = self.word_wrap;

                            let mut layouter = move |ui: &Ui, text: &str, wrap_width: f32| {
                                spellcheck::layout_editor_text(
                                    ui,
                                    text,
                                    wrap_width,
                                    &theme,
                                    &flagged_words,
                                    highlight_spelling,
                                    word_wrap,
                                )
                            };

                            let output = TextEdit::multiline(content)
                                .id(editor_id)
                                .font(FontId::monospace(13.5))
                                .code_editor()
                                .clip_text(false)
                                .desired_width(editor_w)
                                .min_size(Vec2::new(if word_wrap { editor_w } else { 0.0 }, editor_min_h))
                                .layouter(&mut layouter)
                                .show(ui);

                            let resp = output.response;

                            if let Some(range) = output.cursor_range {
                                new_selection = Some((
                                    range.primary.ccursor.index,
                                    range.secondary.ccursor.index,
                                ));
                            }
                            if let Some(caret) = pending_caret {
                                let mut state = output.state;
                                state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::one(egui::text::CCursor::new(caret)),
                                ));
                                state.store(ui.ctx(), editor_id);
                                ui.memory_mut(|m| m.request_focus(editor_id));
                                new_selection = Some((caret, caret));
                            }

                            let _ = resp.context_menu(|ui| {
                                context_menu_ui(
                                    ui,
                                    &self.current_theme,
                                    &errors,
                                    current_lang,
                                    highlight_spelling,
                                    &mut outcome,
                                );
                            });
                        });
                });
            });

        if let Some(selection) = new_selection {
            self.editor_selection = Some(selection);
        }

        let ctx_clone = ui.ctx().clone();

        if let Some((err, suggestion)) = outcome.replace.take() {
            let previous = self.begin_programmatic_edit();
            let doc = self.doc_manager.active_mut();
            self.hunspell.replace_word(&mut doc.content, &err, &suggestion);
            self.end_programmatic_edit(&ctx_clone, previous);
            self.notify(format!("Corregido '{}' -> '{}'", err.word, suggestion), &ctx_clone);
        }
        if let Some(word) = outcome.add_to_dictionary.take() {
            self.hunspell.add_to_personal_dict(&word);
            self.notify(format!("'{}' añadido al diccionario", word), &ctx_clone);
        }
        if let Some(word) = outcome.ignore.take() {
            self.hunspell.ignore_word(&word);
            self.notify(format!("'{}' ignorado en esta sesión", word), &ctx_clone);
        }
        if outcome.fix_all {
            let previous = self.begin_programmatic_edit();
            let doc = self.doc_manager.active_mut();
            let report = self.hunspell.fix_all_errors(&mut doc.content);
            self.end_programmatic_edit(&ctx_clone, previous);
            self.notify(
                format!(
                    "Corregidas {} palabra(s); {} omitida(s) por seguridad",
                    report.fixed, report.skipped
                ),
                &ctx_clone,
            );
        }
        if let Some(lang) = outcome.switch_language.take() {
            self.hunspell.set_language(lang);
            let content = self.doc_manager.active().content.clone();
            self.hunspell.check_document(&content);
            self.notify(format!("Idioma Hunspell: {}", lang.name()), &ctx_clone);
        }
        if outcome.toggle_highlight {
            self.spell_highlight_enabled = !self.spell_highlight_enabled;
        }
    }

    fn render_vscode_status_bar(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        let doc = self.doc_manager.active();
        let words = doc.word_count();
        let chars = doc.char_count();
        let lines = doc.line_count();

        let (cursor_line, cursor_col) = self.cursor_line_col();
        let dirty_marker = if doc.is_dirty() { "●" } else { "✓" };
        let file_label = doc
            .path()
            .map(files::file_title)
            .unwrap_or_else(|| doc.title.clone());

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(12.0, 0.0);

            // Left
            ui.label(RichText::new("⎇ main*").size(11.5).color(Color32::WHITE));
            ui.label(RichText::new(format!("{} {}", dirty_marker, file_label)).size(11.5).color(Color32::WHITE));
            ui.label(RichText::new(format!("{} palabras", words)).size(11.5).color(Color32::WHITE));
            ui.label(RichText::new(format!("{} caracteres", chars)).size(11.5).color(Color32::WHITE));

            if let Some((ref msg, _)) = self.status_notification {
                ui.separator();
                ui.label(RichText::new(format!("✓ {}", msg)).size(11.5).color(Color32::WHITE));
            }

            // Right
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(
                    Button::new(RichText::new(format!("🎨 {}", self.current_theme.name)).size(11.5).color(Color32::WHITE))
                        .fill(Color32::TRANSPARENT),
                ).clicked() {
                    self.activity_tab = ActivityTab::Themes;
                    self.sidebar_visible = true;
                }

                ui.separator();
                let err_count = self.hunspell.error_count();
                let lang = self.hunspell.active_language();
                let spell_label = if err_count == 0 {
                    format!("🔤 Hunspell: {} (✓ 0)", lang.code())
                } else {
                    format!("🔤 Hunspell: {} (⚠ {})", lang.code(), err_count)
                };
                if ui.add(
                    Button::new(RichText::new(spell_label).size(11.5).color(Color32::WHITE))
                        .fill(Color32::TRANSPARENT),
                ).on_hover_text("Abrir panel de Corrector Hunspell").clicked() {
                    self.activity_tab = ActivityTab::SpellCheck;
                    self.sidebar_visible = true;
                }

                ui.separator();
                let wrap_label = if self.word_wrap { "Ajuste: ON" } else { "Ajuste: OFF" };
                if ui.add(
                    Button::new(RichText::new(wrap_label).size(11.5).color(Color32::WHITE))
                        .fill(Color32::TRANSPARENT),
                ).on_hover_text("Ajuste de línea (los números de línea se ocultan al ajustar)").clicked() {
                    self.word_wrap = !self.word_wrap;
                }
                ui.separator();
                ui.label(RichText::new("UTF-8").size(11.5).color(Color32::WHITE));
                ui.separator();
                ui.label(RichText::new("Espacios: 2").size(11.5).color(Color32::WHITE));
                ui.separator();
                ui.label(RichText::new(format!("Ln {}, Col {}", cursor_line, cursor_col)).size(11.5).color(Color32::WHITE));
                ui.separator();
                ui.label(RichText::new(format!("{} líneas", lines)).size(11.5).color(Color32::WHITE));
                ui.separator();
                ui.label(RichText::new("Markdown").size(11.5).color(Color32::WHITE));
            });
        });
    }
}

fn dirs_count(entries: &[files::WorkspaceEntry]) -> usize {
    entries.iter().filter(|e| e.is_dir).count()
}

/// Picks the spellcheck dictionary from the document content, with the title as
/// a weak tie-breaker. This replaces matching on hard-coded sample titles and
/// fixes the "Spanish dictionary on a Galician document" false positives.
fn detect_document_language(content: &str, title: &str) -> SpellLanguage {
    spellcheck::detect_language(content, title)
}

fn truncate_chars(text: &str, max: usize) -> String {
    let mut out: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        out.push('…');
    }
    out
}

/// (1-based line number, number of matches in that line) pairs.
fn search_matches(content: &str, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let haystack = if case_sensitive { line.to_string() } else { line.to_lowercase() };
        let mut count = 0;
        let mut from = 0;
        while let Some(pos) = haystack[from..].find(&needle) {
            count += 1;
            from += pos + needle.len().max(1);
        }
        if count > 0 {
            out.push((idx + 1, count));
        }
    }
    out
}

/// Literal replace-all that reports how many replacements happened.
fn replace_all_matches(content: &mut String, query: &str, replacement: &str, case_sensitive: bool) -> usize {
    if query.is_empty() {
        return 0;
    }
    if case_sensitive {
        let count = content.matches(query).count();
        *content = content.replace(query, replacement);
        return count;
    }

    let mut out = String::with_capacity(content.len());
    let mut rest = content.as_str();
    let needle = query.to_lowercase();
    let mut count = 0;
    loop {
        let lower = rest.to_lowercase();
        match lower.find(&needle) {
            Some(pos) => {
                out.push_str(&rest[..pos]);
                out.push_str(replacement);
                rest = &rest[pos + needle.len()..];
                count += 1;
            }
            None => {
                out.push_str(rest);
                break;
            }
        }
    }
    *content = out;
    count
}

#[derive(Default)]
struct EditorContextOutcome {
    replace: Option<(spellcheck::SpellError, String)>,
    add_to_dictionary: Option<String>,
    ignore: Option<String>,
    fix_all: bool,
    switch_language: Option<SpellLanguage>,
    toggle_highlight: bool,
}

/// Right-click menu for the source editor. Kept as a free function so it can
/// run while the document content is mutably borrowed by the `TextEdit`.
fn context_menu_ui(
    ui: &mut Ui,
    theme: &ColorPalette,
    errors: &[spellcheck::SpellError],
    current_lang: SpellLanguage,
    highlight_spelling: bool,
    outcome: &mut EditorContextOutcome,
) {
    ui.set_min_width(240.0);
    ui.label(
        RichText::new(format!("🔤 Corrector Hunspell ({})", current_lang.name()))
            .strong()
            .size(12.0)
            .color(theme.accent),
    );
    ui.separator();

    if errors.is_empty() {
        ui.label(RichText::new("✓ Sin errores ortográficos detectados").size(11.5).color(Color32::from_rgb(0x27, 0xC9, 0x3F)));
    } else {
        ui.label(RichText::new("💡 Sugerencias de corrección rápida:").size(11.0).strong().color(theme.text_muted));
        for err in errors.iter().take(5) {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&err.word).strong().underline().color(Color32::from_rgb(0xFF, 0x5F, 0x56)));
                    ui.label(RichText::new(format!("(línea {})", err.line)).size(10.5).color(theme.text_muted));
                });
                if err.suggestions.is_empty() {
                    ui.label(RichText::new("Sin sugerencias directas").italics().size(10.5).color(theme.text_muted));
                } else {
                    for sugg in err.suggestions.iter().take(3) {
                        if ui.button(RichText::new(format!("-> {}", sugg)).size(11.5)).clicked() {
                            outcome.replace = Some((err.clone(), sugg.clone()));
                            ui.close_menu();
                        }
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("➕ Añadir").size(10.0)).clicked() {
                        outcome.add_to_dictionary = Some(err.word.clone());
                        ui.close_menu();
                    }
                    if ui.button(RichText::new("👁️ Ignorar").size(10.0)).clicked() {
                        outcome.ignore = Some(err.word.clone());
                        ui.close_menu();
                    }
                });
            });
        }

        if errors.len() > 1 {
            ui.separator();
            if ui.button(RichText::new("✨ Corregir todos los errores automáticamente").size(11.5).strong().color(Color32::from_rgb(0x27, 0xC9, 0x3F))).clicked() {
                outcome.fix_all = true;
                ui.close_menu();
            }
        }
    }

    ui.separator();
    ui.menu_button("🌐 Idioma de corrección", |ui| {
        for lang in SpellLanguage::all() {
            let is_active = current_lang == lang;
            let prefix = if is_active { "● " } else { "  " };
            if ui.button(format!("{}{}", prefix, lang.name())).clicked() {
                outcome.switch_language = Some(lang);
                ui.close_menu();
            }
        }
    });

    let sub_txt = if highlight_spelling {
        "✓ Subrayado ortográfico en rojo (Activo)"
    } else {
        "  Activar subrayado ortográfico en rojo"
    };
    if ui.button(sub_txt).clicked() {
        outcome.toggle_highlight = true;
        ui.close_menu();
    }

}

/// Adds a pre-edit snapshot to egui's per-widget undo stack, so the built-in
/// Cmd/Ctrl+Z also covers toolbar actions, replace-all and spelling fixes.
fn push_editor_undo(
    ctx: &egui::Context,
    editor_id: egui::Id,
    selection: Option<(usize, usize)>,
    previous: String,
) {
    // Very large documents would make 100 text snapshots expensive.
    if previous.len() > 1_500_000 {
        return;
    }
    let state = egui::text_edit::TextEditState::load(ctx, editor_id).unwrap_or_default();
    let (start, end) = selection.unwrap_or((0, 0));
    let range = egui::text::CCursorRange::two(
        egui::text::CCursor::new(start),
        egui::text::CCursor::new(end),
    );
    // `undoer()` hands out a clone of the Undoer value; it must be written back
    // through `set_undoer` for the widget to see the new undo point.
    let mut undoer = state.undoer();
    undoer.add_undo(&(range, previous));
    let mut state = state;
    state.set_undoer(undoer);
    state.store(ctx, editor_id);
}

#[cfg(test)]
mod undo_tests {
    use super::push_editor_undo;

    #[test]
    fn programmatic_edits_become_undo_points() {
        let ctx = egui::Context::default();
        let editor_id = egui::Id::new("undo_test_editor");
        let range = egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(0),
        );

        // Simulate the TextEdit having stored its current state.
        let mut state = egui::text_edit::TextEditState::default();
        state.cursor.set_char_range(Some(range));
        state.store(&ctx, editor_id);

        // A toolbar action turned "old text" into "new text".
        push_editor_undo(&ctx, editor_id, Some((0, 0)), "old text".to_string());

        let mut undoer = egui::text_edit::TextEditState::load(&ctx, editor_id)
            .expect("state persisted")
            .undoer();
        let current = (range, "new text".to_string());
        assert!(undoer.has_undo(&current));

        let restored = undoer.undo(&current).cloned().expect("undo restores content");
        assert_eq!(restored.1, "old text");
    }

    #[test]
    fn oversized_documents_skip_undo_snapshots() {
        let ctx = egui::Context::default();
        let editor_id = egui::Id::new("undo_test_large");
        let previous = "x".repeat(1_500_001);
        push_editor_undo(&ctx, editor_id, None, previous);
        let state = egui::text_edit::TextEditState::load(&ctx, editor_id);
        assert!(state.is_none(), "no state should be written for huge documents");
    }
}
