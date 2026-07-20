use std::{collections::HashMap, ops::Range};

use crossterm::event::{KeyCode, KeyModifiers};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceId {
    #[default]
    Overview,
    Run,
    Tests,
    Logcat,
    Devices,
    Tasks,
    Doctor,
}

impl WorkspaceId {
    pub const ALL: [Self; 7] = [
        Self::Overview,
        Self::Run,
        Self::Tests,
        Self::Logcat,
        Self::Devices,
        Self::Tasks,
        Self::Doctor,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Run => "Run",
            Self::Tests => "Tests",
            Self::Logcat => "Logcat",
            Self::Devices => "Devices",
            Self::Tasks => "Tasks",
            Self::Doctor => "Doctor",
        }
    }

    #[must_use]
    pub const fn number(self) -> char {
        match self {
            Self::Overview => '1',
            Self::Run => '2',
            Self::Tests => '3',
            Self::Logcat => '4',
            Self::Devices => '5',
            Self::Tasks => '6',
            Self::Doctor => '7',
        }
    }

    #[must_use]
    pub const fn from_number(number: char) -> Option<Self> {
        match number {
            '1' => Some(Self::Overview),
            '2' => Some(Self::Run),
            '3' => Some(Self::Tests),
            '4' => Some(Self::Logcat),
            '5' => Some(Self::Devices),
            '6' => Some(Self::Tasks),
            '7' => Some(Self::Doctor),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NamedAction {
    CommandPalette,
    Help,
    Search,
    FocusNext,
    FocusPrevious,
    Overview,
    Run,
    Tests,
    Logcat,
    Devices,
    Tasks,
    Doctor,
    Quit,
}

impl NamedAction {
    pub const ALL: [Self; 13] = [
        Self::CommandPalette,
        Self::Help,
        Self::Search,
        Self::FocusNext,
        Self::FocusPrevious,
        Self::Overview,
        Self::Run,
        Self::Tests,
        Self::Logcat,
        Self::Devices,
        Self::Tasks,
        Self::Doctor,
        Self::Quit,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CommandPalette => "Open command palette",
            Self::Help => "Open help",
            Self::Search => "Search current view",
            Self::FocusNext => "Focus next pane",
            Self::FocusPrevious => "Focus previous pane",
            Self::Overview => "Open overview",
            Self::Run => "Open run workspace",
            Self::Tests => "Open tests workspace",
            Self::Logcat => "Open Logcat workspace",
            Self::Devices => "Open devices",
            Self::Tasks => "Open tasks",
            Self::Doctor => "Open doctor",
            Self::Quit => "Quit",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct KeyChord {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyChord {
    #[must_use]
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: HashMap<KeyChord, NamedAction>,
}

impl Keymap {
    pub fn bind(&mut self, chord: KeyChord, action: NamedAction) -> Result<(), KeyConflict> {
        if let Some(existing) = self.bindings.get(&chord).copied()
            && existing != action
        {
            return Err(KeyConflict {
                chord,
                existing,
                requested: action,
            });
        }
        self.bindings.insert(chord, action);
        Ok(())
    }

    #[must_use]
    pub fn action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<NamedAction> {
        self.bindings.get(&KeyChord::new(code, modifiers)).copied()
    }

    #[must_use]
    pub fn vim() -> Self {
        let mut map = Self::default();
        for (code, action) in [
            (KeyCode::Char('j'), NamedAction::FocusNext),
            (KeyCode::Char('k'), NamedAction::FocusPrevious),
            (KeyCode::Char('/'), NamedAction::Search),
            (KeyCode::Char('?'), NamedAction::Help),
            (KeyCode::Char('q'), NamedAction::Quit),
        ] {
            let _ = map.bind(KeyChord::new(code, KeyModifiers::NONE), action);
        }
        map
    }
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("key {chord:?} is already bound to {existing:?}, cannot bind {requested:?}")]
pub struct KeyConflict {
    pub chord: KeyChord,
    pub existing: NamedAction,
    pub requested: NamedAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteMatch {
    pub action: NamedAction,
    pub score: i32,
}

#[must_use]
pub fn fuzzy_actions(query: &str) -> Vec<PaletteMatch> {
    let query = query.trim().to_ascii_lowercase();
    let mut matches = NamedAction::ALL
        .into_iter()
        .filter_map(|action| {
            fuzzy_score(action.label(), &query).map(|score| PaletteMatch { action, score })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.action.label().cmp(right.action.label()))
    });
    matches
}

fn fuzzy_score(candidate: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let candidate = candidate.to_ascii_lowercase();
    let mut offset = 0;
    let mut score = 0;
    let mut previous = None;
    for character in query.chars() {
        let relative = candidate[offset..].find(character)?;
        let index = offset + relative;
        score += if previous == Some(index.saturating_sub(1)) {
            8
        } else {
            2
        };
        if index == 0 || candidate.as_bytes().get(index.saturating_sub(1)) == Some(&b' ') {
            score += 4;
        }
        score -= i32::try_from(relative).unwrap_or(i32::MAX);
        previous = Some(index);
        offset = index + character.len_utf8();
    }
    Some(score)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VirtualList {
    pub selected: usize,
    pub offset: usize,
}

impl VirtualList {
    pub fn select(&mut self, selected: usize, length: usize, viewport: usize) {
        self.selected = selected.min(length.saturating_sub(1));
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset.saturating_add(viewport) {
            self.offset = self.selected.saturating_add(1).saturating_sub(viewport);
        }
    }

    #[must_use]
    pub fn visible(self, length: usize, viewport: usize) -> Range<usize> {
        let start = self.offset.min(length);
        start..start.saturating_add(viewport).min(length)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusRegion {
    WorkspaceBar,
    #[default]
    Primary,
    Secondary,
    Actions,
}

impl FocusRegion {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::WorkspaceBar => Self::Primary,
            Self::Primary => Self::Secondary,
            Self::Secondary => Self::Actions,
            Self::Actions => Self::WorkspaceBar,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::WorkspaceBar => Self::Actions,
            Self::Primary => Self::WorkspaceBar,
            Self::Secondary => Self::Primary,
            Self::Actions => Self::Secondary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_order_is_deterministic() {
        assert_eq!(fuzzy_actions("op t"), fuzzy_actions("op t"));
        assert_eq!(fuzzy_actions("log")[0].action, NamedAction::Logcat);
    }

    #[test]
    fn reports_binding_conflicts() {
        let mut map = Keymap::default();
        let chord = KeyChord::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(map.bind(chord.clone(), NamedAction::Help).is_ok());
        assert!(map.bind(chord, NamedAction::Quit).is_err());
    }
}
