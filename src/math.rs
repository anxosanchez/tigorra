use std::collections::HashMap;
use egui::{
    Align, FontId, Frame, Margin, RichText, Rounding, Stroke, TextureHandle, TextureOptions, Ui,
    Vec2,
};
use latex_rust::font::MathFont;
use latex_rust::render::png::{latex_to_png, PngBackground, PngOptions};
use latex_rust::Color as LatexColor;
use crate::theme::ColorPalette;

pub struct MathRenderer {
    font: Option<MathFont>,
    texture_cache: HashMap<(String, [u8; 3], bool), TextureHandle>,
}

impl MathRenderer {
    pub fn new() -> Self {
        let font = MathFont::stix_two_math().ok();
        Self {
            font,
            texture_cache: HashMap::new(),
        }
    }

    pub fn render_block_math(&mut self, ui: &mut Ui, raw_latex: &str, theme: &ColorPalette) {
        let trimmed = raw_latex.trim();
        let is_chemistry = is_mhchem(trimmed);
        let expanded = expand_mhchem(trimmed);

        let frame = Frame::none()
            .fill(theme.code_bg)
            .rounding(Rounding::same(6.0_f32))
            .stroke(Stroke::new(1.0_f32, if is_chemistry { theme.accent } else { theme.border }))
            .inner_margin(Margin::symmetric(16.0_f32, 12.0_f32));

        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                if is_chemistry {
                    ui.label(
                        RichText::new("[mhchem] KaTeX Chemistry")
                            .font(FontId::monospace(11.0))
                            .color(theme.accent)
                            .strong(),
                    );
                } else {
                    ui.label(
                        RichText::new("KaTeX Math")
                            .font(FontId::monospace(11.0))
                            .color(theme.accent)
                            .strong(),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button(RichText::new("Copiar LaTeX").size(10.0)).clicked() {
                        ui.output_mut(|o| o.copied_text = expanded.clone());
                    }
                    if is_chemistry {
                        if ui.button(RichText::new("Copiar mhchem").size(10.0)).clicked() {
                            ui.output_mut(|o| o.copied_text = trimmed.to_string());
                        }
                    }
                });
            });

            ui.add_space(8.0);

            // Attempt to render expanded LaTeX with latex-rust
            let rendered = self.render_to_texture(ui.ctx(), &expanded, theme, true);

            if let Some(texture) = rendered {
                ui.vertical_centered(|ui| {
                    let size = texture.size_vec2();
                    let max_width = ui.available_width() - 20.0;
                    let display_size = if size.x > max_width && size.x > 0.0 {
                        let scale = max_width / size.x;
                        Vec2::new(max_width, size.y * scale)
                    } else {
                        // High-DPI crisp scale
                        size * 0.65
                    };
                    ui.image((texture.id(), display_size));
                });
            } else {
                // Fallback formatted display
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new(&expanded)
                            .font(FontId::monospace(14.0))
                            .color(theme.text_primary),
                    );
                });
            }
            ui.add_space(4.0);
        });
    }

    pub fn render_inline_math(&mut self, ui: &mut Ui, raw_latex: &str, theme: &ColorPalette) {
        let trimmed = raw_latex.trim();
        let expanded = expand_mhchem(trimmed);
        let rendered = self.render_to_texture(ui.ctx(), &expanded, theme, false);

        if let Some(texture) = rendered {
            let size = texture.size_vec2();
            let display_size = size * 0.55;
            ui.image((texture.id(), display_size));
        } else {
            ui.label(
                RichText::new(format!(" ${}$ ", expanded))
                    .font(FontId::monospace(13.0))
                    .color(theme.accent),
            );
        }
    }

    fn render_to_texture(
        &mut self,
        ctx: &egui::Context,
        latex: &str,
        theme: &ColorPalette,
        display_mode: bool,
    ) -> Option<&TextureHandle> {
        let text_rgb = [theme.text_primary.r(), theme.text_primary.g(), theme.text_primary.b()];
        let cache_key = (latex.to_string(), text_rgb, display_mode);

        if !self.texture_cache.contains_key(&cache_key) {
            let font = self.font.as_ref()?;
            let mut opts = PngOptions::new();
            opts.color = LatexColor::rgb(text_rgb[0], text_rgb[1], text_rgb[2]);
            opts.background = PngBackground::Transparent;
            opts.display = display_mode;

            let png_bytes = latex_to_png(latex, font, &opts).ok()?;
            let image = image::load_from_memory(&png_bytes).ok()?;
            let size = [image.width() as usize, image.height() as usize];
            let image_buffer = image.to_rgba8();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image_buffer.as_raw());

            let tex_name = format!("math_{}", self.texture_cache.len());
            let handle = ctx.load_texture(tex_name, color_image, TextureOptions::LINEAR);
            self.texture_cache.insert(cache_key.clone(), handle);
        }

        self.texture_cache.get(&cache_key)
    }
}

pub fn is_mhchem(input: &str) -> bool {
    input.contains(r"\ce{")
}

pub fn expand_mhchem(input: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        if idx + 4 <= len && &chars[idx..idx + 4] == ['\\', 'c', 'e', '{'] {
            let start = idx + 4;
            let mut depth = 1;
            let mut end = start;
            while end < len && depth > 0 {
                if chars[end] == '{' {
                    depth += 1;
                } else if chars[end] == '}' {
                    depth -= 1;
                }
                if depth > 0 {
                    end += 1;
                }
            }
            let inner: String = chars[start..end].iter().collect();
            let converted = convert_mhchem_body(&inner);
            result.push_str(&converted);
            idx = if end < len { end + 1 } else { len };
        } else {
            result.push(chars[idx]);
            idx += 1;
        }
    }

    result
}

fn convert_mhchem_body(body: &str) -> String {
    let tokens = tokenize_mhchem(body);
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        if i > 0 && !token.is_empty() && !out.ends_with(' ') && !token.starts_with('^') && !token.starts_with('_') {
            out.push(' ');
        }
        out.push_str(token);
    }
    out
}

fn tokenize_mhchem(body: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = body.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }

        // Equilibrium arrows <=> or <=>> or <==> or <->
        if i + 4 <= len && &chars[i..i + 4] == ['<', '=', '=', '>'] {
            tokens.push(r"\rightleftharpoons".to_string());
            i += 4;
            continue;
        }
        if i + 4 <= len && &chars[i..i + 4] == ['<', '=', '>', '>'] {
            tokens.push(r"\rightleftharpoons".to_string());
            i += 4;
            continue;
        }
        if i + 3 <= len && (&chars[i..i + 3] == ['<', '=', '>'] || &chars[i..i + 3] == ['<', '-', '>']) {
            tokens.push(r"\rightleftharpoons".to_string());
            i += 3;
            continue;
        }

        // Forward arrow -> or --> with optional [above][below]
        if (i + 2 <= len && chars[i] == '-' && chars[i + 1] == '>')
            || (i + 3 <= len && chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>')
        {
            let arrow_len = if i + 3 <= len && chars[i + 1] == '-' { 3 } else { 2 };
            i += arrow_len;

            let mut top = None;
            let mut bot = None;
            while i < len && (chars[i] == '[' || chars[i].is_whitespace()) {
                if chars[i].is_whitespace() {
                    i += 1;
                    continue;
                }
                if chars[i] == '[' {
                    let bstart = i + 1;
                    let mut bend = bstart;
                    while bend < len && chars[bend] != ']' {
                        bend += 1;
                    }
                    let bracket_content: String = chars[bstart..bend].iter().collect();
                    if top.is_none() {
                        top = Some(bracket_content);
                    } else if bot.is_none() {
                        bot = Some(bracket_content);
                    }
                    i = if bend < len { bend + 1 } else { len };
                }
            }

            if let Some(t) = top {
                if let Some(b) = bot {
                    tokens.push(format!(r"\xrightarrow[{}]{{{}}}", b, t));
                } else {
                    tokens.push(format!(r"\xrightarrow{{{}}}", t));
                }
            } else {
                tokens.push(r"\rightarrow".to_string());
            }
            continue;
        }

        // Backward arrow <-
        if i + 2 <= len && chars[i] == '<' && chars[i + 1] == '-' {
            tokens.push(r"\leftarrow".to_string());
            i += 2;
            continue;
        }

        // Standalone operators +, =, .
        if chars[i] == '+' {
            let prev_is_space = i == 0 || chars[i - 1].is_whitespace();
            let next_is_space = i + 1 == len || chars[i + 1].is_whitespace();
            if prev_is_space || next_is_space {
                tokens.push("+".to_string());
                i += 1;
                continue;
            }
        }
        if chars[i] == '=' {
            tokens.push("=".to_string());
            i += 1;
            continue;
        }
        if chars[i] == '.' {
            tokens.push(r"\cdot".to_string());
            i += 1;
            continue;
        }

        // Precipitate 'v' or gas '^'
        if chars[i] == 'v' && (i + 1 == len || chars[i + 1].is_whitespace()) {
            tokens.push(r"\downarrow".to_string());
            i += 1;
            continue;
        }
        if chars[i] == '^' && (i + 1 == len || chars[i + 1].is_whitespace()) {
            tokens.push(r"\uparrow".to_string());
            i += 1;
            continue;
        }

        // Consume word / formula token until whitespace, arrow, or standalone '+' / '='
        let wstart = i;
        while i < len && !chars[i].is_whitespace() {
            if (i + 2 <= len && chars[i] == '-' && chars[i + 1] == '>')
                || (i + 3 <= len && &chars[i..i + 3] == ['<', '=', '>'])
            {
                break;
            }
            if (chars[i] == '+' || chars[i] == '=') && (i + 1 == len || chars[i + 1].is_whitespace()) {
                break;
            }
            i += 1;
        }
        let word: String = chars[wstart..i].iter().collect();
        if !word.is_empty() {
            tokens.push(format_chemical_compound(&word));
        }
    }

    tokens
}

fn format_chemical_compound(word: &str) -> String {
    // Pure numbers (stoichiometric coefficients like 2, 3, 1/2)
    if word.chars().all(|c| c.is_ascii_digit() || c == '/') {
        return format!("{}\\;", word);
    }

    let chars: Vec<char> = word.chars().collect();
    let mut coeff = String::new();
    let mut pos = 0;
    while pos < chars.len() && chars[pos].is_ascii_digit() {
        coeff.push(chars[pos]);
        pos += 1;
    }

    let (prefix, rest_chars) = if !coeff.is_empty() && pos < chars.len() && chars[pos].is_ascii_uppercase() {
        (format!("{}\\;", coeff), &chars[pos..])
    } else {
        (String::new(), &chars[..])
    };

    let mut formatted = String::new();
    let mut j = 0;
    let rlen = rest_chars.len();

    while j < rlen {
        let c = rest_chars[j];

        // Explicit charge e.g. ^2+ or ^{2+} or ^+ or ^-
        if c == '^' {
            j += 1;
            let mut charge = String::new();
            if j < rlen && rest_chars[j] == '{' {
                j += 1;
                while j < rlen && rest_chars[j] != '}' {
                    charge.push(rest_chars[j]);
                    j += 1;
                }
                if j < rlen && rest_chars[j] == '}' {
                    j += 1;
                }
            } else {
                while j < rlen && (rest_chars[j].is_ascii_alphanumeric() || rest_chars[j] == '+' || rest_chars[j] == '-') {
                    charge.push(rest_chars[j]);
                    j += 1;
                }
            }
            formatted.push_str(&format!("^{{{}}}", charge));
            continue;
        }

        // Implicit charge e.g. H+ or Cl- or Ba2+ at end of formula
        if (c == '+' || c == '-') && (j + 1 == rlen) {
            formatted.push_str(&format!("^{{{}}}", c));
            j += 1;
            continue;
        }

        // Numbers (subscript for elements or parentheses)
        if c.is_ascii_digit() {
            let mut num = String::new();
            while j < rlen && rest_chars[j].is_ascii_digit() {
                num.push(rest_chars[j]);
                j += 1;
            }
            if num.len() == 1 {
                formatted.push_str(&format!("_{}", num));
            } else {
                formatted.push_str(&format!("_{{{}}}", num));
            }
            continue;
        }

        // Brackets / hyphens
        if c == '(' || c == ')' || c == '[' || c == ']' {
            formatted.push(c);
            j += 1;
            continue;
        }
        if c == '-' {
            formatted.push('-');
            j += 1;
            continue;
        }

        // Alphabetic characters
        if c.is_ascii_alphabetic() {
            formatted.push(c);
            j += 1;
            continue;
        }

        formatted.push(c);
        j += 1;
    }

    format!("{}\\mathrm{{{}}}", prefix, formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mhchem() {
        assert!(is_mhchem(r"\ce{H2O}"));
        assert!(is_mhchem(r"Reacción: \ce{2 H2 + O2 -> 2 H2O}"));
        assert!(!is_mhchem(r"E = mc^2"));
    }

    #[test]
    fn test_expand_mhchem_reactions() {
        let h2o = expand_mhchem(r"\ce{H2O}");
        assert_eq!(h2o, r"\mathrm{H_2O}");

        let reaction = expand_mhchem(r"\ce{2 H2 + O2 -> 2 H2O}");
        assert!(reaction.contains(r"\rightarrow"));
        assert!(reaction.contains(r"\mathrm{H_2O}"));

        let precip = expand_mhchem(r"\ce{SO4^2- + Ba^2+ -> BaSO4 v}");
        assert!(precip.contains(r"\downarrow"));

        let gas = expand_mhchem(r"\ce{CaCO3 -> CaO + CO2 ^}");
        assert!(gas.contains(r"\uparrow"));

        let eq = expand_mhchem(r"\ce{N2 + 3 H2 <=> 2 NH3}");
        assert!(eq.contains(r"\rightleftharpoons"));

        let hydrate = expand_mhchem(r"\ce{CuSO4 . 5 H2O}");
        assert!(hydrate.contains(r"\cdot"));
    }
}
