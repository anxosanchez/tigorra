use std::sync::Arc;

use egui::{Color32, FontData, FontDefinitions, FontFamily, Rounding, Stroke, Visuals};

/// Installs bundled symbol fallbacks so the UI icons (✓ ✗ ● ○ ⚠ ⌘ ⇧ ⏱ ⏹ ⟳ …)
/// render on every platform. Without this, epaint's default Latin/emoji fonts
/// leave many dingbats as empty boxes — visible in the status bar, panels and
/// toolbars.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "dejavu-sans".to_owned(),
        Arc::new(FontData::from_static(include_bytes!("../assets/fonts/DejaVuSans.ttf"))),
    );
    fonts.font_data.insert(
        "noto-symbols-2".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansSymbols2-Regular.ttf"
        ))),
    );

    // Append as fallbacks: the default proportional/monospace fonts keep their
    // Latin typography, and the symbol fonts only fill in missing glyphs.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .extend(["dejavu-sans".to_owned(), "noto-symbols-2".to_owned()]);
    }

    ctx.set_fonts(fonts);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeId {
    TokyoNight,
    SolarizedDark,
    SolarizedLight,
    VscodeDark,
    CatppuccinMocha,
    TyporaNight,
    GruvboxDark,
}

impl ThemeId {
    /// Stable identifier persisted in the preferences file.
    pub fn key(&self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyo-night",
            Self::SolarizedDark => "solarized-dark",
            Self::SolarizedLight => "solarized-light",
            Self::VscodeDark => "vscode-dark",
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::TyporaNight => "typora-night",
            Self::GruvboxDark => "gruvbox-dark",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let needle = name.trim().to_lowercase();
        [
            Self::TokyoNight,
            Self::SolarizedDark,
            Self::SolarizedLight,
            Self::VscodeDark,
            Self::CatppuccinMocha,
            Self::TyporaNight,
            Self::GruvboxDark,
        ]
        .into_iter()
        .find(|id| id.key() == needle || id.key().replace('-', " ") == needle)
    }
}

#[derive(Clone, Debug)]
pub struct ColorPalette {
    pub id: ThemeId,
    pub name: &'static str,
    pub bg_color: Color32,
    pub sidebar_bg: Color32,
    pub activity_bar_bg: Color32,
    pub header_bg: Color32,
    pub status_bg: Color32,
    pub tab_active_bg: Color32,
    pub tab_inactive_bg: Color32,
    pub text_primary: Color32,
    pub text_body: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub selection_bg: Color32,
    pub border: Color32,
    pub code_bg: Color32,
    pub quote_bg: Color32,
    pub is_dark: bool,
}

impl ColorPalette {
    pub fn get(id: ThemeId) -> Self {
        match id {
            ThemeId::TokyoNight => Self {
                id,
                name: "Tokyo Night",
                bg_color: Color32::from_rgb(0x1A, 0x1B, 0x26),
                sidebar_bg: Color32::from_rgb(0x16, 0x16, 0x1E),
                activity_bar_bg: Color32::from_rgb(0x13, 0x14, 0x1C),
                header_bg: Color32::from_rgb(0x1F, 0x23, 0x35),
                status_bg: Color32::from_rgb(0x1F, 0x23, 0x35),
                tab_active_bg: Color32::from_rgb(0x1A, 0x1B, 0x26),
                tab_inactive_bg: Color32::from_rgb(0x16, 0x16, 0x1E),
                text_primary: Color32::from_rgb(0xC0, 0xCA, 0xF5),
                text_body: Color32::from_rgb(0x9A, 0xA5, 0xCE),
                text_muted: Color32::from_rgb(0x56, 0x5F, 0x89),
                accent: Color32::from_rgb(0x7A, 0xA2, 0xF7),
                selection_bg: Color32::from_rgb(0x28, 0x34, 0x57),
                border: Color32::from_rgb(0x29, 0x2E, 0x42),
                code_bg: Color32::from_rgb(0x13, 0x14, 0x1D),
                quote_bg: Color32::from_rgb(0x1C, 0x1E, 0x2D),
                is_dark: true,
            },
            ThemeId::SolarizedDark => Self {
                id,
                name: "Solarized Dark",
                bg_color: Color32::from_rgb(0x00, 0x2B, 0x36),
                sidebar_bg: Color32::from_rgb(0x07, 0x36, 0x42),
                activity_bar_bg: Color32::from_rgb(0x00, 0x21, 0x2B),
                header_bg: Color32::from_rgb(0x07, 0x36, 0x42),
                status_bg: Color32::from_rgb(0x26, 0x8B, 0xD2),
                tab_active_bg: Color32::from_rgb(0x00, 0x2B, 0x36),
                tab_inactive_bg: Color32::from_rgb(0x07, 0x36, 0x42),
                text_primary: Color32::from_rgb(0x93, 0xA1, 0xA1),
                text_body: Color32::from_rgb(0x83, 0x94, 0x96),
                text_muted: Color32::from_rgb(0x58, 0x6E, 0x75),
                accent: Color32::from_rgb(0x2A, 0xA1, 0x98),
                selection_bg: Color32::from_rgb(0x07, 0x36, 0x42),
                border: Color32::from_rgb(0x0F, 0x46, 0x54),
                code_bg: Color32::from_rgb(0x00, 0x21, 0x2B),
                quote_bg: Color32::from_rgb(0x07, 0x36, 0x42),
                is_dark: true,
            },
            ThemeId::SolarizedLight => Self {
                id,
                name: "Solarized Light",
                bg_color: Color32::from_rgb(0xFD, 0xF6, 0xE3),
                sidebar_bg: Color32::from_rgb(0xEE, 0xE8, 0xD5),
                activity_bar_bg: Color32::from_rgb(0xE0, 0xD9, 0xC4),
                header_bg: Color32::from_rgb(0xEE, 0xE8, 0xD5),
                status_bg: Color32::from_rgb(0x26, 0x8B, 0xD2),
                tab_active_bg: Color32::from_rgb(0xFD, 0xF6, 0xE3),
                tab_inactive_bg: Color32::from_rgb(0xE4, 0xDE, 0xC8),
                text_primary: Color32::from_rgb(0x58, 0x6E, 0x75),
                text_body: Color32::from_rgb(0x65, 0x7B, 0x83),
                text_muted: Color32::from_rgb(0x93, 0xA1, 0xA1),
                accent: Color32::from_rgb(0x26, 0x8B, 0xD2),
                selection_bg: Color32::from_rgb(0xD9, 0xD2, 0xC2),
                border: Color32::from_rgb(0xD3, 0x36, 0x82),
                code_bg: Color32::from_rgb(0xF5, 0xEE, 0xDA),
                quote_bg: Color32::from_rgb(0xEB, 0xE4, 0xD0),
                is_dark: false,
            },
            ThemeId::VscodeDark => Self {
                id,
                name: "VS Code Dark+",
                bg_color: Color32::from_rgb(0x1E, 0x1E, 0x1E),
                sidebar_bg: Color32::from_rgb(0x25, 0x25, 0x26),
                activity_bar_bg: Color32::from_rgb(0x33, 0x33, 0x33),
                header_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                status_bg: Color32::from_rgb(0x00, 0x7A, 0xCC),
                tab_active_bg: Color32::from_rgb(0x1E, 0x1E, 0x1E),
                tab_inactive_bg: Color32::from_rgb(0x2D, 0x2D, 0x2D),
                text_primary: Color32::from_rgb(0xD4, 0xD4, 0xD4),
                text_body: Color32::from_rgb(0xCC, 0xCC, 0xCC),
                text_muted: Color32::from_rgb(0x80, 0x80, 0x80),
                accent: Color32::from_rgb(0x00, 0x7A, 0xCC),
                selection_bg: Color32::from_rgb(0x26, 0x4F, 0x78),
                border: Color32::from_rgb(0x3C, 0x3C, 0x3C),
                code_bg: Color32::from_rgb(0x18, 0x18, 0x18),
                quote_bg: Color32::from_rgb(0x25, 0x25, 0x26),
                is_dark: true,
            },
            ThemeId::CatppuccinMocha => Self {
                id,
                name: "Catppuccin Mocha",
                bg_color: Color32::from_rgb(0x1E, 0x1E, 0x2E),
                sidebar_bg: Color32::from_rgb(0x18, 0x18, 0x25),
                activity_bar_bg: Color32::from_rgb(0x11, 0x11, 0x1B),
                header_bg: Color32::from_rgb(0x18, 0x18, 0x25),
                status_bg: Color32::from_rgb(0x11, 0x11, 0x1B),
                tab_active_bg: Color32::from_rgb(0x1E, 0x1E, 0x2E),
                tab_inactive_bg: Color32::from_rgb(0x18, 0x18, 0x25),
                text_primary: Color32::from_rgb(0xCD, 0xD6, 0xF4),
                text_body: Color32::from_rgb(0xBA, 0xC2, 0xDE),
                text_muted: Color32::from_rgb(0x6C, 0x70, 0x86),
                accent: Color32::from_rgb(0x89, 0xB4, 0xFA),
                selection_bg: Color32::from_rgb(0x45, 0x47, 0x5A),
                border: Color32::from_rgb(0x31, 0x32, 0x44),
                code_bg: Color32::from_rgb(0x11, 0x11, 0x1B),
                quote_bg: Color32::from_rgb(0x18, 0x18, 0x25),
                is_dark: true,
            },
            ThemeId::TyporaNight => Self {
                id,
                name: "Typora Night",
                bg_color: Color32::from_rgb(0x36, 0x3B, 0x40),
                sidebar_bg: Color32::from_rgb(0x2E, 0x30, 0x33),
                activity_bar_bg: Color32::from_rgb(0x24, 0x26, 0x28),
                header_bg: Color32::from_rgb(0x28, 0x2A, 0x2D),
                status_bg: Color32::from_rgb(0x28, 0x2A, 0x2D),
                tab_active_bg: Color32::from_rgb(0x36, 0x3B, 0x40),
                tab_inactive_bg: Color32::from_rgb(0x28, 0x2A, 0x2D),
                text_primary: Color32::from_rgb(0xDE, 0xDE, 0xDE),
                text_body: Color32::from_rgb(0xB8, 0xBF, 0xC6),
                text_muted: Color32::from_rgb(0x82, 0x88, 0x90),
                accent: Color32::from_rgb(0x6D, 0xC1, 0xE7),
                selection_bg: Color32::from_rgb(0x4A, 0x89, 0xDC),
                border: Color32::from_rgb(0x48, 0x4D, 0x54),
                code_bg: Color32::from_rgb(0x24, 0x27, 0x2A),
                quote_bg: Color32::from_rgb(0x2F, 0x33, 0x37),
                is_dark: true,
            },
            ThemeId::GruvboxDark => Self {
                id,
                name: "Gruvbox Dark",
                bg_color: Color32::from_rgb(0x28, 0x28, 0x28),
                sidebar_bg: Color32::from_rgb(0x1D, 0x20, 0x21),
                activity_bar_bg: Color32::from_rgb(0x18, 0x1A, 0x1B),
                header_bg: Color32::from_rgb(0x1D, 0x20, 0x21),
                status_bg: Color32::from_rgb(0x3C, 0x38, 0x36),
                tab_active_bg: Color32::from_rgb(0x28, 0x28, 0x28),
                tab_inactive_bg: Color32::from_rgb(0x1D, 0x20, 0x21),
                text_primary: Color32::from_rgb(0xEB, 0xDB, 0xB2),
                text_body: Color32::from_rgb(0xD5, 0xC4, 0xA1),
                text_muted: Color32::from_rgb(0x92, 0x83, 0x74),
                accent: Color32::from_rgb(0xFA, 0xBD, 0x2F),
                selection_bg: Color32::from_rgb(0x50, 0x49, 0x45),
                border: Color32::from_rgb(0x3C, 0x38, 0x36),
                code_bg: Color32::from_rgb(0x1D, 0x20, 0x21),
                quote_bg: Color32::from_rgb(0x32, 0x30, 0x2F),
                is_dark: true,
            },
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::get(ThemeId::TokyoNight),
            Self::get(ThemeId::VscodeDark),
            Self::get(ThemeId::CatppuccinMocha),
            Self::get(ThemeId::SolarizedDark),
            Self::get(ThemeId::SolarizedLight),
            Self::get(ThemeId::TyporaNight),
            Self::get(ThemeId::GruvboxDark),
        ]
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.is_dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };

        visuals.override_text_color = Some(self.text_body);
        visuals.panel_fill = self.bg_color;
        visuals.window_fill = self.bg_color;
        visuals.faint_bg_color = self.sidebar_bg;
        visuals.extreme_bg_color = self.code_bg;

        visuals.selection.bg_fill = self.selection_bg;
        visuals.selection.stroke = Stroke::new(1.0_f32, self.accent);

        let btn_bg = if self.is_dark {
            Color32::from_rgb(
                self.sidebar_bg.r().saturating_add(15),
                self.sidebar_bg.g().saturating_add(15),
                self.sidebar_bg.b().saturating_add(18),
            )
        } else {
            Color32::from_rgb(
                self.sidebar_bg.r().saturating_sub(15),
                self.sidebar_bg.g().saturating_sub(15),
                self.sidebar_bg.b().saturating_sub(18),
            )
        };

        visuals.widgets.noninteractive.bg_fill = self.bg_color;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.noninteractive.rounding = Rounding::same(4.0_f32);

        visuals.widgets.inactive.bg_fill = btn_bg;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.inactive.rounding = Rounding::same(4.0_f32);

        visuals.widgets.hovered.bg_fill = self.selection_bg;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, self.accent);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.hovered.rounding = Rounding::same(4.0_f32);

        visuals.widgets.active.bg_fill = self.sidebar_bg;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, self.accent);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, self.accent);
        visuals.widgets.active.rounding = Rounding::same(4.0_f32);

        visuals.widgets.open.bg_fill = self.sidebar_bg;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, self.accent);
        visuals.widgets.open.rounding = Rounding::same(4.0_f32);

        ctx.set_visuals(visuals);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_symbol_fonts_are_valid_ttf() {
        let dejavu = include_bytes!("../assets/fonts/DejaVuSans.ttf");
        let noto = include_bytes!("../assets/fonts/NotoSansSymbols2-Regular.ttf");
        assert!(dejavu.starts_with(&[0x00, 0x01, 0x00, 0x00]), "DejaVu is a TrueType font");
        assert!(noto.starts_with(&[0x00, 0x01, 0x00, 0x00]), "Noto Symbols is a TrueType font");
        assert!(dejavu.len() > 500_000);
        assert!(noto.len() > 500_000);
    }

    #[test]
    fn install_fonts_registers_fallbacks_without_panicking() {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
    }
}
