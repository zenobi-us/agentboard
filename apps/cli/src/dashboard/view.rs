use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::store::{SnapshotState, SourceSnapshot};

pub(super) fn render_view(frame: &mut Frame, snapshots: &[SourceSnapshot], selected: usize) {
    if snapshots.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " agentboard dashboard ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("\n\nWorkspace has no configured Sources.\n\nq quit"),
            ]))
            .block(Block::bordered()),
            frame.area(),
        );
        return;
    }

    let areas = dashboard_areas(frame.area());

    let snapshot = &snapshots[selected];
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " agentboard ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Dashboard"),
            Span::styled(
                format!(
                    "   Source {}/{}: {}",
                    selected + 1,
                    snapshots.len(),
                    snapshot.source_id
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .block(Block::bordered()),
        areas[0],
    );

    let tabs = snapshots
        .iter()
        .map(|snapshot| Line::from(snapshot.source_id.clone()))
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(tabs)
            .select(selected)
            .block(Block::default().borders(Borders::BOTTOM).title(" Sources "))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .divider("│"),
        areas[1],
    );

    match snapshot.state {
        SnapshotState::Missing => render_message(
            frame,
            areas[2],
            "This Source has no current Snapshot. Run the Workspace successfully.",
        ),
        SnapshotState::Ready if snapshot.items.is_empty() => render_message(
            frame,
            areas[2],
            "This Source has a valid current Snapshot with zero Items.",
        ),
        SnapshotState::Ready => render_table(frame, areas[2], snapshot),
    }

    frame.render_widget(
        Paragraph::new(" ←/h Previous   →/l Next   q/Ctrl-C Quit")
            .style(Style::default().fg(Color::DarkGray)),
        areas[3],
    );
}

pub(super) fn dashboard_areas(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area)
        .to_vec()
}

fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    frame.render_widget(
        Paragraph::new(message).block(Block::bordered().title(" Status ")),
        area,
    );
}

fn render_table(frame: &mut Frame, area: Rect, snapshot: &SourceSnapshot) {
    let reference_width = snapshot
        .items
        .iter()
        .map(|item| display_width(&item.item.reference_id))
        .max()
        .unwrap_or(0)
        .max(display_width("Reference ID"));
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
        .max(display_width("Action Plan Result"));
    let fixed_width = reference_width + status_width + result_width + 6;
    let title_width = (area.width as usize)
        .saturating_sub(2)
        .saturating_sub(fixed_width);
    if title_width < 4 {
        render_message(
            frame,
            area,
            &format!(
                "Minimum width: {} columns. Terminal is too narrow.",
                fixed_width + 8
            ),
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
        Row::new(["Reference ID", "Title", "Status", "Action Plan Result"]).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(Block::bordered().title(format!(" {} ", snapshot.source_id)))
    .column_spacing(2)
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);
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
) -> String {
    let mut signature = format!("{selected}:{width};");
    for snapshot in snapshots {
        signature.push_str(&snapshot.source_id);
        signature.push(':');
        signature.push_str(match snapshot.state {
            SnapshotState::Missing => "missing",
            SnapshotState::Ready => "ready",
        });
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
