use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::ui::theme::*;

pub fn draw_help(frame: &mut Frame) {
    let area = centered_rect(65, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" ❓ Keyboard Shortcuts & Help ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(2)])
        .split(inner);

    let shortcuts = vec![
        ("j / Down", "Move cursor down"),
        ("k / Up", "Move cursor up"),
        ("g / Home", "Jump to first file in list"),
        ("G / End", "Jump to last file in list"),
        ("Enter", "Open selected folder / view file"),
        ("Backspace", "Navigate up to parent folder"),
        ("Tab / 0..9", "Switch Drive tab (My Drive / Shared Drives)"),
        ("/ ", "Filter items in current folder by name"),
        ("Ctrl+F", "Global Google Drive search with fullText"),
        ("Ctrl+D / PageDown", "Scroll preview downward"),
        ("Ctrl+U / PageUp", "Scroll preview upward"),
        ("c", "Create new file (Markdown or Google Doc)"),
        ("F", "Create new directory/folder"),
        ("r", "Rename selected file or folder"),
        ("m", "Move selected file or folder"),
        ("y", "Copy / duplicate selected file"),
        ("u", "Upload local file to current folder"),
        ("d", "Download selected file to local disk"),
        ("o", "Open selected item in web browser"),
        ("P", "Manage permissions / access control"),
        ("C", "View and add file comments"),
        ("R", "View revision history & download versions"),
        ("t", "Move selected item to trash (Safe Trash)"),
        ("T", "Open Trash Manager view (restore / empty)"),
        ("p", "Switch or manage accounts and profiles"),
        ("A", "View Google Drive storage quota breakdown"),
        ("S", "Check and execute software self-update"),
        ("D", "Run gdrive doctor system diagnostics"),
        ("?", "Toggle this help dialog"),
        ("q / Esc", "Close modal / Quit gdrive TUI"),
    ];

    let rows: Vec<Row> = shortcuts
        .into_iter()
        .map(|(key, desc)| {
            Row::new(vec![key.to_string(), desc.to_string()]).style(Style::default().fg(COLOR_TEXT))
        })
        .collect();

    let header_row = Row::new(vec!["Key", "Action"])
        .style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
        .bottom_margin(1);

    let widths = [Constraint::Percentage(30), Constraint::Percentage(70)];

    let table = Table::new(rows, widths)
        .header(header_row)
        .column_spacing(1);

    frame.render_widget(table, chunks[0]);

    let footer = Paragraph::new("Press Esc, Enter, or '?' to close this help dialog.")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
