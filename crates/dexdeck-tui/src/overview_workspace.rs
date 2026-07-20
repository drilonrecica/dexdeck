use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{DashboardLayout, LazuliTheme};

#[derive(Clone, Debug)]
pub struct OverviewWorkspace {
    pub project: Option<String>,
    pub module: Option<String>,
    pub variant: Option<String>,
    pub device: Option<String>,
    pub model_status: String,
    pub active_jobs: usize,
    pub recent_result: Option<String>,
    pub selected_action: usize,
    pub dirty: bool,
    action_rows: Vec<Rect>,
}

impl Default for OverviewWorkspace {
    fn default() -> Self {
        Self {
            project: None,
            module: None,
            variant: None,
            device: None,
            model_status: "Detecting project".into(),
            active_jobs: 0,
            recent_result: None,
            selected_action: 0,
            dirty: true,
            action_rows: Vec::new(),
        }
    }
}

impl OverviewWorkspace {
    pub fn move_selection(&mut self, down: bool) {
        self.selected_action = if down {
            (self.selected_action + 1).min(2)
        } else {
            self.selected_action.saturating_sub(1)
        };
        self.dirty = true;
    }

    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        let Some(index) = self
            .action_rows
            .iter()
            .position(|area| contains(*area, x, y))
        else {
            return false;
        };
        self.selected_action = index;
        self.dirty = true;
        true
    }

    pub fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        layout: DashboardLayout,
        theme: LazuliTheme,
    ) {
        let columns = if layout == DashboardLayout::Full {
            Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area)
        } else {
            Layout::horizontal([Constraint::Percentage(100)]).split(area)
        };

        let left_rows = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(1),
        ])
        .split(columns[0]);
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("Ready for development", theme.accent()),
                Line::styled(
                    "Service-backed actions are not connected in this build.",
                    theme.muted(),
                ),
            ]),
            left_rows[0],
        );

        let actions = [
            "Run application          Unavailable",
            "Build selected variant   Unavailable",
            "Run tests                Unavailable",
        ];
        self.action_rows = actions
            .iter()
            .enumerate()
            .map(|(index, _)| {
                Rect::new(
                    left_rows[1].x,
                    left_rows[1]
                        .y
                        .saturating_add(u16::try_from(index).unwrap_or(u16::MAX)),
                    left_rows[1].width,
                    1,
                )
            })
            .collect();
        let action_lines = actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let marker = if index == self.selected_action {
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
                        if index == self.selected_action {
                            theme.selected()
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        *action,
                        if index == self.selected_action {
                            theme.selected()
                        } else {
                            Style::default().fg(theme.colors.text_primary)
                        },
                    ),
                ])
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(action_lines), left_rows[1]);

        let context = format!(
            "Project   {}\nTarget    {} / {}\nDevice    {}\nModel     {}",
            self.project.as_deref().unwrap_or("Not detected"),
            self.module.as_deref().unwrap_or("Select module"),
            self.variant.as_deref().unwrap_or("Select variant"),
            self.device.as_deref().unwrap_or("Select device"),
            self.model_status,
        );
        frame.render_widget(Paragraph::new(context), left_rows[2]);

        if layout == DashboardLayout::Full {
            let right = Rect::new(
                columns[1].x.saturating_add(2),
                columns[1].y,
                columns[1].width.saturating_sub(2),
                columns[1].height,
            );
            let rows = Layout::vertical([
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(right);
            frame.render_widget(Paragraph::new("Active work").style(theme.accent()), rows[0]);
            frame.render_widget(
                Paragraph::new(if self.active_jobs == 0 {
                    "No running jobs".into()
                } else {
                    format!("{} job(s) running", self.active_jobs)
                })
                .style(theme.muted()),
                rows[1],
            );
            frame.render_widget(
                Paragraph::new("Recent result").style(theme.accent()),
                rows[2],
            );
            frame.render_widget(
                Paragraph::new(self.recent_result.as_deref().unwrap_or("No recent result"))
                    .style(theme.muted()),
                rows[3],
            );
        }
        self.dirty = false;
    }
}

fn contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}
