use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::RevisionsState;
use crate::ui::theme::*;
use gdrive_core::{format_date, format_size};

pub fn draw_revisions(frame: &mut Frame, state: &RevisionsState) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" 📜 Revision History for \"{}\" ", state.file_name));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(inner);

    if state.revisions.is_empty() {
        let msg = if state.loading {
            "Loading revision history..."
        } else {
            "No historical revisions found."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else {
        let rows: Vec<Row> = state
            .revisions
            .iter()
            .enumerate()
            .map(|(idx, rev)| {
                let user = rev
                    .last_modifying_user
                    .as_ref()
                    .and_then(|u| u.display_name.as_deref().or(u.email_address.as_deref()))
                    .unwrap_or("-");

                let date = format_date(rev.modified_time.as_deref());
                let size = format_size(rev.size.as_deref());

                let is_selected = idx == state.selected_index;
                let style = if is_selected {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };

                Row::new(vec![rev.id.clone(), date, user.to_string(), size]).style(style)
            })
            .collect();

        let header_row = Row::new(vec!["Revision ID", "Modified Time", "Modifier", "Size"])
            .style(
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);

        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ];

        let table = Table::new(rows, widths)
            .header(header_row)
            .column_spacing(1);

        frame.render_widget(table, chunks[0]);
    }

    let footer = Paragraph::new("d: Download selected revision | Esc: Back")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
