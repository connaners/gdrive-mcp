use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::ConfirmAction;
use crate::ui::theme::*;

pub fn draw_confirm(frame: &mut Frame, action: &ConfirmAction) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(
            Style::default()
                .fg(COLOR_ERROR)
                .add_modifier(Modifier::BOLD),
        )
        .title(format!(" ⚠️ Confirmation: {} ", action.title));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(2)])
        .split(inner);

    let p = Paragraph::new(action.prompt.as_str())
        .style(Style::default().fg(COLOR_TEXT))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(p, chunks[0]);

    let footer = Paragraph::new("Press 'y' or Enter to Confirm  |  Press 'n' or Esc to Cancel")
        .style(
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
