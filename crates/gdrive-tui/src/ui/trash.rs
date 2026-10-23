use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::TrashViewState;
use crate::ui::theme::*;
use gdrive_core::{format_date, format_mime, format_size};

pub fn draw_trash(frame: &mut Frame, state: &TrashViewState) {
    let area = centered_rect(75, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(COLOR_ERROR))
        .title(format!(
            " 🗑️ Google Drive Trash Manager ({} items) ",
            state.items.len()
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(inner);

    if state.items.is_empty() {
        let msg = if state.loading {
            "Loading trashed items..."
        } else {
            "Trash is empty."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else {
        let rows: Vec<Row> = state
            .items
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                let (icon, _) = format_mime(&file.mime_type);
                let name = format!("{} {}", icon, file.name);
                let size = format_size(file.size.as_deref());
                let date = format_date(file.modified_time.as_deref());

                let is_selected = idx == state.selected_index;
                let style = if is_selected {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };

                Row::new(vec![name, size, date]).style(style)
            })
            .collect();

        let header_row = Row::new(vec!["Name", "Size", "Modified"])
            .style(
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);

        let widths = [
            Constraint::Percentage(55),
            Constraint::Percentage(20),
            Constraint::Percentage(25),
        ];

        let table = Table::new(rows, widths)
            .header(header_row)
            .column_spacing(1);

        frame.render_widget(table, chunks[0]);
    }

    let footer = Paragraph::new("r: Restore selected item | e: Empty trash | Esc: Back to Drive")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
