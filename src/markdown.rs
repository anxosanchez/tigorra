use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui::{
    text::LayoutJob, Align, Color32, FontId, Frame, Margin, Response, RichText, Rounding, Sense,
    Stroke, TextFormat, TextureHandle, Ui, Vec2,
};
use crate::export::TyporaStylesheet;
use crate::math::MathRenderer;
use crate::theme::ColorPalette;

#[derive(Clone, Debug)]
pub struct HeadingItem {
    pub level: usize,
    pub title: String,
    pub line_number: usize,
}

/// Interaction the preview asks the app to perform on the source document.
#[derive(Clone, Debug)]
pub enum PreviewAction {
    /// Toggle the task-list checkbox on this 1-based source line.
    ToggleTask(usize),
    /// Follow a link to a local Markdown/Quarto document.
    OpenFile(PathBuf),
}

/// Texture cache for images referenced from the preview. Keyed by resolved path
/// plus modification time so editing the image on disk refreshes the preview.
#[derive(Default)]
pub struct PreviewAssets {
    textures: HashMap<String, TextureHandle>,
    failed: HashMap<String, String>,
}

impl PreviewAssets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.failed.clear();
    }

    fn load(&mut self, ctx: &egui::Context, path: &Path) -> Result<TextureHandle, String> {
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let key = format!("{}#{}", path.display(), modified);

        if let Some(handle) = self.textures.get(&key) {
            return Ok(handle.clone());
        }
        if let Some(err) = self.failed.get(&key) {
            return Err(err.clone());
        }

        let result = (|| -> Result<TextureHandle, String> {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
            let rgba = image.to_rgba8();
            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            let pixels = rgba.into_raw();
            let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
            Ok(ctx.load_texture(key.clone(), color_image, egui::TextureOptions::LINEAR))
        })();

        match result {
            Ok(handle) => {
                self.textures.insert(key, handle.clone());
                Ok(handle)
            }
            Err(e) => {
                self.failed.insert(key, e.clone());
                Err(e)
            }
        }
    }
}

pub fn extract_headings(text: &str) -> Vec<HeadingItem> {
    let mut headings = Vec::new();
    let mut in_code_block = false;
    let mut in_math_block = false;
    let lines: Vec<&str> = text.lines().collect();
    let mut start_line = 0;

    // Skip YAML frontmatter if present
    if let Some((_, body_idx)) = crate::quarto::parse_quarto_frontmatter(text) {
        start_line = body_idx;
    }

    for (line_idx, line) in lines.iter().enumerate().skip(start_line) {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if trimmed.starts_with("$$") {
            in_math_block = !in_math_block;
            continue;
        }
        if in_math_block {
            continue;
        }

        if let Some((level, title)) = parse_heading(trimmed) {
            headings.push(HeadingItem {
                level,
                title,
                line_number: line_idx + 1,
            });
        }
    }

    headings
}

fn parse_heading(trimmed: &str) -> Option<(usize, String)> {
    let rest = trimmed.strip_prefix('#')?;
    let mut level = 1;
    let mut remaining = rest;
    while let Some(stripped) = remaining.strip_prefix('#') {
        if level >= 6 {
            break;
        }
        level += 1;
        remaining = stripped;
    }
    if !remaining.starts_with(' ') {
        return None;
    }
    let title = remaining.trim().trim_end_matches('#').trim();
    if title.is_empty() {
        return None;
    }
    Some((level, title.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub fn render_markdown_view(
    ui: &mut Ui,
    text: &str,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math_renderer: &mut MathRenderer,
    kernel: &mut crate::quarto::PythonKernel,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
    scroll_to_line: &mut Option<usize>,
    is_quarto_doc: bool,
) {
    let lines: Vec<&str> = text.lines().collect();
    let mut idx = 0;
    let mut quarto_cell_idx = 0;

    // Check for Quarto / YAML frontmatter at document start
    if let Some((meta, body_start)) = crate::quarto::parse_quarto_frontmatter(text) {
        crate::quarto::render_quarto_frontmatter_ui(ui, &meta, theme, is_quarto_doc);
        idx = body_start;
    }

    while idx < lines.len() {
        let line = lines[idx];
        let trimmed = line.trim();
        let line_no = idx + 1;

        if scroll_to_line.is_some_and(|target| target == line_no) {
            ui.scroll_to_rect(
                egui::Rect::from_min_size(ui.cursor().min, Vec2::new(2.0, 2.0)),
                Some(Align::TOP),
            );
            *scroll_to_line = None;
        }

        // Quarto callout block ::: {.callout-...}
        if trimmed.starts_with(":::") {
            if let Some(callout_type) = crate::quarto::QuartoCalloutType::from_str(trimmed) {
                let mut callout_lines = Vec::new();
                idx += 1;
                while idx < lines.len() && lines[idx].trim() != ":::" {
                    callout_lines.push(lines[idx]);
                    idx += 1;
                }
                if idx < lines.len() {
                    idx += 1;
                }
                crate::quarto::render_quarto_callout_ui(
                    ui,
                    callout_type,
                    &callout_lines,
                    theme,
                    stylesheet,
                    math_renderer,
                );
                ui.add_space(8.0);
                continue;
            }
        }

        // Block math $$ ... $$
        if trimmed.starts_with("$$") {
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let formula = &trimmed[2..trimmed.len() - 2];
                math_renderer.render_block_math(ui, formula, theme);
                ui.add_space(8.0);
                idx += 1;
                continue;
            } else {
                let mut math_lines = Vec::new();
                let first_line = trimmed.trim_start_matches('$').trim();
                if !first_line.is_empty() {
                    math_lines.push(first_line);
                }
                idx += 1;
                while idx < lines.len() && !lines[idx].trim().ends_with("$$") {
                    math_lines.push(lines[idx]);
                    idx += 1;
                }
                if idx < lines.len() {
                    let last_line = lines[idx].trim().trim_end_matches('$').trim();
                    if !last_line.is_empty() {
                        math_lines.push(last_line);
                    }
                    idx += 1;
                }
                math_renderer.render_block_math(ui, &math_lines.join("\n"), theme);
                ui.add_space(8.0);
                continue;
            }
        }

        // Code block ``` or Quarto chunk ```{lang}
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            let mut code_lines = Vec::new();
            idx += 1;
            while idx < lines.len() && !lines[idx].trim().starts_with("```") {
                code_lines.push(lines[idx]);
                idx += 1;
            }
            if idx < lines.len() {
                idx += 1;
            }
            let full_code = code_lines.join("\n");
            if lang.starts_with('{') && lang.ends_with('}') {
                let chunk = crate::quarto::parse_quarto_chunk(lang, &full_code);
                crate::quarto::render_quarto_chunk_ui(ui, &chunk, quarto_cell_idx, theme, kernel, doc_dir);
                quarto_cell_idx += 1;
            } else {
                render_code_block(ui, lang, &full_code, theme);
            }
            ui.add_space(10.0);
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            idx += 1;
            continue;
        }

        // Headings
        if let Some((level, h_text)) = parse_heading(trimmed) {
            render_heading(ui, level, &h_text, theme, stylesheet);
            ui.add_space(6.0);
            idx += 1;
            continue;
        }

        // Blockquotes (may contain emphasis and maths)
        if trimmed.starts_with('>') {
            let mut quote_lines = Vec::new();
            while idx < lines.len() && lines[idx].trim().starts_with('>') {
                let q_line = lines[idx].trim().trim_start_matches('>').trim_start();
                quote_lines.push(q_line.to_string());
                idx += 1;
            }
            render_blockquote(ui, &quote_lines, theme, stylesheet, math_renderer, doc_dir, assets, actions);
            ui.add_space(8.0);
            continue;
        }

        // Table
        if trimmed.starts_with('|') && trimmed.trim_end_matches('|').contains('|') {
            let mut table_rows = Vec::new();
            while idx < lines.len()
                && lines[idx].trim().starts_with('|')
                && lines[idx].trim().ends_with('|')
            {
                table_rows.push(lines[idx].trim());
                idx += 1;
            }
            render_table(ui, &table_rows, theme, stylesheet, math_renderer);
            ui.add_space(10.0);
            continue;
        }

        // Task list item: - [ ] or - [x] (indentation aware)
        let indent = leading_indent(line);
        if let Some((checked, content)) = parse_task_item(trimmed) {
            render_task_item(ui, checked, content, indent, line_no, theme, stylesheet, math_renderer, doc_dir, assets, actions);
            idx += 1;
            continue;
        }

        // Unordered list: -, *, +
        if let Some(content) = parse_bullet_item(trimmed) {
            render_bullet_item(ui, content, indent, theme, stylesheet, math_renderer, doc_dir, assets, actions);
            idx += 1;
            continue;
        }

        // Ordered list: 1. 2. etc
        if let Some((num_str, rest)) = parse_numbered_list(trimmed) {
            render_numbered_item(ui, num_str, rest, indent, theme, stylesheet, math_renderer, doc_dir, assets, actions);
            idx += 1;
            continue;
        }

        // Empty line
        if trimmed.is_empty() {
            ui.add_space(6.0);
            idx += 1;
            continue;
        }

        // Regular paragraph (inline text, math, images and links)
        render_paragraph(ui, trimmed, theme, stylesheet, math_renderer, doc_dir, assets, actions);
        ui.add_space(6.0);
        idx += 1;
    }
}

fn leading_indent(line: &str) -> usize {
    let spaces = line.len() - line.trim_start().len();
    (spaces / 2).min(6)
}

fn parse_task_item(line: &str) -> Option<(bool, &str)> {
    for marker in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "* [X] ", "+ [ ] ", "+ [x] "] {
        if let Some(rest) = line.strip_prefix(marker) {
            let checked = marker.contains("[x]") || marker.contains("[X]");
            return Some((checked, rest));
        }
    }
    None
}

fn parse_bullet_item(line: &str) -> Option<&str> {
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            if !line.starts_with("---") {
                return Some(rest);
            }
        }
    }
    None
}

fn parse_numbered_list(line: &str) -> Option<(&str, &str)> {
    let dot_pos = line.find('.')?;
    let prefix = &line[..dot_pos];
    if prefix.chars().all(|c| c.is_ascii_digit()) && !prefix.is_empty() && prefix.len() <= 3 {
        let rest = line[dot_pos + 1..].trim_start();
        if rest.is_empty() {
            return None;
        }
        Some((prefix, rest))
    } else {
        None
    }
}

fn render_heading(
    ui: &mut Ui,
    level: usize,
    text: &str,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
) -> Response {
    let (font_size, mut color, show_divider) = match level {
        1 => (28.0_f32, theme.text_primary, true),
        2 => (22.0_f32, theme.text_primary, true),
        3 => (18.0_f32, theme.accent, false),
        4 => (16.0_f32, theme.text_primary, false),
        _ => (15.0_f32, theme.text_body, false),
    };

    if stylesheet == TyporaStylesheet::Newsprint && !theme.is_dark {
        color = Color32::from_rgb(0x10, 0x10, 0x10);
    }

    let resp = ui.horizontal_wrapped(|ui| {
        let mut rt = RichText::new(text).font(FontId::proportional(font_size)).color(color);
        if level <= 2 {
            rt = rt.strong();
        }
        if stylesheet == TyporaStylesheet::Newsprint {
            rt = rt.italics();
        }
        ui.label(rt);
    });

    if show_divider {
        ui.add_space(2.0);
        ui.separator();
    }
    resp.response
}

fn render_code_block(ui: &mut Ui, lang: &str, code: &str, theme: &ColorPalette) {
    let frame = Frame::none()
        .fill(theme.code_bg)
        .rounding(Rounding::same(5.0_f32))
        .stroke(Stroke::new(1.0_f32, theme.border))
        .inner_margin(Margin::same(10.0_f32));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            let label = if lang.is_empty() { "code" } else { lang };
            ui.label(
                RichText::new(label)
                    .font(FontId::monospace(11.0))
                    .color(theme.accent),
            );
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if ui.button(RichText::new("Copiar").size(10.0)).clicked() {
                    ui.output_mut(|o| o.copied_text = code.to_string());
                }
            });
        });
        ui.add_space(4.0);

        egui::ScrollArea::horizontal()
            .id_salt(egui::Id::new("code_scroll").with(code))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(code)
                        .font(FontId::monospace(13.5))
                        .color(theme.text_primary),
                );
            });
    });
}

#[allow(clippy::too_many_arguments)]
fn render_blockquote(
    ui: &mut Ui,
    lines: &[String],
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    let frame = Frame::none()
        .fill(theme.quote_bg)
        .rounding(Rounding::same(4.0_f32))
        .stroke(Stroke::new(1.0_f32, theme.border))
        .inner_margin(Margin::symmetric(14.0_f32, 8.0_f32));

    frame.show(ui, |ui| {
        ui.horizontal_top(|ui| {
            let height = ui.available_height().max(20.0);
            let (rect, _response) = ui.allocate_exact_size(Vec2::new(3.5, height), Sense::hover());
            ui.painter().rect_filled(rect, Rounding::same(2.0_f32), theme.accent);
            ui.add_space(6.0);

            ui.vertical(|ui| {
                for line in lines {
                    if line.trim().is_empty() {
                        ui.add_space(4.0);
                        continue;
                    }
                    let mut text = line.as_str();
                    // nested quote markers
                    while let Some(rest) = text.strip_prefix('>') {
                        text = rest.trim_start();
                    }
                    render_inline_segments(ui, text, theme, stylesheet, math, doc_dir, assets, actions);
                }
            });
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn render_task_item(
    ui: &mut Ui,
    checked: bool,
    text: &str,
    indent: usize,
    line_no: usize,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    ui.horizontal(|ui| {
        ui.add_space(4.0 + indent as f32 * 16.0);
        let mut is_checked = checked;
        if ui.checkbox(&mut is_checked, "").changed() {
            actions.push(PreviewAction::ToggleTask(line_no));
        }
        ui.horizontal_wrapped(|ui| {
            if checked {
                let job = inline_markdown_layout(text, theme.text_muted, true, theme, stylesheet, 15.0);
                ui.label(job);
            } else {
                render_inline_segments(ui, text, theme, stylesheet, math, doc_dir, assets, actions);
            }
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn render_bullet_item(
    ui: &mut Ui,
    text: &str,
    indent: usize,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0 + indent as f32 * 16.0);
        ui.label(RichText::new("•").color(theme.accent).size(16.0));
        ui.add_space(2.0);
        render_inline_segments(ui, text, theme, stylesheet, math, doc_dir, assets, actions);
    });
}

#[allow(clippy::too_many_arguments)]
fn render_numbered_item(
    ui: &mut Ui,
    num_str: &str,
    text: &str,
    indent: usize,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0 + indent as f32 * 16.0);
        ui.label(
            RichText::new(format!("{}.", num_str))
                .color(theme.accent)
                .strong(),
        );
        ui.add_space(2.0);
        render_inline_segments(ui, text, theme, stylesheet, math, doc_dir, assets, actions);
    });
}

#[derive(Debug)]
enum InlineSeg<'a> {
    Text(&'a str),
    Math(&'a str),
    Image { alt: &'a str, src: &'a str },
    Link { label: &'a str, url: &'a str },
}

/// Splits a line into text, inline maths, images and links.
///
/// Inline maths follows Typora's rule: `$...$` only when the opening dollar is
/// not followed by whitespace or a digit, and the closing dollar is not
/// preceded by whitespace. This keeps prices like "cuesta $5 y $10" as text.
fn split_inline(text: &str) -> Vec<InlineSeg<'_>> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut text_start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        // escaped character: keep it in the text run
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }

        // image ![alt](src)
        if bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            if let Some((alt, src, end)) = parse_link_like(&text[i + 1..]) {
                if text_start < i {
                    out.push(InlineSeg::Text(&text[text_start..i]));
                }
                out.push(InlineSeg::Image { alt, src });
                i += 1 + end;
                text_start = i;
                continue;
            }
        }

        // link [label](url)
        if bytes[i] == b'[' {
            if let Some((label, url, end)) = parse_link_like(&text[i..]) {
                if text_start < i {
                    out.push(InlineSeg::Text(&text[text_start..i]));
                }
                out.push(InlineSeg::Link { label, url });
                i += end;
                text_start = i;
                continue;
            }
        }

        // inline math $...$
        if bytes[i] == b'$' {
            if let Some(rest) = text.get(i + 1..) {
                if !rest.starts_with('$') {
                    let first = rest.chars().next();
                    let plausible_open = first.is_some_and(|c| !c.is_whitespace() && !c.is_ascii_digit());
                    if plausible_open {
                        if let Some(close_rel) = rest.find('$') {
                            let formula = &rest[..close_rel];
                            let plausible_close = formula
                                .chars()
                                .last()
                                .is_some_and(|c| !c.is_whitespace());
                            if plausible_close && !formula.contains('\n') && !formula.is_empty() {
                                if text_start < i {
                                    out.push(InlineSeg::Text(&text[text_start..i]));
                                }
                                out.push(InlineSeg::Math(formula));
                                i += 1 + close_rel + 1;
                                text_start = i;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        i += 1;
    }

    if text_start < text.len() {
        out.push(InlineSeg::Text(&text[text_start..]));
    }
    out
}

/// Parses `[label](url)` at the start of `s`, returning the label, the url and
/// the number of bytes consumed.
fn parse_link_like(s: &str) -> Option<(&str, &str, usize)> {
    let rest = s.strip_prefix('[')?;
    let close = rest.find(']')?;
    let label = &rest[..close];
    let after = rest.get(close + 1..)?;
    let after = after.strip_prefix('(')?;
    let end = after.find(')')?;
    let url = after[..end].trim();
    let consumed = 1 + close + 1 + 1 + end + 1;
    Some((label, url, consumed))
}

fn render_paragraph(
    ui: &mut Ui,
    text: &str,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    render_inline_segments(ui, text, theme, stylesheet, math, doc_dir, assets, actions);
}

fn render_inline_segments(
    ui: &mut Ui,
    text: &str,
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    math: &mut MathRenderer,
    doc_dir: Option<&Path>,
    assets: &mut PreviewAssets,
    actions: &mut Vec<PreviewAction>,
) {
    let segments = split_inline(text);
    let has_widget = segments
        .iter()
        .any(|s| !matches!(s, InlineSeg::Text(_)));

    if !has_widget {
        let wrap = if ui.available_width().is_finite() {
            ui.available_width()
        } else {
            f32::INFINITY
        };
        let job = inline_markdown_layout(text, theme.text_body, false, theme, stylesheet, 15.0)
            .with_wrap_width(wrap);
        ui.label(job);
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
        for seg in segments {
            match seg {
                InlineSeg::Text(chunk) => {
                    if chunk.trim().is_empty() {
                        ui.add_space(4.0);
                        continue;
                    }
                    let wrap = ui.available_width().max(60.0);
                    let job = inline_markdown_layout(chunk, theme.text_body, false, theme, stylesheet, 15.0)
                        .with_wrap_width(wrap);
                    ui.label(job);
                }
                InlineSeg::Math(formula) => {
                    math.render_inline_math(ui, formula, theme);
                }
                InlineSeg::Image { alt, src } => {
                    render_image(ui, alt, src, doc_dir, theme, assets);
                }
                InlineSeg::Link { label, url } => {
                    let label = if label.is_empty() { url } else { label };
                    let resp = ui.link(RichText::new(label).size(15.0).color(theme.accent));
                    if resp.clicked() {
                        handle_link(url, doc_dir, actions, ui.ctx());
                    }
                    if resp.hovered() && !url.is_empty() {
                        resp.on_hover_text(url);
                    }
                }
            }
        }
    });
}

fn handle_link(url: &str, doc_dir: Option<&Path>, actions: &mut Vec<PreviewAction>, ctx: &egui::Context) {
    if url.is_empty() {
        return;
    }
    let cleaned = url.trim_matches(|c| c == '<' || c == '>');
    if cleaned.starts_with("http://") || cleaned.starts_with("https://") || cleaned.starts_with("mailto:") {
        ctx.open_url(egui::OpenUrl::new_tab(cleaned.to_string()));
        return;
    }
    if let Some(local) = resolve_local_path(cleaned, doc_dir) {
        if crate::files::is_markdown_path(&local) && local.is_file() {
            actions.push(PreviewAction::OpenFile(local));
            return;
        }
        if local.is_file() {
            let abs = crate::files::normalize_path(&local);
            ctx.open_url(egui::OpenUrl::new_tab(abs.display().to_string()));
            return;
        }
    }
    // Unknown scheme: let the OS handle it.
    ctx.open_url(egui::OpenUrl::new_tab(cleaned.to_string()));
}

fn resolve_local_path(src: &str, doc_dir: Option<&Path>) -> Option<PathBuf> {
    let src = src.trim();
    let src = src.strip_prefix("file://").unwrap_or(src);
    let src = src.split('#').next().unwrap_or(src);
    let src = src.split('?').next().unwrap_or(src);
    let decoded = percent_decode(src);
    if decoded.is_empty() {
        return None;
    }
    let path = PathBuf::from(&decoded);
    if path.is_absolute() {
        return Some(path);
    }
    doc_dir
        .map(|d| d.join(&path))
        .or_else(|| Some(path))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn render_image(
    ui: &mut Ui,
    alt: &str,
    src: &str,
    doc_dir: Option<&Path>,
    theme: &ColorPalette,
    assets: &mut PreviewAssets,
) {
    if src.starts_with("http://") || src.starts_with("https://") {
        let resp = ui.link(RichText::new(format!("🖼 {}", if alt.is_empty() { src } else { alt })).color(theme.accent));
        if resp.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab(src.to_string()));
        }
        return;
    }

    let Some(path) = resolve_local_path(src, doc_dir) else {
        return;
    };

    match assets.load(ui.ctx(), &path) {
        Ok(texture) => {
            let available = ui.available_width().max(200.0);
            let size = texture.size_vec2();
            let max_w = available.min(820.0);
            let scale = if size.x > max_w { max_w / size.x } else { 1.0 };
            let display = Vec2::new(size.x * scale, size.y * scale);
            ui.add(egui::Image::new(&texture).fit_to_exact_size(display).rounding(Rounding::same(4.0)));
            if !alt.is_empty() {
                ui.label(
                    RichText::new(alt)
                        .size(11.0)
                        .italics()
                        .color(theme.text_muted),
                );
            }
        }
        Err(err) => {
            Frame::none()
                .fill(theme.code_bg)
                .rounding(Rounding::same(4.0_f32))
                .stroke(Stroke::new(1.0_f32, theme.border))
                .inner_margin(Margin::same(8.0_f32))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("🖼 Imagen no disponible: {}", alt.if_empty(src)))
                            .size(11.5)
                            .color(theme.text_muted),
                    );
                    ui.label(
                        RichText::new(format!("{} — {}", path.display(), err))
                            .size(10.0)
                            .monospace()
                            .color(theme.text_muted),
                    );
                });
        }
    }
}

trait IfEmpty {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str;
}

impl IfEmpty for str {
    fn if_empty<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn render_table(
    ui: &mut Ui,
    rows: &[&str],
    theme: &ColorPalette,
    stylesheet: TyporaStylesheet,
    _math: &mut MathRenderer,
) {
    if rows.is_empty() {
        return;
    }

    let parse_cells = |row: &str| -> Vec<String> {
        let trimmed = row.trim().trim_start_matches('|').trim_end_matches('|');
        trimmed.split('|').map(|s| s.trim().to_string()).collect()
    };

    let header_cells = parse_cells(rows[0]);
    let cols = header_cells.len().max(1);

    // Delimiter row detection: only `-`, `:` and spaces.
    let is_separator = |row: &str| -> bool {
        !row.trim().is_empty()
            && row
                .trim()
                .trim_matches('|')
                .chars()
                .all(|c| c == '-' || c == ':' || c == '|' || c == ' ')
            && row.contains('-')
    };

    let mut alignments: Vec<egui::Align> = vec![egui::Align::LEFT; cols];
    let mut data_start = 1;
    if rows.len() > 1 && is_separator(rows[1]) {
        for (i, cell) in parse_cells(rows[1]).iter().enumerate().take(cols) {
            alignments[i] = if cell.starts_with(':') && cell.ends_with(':') {
                egui::Align::Center
            } else if cell.ends_with(':') {
                egui::Align::RIGHT
            } else {
                egui::Align::LEFT
            };
        }
        data_start = 2;
    }

    let frame = Frame::none()
        .fill(theme.code_bg)
        .rounding(Rounding::same(4.0_f32))
        .stroke(Stroke::new(1.0_f32, theme.border))
        .inner_margin(Margin::same(6.0_f32));

    frame.show(ui, |ui| {
        egui::Grid::new("markdown_table")
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                for (i, cell) in header_cells.iter().enumerate() {
                    let align = alignments.get(i).copied().unwrap_or(egui::Align::LEFT);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let job = inline_markdown_layout(cell, theme.text_primary, false, theme, stylesheet, 14.0);
                        let mut job = job;
                        job.halign = align;
                        ui.label(job);
                    });
                }
                ui.end_row();

                for r in &rows[data_start.min(rows.len())..] {
                    let cells = parse_cells(r);
                    for i in 0..cols {
                        let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
                        let align = alignments.get(i).copied().unwrap_or(egui::Align::LEFT);
                        let job = inline_markdown_layout(text, theme.text_body, false, theme, stylesheet, 14.0)
                            .with_wrap_width((ui.available_width() / cols as f32).max(80.0));
                        let mut job = job;
                        job.halign = align;
                        ui.label(job);
                    }
                    ui.end_row();
                }
            });
    });
}

pub fn inline_markdown_layout(
    text: &str,
    base_color: Color32,
    strikethrough: bool,
    theme: &ColorPalette,
    _stylesheet: TyporaStylesheet,
    font_size: f32,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let default_fmt = TextFormat {
        font_id: FontId::proportional(font_size),
        color: base_color,
        strikethrough: if strikethrough {
            Stroke::new(1.0_f32, base_color)
        } else {
            Stroke::NONE
        },
        ..Default::default()
    };

    while i < len {
        // Bold: **text**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            i += 2;
            let start = i;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                i += 1;
            }
            let bold_text: String = chars[start..i].iter().collect();
            if i + 1 < len {
                i += 2;
            }
            let mut fmt = default_fmt.clone();
            fmt.color = theme.text_primary;
            job.append(&bold_text, 0.0, fmt);
            continue;
        }

        // Strikethrough: ~~text~~
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            i += 2;
            let start = i;
            while i + 1 < len && !(chars[i] == '~' && chars[i + 1] == '~') {
                i += 1;
            }
            let struck: String = chars[start..i].iter().collect();
            if i + 1 < len {
                i += 2;
            }
            let mut fmt = default_fmt.clone();
            fmt.strikethrough = Stroke::new(1.0_f32, base_color);
            job.append(&struck, 0.0, fmt);
            continue;
        }

        // Inline code: `text`
        if chars[i] == '`' {
            i += 1;
            let start = i;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            let code_text: String = chars[start..i].iter().collect();
            if i < len {
                i += 1;
            }
            let mut fmt = default_fmt.clone();
            fmt.font_id = FontId::monospace((font_size - 1.5).max(10.0));
            fmt.color = theme.accent;
            fmt.background = theme.code_bg;
            job.append(&format!(" {} ", code_text), 0.0, fmt);
            continue;
        }

        // Italic: *text* or _text_
        if chars[i] == '*' || chars[i] == '_' {
            let marker = chars[i];
            let next_is_space = i + 1 < len && chars[i + 1].is_whitespace();
            if !next_is_space {
                i += 1;
                let start = i;
                while i < len && chars[i] != marker {
                    i += 1;
                }
                let ital_text: String = chars[start..i].iter().collect();
                if i < len {
                    i += 1;
                }
                let mut fmt = default_fmt.clone();
                fmt.italics = true;
                fmt.color = theme.text_primary;
                job.append(&ital_text, 0.0, fmt);
                continue;
            }
        }

        // Link: [text](url) -> styled text (clickable links are rendered by
        // `render_inline_segments`; this path is used by tables/headings).
        if chars[i] == '[' {
            if let Some((label, _url, consumed)) = parse_link_like(&chars[i..].iter().collect::<String>()) {
                let label = label.to_string();
                i += consumed;
                let mut fmt = default_fmt.clone();
                fmt.color = theme.accent;
                fmt.underline = Stroke::new(1.0_f32, theme.accent);
                job.append(&label, 0.0, fmt);
                continue;
            }
        }

        // Regular character accumulation
        let start = i;
        while i < len && chars[i] != '*' && chars[i] != '`' && chars[i] != '[' && chars[i] != '~' && chars[i] != '_' {
            i += 1;
        }
        if start == i {
            i += 1;
        }
        let chunk: String = chars[start..i].iter().collect();
        job.append(&chunk, 0.0, default_fmt.clone());
    }

    job
}

trait LayoutJobExt {
    fn with_wrap_width(self, width: f32) -> Self;
}

impl LayoutJobExt for LayoutJob {
    fn with_wrap_width(mut self, width: f32) -> Self {
        self.wrap.max_width = width;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(text: &str) -> Vec<String> {
        split_inline(text)
            .into_iter()
            .map(|s| match s {
                InlineSeg::Text(t) => format!("T:{}", t),
                InlineSeg::Math(m) => format!("M:{}", m),
                InlineSeg::Image { alt, src } => format!("I:{}:{}", alt, src),
                InlineSeg::Link { label, url } => format!("L:{}:{}", label, url),
            })
            .collect()
    }

    #[test]
    fn splits_inline_math() {
        assert_eq!(segs("La energía es $E = mc^2$ famosa"), vec![
            "T:La energía es ".to_string(),
            "M:E = mc^2".to_string(),
            "T: famosa".to_string(),
        ]);
    }

    #[test]
    fn does_not_treat_prices_as_math() {
        assert_eq!(segs("cuesta $5 y $10 en total"), vec!["T:cuesta $5 y $10 en total".to_string()]);
    }

    #[test]
    fn splits_images_and_links() {
        assert_eq!(segs("Ver ![fig](img/plot.png) y [guía](guia.md)"), vec![
            "T:Ver ".to_string(),
            "I:fig:img/plot.png".to_string(),
            "T: y ".to_string(),
            "L:guía:guia.md".to_string(),
        ]);
    }

    #[test]
    fn parses_link_consumed_length() {
        let (label, url, consumed) = parse_link_like("[texto](https://x.dev/a)").unwrap();
        assert_eq!(label, "texto");
        assert_eq!(url, "https://x.dev/a");
        assert_eq!(&"[texto](https://x.dev/a)"[consumed..], "");
    }

    #[test]
    fn extracts_nested_headings() {
        let headings = extract_headings("# Uno\n\n```\n# not a heading\n```\n\n### Tres\n");
        let titles: Vec<_> = headings.iter().map(|h| (h.level, h.title.as_str())).collect();
        assert_eq!(titles, vec![(1, "Uno"), (3, "Tres")]);
    }

    #[test]
    fn resolves_relative_image_paths() {
        let dir = Path::new("/tmp/notas");
        assert_eq!(
            resolve_local_path("img/plot.png", Some(dir)),
            Some(PathBuf::from("/tmp/notas/img/plot.png"))
        );
        assert_eq!(
            resolve_local_path("file:///tmp/x%20y.png", None),
            Some(PathBuf::from("/tmp/x y.png"))
        );
    }

    #[test]
    fn detects_table_separators() {
        assert!(!["| a | b |", "| --- | :---: |", "| 1 | 2 |"].is_empty());
        let rows = ["| a | b |", "| :--- | ---: |", "| 1 | 2 |"];
        let is_separator = |row: &str| -> bool {
            row.trim().trim_matches('|').chars().all(|c| c == '-' || c == ':' || c == '|' || c == ' ')
                && row.contains('-')
        };
        assert!(is_separator(rows[1]));
        assert!(!is_separator(rows[0]));
    }
}
