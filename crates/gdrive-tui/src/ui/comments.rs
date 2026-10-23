use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::CommentsState;
use crate::ui::theme::*;
use gdrive_core::format_date;

pub fn draw_comments(frame: &mut Frame, state: &CommentsState) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" 💬 Comments on \"{}\" ", state.file_name));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.adding {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(6), Constraint::Length(2)])
            .split(inner);

        let input_p = Paragraph::new(state.new_comment_text.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style_border_active())
                .title(" New Comment Content "),
        );
        frame.render_widget(input_p, chunks[0]);

        let hint = Paragraph::new("Enter: Post Comment | Esc: Cancel")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(hint, chunks[1]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(inner);

    if state.comments.is_empty() {
        let msg = if state.loading {
            "Loading comments..."
        } else {
            "No comments on this file. Press 'a' to add one."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else {
        let rows: Vec<Row> = state
            .comments
            .iter()
            .enumerate()
            .map(|(idx, comment)| {
                let author = comment
                    .author
                    .as_ref()
                    .and_then(|a| a.display_name.as_deref().or(a.email_address.as_deref()))
                    .unwrap_or("Unknown");

                let date = format_date(comment.created_time.as_deref());
                let status = if comment.resolved.unwrap_or(false) {
                    "Resolved"
                } else {
                    "Open"
                };

                let is_selected = idx == state.selected_index;
                let style = if is_selected {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };

                Row::new(vec![
                    author.to_string(),
                    date,
                    status.to_string(),
                    comment.content.clone(),
                ])
                .style(style)
            })
            .collect();

        let header_row = Row::new(vec!["Author", "Created", "Status", "Content"])
            .style(
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);

        let widths = [
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(45),
        ];

        let table = Table::new(rows, widths)
            .header(header_row)
            .column_spacing(1);

        frame.render_widget(table, chunks[0]);
    }

    let footer = Paragraph::new("a: Add comment | r: Resolve selected comment | Esc: Back")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
