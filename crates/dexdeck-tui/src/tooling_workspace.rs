use dexdeck_protocol::{AndroidAvd, AndroidDevice, DoctorCheck, DoctorStatus, GradleTask};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{LazuliTheme, VirtualList};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToolingTab {
    #[default]
    Devices,
    Emulators,
    GradleTasks,
    Commands,
    Doctor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolCommandView {
    pub name: String,
    pub argv: Vec<String>,
    pub trusted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolingAction {
    None,
    SelectDevice,
    StartEmulator { cold_boot: bool },
    StopEmulator,
    RunGradleTask,
    RunTrustedCommand,
    TrustCommand,
    Refresh,
}

#[derive(Clone, Debug, Default)]
pub struct ToolingWorkspace {
    pub tab: ToolingTab,
    pub devices: Vec<AndroidDevice>,
    pub selected_serial: Option<String>,
    pub emulators: Vec<AndroidAvd>,
    pub tasks: Vec<GradleTask>,
    pub commands: Vec<ToolCommandView>,
    pub doctor: Vec<DoctorCheck>,
    pub search: String,
    pub list: VirtualList,
    pub dirty: bool,
    row_regions: Vec<(usize, Rect)>,
    subview_regions: Vec<(ToolingTab, Rect)>,
}

impl ToolingWorkspace {
    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        if let Some(tab) = self
            .subview_regions
            .iter()
            .find(|(_, area)| contains(*area, x, y))
            .map(|(tab, _)| *tab)
        {
            self.set_tab(tab);
            return true;
        }
        let Some(index) = self
            .row_regions
            .iter()
            .find(|(_, area)| contains(*area, x, y))
            .map(|(index, _)| *index)
        else {
            return false;
        };
        self.list.selected = index;
        self.dirty = true;
        true
    }

    pub fn set_tab(&mut self, tab: ToolingTab) {
        self.tab = tab;
        self.list = VirtualList::default();
        self.dirty = true;
    }

    pub fn set_search(&mut self, search: impl Into<String>) {
        self.search = search.into();
        self.list = VirtualList::default();
        self.dirty = true;
    }

    pub fn move_subview(&mut self, _forward: bool) {
        self.set_tab(match self.tab {
            ToolingTab::Devices => ToolingTab::Emulators,
            ToolingTab::Emulators => ToolingTab::Devices,
            ToolingTab::GradleTasks => ToolingTab::Commands,
            ToolingTab::Commands => ToolingTab::GradleTasks,
            ToolingTab::Doctor => ToolingTab::Doctor,
        });
    }

    pub fn handle_key(&mut self, key: char) -> ToolingAction {
        let action = match key {
            '1' => {
                self.set_tab(ToolingTab::Devices);
                ToolingAction::None
            }
            '2' => {
                self.set_tab(ToolingTab::Emulators);
                ToolingAction::None
            }
            '3' => {
                self.set_tab(ToolingTab::GradleTasks);
                ToolingAction::None
            }
            '4' => {
                self.set_tab(ToolingTab::Commands);
                ToolingAction::None
            }
            '5' => {
                self.set_tab(ToolingTab::Doctor);
                ToolingAction::None
            }
            'j' => {
                self.list
                    .select(self.list.selected.saturating_add(1), self.item_count(), 20);
                ToolingAction::None
            }
            'k' => {
                self.list
                    .select(self.list.selected.saturating_sub(1), self.item_count(), 20);
                ToolingAction::None
            }
            'r' => ToolingAction::Refresh,
            's' if self.tab == ToolingTab::Devices => ToolingAction::SelectDevice,
            's' if self.tab == ToolingTab::Emulators => {
                ToolingAction::StartEmulator { cold_boot: false }
            }
            'c' if self.tab == ToolingTab::Emulators => {
                ToolingAction::StartEmulator { cold_boot: true }
            }
            'x' if self.tab == ToolingTab::Emulators => ToolingAction::StopEmulator,
            'e' if self.tab == ToolingTab::GradleTasks => ToolingAction::RunGradleTask,
            'e' if self.tab == ToolingTab::Commands => self
                .visible_commands()
                .get(self.list.selected)
                .map_or(ToolingAction::None, |command| {
                    if command.trusted {
                        ToolingAction::RunTrustedCommand
                    } else {
                        ToolingAction::TrustCommand
                    }
                }),
            _ => ToolingAction::None,
        };
        self.dirty = true;
        action
    }

    #[must_use]
    pub fn visible_tasks(&self) -> Vec<&GradleTask> {
        let query = self.search.to_ascii_lowercase();
        self.tasks
            .iter()
            .filter(|task| query.is_empty() || task.path.to_ascii_lowercase().contains(&query))
            .collect()
    }

    #[must_use]
    pub fn visible_commands(&self) -> Vec<&ToolCommandView> {
        let query = self.search.to_ascii_lowercase();
        self.commands
            .iter()
            .filter(|command| {
                query.is_empty() || command.name.to_ascii_lowercase().contains(&query)
            })
            .collect()
    }

    fn item_count(&self) -> usize {
        match self.tab {
            ToolingTab::Devices => self.devices.len(),
            ToolingTab::Emulators => self.emulators.len(),
            ToolingTab::GradleTasks => self.visible_tasks().len(),
            ToolingTab::Commands => self.visible_commands().len(),
            ToolingTab::Doctor => self.doctor.len(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect, theme: LazuliTheme) {
        let rows = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);
        let subviews: &[(ToolingTab, &str)] = match self.tab {
            ToolingTab::Devices | ToolingTab::Emulators => &[
                (ToolingTab::Devices, "Devices"),
                (ToolingTab::Emulators, "Emulators"),
            ],
            ToolingTab::GradleTasks | ToolingTab::Commands => &[
                (ToolingTab::GradleTasks, "Gradle tasks"),
                (ToolingTab::Commands, "Commands"),
            ],
            ToolingTab::Doctor => &[(ToolingTab::Doctor, "Environment checks")],
        };
        self.subview_regions.clear();
        let mut x = rows[0].x;
        let heading = Line::from(
            subviews
                .iter()
                .flat_map(|(tab, label)| {
                    let width = u16::try_from(label.len()).unwrap_or(u16::MAX);
                    self.subview_regions
                        .push((*tab, Rect::new(x, rows[0].y, width, 1)));
                    x = x.saturating_add(width).saturating_add(3);
                    [
                        Span::styled(
                            *label,
                            if *tab == self.tab {
                                theme.accent().add_modifier(Modifier::UNDERLINED)
                            } else {
                                theme.muted()
                            },
                        ),
                        Span::raw("   "),
                    ]
                })
                .collect::<Vec<_>>(),
        );
        frame.render_widget(
            Paragraph::new(vec![
                heading,
                Line::styled(
                    if self.search.is_empty() {
                        "No active filter".into()
                    } else {
                        format!("Filter: {}", self.search)
                    },
                    theme.muted(),
                ),
            ]),
            rows[0],
        );
        let lines = self.lines(theme);
        let viewport = usize::from(rows[1].height);
        let visible = self.list.visible(lines.len(), viewport);
        self.row_regions = visible
            .clone()
            .enumerate()
            .map(|(row, index)| {
                (
                    index,
                    Rect::new(
                        rows[1].x,
                        rows[1]
                            .y
                            .saturating_add(u16::try_from(row).unwrap_or(u16::MAX)),
                        rows[1].width,
                        1,
                    ),
                )
            })
            .collect();
        let visible_lines = if lines.is_empty() {
            vec![Line::styled("No items available", theme.muted())]
        } else {
            lines[visible].to_vec()
        };
        frame.render_widget(Paragraph::new(visible_lines).block(Block::new()), rows[1]);
        frame.render_widget(
            Paragraph::new(if theme.glyphs == crate::GlyphMode::Ascii {
                "Left/Right View  / Search  Ctrl+P Commands"
            } else {
                "←/→ View  / Search  Ctrl+P Commands"
            })
            .style(theme.muted()),
            rows[2],
        );
        self.dirty = false;
    }

    fn lines(&self, theme: LazuliTheme) -> Vec<Line<'static>> {
        let entries: Vec<String> = match self.tab {
            ToolingTab::Devices => self
                .devices
                .iter()
                .map(|device| {
                    format!(
                        "{} {:?} {}{}",
                        if self.selected_serial.as_deref() == Some(&device.serial) {
                            "SELECTED"
                        } else {
                            "AVAILABLE"
                        },
                        device.state,
                        device.serial,
                        device
                            .model
                            .as_deref()
                            .map_or(String::new(), |model| format!(" ({model})"))
                    )
                })
                .collect(),
            ToolingTab::Emulators => self
                .emulators
                .iter()
                .map(|avd| {
                    format!(
                        "{} {}",
                        if avd.running_serial.is_some() {
                            "RUNNING"
                        } else {
                            "STOPPED"
                        },
                        avd.name
                    )
                })
                .collect(),
            ToolingTab::GradleTasks => self
                .visible_tasks()
                .into_iter()
                .map(|task| {
                    format!(
                        "TASK {}  {}",
                        task.path,
                        task.description.as_deref().unwrap_or("")
                    )
                })
                .collect(),
            ToolingTab::Commands => self
                .visible_commands()
                .into_iter()
                .map(|command| {
                    format!(
                        "{} {}  {}",
                        if command.trusted {
                            "TRUSTED"
                        } else {
                            "CONFIRM"
                        },
                        command.name,
                        command.argv.join(" ")
                    )
                })
                .collect(),
            ToolingTab::Doctor => self
                .doctor
                .iter()
                .map(|check| {
                    let status = match check.status {
                        DoctorStatus::Ok => "OK",
                        DoctorStatus::Warning => "WARN",
                        DoctorStatus::Error => "ERROR",
                    };
                    format!(
                        "{status} {}: {}{}",
                        check.code,
                        check.message,
                        check
                            .suggestion
                            .as_deref()
                            .map_or(String::new(), |suggestion| format!(" — {suggestion}"))
                    )
                })
                .collect(),
        };
        entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                let selected = index == self.list.selected;
                let marker = if selected {
                    if theme.glyphs == crate::GlyphMode::Ascii {
                        ">"
                    } else {
                        "▌"
                    }
                } else {
                    " "
                };
                Line::from(vec![
                    Span::styled(
                        format!("{marker} "),
                        if selected {
                            theme.selected()
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        entry,
                        if selected {
                            theme.selected()
                        } else {
                            Style::default().fg(theme.colors.text_primary)
                        },
                    ),
                ])
            })
            .collect()
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}
