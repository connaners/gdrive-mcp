use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{AppState, UpdateModalState};
use crate::ui::theme::*;
use gdrive_core::format_size;

pub fn draw_about_modal(frame: &mut Frame, state: &AppState) {
    let area = centered_rect(60, 50, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" ℹ️ Google Drive Account & Storage Quota ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(2)])
        .split(inner);

    let mut lines = Vec::new();

    if let Some(ref user) = state.user_info {
        lines.push(Line::from(vec![
            Span::styled("Account: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                user.display_name.as_deref().unwrap_or(""),
                Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" <{}>", user.email_address.as_deref().unwrap_or("")),
                Style::default().fg(COLOR_PRIMARY),
            ),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(ref quota) = state.storage_quota {
        let usage = quota
            .usage
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let limit = quota
            .limit
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let usage_drive = quota
            .usage_in_drive
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let usage_trash = quota
            .usage_in_drive_trash
            .as_deref()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        lines.push(Line::from(Span::styled(
            "Storage Usage Breakdown:",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("  • Total Used: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format_size(Some(&usage.to_string())),
                Style::default().fg(COLOR_TEXT),
            ),
            Span::styled(
                format!(
                    " of {}",
                    if limit > 0 {
                        format_size(Some(&limit.to_string()))
                    } else {
                        "Unlimited".to_string()
                    }
                ),
                Style::default().fg(COLOR_MUTED),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  • Files in Drive: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format_size(Some(&usage_drive.to_string())),
                Style::default().fg(COLOR_TEXT),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("  • Trash: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format_size(Some(&usage_trash.to_string())),
                Style::default().fg(COLOR_TEXT),
            ),
        ]));
        lines.push(Line::from(""));

        if limit > 0 {
            let pct = (usage as f64 / limit as f64 * 100.0).min(100.0) as u16;
            let ratio = (usage as f64 / limit as f64).min(1.0);
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::NONE))
                .gauge_style(Style::default().fg(if pct > 90 {
                    COLOR_ERROR
                } else {
                    COLOR_SUCCESS
                }))
                .ratio(ratio)
                .label(format!("{}% Used", pct));
            let gauge_area = Rect {
                x: chunks[0].x + 2,
                y: chunks[0].y + lines.len() as u16,
                width: chunks[0].width.saturating_sub(4),
                height: 1,
            };
            frame.render_widget(gauge, gauge_area);
        }
    } else {
        lines.push(Line::from("Storage information not available."));
    }

    let p = Paragraph::new(lines);
    frame.render_widget(p, chunks[0]);

    let footer = Paragraph::new("Press Esc or Enter to close.")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}

pub fn draw_update_modal(frame: &mut Frame, state: &UpdateModalState) {
    let area = centered_rect(65, 55, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 🚀 Software Self-Update ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(2)])
        .split(inner);

    let mut lines = Vec::new();

    if state.checking {
        lines.push(Line::from(Span::styled(
            "Checking for updates on GitHub...",
            Style::default().fg(COLOR_ACCENT),
        )));
    } else if state.updating {
        lines.push(Line::from(Span::styled(
            "Downloading and installing latest version...",
            Style::default().fg(COLOR_ACCENT),
        )));
    } else if let Some(ref success) = state.success {
        lines.push(Line::from(Span::styled(
            "🎉 Update Successful!",
            Style::default()
                .fg(COLOR_SUCCESS)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(format!(
            "Updated from {} to {}",
            success.current_version, success.updated_to
        )));
        lines.push(Line::from(format!(
            "Binary location: {}",
            success.binary_path
        )));
        lines.push(Line::from("Please restart gdrive to run the new version."));
    } else if let Some(ref err) = state.error {
        lines.push(Line::from(Span::styled(
            format!("⚠️ Update Error: {}", err),
            Style::default().fg(COLOR_ERROR),
        )));
    } else if let Some(ref info) = state.info {
        if info.update_available {
            lines.push(Line::from(vec![
                Span::styled(
                    "New version available: ",
                    Style::default().fg(COLOR_SUCCESS),
                ),
                Span::styled(
                    &info.latest_version,
                    Style::default()
                        .fg(COLOR_SUCCESS)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" (current: {})", info.current_version),
                    Style::default().fg(COLOR_MUTED),
                ),
            ]));
            lines.push(Line::from(""));
            if let Some(ref notes) = info.release_notes {
                lines.push(Line::from(Span::styled(
                    "Release Notes:",
                    Style::default().fg(COLOR_ACCENT),
                )));
                for note_line in notes.lines().take(8) {
                    lines.push(Line::from(format!("  {}", note_line)));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press 'u' to download and install this update now.",
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "You are using the latest version: ",
                    Style::default().fg(COLOR_SUCCESS),
                ),
                Span::styled(
                    &info.current_version,
                    Style::default()
                        .fg(COLOR_SUCCESS)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(p, chunks[0]);

    let footer = Paragraph::new("u: Install update | Esc: Close")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
