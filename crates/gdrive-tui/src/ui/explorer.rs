use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::Frame;

use crate::app::{AppState, DriveCategory};
use crate::ui::theme::*;
use gdrive_core::{format_date, format_mime, format_size};

pub fn draw_explorer(frame: &mut Frame, state: &mut AppState) {
    let size = frame.area();

    // Responsive guard: Minimum terminal size 80x24
    if size.width < 80 || size.height < 24 {
        let warning_p = Paragraph::new(format!(
            "⚠️ Terminal too small!\n\nMinimum required: 80 x 24\nCurrent size:     {} x {}\n\nPlease resize your terminal window.",
            size.width, size.height
        ))
        .style(Style::default().fg(COLOR_ERROR))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(style_border_active())
                .title(" Screen Size Notice "),
        );
        let center_box = centered_rect_exact(50, 10, size);
        frame.render_widget(warning_p, center_box);
        return;
    }

    let has_toast = state.status_toast.is_some();

    if has_toast {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header & Breadcrumbs
                Constraint::Length(3), // Drives Tabs
                Constraint::Length(1), // Top Notification Toast Banner
                Constraint::Min(10),   // Main Explorer (Dual Pane)
                Constraint::Length(1), // Bottom Bar / Keymaps
            ])
            .split(size);

        draw_header(frame, state, chunks[0]);
        draw_drives_tabs(frame, state, chunks[1]);
        draw_top_notification(frame, state, chunks[2]);
        draw_main_split(frame, state, chunks[3]);
        draw_bottom_bar(frame, state, chunks[4]);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header & Breadcrumbs
                Constraint::Length(3), // Drives Tabs
                Constraint::Min(10),   // Main Explorer (Dual Pane)
                Constraint::Length(1), // Bottom Bar / Keymaps
            ])
            .split(size);

        draw_header(frame, state, chunks[0]);
        draw_drives_tabs(frame, state, chunks[1]);
        draw_main_split(frame, state, chunks[2]);
        draw_bottom_bar(frame, state, chunks[3]);
    }
}

fn draw_top_notification(frame: &mut Frame, state: &AppState, area: Rect) {
    if let Some((ref msg, is_err, _)) = state.status_toast {
        let (icon, bg, fg) = if is_err {
            ("⚠️", COLOR_ERROR, Color::White)
        } else {
            ("✔", COLOR_SUCCESS, Color::Black)
        };
        let p = Paragraph::new(format!(" {} {} ", icon, msg))
            .style(Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
    }
}

fn draw_header(frame: &mut Frame, state: &AppState, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60), // App title & Breadcrumbs
            Constraint::Percentage(40), // Profile info & Storage meter
        ])
        .split(area);

    let breadcrumb_text = state
        .folder_path_stack
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<&str>>()
        .join(" > ");

    let left_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" Google Drive MCP ");

    let breadcrumbs_paragraph = Paragraph::new(Line::from(vec![
        Span::styled("📂 ", Style::default().fg(COLOR_ACCENT)),
        Span::styled(
            breadcrumb_text,
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(left_block);
    frame.render_widget(breadcrumbs_paragraph, header_chunks[0]);

    let mut profile_spans = vec![
        Span::styled("👤 [", Style::default().fg(COLOR_MUTED)),
        Span::styled(
            &state.current_profile,
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if let Some(ref user) = state.user_info {
        if let Some(ref email) = user.email_address {
            profile_spans.push(Span::styled(
                format!(": {}", email),
                Style::default().fg(COLOR_TEXT),
            ));
        }
    }
    profile_spans.push(Span::styled("] ", Style::default().fg(COLOR_MUTED)));

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
        if limit > 0 {
            let pct = (usage as f64 / limit as f64 * 100.0).min(100.0) as u16;
            profile_spans.push(Span::styled(
                format!(
                    "{}/{} ({}%)",
                    format_size(quota.usage.as_deref()),
                    format_size(quota.limit.as_deref()),
                    pct
                ),
                Style::default().fg(if pct > 90 { COLOR_ERROR } else { COLOR_SUCCESS }),
            ));
        } else {
            profile_spans.push(Span::styled(
                format!("Used: {}", format_size(quota.usage.as_deref())),
                Style::default().fg(COLOR_MUTED),
            ));
        }
    }

    let right_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_inactive());

    let profile_paragraph = Paragraph::new(Line::from(profile_spans))
        .alignment(Alignment::Right)
        .block(right_block);
    frame.render_widget(profile_paragraph, header_chunks[1]);
}

fn draw_drives_tabs(frame: &mut Frame, state: &AppState, area: Rect) {
    let categories = state.categories();
    let mut titles = Vec::new();

    for (i, cat) in categories.iter().enumerate() {
        let label = match cat {
            DriveCategory::MyDrive => format!("{}: My Drive", i),
            DriveCategory::SharedWithMe => format!("{}: Shared with Me", i),
            DriveCategory::Starred => format!("{}: Starred", i),
            DriveCategory::SharedDrive { name, .. } => format!("{}: 👥 {}", i, name),
        };
        titles.push(Line::from(label));
    }

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(style_border_inactive())
                .title(" Categories / Drives (Tab/0-9) "),
        )
        .select(state.selected_drive_tab)
        .style(Style::default().fg(COLOR_MUTED))
        .highlight_style(
            Style::default()
                .fg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED),
        );

    frame.render_widget(tabs, area);
}

fn draw_main_split(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let split_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // Left: File list
            Constraint::Percentage(55), // Right: Preview
        ])
        .split(area);

    draw_file_list(frame, state, split_chunks[0]);
    draw_file_preview(frame, state, split_chunks[1]);
}

fn draw_file_list(frame: &mut Frame, state: &mut AppState, area: Rect) {
    let visible_height = area.height.saturating_sub(4).max(1) as usize;
    state.update_table_scroll(visible_height);

    let visible = state.visible_items();
    let total_count = visible.len();

    let scroll = state.table_scroll;
    let end_idx = std::cmp::min(total_count, scroll + visible_height);

    let tag_count = state.tagged_files.len();
    let tag_str = if tag_count > 0 {
        format!(" | {} tagged", tag_count)
    } else {
        String::new()
    };

    let mut title = if let Some(ref desc) = state.search_query_desc {
        format!(
            " Results: {} ({} files{}) - Esc/x to clear ",
            desc, total_count, tag_str
        )
    } else {
        format!(
            " Items ({}-{} of {}{}| Sort: {}) ",
            if total_count > 0 { scroll + 1 } else { 0 },
            end_idx,
            total_count,
            tag_str,
            state.sort_mode.label()
        )
    };

    if !state.filter_query.is_empty() {
        title = format!(
            " Filter: \"{}\" ({}/{} matched) ",
            state.filter_query.as_str(),
            total_count,
            state.folder_items.len()
        );
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.filtering {
            Style::default().fg(COLOR_ACCENT)
        } else {
            style_border_active()
        })
        .title(title);

    if visible.is_empty() {
        let msg = if state.loading {
            let spinner = state.spinner_frame();
            format!("{} Loading Google Drive items...", spinner)
        } else if !state.filter_query.is_empty() {
            "No items match filter query. (Press Esc to clear)".to_string()
        } else {
            "This folder is empty. Press 'c' to create a file or 'u' to upload.".to_string()
        };
        let style = if state.loading {
            Style::default()
                .fg(state.pulsing_color())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_MUTED)
        };
        let empty_p = Paragraph::new(msg)
            .style(style)
            .alignment(Alignment::Center)
            .block(block);
        frame.render_widget(empty_p, area);
        return;
    }

    // Windowed rows slice
    let window_items = &visible[scroll..end_idx];

    let rows: Vec<Row> = window_items
        .iter()
        .enumerate()
        .map(|(offset, file)| {
            let actual_idx = scroll + offset;
            let (icon, _) = format_mime(&file.mime_type);
            let tag_box = if state.tagged_files.contains(&file.id) {
                "[✔] "
            } else {
                "    "
            };
            let name_cell = format!("{}{}{}", tag_box, icon, file.name);
            let size_cell = if file.mime_type == "application/vnd.google-apps.folder" {
                "-".to_string()
            } else {
                format_size(file.size.as_deref())
            };
            let date_cell = format_date(file.modified_time.as_deref());

            let is_selected = actual_idx == state.selected_index;
            let style = if is_selected {
                style_selected()
            } else {
                Style::default().fg(COLOR_TEXT)
            };

            Row::new(vec![name_cell, size_cell, date_cell]).style(style)
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
        .block(block)
        .column_spacing(1);

    frame.render_widget(table, area);
}

fn draw_file_preview(frame: &mut Frame, state: &AppState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_inactive())
        .title(" Preview / Details ");

    let selected_opt = state.current_selected_file();
    let file = match selected_opt {
        Some(f) => f,
        None => {
            let p = Paragraph::new("No file selected.")
                .style(Style::default().fg(COLOR_MUTED))
                .alignment(Alignment::Center)
                .block(block);
            frame.render_widget(p, area);
            return;
        }
    };

    let (icon, mime_desc) = format_mime(&file.mime_type);

    // If selected is a directory: Render Folder Info + Peek Level 1 contents!
    if file.mime_type == "application/vnd.google-apps.folder" {
        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(COLOR_ACCENT)),
                Span::styled(
                    &file.name,
                    Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Type: ", Style::default().fg(COLOR_MUTED)),
                Span::styled("Google Drive Folder", Style::default().fg(COLOR_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(&file.id, Style::default().fg(COLOR_TEXT)),
            ]),
            Line::from(vec![
                Span::styled("Modified: ", Style::default().fg(COLOR_MUTED)),
                Span::styled(
                    format_date(file.modified_time.as_deref()),
                    Style::default().fg(COLOR_TEXT),
                ),
            ]),
            Line::from(""),
        ];

        // Folder Peek Contents
        if let Some(children) = state.folder_peek_cache.get(&file.id) {
            lines.push(Line::from(Span::styled(
                format!("Contents (Level 1 Peek - {} items):", children.len()),
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )));
            if children.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  (This folder is empty)",
                    Style::default().fg(COLOR_MUTED),
                )));
            } else {
                for child in children.iter().take(12) {
                    let (c_icon, _) = format_mime(&child.mime_type);
                    let sz = if child.mime_type == "application/vnd.google-apps.folder" {
                        "".to_string()
                    } else {
                        format!(" ({})", format_size(child.size.as_deref()))
                    };
                    lines.push(Line::from(vec![
                        Span::raw(format!("  {} {} ", c_icon, child.name)),
                        Span::styled(sz, Style::default().fg(COLOR_MUTED)),
                    ]));
                }
                if children.len() > 12 {
                    lines.push(Line::from(Span::styled(
                        format!(
                            "  ... and {} more items (Enter to open)",
                            children.len() - 12
                        ),
                        Style::default().fg(COLOR_MUTED),
                    )));
                }
            }
        } else if state.folder_peek_loading.as_deref() == Some(&file.id) {
            let spinner = state.spinner_frame();
            lines.push(Line::from(Span::styled(
                format!("{} Peeking contents inside folder...", spinner),
                Style::default()
                    .fg(state.pulsing_color())
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Press 'Enter' to open this folder.",
                Style::default().fg(COLOR_SUCCESS),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Actions: Enter to open | Left/h/Backspace to go back | m: move | r: rename",
            Style::default().fg(COLOR_MUTED),
        )));

        let p = Paragraph::new(lines).block(block);
        frame.render_widget(p, area);
        return;
    }

    // If preview is loading:
    if state.preview_loading {
        let spinner = state.spinner_frame();
        let p = Paragraph::new(format!(
            "{} Loading preview for \"{}\"...",
            spinner, file.name
        ))
        .style(
            Style::default()
                .fg(state.pulsing_color())
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(block);
        frame.render_widget(p, area);
        return;
    }

    // If text preview content exists:
    if let Some(ref text) = state.preview_content {
        let scroll_lines = text
            .lines()
            .skip(state.preview_scroll as usize)
            .collect::<Vec<&str>>();
        let content_p = Paragraph::new(scroll_lines.join("\n"))
            .style(Style::default().fg(COLOR_TEXT))
            .block(block.title(format!(
                " Preview: {} (Scroll: {}) ",
                file.name, state.preview_scroll
            )))
            .wrap(Wrap { trim: false });
        frame.render_widget(content_p, area);
        return;
    }

    // Binary / Media / Non-text file card:
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(COLOR_ACCENT)),
            Span::styled(
                &file.name,
                Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Type: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(mime_desc, Style::default().fg(COLOR_PRIMARY)),
            Span::styled(
                format!(" ({})", file.mime_type),
                Style::default().fg(COLOR_MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled("Size: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format_size(file.size.as_deref()),
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("Modified: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                format_date(file.modified_time.as_deref()),
                Style::default().fg(COLOR_TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("ID: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(&file.id, Style::default().fg(COLOR_TEXT)),
        ]),
        Line::from(""),
    ];

    if let Some(ref link) = file.web_view_link {
        lines.push(Line::from(vec![
            Span::styled("Web Link: ", Style::default().fg(COLOR_MUTED)),
            Span::styled(
                link,
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(Span::styled(
        "Actions:",
        Style::default()
            .fg(COLOR_ACCENT)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "  • Press 'd' to download file to disk",
        Style::default().fg(COLOR_TEXT),
    )));
    lines.push(Line::from(Span::styled(
        "  • Press 'o' to open in web browser",
        Style::default().fg(COLOR_TEXT),
    )));
    lines.push(Line::from(Span::styled(
        "  • Press 'P' to view/edit permissions",
        Style::default().fg(COLOR_TEXT),
    )));
    lines.push(Line::from(Span::styled(
        "  • Press 'C' to view/add comments",
        Style::default().fg(COLOR_TEXT),
    )));
    lines.push(Line::from(Span::styled(
        "  • Press 'R' to view revision history",
        Style::default().fg(COLOR_TEXT),
    )));

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, area);
}

fn draw_bottom_bar(frame: &mut Frame, state: &AppState, area: Rect) {
    if state.filtering {
        let line = Line::from(vec![
            Span::styled(
                " Filter: ",
                Style::default()
                    .fg(Color::Black)
                    .bg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} ", state.filter_query.display_string()),
                Style::default().fg(COLOR_TEXT),
            ),
            Span::styled(
                "(Enter to apply, Esc to clear)",
                Style::default().fg(COLOR_MUTED),
            ),
        ]);
        let p = Paragraph::new(line);
        frame.render_widget(p, area);
        return;
    }

    let hints = vec![
        Span::styled(
            "Space: search ",
            Style::default()
                .fg(COLOR_ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("/: filter ", Style::default().fg(COLOR_PRIMARY)),
        Span::styled("x/Esc: clear ", Style::default().fg(COLOR_TEXT)),
        Span::styled("v: tag ", Style::default().fg(COLOR_TEXT)),
        Span::styled("s: sort ", Style::default().fg(COLOR_TEXT)),
        Span::styled("h/Left: back ", Style::default().fg(COLOR_TEXT)),
        Span::styled("~: root ", Style::default().fg(COLOR_TEXT)),
        Span::styled("c: create ", Style::default().fg(COLOR_TEXT)),
        Span::styled("u: upload ", Style::default().fg(COLOR_TEXT)),
        Span::styled("d: download ", Style::default().fg(COLOR_TEXT)),
        Span::styled("m: move ", Style::default().fg(COLOR_TEXT)),
        Span::styled("r: rename ", Style::default().fg(COLOR_TEXT)),
        Span::styled("t: trash ", Style::default().fg(COLOR_TEXT)),
        Span::styled("T: trash view ", Style::default().fg(COLOR_TEXT)),
        Span::styled("p: profiles ", Style::default().fg(COLOR_TEXT)),
        Span::styled("D: doctor ", Style::default().fg(COLOR_TEXT)),
        Span::styled("?: help ", Style::default().fg(COLOR_ACCENT)),
        Span::styled("q: quit", Style::default().fg(COLOR_MUTED)),
    ];

    let p = Paragraph::new(Line::from(hints));
    frame.render_widget(p, area);
}
