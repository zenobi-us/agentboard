use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table},
    Frame,
};

use agentboard_core::model::{ActionConfig, Workspace, WorkspaceSource};
use serde_json::Value;

use crate::store::{
    ActionState, CollectionState, SnapshotState, SourceCollectionStatus, SourceSnapshot,
};

use super::logo::Logo;

pub(super) fn render_view(
    frame: &mut Frame,
    workspace: &Workspace,
    snapshots: &[SourceSnapshot],
    selected: usize,
    watching: bool,
    footer: &str,
) {
    let areas = dashboard_areas(frame.area());
    frame.render_widget(Logo::new(watching), areas[0]);

    if snapshots.is_empty() {
        frame.render_widget(
            Paragraph::new("Workspace has no configured Sources.")
                .block(Block::bordered().title(" Tickets ")),
            dashboard_columns(areas[1])[1],
        );
    } else {
        let columns = dashboard_columns(areas[1]);
        render_workspace_tree(frame, columns[0], workspace, snapshots, selected);
        render_ticket_list(frame, columns[1], &snapshots[selected]);
    }

    frame.render_widget(
        Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
        areas[2],
    );
}

pub(super) const WATCH_BUTTON_WIDTH: u16 = 14;

pub(super) fn watch_button_area(area: Rect) -> Rect {
    if area.width < WATCH_BUTTON_WIDTH + 2 || area.height < 3 {
        return Rect::default();
    }
    Rect {
        x: area.right() - WATCH_BUTTON_WIDTH - 1,
        y: area.y + 1,
        width: WATCH_BUTTON_WIDTH,
        height: 1,
    }
}

pub(super) fn dashboard_areas(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area)
        .to_vec()
}

pub(super) fn dashboard_columns(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Min(1)])
        .split(area)
        .to_vec()
}

fn render_workspace_tree(
    frame: &mut Frame,
    area: Rect,
    workspace: &Workspace,
    snapshots: &[SourceSnapshot],
    selected: usize,
) {
    let inner = Block::bordered().title(" Workspace config ");
    let content = inner.inner(area);
    frame.render_widget(inner, area);

    let mut row = content.y;
    for (index, source) in workspace.sources.iter().enumerate() {
        let snapshot = &snapshots[index];
        let source_style = if index == selected {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::BOLD)
        };
        render_tree_line(
            frame,
            Rect { y: row, ..content },
            vec![
                Span::styled(
                    snapshot_symbol(snapshot.state),
                    source_status_style(snapshot.state),
                ),
                Span::raw(" "),
                Span::styled(source.configured.id.clone(), source_style),
                Span::raw(format!("  {} items · ", snapshot.items.len())),
                Span::styled(
                    collection_label(snapshot.collection.as_ref()),
                    collection_status_style(snapshot.collection.as_ref()),
                ),
            ],
        );
        row = row.saturating_add(1);

        render_tree_line(
            frame,
            Rect { y: row, ..content },
            vec![
                Span::styled("  ├─ query: ", Style::default().fg(Color::DarkGray)),
                Span::raw(source_query(source)),
            ],
        );
        row = row.saturating_add(1);

        for (action_index, action) in source.configured.actions.iter().enumerate() {
            let state = if snapshot.state == SnapshotState::Missing {
                None
            } else {
                snapshot.actions.get(action_index).copied()
            };
            render_tree_line(
                frame,
                Rect { y: row, ..content },
                vec![
                    Span::raw(if action_index + 1 == source.configured.actions.len() {
                        "  └─ "
                    } else {
                        "  ├─ "
                    }),
                    Span::styled(action_symbol(state), action_status_style(state)),
                    Span::raw(" "),
                    Span::raw(action_label(action)),
                ],
            );
            row = row.saturating_add(1);
        }
    }
}

fn render_tree_line(frame: &mut Frame, area: Rect, line: Vec<Span<'static>>) {
    if area.height > 0 {
        frame.render_widget(Paragraph::new(Line::from(line)), area);
    }
}

fn render_ticket_list(frame: &mut Frame, area: Rect, snapshot: &SourceSnapshot) {
    if snapshot.state == SnapshotState::Missing {
        frame.render_widget(
            Paragraph::new("No current Snapshot. Run the Workspace successfully.")
                .block(Block::bordered().title(format!(" Tickets · {} ", snapshot.source_id))),
            area,
        );
        return;
    }
    if snapshot.items.is_empty() {
        frame.render_widget(
            Paragraph::new("No issue tickets found.")
                .block(Block::bordered().title(format!(" Tickets · {} ", snapshot.source_id))),
            area,
        );
        return;
    }

    let reference_width = snapshot
        .items
        .iter()
        .map(|item| display_width(&item.item.reference_id))
        .max()
        .unwrap_or(0)
        .max(display_width("Reference"));
    let status_width = snapshot
        .items
        .iter()
        .map(|item| display_width(&item.item.status))
        .max()
        .unwrap_or(0)
        .max(display_width("Status"));
    let result_width = snapshot
        .items
        .iter()
        .map(|item| display_width(item.result))
        .max()
        .unwrap_or(0)
        .max(display_width("Actions"));
    let fixed_width = reference_width + status_width + result_width + 6;
    let title_width = (area.width as usize)
        .saturating_sub(2)
        .saturating_sub(fixed_width);
    if title_width < 4 {
        frame.render_widget(
            Paragraph::new(format!(
                "Minimum width: {} columns. Terminal is too narrow.",
                fixed_width + 8
            ))
            .block(Block::bordered().title(format!(" Tickets · {} ", snapshot.source_id))),
            area,
        );
        return;
    }

    let rows = snapshot.items.iter().map(|item| {
        Row::new([
            Cell::from(item.item.reference_id.clone()),
            Cell::from(truncate_title(&item.item.title, title_width)),
            Cell::from(item.item.status.clone()),
            Cell::from(item.result).style(result_style(item.result)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(reference_width as u16),
            Constraint::Length(title_width as u16),
            Constraint::Length(status_width as u16),
            Constraint::Length(result_width as u16),
        ],
    )
    .header(
        Row::new(["Reference", "Title", "Status", "Actions"]).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::bordered().title(format!(" Tickets · {} ", snapshot.source_id)))
    .column_spacing(2)
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);
}

fn source_query(source: &WorkspaceSource) -> String {
    let config = &source.configured.source.config;
    let query = config
        .get("query")
        .or_else(|| config.get("jql"))
        .and_then(Value::as_str)
        .unwrap_or("no query");
    if let Some(collections) = config.get("collections").and_then(Value::as_array) {
        let names = collections
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return format!("{} · {}", query, names.join(", "));
        }
    }
    query.to_owned()
}

fn action_label(action: &ActionConfig) -> String {
    action.id.clone().unwrap_or_else(|| action.uses.clone())
}

fn snapshot_symbol(state: SnapshotState) -> &'static str {
    match state {
        SnapshotState::Missing => "✗",
        SnapshotState::Ready => "✓",
    }
}

fn source_status_style(state: SnapshotState) -> Style {
    Style::default().fg(match state {
        SnapshotState::Missing => Color::Red,
        SnapshotState::Ready => Color::Green,
    })
}

fn collection_label(status: Option<&SourceCollectionStatus>) -> &'static str {
    match status.map(|status| status.state) {
        Some(CollectionState::Collecting) => "collecting",
        Some(CollectionState::Complete) => "complete",
        Some(CollectionState::Failed) => "failed",
        Some(CollectionState::Cancelled) => "cancelled",
        None => "not run",
    }
}

fn collection_status_style(status: Option<&SourceCollectionStatus>) -> Style {
    Style::default().fg(match status.map(|status| status.state) {
        Some(CollectionState::Collecting) => Color::Yellow,
        Some(CollectionState::Complete) => Color::Green,
        Some(CollectionState::Failed) => Color::Red,
        Some(CollectionState::Cancelled) => Color::Yellow,
        None => Color::DarkGray,
    })
}

fn action_symbol(state: Option<ActionState>) -> &'static str {
    match state {
        Some(ActionState::Idle) => "·",
        Some(ActionState::Success) => "✓",
        Some(ActionState::Error) => "✗",
        Some(ActionState::Pending) => "⟳",
        None => "—",
    }
}

fn action_status_style(state: Option<ActionState>) -> Style {
    Style::default().fg(match state {
        Some(ActionState::Idle) => Color::DarkGray,
        Some(ActionState::Success) => Color::Green,
        Some(ActionState::Error) => Color::Red,
        Some(ActionState::Pending) => Color::Yellow,
        None => Color::DarkGray,
    })
}

fn result_style(result: &str) -> Style {
    let color = match result {
        "success" => Color::Green,
        "error" => Color::Red,
        "pending" => Color::Yellow,
        _ => Color::Reset,
    };
    Style::default().fg(color)
}

fn truncate_title(title: &str, width: usize) -> String {
    let title = title.replace(['\n', '\r'], " ");
    if display_width(&title) <= width {
        return title;
    }
    title
        .chars()
        .take(width.saturating_sub(1))
        .chain(['…'])
        .collect()
}

fn display_width(value: &str) -> usize {
    value.chars().count()
}

pub(super) fn view_signature(
    snapshots: &[SourceSnapshot],
    selected: usize,
    width: usize,
    watching: bool,
    footer: &str,
) -> String {
    let mut signature = format!("{selected}:{width}:{watching}:{footer};");
    for snapshot in snapshots {
        signature.push_str(&snapshot.source_id);
        signature.push(':');
        signature.push_str(match snapshot.state {
            SnapshotState::Missing => "missing",
            SnapshotState::Ready => "ready",
        });
        if let Some(collection) = &snapshot.collection {
            signature.push(':');
            signature.push_str(match collection.state {
                CollectionState::Collecting => "collecting",
                CollectionState::Complete => "complete",
                CollectionState::Failed => "failed",
                CollectionState::Cancelled => "cancelled",
            });
            signature.push(':');
            signature.push_str(&collection.updated_at);
            if let Some(error) = &collection.error {
                signature.push(':');
                signature.push_str(error);
            }
        }
        for action in &snapshot.actions {
            signature.push(':');
            signature.push_str(match action {
                ActionState::Idle => "idle",
                ActionState::Pending => "pending",
                ActionState::Success => "success",
                ActionState::Error => "error",
            });
        }
        for item in &snapshot.items {
            signature.push('|');
            signature.push_str(&item.item.reference_id);
            signature.push(':');
            signature.push_str(&item.item.title);
            signature.push(':');
            signature.push_str(&item.item.status);
            signature.push(':');
            signature.push_str(item.result);
        }
        signature.push(';');
    }
    signature
}
