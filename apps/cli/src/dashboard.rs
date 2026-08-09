use std::io::{self, IsTerminal};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal as crossterm_terminal,
};
use ratatui::layout::Rect;

use agentboard_core::{model::Workspace, CancellationToken};

use crate::store::source_snapshots;

mod input;
mod terminal;
mod view;

use input::{clicked_source, dashboard_command, next_source, previous_source, DashboardCommand};
use terminal::TerminalSession;
use view::{dashboard_areas, render_view, view_signature};

pub fn require_dashboard_terminals(stdin: bool, stdout: bool) -> Result<()> {
    if stdin && stdout {
        Ok(())
    } else {
        bail!("Dashboard requires interactive stdin and stdout")
    }
}

pub fn dashboard(ws: &Workspace, cancellation: CancellationToken) -> Result<()> {
    require_dashboard_terminals(io::stdin().is_terminal(), io::stdout().is_terminal())?;
    let mut terminal = TerminalSession::enter()?;
    let mut selected = 0_usize;
    let mut last_view = None;

    loop {
        if cancellation.is_cancelled() {
            return Err(crate::runtime::InvocationCancelled.into());
        }
        let (width, height) = crossterm_terminal::size().context("read Dashboard terminal size")?;
        let snapshots = source_snapshots(ws)?;
        if selected >= snapshots.len() {
            selected = 0;
        }

        let view = view_signature(&snapshots, selected, width as usize);
        if visible_view_changed(last_view.as_deref(), &view) {
            terminal
                .terminal
                .draw(|frame| render_view(frame, &snapshots, selected))?;
            last_view = Some(view);
        }

        if event::poll(Duration::from_secs(1)).context("poll Dashboard input")? {
            while event::poll(Duration::ZERO).context("poll Dashboard input")? {
                match event::read().context("read Dashboard input")? {
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Press {
                            continue;
                        }
                        match dashboard_command(key.code, key.modifiers) {
                            DashboardCommand::Quit => return Ok(()),
                            DashboardCommand::Previous if !snapshots.is_empty() => {
                                selected = previous_source(selected, snapshots.len());
                                last_view = None;
                            }
                            DashboardCommand::Next if !snapshots.is_empty() => {
                                selected = next_source(selected, snapshots.len());
                                last_view = None;
                            }
                            DashboardCommand::Previous
                            | DashboardCommand::Next
                            | DashboardCommand::Ignore => {}
                        }
                    }
                    Event::Mouse(mouse) => {
                        if let Some(source) = clicked_source(
                            &mouse,
                            dashboard_areas(Rect::new(0, 0, width, height))[1],
                            &snapshots,
                        ) {
                            selected = source;
                            last_view = None;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn visible_view_changed(previous: Option<&str>, current: &str) -> bool {
    previous != Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{SnapshotItem, SnapshotState, SourceSnapshot};
    use agentboard_core::model::Item;
    use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use serde_json::json;

    fn snapshot(state: SnapshotState, items: Vec<SnapshotItem>) -> SourceSnapshot {
        SourceSnapshot {
            source_id: "issues".into(),
            state,
            items,
        }
    }

    fn item(reference_id: &str, title: &str) -> SnapshotItem {
        SnapshotItem {
            item: Item {
                id: reference_id.into(),
                reference_id: reference_id.into(),
                title: title.into(),
                status: "ready".into(),
                url: "https://example.test".into(),
                source_id: "issues".into(),
                source_kind: "test".into(),
                raw: json!({}),
            },
            result: "pending",
        }
    }

    fn rendered_view(snapshots: &[SourceSnapshot], selected: usize, width: usize) -> String {
        let backend = TestBackend::new(width as u16, 12);
        let mut terminal = Terminal::new(backend).unwrap();
        let frame = terminal
            .draw(|frame| render_view(frame, snapshots, selected))
            .unwrap();
        buffer_text(frame.buffer)
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let area = buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rejects_non_interactive_streams() {
        assert!(require_dashboard_terminals(false, true).is_err());
        assert!(require_dashboard_terminals(true, false).is_err());
        assert!(require_dashboard_terminals(true, true).is_ok());
    }

    #[test]
    fn renders_distinct_empty_states() {
        assert!(rendered_view(&[], 0, 80).contains("no configured Sources"));
        assert!(
            rendered_view(&[snapshot(SnapshotState::Missing, vec![])], 0, 80)
                .contains("no current Snapshot")
        );
        assert!(
            rendered_view(&[snapshot(SnapshotState::Ready, vec![])], 0, 80).contains("zero Items")
        );
    }

    #[test]
    fn wraps_source_navigation() {
        let snapshots = vec![
            snapshot(SnapshotState::Ready, vec![]),
            SourceSnapshot {
                source_id: "mine".into(),
                state: SnapshotState::Ready,
                items: vec![],
            },
        ];
        assert!(rendered_view(&snapshots, 0, 80).contains("issues"));
        assert!(rendered_view(&snapshots, 1, 80).contains("mine"));
    }

    #[test]
    fn truncates_only_the_title_when_width_is_limited() {
        let text = rendered_view(
            &[snapshot(
                SnapshotState::Ready,
                vec![item("AB-123", "A very long title that must be truncated")],
            )],
            0,
            80,
        );
        assert!(text.contains("AB-123"));
        assert!(text.contains("ready"));
        assert!(text.contains("pending"));
        assert!(text.contains('…'));
    }

    #[test]
    fn mouse_clicks_select_source_tabs() {
        let snapshots = vec![
            snapshot(SnapshotState::Ready, vec![]),
            SourceSnapshot {
                source_id: "mine".into(),
                state: SnapshotState::Ready,
                items: vec![],
            },
        ];
        let tabs_area = dashboard_areas(Rect::new(0, 0, 80, 12))[1];
        let first = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tabs_area.x + 1,
            row: tabs_area.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        let second = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: tabs_area.x + "issues".chars().count() as u16 + 2,
            row: tabs_area.y + 1,
            modifiers: KeyModifiers::NONE,
        };
        let outside = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: tabs_area.x + 1,
            row: tabs_area.y + 1,
            modifiers: KeyModifiers::NONE,
        };

        assert_eq!(clicked_source(&first, tabs_area, &snapshots), Some(0));
        assert_eq!(clicked_source(&second, tabs_area, &snapshots), Some(1));
        assert_eq!(clicked_source(&outside, tabs_area, &snapshots), None);
    }

    #[test]
    fn navigation_commands_wrap_and_quit() {
        assert_eq!(previous_source(0, 3), 2);
        assert_eq!(next_source(2, 3), 0);
        assert_eq!(
            dashboard_command(KeyCode::Char('c'), KeyModifiers::CONTROL),
            DashboardCommand::Quit
        );
        assert_eq!(
            dashboard_command(KeyCode::Left, KeyModifiers::NONE),
            DashboardCommand::Previous
        );
    }

    #[test]
    fn unchanged_visible_view_does_not_redraw() {
        assert!(!visible_view_changed(Some("same"), "same"));
        assert!(visible_view_changed(Some("old"), "new"));
        assert!(visible_view_changed(None, "first"));
    }

    #[test]
    fn table_fits_width_or_reports_minimum_width() {
        let snapshots = [snapshot(
            SnapshotState::Ready,
            vec![item("AB-123", "A long title")],
        )];
        let narrow = rendered_view(&snapshots, 0, 10);
        assert!(narrow.contains("Minimum"));

        let rendered = rendered_view(&snapshots, 0, 80);
        assert!(rendered.lines().all(|line| line.chars().count() <= 80));
    }
}
