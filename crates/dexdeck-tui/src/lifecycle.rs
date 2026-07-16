use std::{env, time::Duration};

use crate::GlyphMode;

pub const MAX_ACTIVE_ANIMATION_FRAMES: u8 = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalProfile {
    pub remote: bool,
    pub tmux: bool,
    pub reduced_motion: bool,
    pub utf8: bool,
}

impl TerminalProfile {
    #[must_use]
    pub fn detect() -> Self {
        let locale = env::var("LC_ALL")
            .or_else(|_| env::var("LC_CTYPE"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        Self {
            remote: env::var_os("SSH_CONNECTION").is_some() || env::var_os("SSH_TTY").is_some(),
            tmux: env::var_os("TMUX").is_some(),
            reduced_motion: env::var_os("DEXDECK_REDUCED_MOTION").is_some(),
            utf8: locale.contains("utf-8") || locale.contains("utf8"),
        }
    }

    #[must_use]
    pub const fn glyph_mode(self, ascii_requested: bool) -> GlyphMode {
        if ascii_requested || !self.utf8 {
            GlyphMode::Ascii
        } else {
            GlyphMode::Unicode
        }
    }

    #[must_use]
    pub const fn frame_interval(self) -> Duration {
        if self.remote && !self.tmux {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(34)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ActiveAnimation {
    remaining: u8,
}

impl ActiveAnimation {
    pub fn start(&mut self, frames: u8, reduced_motion: bool) {
        self.remaining = if reduced_motion {
            0
        } else {
            frames.min(MAX_ACTIVE_ANIMATION_FRAMES)
        };
    }

    pub fn advance(&mut self) -> bool {
        if self.remaining == 0 {
            return false;
        }
        self.remaining -= 1;
        true
    }

    #[must_use]
    pub const fn active(self) -> bool {
        self.remaining > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_is_bounded_and_reduced_motion_disables_it() {
        let mut animation = ActiveAnimation::default();
        animation.start(u8::MAX, false);
        let mut frames = 0;
        while animation.advance() {
            frames += 1;
        }
        assert_eq!(frames, usize::from(MAX_ACTIVE_ANIMATION_FRAMES));
        animation.start(10, true);
        assert!(!animation.active());
    }
}
