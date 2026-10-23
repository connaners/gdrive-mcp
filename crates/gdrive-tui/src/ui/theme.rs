use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

pub const COLOR_PRIMARY: Color = Color::Cyan;
pub const COLOR_SECONDARY: Color = Color::Blue;
pub const COLOR_ACCENT: Color = Color::Yellow;
pub const COLOR_WARNING: Color = Color::Yellow;
pub const COLOR_SUCCESS: Color = Color::Green;
pub const COLOR_ERROR: Color = Color::Red;
pub const COLOR_MUTED: Color = Color::DarkGray;
pub const COLOR_TEXT: Color = Color::White;

pub fn style_header() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(COLOR_PRIMARY)
        .add_modifier(Modifier::BOLD)
}

pub fn style_selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn style_border_active() -> Style {
    Style::default().fg(COLOR_PRIMARY)
}

pub fn style_border_inactive() -> Style {
    Style::default().fg(COLOR_MUTED)
}

/// Helper function to create a centered Rect with given percentage width and height
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Helper function to create a centered Rect with absolute size bounded by area
pub fn centered_rect_exact(width: u16, height: u16, r: Rect) -> Rect {
    let actual_width = width.min(r.width.saturating_sub(4));
    let actual_height = height.min(r.height.saturating_sub(2));

    let x = r.x + (r.width.saturating_sub(actual_width)) / 2;
    let y = r.y + (r.height.saturating_sub(actual_height)) / 2;

    Rect {
        x,
        y,
        width: actual_width,
        height: actual_height,
    }
}
