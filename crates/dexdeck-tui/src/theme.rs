use std::env;

use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorCapability {
    TrueColor,
    Ansi256,
    Ansi16,
    NoColor,
}

impl ColorCapability {
    #[must_use]
    pub fn detect(no_color: bool) -> Self {
        if no_color || env::var_os("NO_COLOR").is_some() {
            return Self::NoColor;
        }
        let color_term = env::var("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        if color_term.contains("truecolor") || color_term.contains("24bit") {
            return Self::TrueColor;
        }
        let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
        if term.contains("256color") {
            Self::Ansi256
        } else {
            Self::Ansi16
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlyphMode {
    Unicode,
    Ascii,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticColors {
    pub background: Color,
    pub surface: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub action: Color,
    pub focus: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LazuliTheme {
    pub colors: SemanticColors,
    pub glyphs: GlyphMode,
}

impl LazuliTheme {
    #[must_use]
    pub fn new(capability: ColorCapability, glyphs: GlyphMode) -> Self {
        Self::for_background(capability, glyphs, light_background_detected())
    }

    #[must_use]
    pub fn for_background(
        capability: ColorCapability,
        glyphs: GlyphMode,
        light_background: bool,
    ) -> Self {
        let colors = match capability {
            ColorCapability::TrueColor if light_background => SemanticColors {
                background: Color::Rgb(247, 249, 253),
                surface: Color::Rgb(229, 237, 250),
                border: Color::Rgb(88, 105, 134),
                text_primary: Color::Rgb(22, 31, 48),
                text_muted: Color::Rgb(75, 88, 110),
                action: Color::Rgb(20, 82, 185),
                focus: Color::Rgb(0, 93, 199),
                success: Color::Rgb(18, 119, 75),
                warning: Color::Rgb(151, 91, 0),
                error: Color::Rgb(180, 38, 55),
                info: Color::Rgb(0, 112, 128),
            },
            ColorCapability::TrueColor => SemanticColors {
                background: Color::Rgb(11, 16, 32),
                surface: Color::Rgb(20, 38, 74),
                border: Color::Rgb(83, 96, 120),
                text_primary: Color::Rgb(232, 239, 255),
                text_muted: Color::Rgb(150, 164, 188),
                action: Color::Rgb(57, 119, 246),
                focus: Color::Rgb(101, 155, 255),
                success: Color::Rgb(69, 209, 154),
                warning: Color::Rgb(242, 184, 91),
                error: Color::Rgb(255, 102, 122),
                info: Color::Rgb(89, 221, 234),
            },
            ColorCapability::Ansi256 if light_background => SemanticColors {
                background: Color::Indexed(255),
                surface: Color::Indexed(153),
                border: Color::Indexed(60),
                text_primary: Color::Indexed(234),
                text_muted: Color::Indexed(60),
                action: Color::Indexed(25),
                focus: Color::Indexed(27),
                success: Color::Indexed(28),
                warning: Color::Indexed(130),
                error: Color::Indexed(160),
                info: Color::Indexed(30),
            },
            ColorCapability::Ansi256 => SemanticColors {
                background: Color::Indexed(17),
                surface: Color::Indexed(24),
                border: Color::Indexed(60),
                text_primary: Color::Indexed(255),
                text_muted: Color::Indexed(110),
                action: Color::Indexed(33),
                focus: Color::Indexed(75),
                success: Color::Indexed(78),
                warning: Color::Indexed(221),
                error: Color::Indexed(204),
                info: Color::Indexed(80),
            },
            ColorCapability::Ansi16 => SemanticColors {
                background: Color::Black,
                surface: Color::Black,
                border: Color::DarkGray,
                text_primary: Color::White,
                text_muted: Color::Gray,
                action: Color::Blue,
                focus: Color::LightBlue,
                success: Color::Green,
                warning: Color::Yellow,
                error: Color::LightRed,
                info: Color::Cyan,
            },
            ColorCapability::NoColor => SemanticColors {
                background: Color::Reset,
                surface: Color::Reset,
                border: Color::Reset,
                text_primary: Color::Reset,
                text_muted: Color::Reset,
                action: Color::Reset,
                focus: Color::Reset,
                success: Color::Reset,
                warning: Color::Reset,
                error: Color::Reset,
                info: Color::Reset,
            },
        };
        Self { colors, glyphs }
    }

    #[must_use]
    pub const fn canvas(self) -> Style {
        Style::new()
            .bg(self.colors.background)
            .fg(self.colors.text_primary)
    }

    #[must_use]
    pub const fn surface(self) -> Style {
        Style::new()
            .bg(self.colors.surface)
            .fg(self.colors.text_primary)
    }

    #[must_use]
    pub const fn muted(self) -> Style {
        Style::new().fg(self.colors.text_muted)
    }

    #[must_use]
    pub const fn accent(self) -> Style {
        Style::new()
            .fg(self.colors.action)
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub const fn selected(self) -> Style {
        Style::new()
            .fg(self.colors.focus)
            .bg(self.colors.surface)
            .add_modifier(Modifier::BOLD)
    }

    #[must_use]
    pub const fn separator(self) -> Style {
        Style::new().fg(self.colors.border)
    }
}

fn light_background_detected() -> bool {
    let Some(value) = env::var_os("COLORFGBG") else {
        return false;
    };
    value
        .to_string_lossy()
        .rsplit(';')
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|background| background >= 7)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_resets_every_semantic_token() {
        let colors = LazuliTheme::new(ColorCapability::NoColor, GlyphMode::Ascii).colors;
        assert_eq!(colors.background, Color::Reset);
        assert_eq!(colors.surface, Color::Reset);
        assert_eq!(colors.border, Color::Reset);
        assert_eq!(colors.text_primary, Color::Reset);
        assert_eq!(colors.text_muted, Color::Reset);
        assert_eq!(colors.action, Color::Reset);
        assert_eq!(colors.focus, Color::Reset);
        assert_eq!(colors.success, Color::Reset);
        assert_eq!(colors.warning, Color::Reset);
        assert_eq!(colors.error, Color::Reset);
        assert_eq!(colors.info, Color::Reset);
    }

    #[test]
    fn selected_rows_use_the_surface_and_focus_tokens() {
        let theme =
            LazuliTheme::for_background(ColorCapability::TrueColor, GlyphMode::Unicode, false);
        let style = theme.selected();
        assert_eq!(style.bg, Some(theme.colors.surface));
        assert_eq!(style.fg, Some(theme.colors.focus));
        assert!(style.add_modifier.contains(ratatui::style::Modifier::BOLD));
    }

    #[test]
    fn light_and_dark_true_color_palettes_are_distinct() {
        let dark =
            LazuliTheme::for_background(ColorCapability::TrueColor, GlyphMode::Unicode, false);
        let light =
            LazuliTheme::for_background(ColorCapability::TrueColor, GlyphMode::Unicode, true);
        assert_ne!(dark.colors.background, light.colors.background);
        assert_ne!(dark.colors.text_primary, light.colors.text_primary);
    }
}
