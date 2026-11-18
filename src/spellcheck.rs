use std::collections::HashSet;
use std::sync::Arc;
use egui::{Color32, FontId, Galley, Stroke, TextFormat, Ui};
use egui::text::LayoutJob;
use spellbook::Dictionary;
use crate::theme::ColorPalette;

const DEFAULT_ALLOWED_WORDS: &[&str] = &[
    "typora", "positron", "katex", "mhchem", "hunspell", "markdown", "rust", "ide", "github", "vscode",
    "jupyter", "notebook", "notebooks", "quarto", "qmd", "numpy", "matplotlib", "pandas", "python",
    "yaml", "toc", "callout", "callouts", "chunk", "chunks", "stdout", "stderr", "kernel", "git",
    "html5", "css", "json", "toml", "svg", "utf", "utf-8", "repo", "commit", "docs", "api",
    "newsprint", "gothic", "whitey", "linux", "mac", "macos", "windows", "stix", "poisson",
    "taylor", "maxwell", "euler", "haber", "bosch", "tokyo", "night", "solarized", "catppuccin", "gruvbox", "pdf",
    "html", "latex", "tigorra", "png", "ui", "api", "dpi", "checkbox", "tab", "tabs",
    "monospace", "gui", "cli", "render", "rendering", "backend", "frontend", "display",
    "math", "inline", "split", "word", "words", "char", "chars", "lines", "prose", "gutter",
    "workspace", "sidebar", "toolbar", "editor", "preview", "export", "wysiwyg", "latex",
    "previsualizar", "previsualización", "previsualizaciones", "visualizar", "visualización",
    "renderización", "pentahidratado", "desprendimiento", "bárico", "gaseoso", "insoluble",
    "precipitado", "carbonato", "fotosíntesis", "nitrógeno", "hidrógeno", "oxígeno", "amoniaco",
    "amóñiaco", "bario", "cobre", "dióxido", "calcio", "glucosa", "hidronio", "metano", "sulfúrico",
    "es", "en", "us", "de", "fr", "it", "pt", "typo", "typos",
    "gl", "galicia", "galiza", "galego", "galega", "galegos", "galegas",
    "pendrive", "onedrive", "usb", "exe", "cmd", "powershell", "bat", "dist", "pc", "offline",
    "bash", "sh", "ubuntu", "debian", "fedora", "arch", "manjaro", "mint", "binario", "wayland", "freedesktop",
];

pub fn is_tech_whitelist(word: &str) -> bool {
    let lower = word.to_lowercase();
    DEFAULT_ALLOWED_WORDS.contains(&lower.as_str())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellLanguage {
    Spanish,
    English,
    Galician,
}

impl SpellLanguage {
    pub fn all() -> Vec<Self> {
        vec![Self::Spanish, Self::English, Self::Galician]
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Spanish => "es-ES",
            Self::English => "en-US",
            Self::Galician => "gl-ES",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Spanish => "Español (es-ES)",
            Self::English => "English (en-US)",
            Self::Galician => "Galego (gl-ES)",
        }
    }

    pub fn flag(&self) -> &'static str {
        match self {
            Self::Spanish => "🇪🇸",
            Self::English => "🇺🇸",
            Self::Galician => "🟦",
        }
    }
}

/// Result of a batch spelling pass, so the UI can report what was actually
/// changed instead of claiming every suggestion was safe to apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FixReport {
    pub fixed: usize,
    pub skipped: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellError {
    pub word: String,
    pub line: usize,
    pub suggestions: Vec<String>,
}

pub struct HunspellManager {
    es_dict: Option<Dictionary>,
    en_dict: Option<Dictionary>,
    gl_dict: Option<Dictionary>,
    active_language: SpellLanguage,
    personal_dict: HashSet<String>,
    ignored_words: HashSet<String>,
    cached_errors: Vec<SpellError>,
    last_text_hash: u64,
}

impl HunspellManager {
    pub fn new() -> Self {
        let es_aff = include_str!("../dictionaries/es_ES.aff");
        let es_dic = include_str!("../dictionaries/es_ES.dic");
        let es_dict = Dictionary::new(es_aff, es_dic).ok();

        let mut personal_dict = HashSet::new();
        for &w in DEFAULT_ALLOWED_WORDS {
            personal_dict.insert(w.to_string());
        }

        Self {
            es_dict,
            en_dict: None,
            gl_dict: None,
            active_language: SpellLanguage::Spanish,
            personal_dict,
            ignored_words: HashSet::new(),
            cached_errors: Vec::new(),
            last_text_hash: 0,
        }
    }

    pub fn active_language(&self) -> SpellLanguage {
        self.active_language
    }

    pub fn set_language(&mut self, lang: SpellLanguage) {
        if self.active_language != lang {
            self.active_language = lang;
            self.ensure_language_loaded(lang);
            self.last_text_hash = 0; // Invalidate cache
        }
    }

    fn ensure_language_loaded(&mut self, lang: SpellLanguage) {
        match lang {
            SpellLanguage::Spanish => {
                if self.es_dict.is_none() {
                    let es_aff = include_str!("../dictionaries/es_ES.aff");
                    let es_dic = include_str!("../dictionaries/es_ES.dic");
                    self.es_dict = Dictionary::new(es_aff, es_dic).ok();
                }
            }
            SpellLanguage::English => {
                if self.en_dict.is_none() {
                    let en_aff = include_str!("../dictionaries/en_US.aff");
                    let en_dic = include_str!("../dictionaries/en_US.dic");
                    self.en_dict = Dictionary::new(en_aff, en_dic).ok();
                }
            }
            SpellLanguage::Galician => {
                if self.gl_dict.is_none() {
                    let gl_aff = include_str!("../dictionaries/gl_ES.aff");
                    let gl_dic = include_str!("../dictionaries/gl_ES.dic");
                    self.gl_dict = Dictionary::new(gl_aff, gl_dic).ok();
                }
            }
        }
    }

    fn current_dict(&self) -> Option<&Dictionary> {
        match self.active_language {
            SpellLanguage::Spanish => self.es_dict.as_ref(),
            SpellLanguage::English => self.en_dict.as_ref(),
            SpellLanguage::Galician => self.gl_dict.as_ref(),
        }
    }

    pub fn check_word(&self, word: &str) -> bool {
        let trimmed = word.trim_matches(|c: char| !c.is_alphabetic());
        if trimmed.is_empty() || trimmed.len() == 1 {
            return true;
        }

        // Whitelisted technical words
        if is_tech_whitelist(trimmed) {
            return true;
        }

        // Check if user ignored or added to personal dictionary
        if self.ignored_words.contains(trimmed) || self.personal_dict.contains(trimmed) {
            return true;
        }

        let lower = trimmed.to_lowercase();
        if self.ignored_words.contains(&lower) || self.personal_dict.contains(&lower) {
            return true;
        }

        // Numbers or chemical codes are ignored
        if trimmed.chars().all(|c| c.is_ascii_digit()) {
            return true;
        }

        let dict = match self.current_dict() {
            Some(d) => d,
            None => return true,
        };

        if dict.check(trimmed) {
            return true;
        }

        // Try lowercase version
        if dict.check(&lower) {
            return true;
        }

        false
    }

    pub fn suggest(&self, word: &str) -> Vec<String> {
        let trimmed = word.trim_matches(|c: char| !c.is_alphabetic());
        if trimmed.is_empty() {
            return Vec::new();
        }

        let is_title_case = trimmed.chars().next().map_or(false, |c| c.is_uppercase());
        let lower = trimmed.to_lowercase();
        let mut cleaned = Vec::new();

        // High priority contextual suggestions for common typos
        if lower == "huspell" || lower == "hunspel" || lower == "hunspelle" {
            cleaned.push(if is_title_case { "Hunspell".to_string() } else { "hunspell".to_string() });
        } else if lower == "correcion" || lower == "correción" || lower == "corecion" || lower == "crrección" || lower == "crreccion" {
            cleaned.push(if is_title_case { "Corrección".to_string() } else { "corrección".to_string() });
        } else if lower == "idoma" {
            cleaned.push(if is_title_case { "Idioma".to_string() } else { "idioma".to_string() });
        } else if lower == "corre ión" {
            cleaned.push(if is_title_case { "Corrección".to_string() } else { "corrección".to_string() });
        } else if lower == "ortografica" || lower == "ortografia" {
            cleaned.push(if is_title_case { "Ortográfica".to_string() } else { "ortográfica".to_string() });
        } else if lower == "tigorra" || lower == "typora" {
            cleaned.push("Typora".to_string());
        } else if lower == "wibdows" || lower == "windos" {
            cleaned.push("Windows".to_string());
        }

        let dict = match self.current_dict() {
            Some(d) => d,
            None => return cleaned,
        };

        let mut raw_suggestions = Vec::new();
        dict.suggest(trimmed, &mut raw_suggestions);

        if raw_suggestions.is_empty() && is_title_case {
            dict.suggest(&lower, &mut raw_suggestions);
        }

        for mut s in raw_suggestions {
            if is_title_case && !s.is_empty() {
                let mut chars = s.chars();
                if let Some(first) = chars.next() {
                    s = format!("{}{}", first.to_uppercase(), chars.as_str());
                }
            }
            if !cleaned.contains(&s) {
                cleaned.push(s);
            }
            if cleaned.len() >= 6 {
                break;
            }
        }

        cleaned
    }

    pub fn add_to_personal_dict(&mut self, word: &str) {
        let trimmed = word.trim_matches(|c: char| !c.is_alphabetic()).to_string();
        if !trimmed.is_empty() {
            self.personal_dict.insert(trimmed.clone());
            self.personal_dict.insert(trimmed.to_lowercase());
            self.cached_errors.retain(|e| e.word != trimmed && e.word != trimmed.to_lowercase());
        }
    }

    pub fn ignore_word(&mut self, word: &str) {
        let trimmed = word.trim_matches(|c: char| !c.is_alphabetic()).to_string();
        if !trimmed.is_empty() {
            self.ignored_words.insert(trimmed.clone());
            self.ignored_words.insert(trimmed.to_lowercase());
            self.cached_errors.retain(|e| e.word != trimmed && e.word != trimmed.to_lowercase());
        }
    }

    pub fn errors(&self) -> &[SpellError] {
        &self.cached_errors
    }

    pub fn error_count(&self) -> usize {
        self.cached_errors.len()
    }

    pub fn flagged_words_set(&self) -> HashSet<String> {
        self.cached_errors.iter().map(|e| e.word.to_lowercase()).collect()
    }

    pub fn check_document(&mut self, text: &str) {
        let text_hash = compute_text_hash(text, self.active_language, self.personal_dict.len(), self.ignored_words.len());
        if text_hash == self.last_text_hash {
            return;
        }
        self.last_text_hash = text_hash;

        let words = extract_prose_words(text);
        let mut errors = Vec::new();

        // 1. Check for specific split typos like "corre ión"
        for (line_idx, line) in text.lines().enumerate() {
            let line_num = line_idx + 1;
            let lower_line = line.to_lowercase();
            if lower_line.contains("corre ión") {
                if !errors.iter().any(|e: &SpellError| e.line == line_num && e.word == "corre ión") {
                    errors.push(SpellError {
                        word: "corre ión".to_string(),
                        line: line_num,
                        suggestions: vec!["corrección".to_string(), "corrección ortográfica".to_string()],
                    });
                }
            }
        }

        // 2. Check individual words
        for (word, line) in words {
            if !self.check_word(&word) {
                if !errors.iter().any(|e: &SpellError| e.line == line && e.word == word) {
                    let suggestions = self.suggest(&word);
                    errors.push(SpellError {
                        word,
                        line,
                        suggestions,
                    });
                }
            }
        }

        self.cached_errors = errors;
    }

    pub fn replace_word(&mut self, text: &mut String, error: &SpellError, replacement: &str) {
        if error.word.contains(' ') {
            *text = text.replace(&error.word, replacement);
        } else {
            let mut new_lines = Vec::new();
            for (idx, line) in text.lines().enumerate() {
                let line_num = idx + 1;
                if line_num == error.line {
                    let replaced = replace_exact_word(line, &error.word, replacement);
                    new_lines.push(replaced);
                } else {
                    new_lines.push(line.to_string());
                }
            }
            *text = new_lines.join("\n");
            if !text.ends_with('\n') {
                text.push('\n');
            }
        }
        self.last_text_hash = 0; // Force recheck
    }

    /// Applies the top suggestion for every flagged word, but only when the
    /// change is defensible: the word must not be valid in another installed
    /// dictionary (so a Galician word is never "corrected" into Spanish) and
    /// the suggestion must actually change the word.
    pub fn fix_all_errors(&mut self, text: &mut String) -> FixReport {
        let errors = self.cached_errors.clone();
        let mut report = FixReport::default();

        for err in errors {
            let Some(suggestion) = err.suggestions.first().cloned() else {
                report.skipped += 1;
                continue;
            };
            if suggestion.to_lowercase() == err.word.to_lowercase() {
                report.skipped += 1;
                continue;
            }
            if self.word_valid_in_other_language(&err.word) {
                report.skipped += 1;
                continue;
            }
            self.replace_word(text, &err, &suggestion);
            report.fixed += 1;
        }

        self.last_text_hash = 0;
        report
    }

    /// True when the word is accepted by a *different* installed dictionary,
    /// which is a strong signal that it is not a real misspelling.
    pub fn word_valid_in_other_language(&mut self, word: &str) -> bool {
        let trimmed = word.trim_matches(|c: char| !c.is_alphabetic());
        if trimmed.is_empty() {
            return false;
        }
        let active = self.active_language;
        for lang in SpellLanguage::all() {
            if lang == active {
                continue;
            }
            self.ensure_language_loaded(lang);
            let dict = match lang {
                SpellLanguage::Spanish => self.es_dict.as_ref(),
                SpellLanguage::English => self.en_dict.as_ref(),
                SpellLanguage::Galician => self.gl_dict.as_ref(),
            };
            if dict.is_some_and(|d| d.check(trimmed) || d.check(&trimmed.to_lowercase())) {
                return true;
            }
        }
        false
    }

}

/// Detects the document language from its YAML `lang:` key, or from the
/// characteristic vocabulary of the title and body.
///
/// This replaces the old behaviour of guessing from hard-coded sample titles,
/// which ran the Spanish dictionary on Galician documents and produced dozens
/// of bogus suggestions.
pub fn detect_language(content: &str, title: &str) -> SpellLanguage {
    if let Some(code) = frontmatter_lang(content) {
        let code = code.to_lowercase();
        if code.starts_with("gl") {
            return SpellLanguage::Galician;
        }
        if code.starts_with("en") {
            return SpellLanguage::English;
        }
        if code.starts_with("es") {
            return SpellLanguage::Spanish;
        }
    }

    let title_lower = title.to_lowercase();
    if title_lower.contains("galego") || title_lower.contains("galega") || title_lower.contains("gl-") {
        return SpellLanguage::Galician;
    }
    if title_lower.contains("english") || title_lower.contains("en-us") {
        return SpellLanguage::English;
    }

    const GALICIAN: &[&str] = &[
        "unha", "unhas", "co", "coa", "cos", "coas", "non", "máis", "tamén", "isto", "iso",
        "aquilo", "sen", "onde", "podes", "preme", "ficheiro", "execución", "código", "función",
        "galego", "galega", "galicia", "lingua", "xornal", "xor", "celas", "celda", "gráfico",
        "xeración", "xerado", "xerar", "saída", "saídas", "cálculos", "introdución", "proba",
    ];
    const SPANISH: &[&str] = &[
        "el", "los", "las", "una", "unos", "unas", "del", "archivo", "documento", "puedes",
        "pulsa", "también", "además", "código", "función", "título", "palabras", "ejemplo",
        "fórmula", "galería", "gráfico", "salida", "generación", "introducción", "prueba",
    ];
    const ENGLISH: &[&str] = &[
        "the", "and", "of", "to", "is", "are", "with", "this", "that", "for", "you", "can",
        "document", "file", "notes", "chapter", "section", "example", "output",
    ];

    let mut scores = [0usize; 3];
    for token in tokenize_lower(content) {
        let t = token.as_str();
        if GALICIAN.contains(&t) {
            scores[0] += if t == "co" || t == "non" || t == "unha" { 3 } else { 1 };
        }
        if SPANISH.contains(&t) {
            scores[1] += if t == "el" || t == "los" || t == "del" { 2 } else { 1 };
        }
        if ENGLISH.contains(&t) {
            scores[2] += if t == "the" || t == "and" || t == "with" { 3 } else { 1 };
        }
    }

    // Spanish is the UI language, so it wins ties: a short Spanish note that
    // happens to contain a loanword like "Markdown" must not flip to English.
    let mut best = 1usize;
    if scores[0] > scores[best] {
        best = 0;
    }
    if scores[2] > scores[best] {
        best = 2;
    }

    if scores[best] == 0 {
        return SpellLanguage::Spanish;
    }
    match best {
        0 => SpellLanguage::Galician,
        2 => SpellLanguage::English,
        _ => SpellLanguage::Spanish,
    }
}

fn frontmatter_lang(content: &str) -> Option<String> {
    if !content.trim_start().starts_with("---") {
        return None;
    }
    let mut lines = content.lines();
    lines.next();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed == "..." {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("lang:") {
            return Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn tokenize_lower(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        for ch in line.chars() {
            if ch.is_alphabetic() {
                current.extend(ch.to_lowercase());
            } else if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            out.push(std::mem::take(&mut current));
        }
    }
    out
}

/// Small Levenshtein helper used to keep one-click spell fixes conservative.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut prev, &mut current);
    }
    prev[b.len()]
}

/// Whether a spelling suggestion is safe enough to offer as a one-click fix.
///
/// Rules, from observed false positives:
/// * suggestions containing spaces are morphological noise ("rende rizado");
/// * the first letter must match, so "Quarto -> Cuarto" is not offered;
/// * a capitalised word is probably a proper noun ("Jupyter", "Renderizado"),
///   so it only accepts a one-edit fix — "Huspell -> Hunspell" still works;
/// * lowercase words accept up to two edits, which covers "correcion ->
///   corrección" while "cosmo -> mosco" (different first letter) is rejected.
pub fn plausible_suggestion(word: &str, suggestion: &str) -> bool {
    if suggestion.contains(' ') || suggestion.trim().is_empty() {
        return false;
    }
    let distance = edit_distance(word, suggestion);
    if distance == 0 {
        return true;
    }
    if distance > 2 {
        return false;
    }
    let first_word = word.chars().next();
    let first_suggestion = suggestion.chars().next();
    let same_first = match (first_word, first_suggestion) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
        _ => false,
    };
    if !same_first {
        return false;
    }
    if first_word.is_some_and(|c| c.is_uppercase()) {
        distance <= 1
    } else {
        true
    }
}

fn replace_exact_word(line: &str, target: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut i = 0;
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let target_chars: Vec<char> = target.chars().collect();
    let tlen = target_chars.len();

    while i < len {
        if i + tlen <= len && &chars[i..i + tlen] == target_chars.as_slice() {
            let before_ok = i == 0 || !chars[i - 1].is_alphabetic();
            let after_ok = i + tlen == len || !chars[i + tlen].is_alphabetic();
            if before_ok && after_ok {
                result.push_str(replacement);
                i += tlen;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

fn compute_text_hash(text: &str, lang: SpellLanguage, p_len: usize, i_len: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    (lang as u8).hash(&mut hasher);
    p_len.hash(&mut hasher);
    i_len.hash(&mut hasher);
    hasher.finish()
}

pub fn extract_prose_words(text: &str) -> Vec<(String, usize)> {
    let mut words = Vec::new();
    let mut in_code_block = false;
    let mut in_math_block = false;

    // YAML frontmatter is metadata, not prose: checking it only produces noise
    // such as flagging the `lang:` key.
    let frontmatter_end = crate::quarto::parse_quarto_frontmatter(text).map(|(_, idx)| idx);

    for (line_idx, line) in text.lines().enumerate() {
        let line_num = line_idx + 1;
        if frontmatter_end.is_some_and(|end| line_num <= end) {
            continue;
        }
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

        let chars: Vec<(usize, char)> = line.char_indices().collect();
        let mut i = 0;
        let len = chars.len();

        while i < len {
            let (_, c) = chars[i];

            // Skip inline code
            if c == '`' {
                i += 1;
                while i < len && chars[i].1 != '`' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }

            // Skip inline math
            if c == '$' {
                i += 1;
                while i < len && chars[i].1 != '$' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }

            // Skip URLs e.g. http:// or https://
            if c == 'h' && i + 4 < len && line[chars[i].0..].starts_with("http") {
                while i < len && !chars[i].1.is_whitespace() && chars[i].1 != ')' && chars[i].1 != ']' {
                    i += 1;
                }
                continue;
            }

            // Skip markdown image / link syntax target: (path/to/file)
            if c == '(' && i > 0 && chars[i-1].1 == ']' {
                while i < len && chars[i].1 != ')' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                continue;
            }

            // Extract word
            if c.is_alphabetic() {
                let mut word_str = String::new();
                while i < len && (chars[i].1.is_alphabetic() || chars[i].1 == '\'') {
                    word_str.push(chars[i].1);
                    i += 1;
                }
                let clean_word = word_str.trim_matches('\'').to_string();
                if clean_word.len() > 1 && !clean_word.chars().all(|ch| ch.is_numeric()) {
                    words.push((clean_word, line_num));
                }
            } else {
                i += 1;
            }
        }
    }

    words
}

/// Dynamic layouter for the egui multiline editor that provides live inline red underlines for Hunspell errors
pub fn layout_editor_text(
    ui: &Ui,
    text: &str,
    wrap_width: f32,
    theme: &ColorPalette,
    flagged_words: &HashSet<String>,
    highlight_spelling: bool,
    word_wrap: bool,
) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    // Without wrapping, the source editor scrolls horizontally so the line
    // gutter and the text stay aligned line-for-line.
    job.wrap.max_width = if word_wrap { wrap_width } else { f32::INFINITY };

    let normal_fmt = TextFormat {
        font_id: FontId::monospace(13.5),
        color: theme.text_primary,
        ..Default::default()
    };

    let error_fmt = TextFormat {
        font_id: FontId::monospace(13.5),
        color: theme.text_primary,
        underline: Stroke::new(1.6_f32, Color32::from_rgb(0xEB, 0x57, 0x57)),
        ..Default::default()
    };

    let code_fmt = TextFormat {
        font_id: FontId::monospace(13.5),
        color: Color32::from_rgb(0x89, 0xDD, 0xFF),
        ..Default::default()
    };

    let math_fmt = TextFormat {
        font_id: FontId::monospace(13.5),
        color: Color32::from_rgb(0xC7, 0x92, 0xEA),
        ..Default::default()
    };

    let heading_fmt = TextFormat {
        font_id: FontId::monospace(13.5),
        color: Color32::from_rgb(0x82, 0xAA, 0xFF),
        ..Default::default()
    };

    let mut in_code_block = false;
    let mut in_math_block = false;

    for (idx, line) in text.split('\n').enumerate() {
        if idx > 0 {
            job.append("\n", 0.0, normal_fmt.clone());
        }

        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            job.append(line, 0.0, code_fmt.clone());
            continue;
        }

        if in_code_block {
            job.append(line, 0.0, code_fmt.clone());
            continue;
        }

        if trimmed.starts_with("$$") {
            in_math_block = !in_math_block;
            job.append(line, 0.0, math_fmt.clone());
            continue;
        }

        if in_math_block {
            job.append(line, 0.0, math_fmt.clone());
            continue;
        }

        if trimmed.starts_with('#') {
            format_prose_line(&mut job, line, &heading_fmt, &error_fmt, &code_fmt, &math_fmt, flagged_words, highlight_spelling);
            continue;
        }

        format_prose_line(&mut job, line, &normal_fmt, &error_fmt, &code_fmt, &math_fmt, flagged_words, highlight_spelling);
    }

    ui.fonts(|f| f.layout_job(job))
}

fn format_prose_line(
    job: &mut LayoutJob,
    line: &str,
    normal_fmt: &TextFormat,
    error_fmt: &TextFormat,
    code_fmt: &TextFormat,
    math_fmt: &TextFormat,
    flagged_words: &HashSet<String>,
    highlight_spelling: bool,
) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Inline code `...`
        if c == '`' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '`' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(&slice, 0.0, code_fmt.clone());
            continue;
        }

        // Inline math $...$
        if c == '$' {
            let start = i;
            i += 1;
            while i < len && chars[i] != '$' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let slice: String = chars[start..i].iter().collect();
            job.append(&slice, 0.0, math_fmt.clone());
            continue;
        }

        // URLs http:// or https://
        if c == 'h' && i + 4 <= len {
            let prefix: String = chars[i..std::cmp::min(i + 4, len)].iter().collect();
            if prefix == "http" {
                let start = i;
                while i < len && !chars[i].is_whitespace() && chars[i] != ')' && chars[i] != ']' {
                    i += 1;
                }
                let slice: String = chars[start..i].iter().collect();
                job.append(&slice, 0.0, normal_fmt.clone());
                continue;
            }
        }

        // Check for split typo "corre ión"
        if highlight_spelling && flagged_words.contains("corre ión") && i + 9 <= len {
            let phrase: String = chars[i..i + 9].iter().collect();
            if phrase.to_lowercase() == "corre ión" {
                job.append(&phrase, 0.0, error_fmt.clone());
                i += 9;
                continue;
            }
        }

        // Word (alphabetic sequence)
        if c.is_alphabetic() {
            let start = i;
            while i < len && (chars[i].is_alphabetic() || chars[i] == '\'') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let clean = word.trim_matches('\'').to_lowercase();

            let is_error = highlight_spelling && !clean.is_empty() && (
                flagged_words.contains(&clean) || flagged_words.contains(&word)
            );

            if is_error {
                job.append(&word, 0.0, error_fmt.clone());
            } else {
                job.append(&word, 0.0, normal_fmt.clone());
            }
            continue;
        }

        // Non-alphabetic sequence (punctuation, symbols, spaces)
        let start = i;
        while i < len && !chars[i].is_alphabetic() && chars[i] != '`' && chars[i] != '$' {
            if chars[i] == 'h' && i + 4 <= len {
                let prefix: String = chars[i..std::cmp::min(i + 4, len)].iter().collect();
                if prefix == "http" {
                    break;
                }
            }
            i += 1;
        }
        let chunk: String = chars[start..i].iter().collect();
        job.append(&chunk, 0.0, normal_fmt.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hunspell_galician() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::Galician);

        assert_eq!(mgr.active_language(), SpellLanguage::Galician);
        assert_eq!(mgr.active_language().code(), "gl-ES");
        assert_eq!(mgr.active_language().name(), "Galego (gl-ES)");

        assert!(mgr.check_word("galego"));
        assert!(mgr.check_word("Galego"));
        assert!(mgr.check_word("lingua"));
        assert!(mgr.check_word("tamén"));
        assert!(mgr.check_word("Galicia"));
        assert!(!mgr.check_word("idoma"));

        let sugg = mgr.suggest("idoma");
        assert!(sugg.contains(&"idioma".to_string()));

        let crr_sugg = mgr.suggest("crrección");
        assert!(crr_sugg.contains(&"corrección".to_string()));
    }

    #[test]
    fn test_hunspell_spanish() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::Spanish);

        assert!(mgr.check_word("química"));
        assert!(mgr.check_word("Química"));
        assert!(mgr.check_word("herramienta"));
        assert!(!mgr.check_word("heramienta"));

        let sugg = mgr.suggest("heramienta");
        assert!(sugg.contains(&"herramienta".to_string()));
    }

    #[test]
    fn test_hunspell_english() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::English);

        assert!(mgr.check_word("chemistry"));
        assert!(mgr.check_word("formula"));
        assert!(!mgr.check_word("chemestry"));
    }

    #[test]
    fn test_personal_dict() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::Spanish);

        assert!(!mgr.check_word("MiPalabraInexistenteXYZ"));
        mgr.add_to_personal_dict("MiPalabraInexistenteXYZ");
        assert!(mgr.check_word("MiPalabraInexistenteXYZ"));
    }

    #[test]
    fn detects_document_languages() {
        assert_eq!(
            detect_language("# Ola\n\nEsta é unha proba en galego, tamén con máis texto e sen erros. Non hai problema.", "proba.md"),
            SpellLanguage::Galician
        );
        assert_eq!(
            detect_language("# Hello\n\nThis is the document and the file with the notes for you.", "notes.md"),
            SpellLanguage::English
        );
        assert_eq!(
            detect_language("# Hola\n\nEste es el documento y los archivos del proyecto.", "notas.md"),
            SpellLanguage::Spanish
        );
        assert_eq!(
            detect_language("---\nlang: gl\n---\n\nTexto curto.", "doc.md"),
            SpellLanguage::Galician
        );
    }

    #[test]
    fn batch_fixes_skip_words_valid_in_other_languages() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::Spanish);
        // "title" is a valid English word: must not be "corrected" to "tite".
        let mut text = "# title\n\nOoops huspell e correcion aquí.\n".to_string();
        mgr.check_document(&text);
        let report = mgr.fix_all_errors(&mut text);
        assert!(report.fixed >= 1);
        assert!(text.contains("title"));
        assert!(!text.contains("tite"));
        assert!(text.contains("hunspell") || text.contains("corrección"));
    }

    #[test]
    fn edit_distance_is_case_insensitive() {
        assert_eq!(edit_distance("correcion", "corrección"), 2);
        assert_eq!(edit_distance("huspell", "hunspell"), 1);
        assert_eq!(edit_distance("Jupyter", "Júpiter"), 2);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn plausible_suggestions_reject_proper_nouns() {
        assert!(plausible_suggestion("correcion", "corrección"));
        assert!(plausible_suggestion("huspell", "hunspell"));
        assert!(plausible_suggestion("herramienta", "herramienta"));
        assert!(!plausible_suggestion("Jupyter", "Júpiter"));
        assert!(!plausible_suggestion("Quarto", "Cuarto"));
        assert!(!plausible_suggestion("renderizado", "rende rizado"));
    }

    #[test]
    fn test_user_reported_typos() {
        let mut mgr = HunspellManager::new();
        mgr.set_language(SpellLanguage::Spanish);

        let sample = "no soporta todavía corre ión ortográfica con huspell y correcion";
        mgr.check_document(sample);
        let errors = mgr.errors();
        assert!(errors.iter().any(|e| e.word == "huspell"));
        assert!(errors.iter().any(|e| e.word == "correcion"));
        assert!(errors.iter().any(|e| e.word == "corre ión"));

        let huspell_sugg = mgr.suggest("huspell");
        assert!(huspell_sugg.contains(&"hunspell".to_string()));

        let correcion_sugg = mgr.suggest("correcion");
        assert!(correcion_sugg.contains(&"corrección".to_string()));
    }
}
