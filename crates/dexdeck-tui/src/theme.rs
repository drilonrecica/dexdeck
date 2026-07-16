use std::env;

use ratatui::style::Color;

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
        let colors = match capability {
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
}
