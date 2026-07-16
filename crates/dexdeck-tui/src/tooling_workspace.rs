use dexdeck_protocol::{AndroidAvd, AndroidDevice, DoctorCheck, DoctorStatus, GradleTask};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
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
}

impl ToolingWorkspace {
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
        let block = Block::default()
            .title(" Devices & tooling ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.colors.border));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let rows = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);
        frame.render_widget(
            Paragraph::new(format!(
                "1 Devices  2 Emulators  3 Gradle tasks  4 Commands  5 Doctor\nview: {:?} | search: {}",
                self.tab,
                if self.search.is_empty() { "none" } else { &self.search }
            )),
            rows[0],
        );
        let lines = self.lines(theme);
        let viewport = usize::from(rows[1].height);
        let visible = self.list.visible(lines.len(), viewport);
        frame.render_widget(Paragraph::new(lines[visible].to_vec()), rows[1]);
        frame.render_widget(
            Paragraph::new("j/k select  r refresh  s select/start  c cold boot  x stop  e execute")
                .style(Style::default().fg(theme.colors.text_muted)),
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
                Line::styled(
                    entry,
                    if index == self.list.selected {
                        Style::default()
                            .fg(theme.colors.focus)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.colors.text_primary)
                    },
                )
            })
            .collect()
    }
}
