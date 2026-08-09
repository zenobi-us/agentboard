use ratatui::{
    buffer::Buffer,
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use super::view::watch_button_area;

pub(super) struct Logo {
    watching: bool,
}

impl Logo {
    pub(super) fn new(watching: bool) -> Self {
        Self { watching }
    }
}

impl Widget for Logo {
    fn render(self, area: ratatui::layout::Rect, buffer: &mut Buffer) {
        Paragraph::new(Line::from(vec![
            Span::styled(
                " agentboard ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Dashboard"),
        ]))
        .block(Block::bordered())
        .render(area, buffer);

        let watch_style = Style::default()
            .fg(Color::Black)
            .bg(if self.watching {
                Color::Green
            } else {
                Color::DarkGray
            })
            .add_modifier(Modifier::BOLD);
        let button_area = watch_button_area(area);
        if button_area.width > 0 {
            Paragraph::new(if self.watching {
                "[ Watch: ON ]"
            } else {
                "[ Watch: OFF ]"
            })
            .alignment(Alignment::Center)
            .style(watch_style)
            .render(button_area, buffer);
        }
    }
}
