use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::AppState;
use crate::ui::theme::*;

pub fn draw_profiles(frame: &mut Frame, state: &AppState) {
    let area = centered_rect(65, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 👤 Google Drive Accounts & Profiles ");

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),    // Profiles Table
            Constraint::Length(3), // Footer Actions
        ])
        .split(inner_area);

    let rows: Vec<Row> = state
        .available_profiles
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let active_mark = if p.is_active { "✔ (Active)" } else { "" };
            let _status_style = if p.is_authenticated {
                Style::default().fg(COLOR_SUCCESS)
            } else {
                Style::default().fg(COLOR_ERROR)
            };
            let status_text = if p.is_authenticated {
                "Valid"
            } else {
                "Expired"
            };
            let email_text = p.email.as_deref().unwrap_or("-");

            let is_selected = idx == state.profile_modal_selected_index;
            let row_style = if is_selected {
                style_selected()
            } else {
                Style::default().fg(COLOR_TEXT)
            };

            Row::new(vec![
                p.name.clone(),
                active_mark.to_string(),
                status_text.to_string(),
                email_text.to_string(),
            ])
            .style(row_style)
        })
        .collect();

    let header_row = Row::new(vec!["Profile", "Active", "Status", "Account Email"])
        .style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let widths = [
        Constraint::Percentage(25),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
        Constraint::Percentage(35),
    ];

    let table = Table::new(rows, widths)
        .header(header_row)
        .column_spacing(1);

    frame.render_widget(table, chunks[0]);

    let footer = Paragraph::new(
        "Enter: Switch to profile | a: Add new profile | d: Delete profile | Esc: Back",
    )
    .style(Style::default().fg(COLOR_MUTED))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
