use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TyporaStylesheet {
    GitHub,
    Night,
    Newsprint,
    Gothic,
    Whitey,
}

impl TyporaStylesheet {
    pub fn all() -> Vec<Self> {
        vec![
            Self::GitHub,
            Self::Night,
            Self::Newsprint,
            Self::Gothic,
            Self::Whitey,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::Night => "Night (Typora Oscuro)",
            Self::Newsprint => "Newsprint (Editorial Serif)",
            Self::Gothic => "Gothic (Alto Contraste)",
            Self::Whitey => "Whitey (Moderno Minimalista)",
        }
    }

    pub fn font_family(&self) -> &'static str {
        match self {
            Self::Newsprint => "Georgia, 'PT Serif', 'Times New Roman', serif",
            Self::GitHub => "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif",
            Self::Night => "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Helvetica Neue', sans-serif",
            Self::Gothic => "'Century Gothic', 'Franklin Gothic Medium', Arial, sans-serif",
            Self::Whitey => "'Segoe UI', 'Helvetica Neue', Arial, sans-serif",
        }
    }

    #[allow(dead_code)]
    pub fn is_dark(&self) -> bool {
        matches!(self, Self::Night)
    }

    pub fn pdf_base_font(&self) -> (&'static str, &'static str) {
        match self {
            Self::Newsprint => ("Times-Roman", "Times-Bold"),
            _ => ("Helvetica", "Helvetica-Bold"),
        }
    }
}

pub fn clean_filename(title: &str) -> String {
    let clean: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if clean.is_empty() {
        "documento".to_string()
    } else {
        clean
    }
}

pub fn export_to_html(title: &str, content: &str, style: TyporaStylesheet) -> String {
    let bg_color = match style {
        TyporaStylesheet::Night => "#363B40",
        TyporaStylesheet::Newsprint => "#F7F6F2",
        TyporaStylesheet::GitHub => "#FFFFFF",
        TyporaStylesheet::Gothic => "#FFFFFF",
        TyporaStylesheet::Whitey => "#FAFAFA",
    };

    let text_color = match style {
        TyporaStylesheet::Night => "#B8BFC6",
        TyporaStylesheet::Newsprint => "#1A1A1A",
        TyporaStylesheet::GitHub => "#24292E",
        TyporaStylesheet::Gothic => "#111111",
        TyporaStylesheet::Whitey => "#2C3E50",
    };

    let heading_color = match style {
        TyporaStylesheet::Night => "#DEDEDE",
        TyporaStylesheet::Newsprint => "#000000",
        TyporaStylesheet::GitHub => "#1F2328",
        TyporaStylesheet::Gothic => "#000000",
        TyporaStylesheet::Whitey => "#1A252F",
    };

    let accent_color = match style {
        TyporaStylesheet::Night => "#6DC1E7",
        TyporaStylesheet::Newsprint => "#8A3B14",
        TyporaStylesheet::GitHub => "#0969DA",
        TyporaStylesheet::Gothic => "#2B4C7E",
        TyporaStylesheet::Whitey => "#16A085",
    };

    let code_bg = match style {
        TyporaStylesheet::Night => "#24272A",
        TyporaStylesheet::Newsprint => "#ECE8DE",
        TyporaStylesheet::GitHub => "#F6F8FA",
        TyporaStylesheet::Gothic => "#EFEFEF",
        TyporaStylesheet::Whitey => "#F0F3F4",
    };

    let mut body_html = String::new();
    let mut in_code = false;
    let mut in_quote = false;
    let mut in_list = false;

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_code {
                body_html.push_str("</code></pre>\n");
                in_code = false;
            } else {
                let lang = trimmed.trim_start_matches('`').trim();
                body_html.push_str(&format!("<pre><code class=\"language-{}\">\n", lang));
                in_code = true;
            }
            i += 1;
            continue;
        }

        if in_code {
            body_html.push_str(&html_escape(line));
            body_html.push('\n');
            i += 1;
            continue;
        }

        // Display Math $$ (single-line or multiline block)
        if trimmed.starts_with("$$") {
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let formula = &trimmed[2..trimmed.len() - 2];
                body_html.push_str(&format!("<div class=\"math-block\">$${}$$</div>\n", html_escape(formula.trim())));
                i += 1;
                continue;
            } else {
                let mut math_lines = Vec::new();
                let first = trimmed.trim_start_matches('$').trim();
                if !first.is_empty() {
                    math_lines.push(first);
                }
                i += 1;
                while i < lines.len() && !lines[i].trim().ends_with("$$") {
                    math_lines.push(lines[i]);
                    i += 1;
                }
                if i < lines.len() {
                    let last = lines[i].trim().trim_end_matches('$').trim();
                    if !last.is_empty() {
                        math_lines.push(last);
                    }
                    i += 1;
                }
                let formula = math_lines.join("\n");
                body_html.push_str(&format!("<div class=\"math-block\">$${}$$</div>\n", html_escape(formula.trim())));
                continue;
            }
        }

        if trimmed.starts_with('#') {
            if in_list { body_html.push_str("</ul>\n"); in_list = false; }
            if in_quote { body_html.push_str("</blockquote>\n"); in_quote = false; }

            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[level..].trim();
            body_html.push_str(&format!("<h{}>{}</h{}>\n", level, html_escape(text), level));
            i += 1;
            continue;
        }

        if trimmed.starts_with('>') {
            if !in_quote {
                body_html.push_str("<blockquote>\n");
                in_quote = true;
            }
            let quote_text = trimmed.trim_start_matches('>').trim();
            body_html.push_str(&format!("<p>{}</p>\n", format_inline_html(quote_text)));
            i += 1;
            continue;
        } else if in_quote {
            body_html.push_str("</blockquote>\n");
            in_quote = false;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                body_html.push_str("<ul>\n");
                in_list = true;
            }
            let item_text = &trimmed[2..];
            body_html.push_str(&format!("<li>{}</li>\n", format_inline_html(item_text)));
            i += 1;
            continue;
        } else if in_list {
            body_html.push_str("</ul>\n");
            in_list = false;
        }

        if trimmed == "---" {
            body_html.push_str("<hr />\n");
            i += 1;
            continue;
        }

        if trimmed.is_empty() {
            body_html.push_str("<p></p>\n");
        } else {
            body_html.push_str(&format!("<p>{}</p>\n", format_inline_html(trimmed)));
        }
        i += 1;
    }

    if in_code { body_html.push_str("</code></pre>\n"); }
    if in_quote { body_html.push_str("</blockquote>\n"); }
    if in_list { body_html.push_str("</ul>\n"); }

    format!(
        r#"<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/mhchem.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.8/dist/contrib/auto-render.min.js"></script>
<style>
  body {{
    font-family: {font_family};
    background-color: {bg_color};
    color: {text_color};
    line-height: 1.65;
    margin: 0;
    padding: 40px 20px;
    font-size: 16px;
  }}
  .typora-export {{
    max-width: 880px;
    margin: 0 auto;
    background: transparent;
  }}
  h1, h2, h3, h4, h5, h6 {{
    color: {heading_color};
    margin-top: 1.5em;
    margin-bottom: 0.6em;
    font-weight: 600;
  }}
  h1 {{ font-size: 2.2em; border-bottom: 1px solid rgba(128,128,128,0.2); padding-bottom: 0.3em; }}
  h2 {{ font-size: 1.6em; border-bottom: 1px solid rgba(128,128,128,0.15); padding-bottom: 0.2em; }}
  h3 {{ font-size: 1.25em; }}
  a {{ color: {accent_color}; text-decoration: none; }}
  a:hover {{ text-decoration: underline; }}
  pre {{
    background: {code_bg};
    padding: 14px;
    border-radius: 6px;
    overflow-x: auto;
    font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
    font-size: 14px;
    border: 1px solid rgba(128,128,128,0.2);
  }}
  code {{
    font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
    font-size: 0.9em;
    background: {code_bg};
    padding: 2px 5px;
    border-radius: 3px;
  }}
  blockquote {{
    border-left: 4px solid {accent_color};
    margin: 1.2em 0;
    padding: 0.6em 1.2em;
    background: {code_bg};
    border-radius: 0 4px 4px 0;
    font-style: italic;
  }}
  .math-block {{
    text-align: center;
    margin: 1.5em 0;
    padding: 12px;
    background: {code_bg};
    border-radius: 6px;
    border: 1px solid rgba(128,128,128,0.15);
    overflow-x: auto;
  }}
  hr {{ border: none; border-top: 1px solid rgba(128,128,128,0.25); margin: 2em 0; }}
  @media print {{
    @page {{ margin: 20mm; size: A4; }}
    body {{ background: #fff !important; color: #000 !important; padding: 0; }}
    .typora-export {{ max-width: 100%; }}
    pre, blockquote, .math-block {{ border-color: #ddd; }}
  }}
</style>
</head>
<body>
<div class="typora-export">
{body_html}
</div>
<script>
  function renderAllMath() {{
    if (typeof renderMathInElement === 'function' && document.body) {{
      renderMathInElement(document.body, {{
        delimiters: [
          {{left: '$$', right: '$$', display: true}},
          {{left: '$', right: '$', display: false}},
          {{left: '\\(', right: '\\)', display: false}},
          {{left: '\\[', right: '\\]', display: true}}
        ],
        ignoredTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'],
        throwOnError: false
      }});
      return true;
    }}
    return false;
  }}

  if (document.readyState === 'loading') {{
    document.addEventListener('DOMContentLoaded', renderAllMath);
  }} else {{
    renderAllMath();
  }}
  window.addEventListener('load', renderAllMath);

  var attempts = 0;
  var timer = setInterval(function() {{
    attempts++;
    if (renderAllMath() || attempts > 60) {{
      clearInterval(timer);
    }}
  }}, 100);
</script>
</body>
</html>
"#,
        title = html_escape(title),
        font_family = style.font_family(),
        bg_color = bg_color,
        text_color = text_color,
        heading_color = heading_color,
        accent_color = accent_color,
        code_bg = code_bg,
        body_html = body_html
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn format_inline_html(text: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_math = false;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        if chars[idx] == '`' {
            if in_code {
                out.push_str("</code>");
                in_code = false;
            } else {
                out.push_str("<code>");
                in_code = true;
            }
            idx += 1;
            continue;
        }

        if in_code {
            match chars[idx] {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                c => out.push(c),
            }
            idx += 1;
            continue;
        }

        if chars[idx] == '$' {
            if idx + 1 < len && chars[idx + 1] == '$' {
                out.push_str("$$");
                idx += 2;
                continue;
            }
            in_math = !in_math;
            out.push('$');
            idx += 1;
            continue;
        }

        if !in_math && idx + 4 <= len && &chars[idx..idx + 4] == ['\\', 'c', 'e', '{'] {
            let start = idx;
            let mut depth = 1;
            let mut end = idx + 4;
            while end < len && depth > 0 {
                if chars[end] == '{' {
                    depth += 1;
                } else if chars[end] == '}' {
                    depth -= 1;
                }
                end += 1;
            }
            let ce_str: String = chars[start..end].iter().collect();
            out.push('$');
            for c in ce_str.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    '"' => out.push_str("&quot;"),
                    ch => out.push(ch),
                }
            }
            out.push('$');
            idx = end;
            continue;
        }

        if !in_math && idx + 1 < len && chars[idx] == '*' && chars[idx + 1] == '*' {
            if in_bold {
                out.push_str("</strong>");
                in_bold = false;
            } else {
                out.push_str("<strong>");
                in_bold = true;
            }
            idx += 2;
            continue;
        }

        if !in_math && chars[idx] == '*' {
            if in_italic {
                out.push_str("</em>");
                in_italic = false;
            } else {
                out.push_str("<em>");
                in_italic = true;
            }
            idx += 1;
            continue;
        }

        match chars[idx] {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
        idx += 1;
    }

    if in_bold {
        out.push_str("</strong>");
    }
    if in_italic {
        out.push_str("</em>");
    }
    if in_code {
        out.push_str("</code>");
    }

    out
}

/// Generates a valid standard PDF 1.4 file formatted according to the Typora stylesheet
pub fn export_to_pdf(
    path: &Path,
    title: &str,
    content: &str,
    style: TyporaStylesheet,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (font_regular, font_bold) = style.pdf_base_font();

    // A4 dimensions: 595.28 x 841.89 points
    let page_width = 595.28_f32;
    let page_height = 841.89_f32;
    let margin_left = 54.0_f32;
    let margin_top = 64.0_f32;
    let margin_bottom = 60.0_f32;
    let content_width = page_width - margin_left * 2.0;

    let mut pages_streams: Vec<Vec<u8>> = Vec::new();
    let mut current_stream = Vec::new();
    let mut current_y = page_height - margin_top;

    let start_page = |stream: &mut Vec<u8>, p_num: usize| {
        stream.extend_from_slice(b"BT\n");
        // Header
        let _ = write!(
            stream,
            "/F1 9 Tf\n0.5 0.5 0.5 rg\n{} {} Td\n({}) Tj\n",
            margin_left,
            page_height - 36.0,
            pdf_escape(title)
        );
        // Page footer
        let _ = write!(
            stream,
            "{} {} Td\n(Pagina {}) Tj\n",
            content_width - 40.0,
            0.0,
            p_num
        );
        stream.extend_from_slice(b"ET\n");
    };

    let mut page_count = 1;
    start_page(&mut current_stream, page_count);

    let check_page_break = |stream: &mut Vec<u8>,
                            pages: &mut Vec<Vec<u8>>,
                            y: &mut f32,
                            p_num: &mut usize,
                            needed: f32| {
        if *y - needed < margin_bottom {
            pages.push(std::mem::take(stream));
            *p_num += 1;
            *y = page_height - margin_top;
            let mut s = Vec::new();
            s.extend_from_slice(b"BT\n");
            let _ = write!(
                s,
                "/F1 9 Tf\n0.5 0.5 0.5 rg\n{} {} Td\n({}) Tj\n",
                margin_left,
                page_height - 36.0,
                pdf_escape(title)
            );
            let _ = write!(
                s,
                "{} {} Td\n(Pagina {}) Tj\n",
                content_width - 40.0,
                0.0,
                *p_num
            );
            s.extend_from_slice(b"ET\n");
            *stream = s;
        }
    };

    let mut content_lines: Vec<&str> = content.lines().collect();
    if content.trim_start().starts_with("---") {
        if let Some((_, body_start)) = crate::quarto::parse_quarto_frontmatter(content) {
            content_lines = content_lines.split_off(body_start.min(content_lines.len()));
        }
    }

    for line in content_lines {
        let trimmed = line.trim();

        if trimmed.starts_with("# ") {
            let text = plain_markdown(&trimmed[2..]);
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 44.0);
            current_y -= 26.0;
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F2 20 Tf\n0.1 0.1 0.1 rg\n{} {} Td\n({}) Tj\nET\n",
                margin_left,
                current_y,
                pdf_escape(&text)
            );
            current_y -= 8.0;
            // Draw underline
            let _ = write!(
                current_stream,
                "0.8 0.8 0.8 RG\n1 w\n{} {} m {} {} l S\n",
                margin_left, current_y, margin_left + content_width, current_y
            );
            current_y -= 14.0;
            continue;
        }

        if trimmed.starts_with("## ") {
            let text = plain_markdown(&trimmed[3..]);
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 36.0);
            current_y -= 20.0;
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F2 15 Tf\n0.15 0.15 0.15 rg\n{} {} Td\n({}) Tj\nET\n",
                margin_left,
                current_y,
                pdf_escape(&text)
            );
            current_y -= 12.0;
            continue;
        }

        if trimmed.starts_with("### ") {
            let text = plain_markdown(&trimmed[4..]);
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 28.0);
            current_y -= 16.0;
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F2 12.5 Tf\n0.2 0.3 0.5 rg\n{} {} Td\n({}) Tj\nET\n",
                margin_left,
                current_y,
                pdf_escape(&text)
            );
            current_y -= 10.0;
            continue;
        }

        if trimmed.starts_with("$$") {
            let formula = trimmed.trim_matches('$').trim();
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 40.0);
            current_y -= 16.0;
            // Draw centered box
            let box_h = 24.0;
            let _ = write!(
                current_stream,
                "0.94 0.95 0.96 rg\n{} {} {} {} re f\n0.8 0.85 0.9 RG\n1 w\n{} {} {} {} re s\n",
                margin_left + 20.0, current_y - 6.0, content_width - 40.0, box_h,
                margin_left + 20.0, current_y - 6.0, content_width - 40.0, box_h
            );
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F1 11.5 Tf\n0.1 0.25 0.6 rg\n{} {} Td\n([KaTeX]  {}) Tj\nET\n",
                margin_left + 35.0,
                current_y,
                pdf_escape(formula)
            );
            current_y -= 18.0;
            continue;
        }

        if trimmed.starts_with("- [ ] ") || trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
            let checked = trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ");
            let text = plain_markdown(&trimmed[6..]);
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 18.0);
            current_y -= 14.0;
            let marker = if checked { "[x]" } else { "[ ]" };
            let _ = write!(
                current_stream,
                "BT\n/F1 10.5 Tf\n0.2 0.2 0.2 rg\n{} {} Td\n({} {}) Tj\nET\n",
                margin_left + 10.0,
                current_y,
                marker,
                pdf_escape(&text)
            );
            continue;
        }

        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            let text = plain_markdown(&trimmed[2..]);
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 18.0);
            current_y -= 14.0;
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F1 10.5 Tf\n0.2 0.2 0.2 rg\n{} {} Td\n(  *  {}) Tj\nET\n",
                margin_left + 10.0,
                current_y,
                pdf_escape(&text)
            );
            continue;
        }

        if trimmed == "---" {
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 20.0);
            current_y -= 10.0;
            let _ = write!(
                current_stream,
                "0.85 0.85 0.85 RG\n1 w\n{} {} m {} {} l S\n",
                margin_left, current_y, margin_left + content_width, current_y
            );
            current_y -= 10.0;
            continue;
        }

        if trimmed.is_empty() {
            current_y -= 8.0;
            continue;
        }

        // Numbered list "1. text"
        if let Some((num, rest)) = split_numbered_item(trimmed) {
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 18.0);
            current_y -= 14.0;
            let text = plain_markdown(rest);
            let _ = write!(
                current_stream,
                "BT\n/F1 10.5 Tf\n0.2 0.2 0.2 rg\n{} {} Td\n({}.  {}) Tj\nET\n",
                margin_left + 10.0,
                current_y,
                num,
                pdf_escape(&text)
            );
            continue;
        }

        // Regular paragraph with wrap
        let paragraph = plain_markdown(trimmed);
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut line_buf = String::new();
        let wrap_chars = ((content_width / 5.3) as usize).max(40);

        for w in words {
            if line_buf.len() + w.len() + 1 > wrap_chars {
                check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 16.0);
                current_y -= 14.0;
                current_stream.extend_from_slice(b"BT\n");
                let _ = write!(
                    current_stream,
                    "/F1 10.5 Tf\n0.2 0.2 0.2 rg\n{} {} Td\n({}) Tj\nET\n",
                    margin_left,
                    current_y,
                    pdf_escape(&line_buf)
                );
                line_buf.clear();
            }
            if !line_buf.is_empty() {
                line_buf.push(' ');
            }
            line_buf.push_str(w);
        }

        if !line_buf.is_empty() {
            check_page_break(&mut current_stream, &mut pages_streams, &mut current_y, &mut page_count, 16.0);
            current_y -= 14.0;
            current_stream.extend_from_slice(b"BT\n");
            let _ = write!(
                current_stream,
                "/F1 10.5 Tf\n0.2 0.2 0.2 rg\n{} {} Td\n({}) Tj\nET\n",
                margin_left,
                current_y,
                pdf_escape(&line_buf)
            );
        }
    }

    pages_streams.push(current_stream);

    // Assemble PDF objects
    let total_pages = pages_streams.len();
    let mut file = File::create(path)?;
    let mut offsets = Vec::new();

    file.write_all(b"%PDF-1.4\n")?;

    // Obj 1: Catalog
    offsets.push(file.stream_position()?);
    file.write_all(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")?;

    // Obj 2: Pages
    offsets.push(file.stream_position()?);
    let mut kids = String::new();
    for p in 0..total_pages {
        kids.push_str(&format!("{} 0 R ", 3 + p * 2));
    }
    file.write_all(format!("2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n", kids, total_pages).as_bytes())?;

    // Page objects and content stream objects
    let font_reg_id = 3 + total_pages * 2;
    let font_bold_id = font_reg_id + 1;

    for (p, stream) in pages_streams.iter().enumerate() {
        let page_obj_id = 3 + p * 2;
        let content_obj_id = page_obj_id + 1;

        offsets.push(file.stream_position()?);
        file.write_all(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 {} 0 R /F2 {} 0 R >> >> /Contents {} 0 R >>\nendobj\n",
                page_obj_id, page_width, page_height, font_reg_id, font_bold_id, content_obj_id
            ).as_bytes(),
        )?;

        offsets.push(file.stream_position()?);
        file.write_all(format!("{} 0 obj\n<< /Length {} >>\nstream\n", content_obj_id, stream.len()).as_bytes())?;
        file.write_all(stream)?;
        file.write_all(b"\nendstream\nendobj\n")?;
    }

    // Font regular
    offsets.push(file.stream_position()?);
    file.write_all(format!("{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>\nendobj\n", font_reg_id, font_regular).as_bytes())?;

    // Font bold
    offsets.push(file.stream_position()?);
    file.write_all(format!("{} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>\nendobj\n", font_bold_id, font_bold).as_bytes())?;

    let xref_pos = file.stream_position()?;
    let total_objs = offsets.len() + 1;

    file.write_all(format!("xref\n0 {}\n0000000000 65535 f \n", total_objs).as_bytes())?;
    for off in offsets {
        file.write_all(format!("{:010} 00000 n \n", off).as_bytes())?;
    }

    file.write_all(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objs, xref_pos
        ).as_bytes(),
    )?;

    Ok(())
}

/// Encodes text for a PDF string literal using WinAnsiEncoding (CP1252).
///
/// The previous implementation replaced accented characters with their ASCII
/// base letter, so "Título" was exported as "Titulo". With `/WinAnsiEncoding`
/// declared on the font, Spanish/Galician/French accents and typographic quotes
/// round-trip correctly; anything outside CP1252 becomes '?'.
fn split_numbered_item(line: &str) -> Option<(&str, &str)> {
    let dot = line.find('.')?;
    let prefix = &line[..dot];
    if !prefix.is_empty() && prefix.len() <= 3 && prefix.chars().all(|c| c.is_ascii_digit()) {
        let rest = line[dot + 1..].trim_start();
        if !rest.is_empty() {
            return Some((prefix, rest));
        }
    }
    None
}

fn pdf_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            c => match winansi_code(c) {
                Some(code) if (0x20..=0x7E).contains(&code) => out.push(code as char),
                Some(code) => out.push_str(&format!("\\{:03o}", code)),
                None => out.push('?'),
            },
        }
    }
    out
}

fn winansi_code(c: char) -> Option<u8> {
    let u = c as u32;
    if (0x20..=0x7E).contains(&u) {
        return Some(u as u8);
    }
    if (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    Some(match c {
        '\t' => 0x09,
        '\n' => 0x0A,
        '\u{20AC}' => 0x80, // €
        '\u{201A}' => 0x82,
        '\u{0192}' => 0x83,
        '\u{201E}' => 0x84,
        '\u{2026}' => 0x85,
        '\u{2020}' => 0x86,
        '\u{2021}' => 0x87,
        '\u{02C6}' => 0x88,
        '\u{2030}' => 0x89,
        '\u{0160}' => 0x8A,
        '\u{2039}' => 0x8B,
        '\u{0152}' => 0x8C,
        '\u{017D}' => 0x8E,
        '\u{2018}' => 0x91,
        '\u{2019}' => 0x92,
        '\u{201C}' => 0x93,
        '\u{201D}' => 0x94,
        '\u{2022}' => 0x95,
        '\u{2013}' => 0x96,
        '\u{2014}' => 0x97,
        '\u{02DC}' => 0x98,
        '\u{2122}' => 0x99,
        '\u{0161}' => 0x9A,
        '\u{203A}' => 0x9B,
        '\u{0153}' => 0x9C,
        '\u{017E}' => 0x9E,
        '\u{0178}' => 0x9F,
        _ => return None,
    })
}

/// Removes Markdown syntax so PDF text is prose rather than source code.
fn plain_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '`' => {
                i += 1;
                while i < chars.len() && chars[i] != '`' {
                    out.push(chars[i]);
                    i += 1;
                }
                i += 1;
            }
            '*' | '_' | '~' => {
                // Drop emphasis markers, keep the words.
                i += 1;
            }
            '[' => {
                // [label](url) -> label
                let mut label = String::new();
                let mut j = i + 1;
                while j < chars.len() && chars[j] != ']' {
                    label.push(chars[j]);
                    j += 1;
                }
                if j + 1 < chars.len() && chars[j + 1] == '(' {
                    let mut k = j + 2;
                    while k < chars.len() && chars[k] != ')' {
                        k += 1;
                    }
                    out.push_str(&label);
                    i = (k + 1).min(chars.len());
                } else {
                    out.push('[');
                    i += 1;
                }
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '[' => {
                // ![alt](src) -> alt
                let mut alt = String::new();
                let mut j = i + 2;
                while j < chars.len() && chars[j] != ']' {
                    alt.push(chars[j]);
                    j += 1;
                }
                let mut k = j;
                while k < chars.len() && chars[k] != ')' {
                    k += 1;
                }
                out.push_str(&alt);
                i = (k + 1).min(chars.len());
            }
            '\\' => {
                if i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_html() {
        let content = "# Título\n\nEste es un párrafo con **negrita** y `código`.\n\n$$E = mc^2$$\n";
        let html = export_to_html("Prueba", content, TyporaStylesheet::GitHub);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Título"));
        assert!(html.contains("katex.min.css"));
        assert!(html.contains("contrib/mhchem.min.js"));
        assert!(html.contains("E = mc^2"));
    }

    #[test]
    fn test_export_pdf() {
        let content = "# Título Documento\n\nTexto de prueba con formulas KaTeX.\n\n$$x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}$$\n";
        let out_path = std::path::Path::new("target/test_export.pdf");
        let res = export_to_pdf(out_path, "Título Documento", content, TyporaStylesheet::Newsprint);
        assert!(res.is_ok());
        assert!(out_path.exists());
        let meta = std::fs::metadata(out_path).unwrap();
        assert!(meta.len() > 100);
        let _ = std::fs::remove_file(out_path);
    }
}
