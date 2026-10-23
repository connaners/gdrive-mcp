use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::SearchModalState;
use crate::ui::theme::*;

pub fn draw_search_modal(frame: &mut Frame, state: &SearchModalState) {
    let area = centered_rect(60, 45, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 🔍 Global Google Drive Search ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Query input
            Constraint::Length(3), // Checkbox full text
            Constraint::Length(3), // Sort order
            Constraint::Length(2), // Hints
        ])
        .split(inner_area);

    let q_style = if state.active_field == 0 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let q_block = Block::default()
        .borders(Borders::ALL)
        .border_style(q_style)
        .title(" Search Query ");
    let q_p = Paragraph::new(state.query.display_string()).block(q_block);
    frame.render_widget(q_p, chunks[0]);

    let cb_style = if state.active_field == 1 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let cb_mark = if state.full_text {
        "[✔] Search inside file contents (fullText)"
    } else {
        "[ ] Search inside file contents (fullText)"
    };
    let cb_block = Block::default()
        .borders(Borders::ALL)
        .border_style(cb_style)
        .title(" Content Search ");
    let cb_p = Paragraph::new(cb_mark)
        .style(if state.full_text {
            Style::default().fg(COLOR_SUCCESS)
        } else {
            Style::default().fg(COLOR_TEXT)
        })
        .block(cb_block);
    frame.render_widget(cb_p, chunks[1]);

    let order_style = if state.active_field == 2 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let orders = [
        "Relevance / Default",
        "Modified Time (Newest)",
        "Name (A-Z)",
        "Size / Quota Used",
    ];
    let current_order = orders.get(state.order_by_index).unwrap_or(&"Default");
    let order_block = Block::default()
        .borders(Borders::ALL)
        .border_style(order_style)
        .title(" Sort Order (Space to cycle) ");
    let order_p = Paragraph::new(format!("◀ {} ▶", current_order)).block(order_block);
    frame.render_widget(order_p, chunks[2]);

    let footer =
        Paragraph::new("Tab: Next field | Space: Toggle/Cycle | Enter: Search | Esc: Cancel")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}
