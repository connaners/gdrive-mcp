use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Gauge, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::FileOpState;
use crate::input::InputBuffer;
use crate::ui::theme::*;
use gdrive_core::format_size;

pub fn draw_file_op(frame: &mut Frame, op: &FileOpState) {
    match op {
        FileOpState::Create {
            name,
            content,
            as_doc,
            active_field,
        } => draw_create_file(frame, name, content, *as_doc, *active_field),
        FileOpState::CreateFolder { name } => draw_create_folder(frame, name),
        FileOpState::Rename {
            old_name, new_name, ..
        } => draw_rename(frame, old_name, new_name),
        FileOpState::Move { .. } => draw_move(frame, op),
        FileOpState::Copy {
            old_name,
            new_name,
            target_folder_id,
            active_field,
            ..
        } => draw_copy(frame, old_name, new_name, target_folder_id, *active_field),
        FileOpState::Upload {
            local_path,
            dest_name,
            convert_doc,
            active_field,
        } => draw_upload(frame, local_path, dest_name, *convert_doc, *active_field),
        FileOpState::Download {
            file_name,
            dest_path,
            downloaded,
            total,
            in_progress,
            ..
        } => draw_download(
            frame,
            file_name,
            dest_path,
            *downloaded,
            *total,
            *in_progress,
        ),
        FileOpState::BatchDownload { .. } => draw_batch_download(frame, op),
    }
}

fn draw_create_file(
    frame: &mut Frame,
    name: &InputBuffer,
    content: &InputBuffer,
    as_doc: bool,
    active_field: usize,
) {
    let area = centered_rect(65, 60, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 📝 Create New File ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let name_style = if active_field == 0 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let name_p = Paragraph::new(name.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(name_style)
            .title(" File Name (e.g. notes.md) "),
    );
    frame.render_widget(name_p, chunks[0]);

    let content_style = if active_field == 1 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let content_p = Paragraph::new(content.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(content_style)
            .title(" Initial Content "),
    );
    frame.render_widget(content_p, chunks[1]);

    let doc_style = if active_field == 2 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let doc_mark = if as_doc {
        "[✔] Convert to native Google Docs"
    } else {
        "[ ] Save as plain file"
    };
    let doc_p = Paragraph::new(doc_mark)
        .style(if as_doc {
            Style::default().fg(COLOR_SUCCESS)
        } else {
            Style::default().fg(COLOR_TEXT)
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(doc_style)
                .title(" Format "),
        );
    frame.render_widget(doc_p, chunks[2]);

    let hint = Paragraph::new("Tab: Next field | Space: Toggle Docs | Enter: Create | Esc: Cancel")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[3]);
}

fn draw_create_folder(frame: &mut Frame, name: &InputBuffer) {
    let area = centered_rect(50, 30, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 📁 Create New Folder ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2)])
        .split(inner);

    let p = Paragraph::new(name.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_active())
            .title(" Folder Name "),
    );
    frame.render_widget(p, chunks[0]);

    let hint = Paragraph::new("Enter: Create Folder | Esc: Cancel")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[1]);
}

fn draw_rename(frame: &mut Frame, old_name: &str, new_name: &InputBuffer) {
    let area = centered_rect(55, 35, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" ✏️ Rename \"{}\" ", old_name));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2)])
        .split(inner);

    let p = Paragraph::new(new_name.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_active())
            .title(" New File Name (Left/Right to edit) "),
    );
    frame.render_widget(p, chunks[0]);

    let hint = Paragraph::new("Enter: Save Rename | Left/Right: Move cursor | Esc: Cancel")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[1]);
}

fn draw_move(frame: &mut Frame, op: &FileOpState) {
    let (
        items_to_move,
        current_folder_id,
        current_folder_name,
        path_stack,
        subfolders,
        search_query,
        search_results,
        selected_index,
        active_field,
        loading,
    ) = match op {
        FileOpState::Move {
            items_to_move,
            current_folder_id,
            current_folder_name,
            path_stack,
            subfolders,
            search_query,
            search_results,
            selected_index,
            active_field,
            loading,
        } => (
            items_to_move.as_slice(),
            current_folder_id.as_str(),
            current_folder_name.as_str(),
            path_stack.as_slice(),
            subfolders.as_slice(),
            search_query,
            search_results.as_ref(),
            *selected_index,
            *active_field,
            *loading,
        ),
        _ => return,
    };
    let area = centered_rect(72, 75, frame.area());
    frame.render_widget(Clear, area);

    let title_str = if items_to_move.len() > 1 {
        format!(" 🚚 Batch Move ({} items selected) ", items_to_move.len())
    } else if let Some((_, name)) = items_to_move.first() {
        format!(" 🚚 Move \"{}\" to Folder ", name)
    } else {
        " 🚚 Move to Folder ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(title_str);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Location breadcrumbs
            Constraint::Length(3), // Search box
            Constraint::Min(8),    // Folder list
            Constraint::Length(2), // Hints
        ])
        .split(inner);

    let breadcrumb_str = path_stack
        .iter()
        .map(|(n, _)| n.as_str())
        .collect::<Vec<&str>>()
        .join(" > ");

    let bc_p = Paragraph::new(format!("📍 Destination Location: {}", breadcrumb_str)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Current Location "),
    );
    frame.render_widget(bc_p, chunks[0]);

    let search_style = if active_field == 0 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let search_p = Paragraph::new(search_query.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(search_style)
            .title(" 🔍 Search All Folders in Drive (Type to filter, Tab to switch focus) "),
    );
    frame.render_widget(search_p, chunks[1]);

    if loading {
        let p = Paragraph::new("⏳ Loading folders...")
            .style(Style::default().fg(COLOR_ACCENT))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[2]);
    } else {
        let mut rows: Vec<Row> = Vec::new();

        if let Some(results) = search_results {
            for (idx, folder) in results.iter().enumerate() {
                let is_sel = idx == selected_index;
                let style = if is_sel {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };
                rows.push(
                    Row::new(vec![format!("📁 {}", folder.name), folder.id.clone()]).style(style),
                );
            }
        } else {
            let row0_style = if selected_index == 0 {
                style_selected()
            } else {
                Style::default()
                    .fg(COLOR_SUCCESS)
                    .add_modifier(Modifier::BOLD)
            };
            rows.push(
                Row::new(vec![
                    format!("📥 [MOVE HERE into '{}']", current_folder_name),
                    current_folder_id.to_string(),
                ])
                .style(row0_style),
            );

            let mut offset = 1;
            if path_stack.len() > 1 {
                let is_sel = offset == selected_index;
                let style = if is_sel {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_ACCENT)
                };
                rows.push(
                    Row::new(vec![
                        "📁 [.. Go to Parent Folder]".to_string(),
                        "..".to_string(),
                    ])
                    .style(style),
                );
                offset += 1;
            }

            if current_folder_id != "root" {
                let is_sel = offset == selected_index;
                let style = if is_sel {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_ACCENT)
                };
                rows.push(
                    Row::new(vec!["📁 [Root: My Drive]".to_string(), "root".to_string()])
                        .style(style),
                );
                offset += 1;
            }

            for (idx, folder) in subfolders.iter().enumerate() {
                let row_idx = offset + idx;
                let is_sel = row_idx == selected_index;
                let style = if is_sel {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };
                rows.push(
                    Row::new(vec![format!("📁 {} ▶", folder.name), folder.id.clone()]).style(style),
                );
            }
        }

        let list_style = if active_field == 1 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let table = Table::new(
            rows,
            [Constraint::Percentage(65), Constraint::Percentage(35)],
        )
        .header(
            Row::new(vec!["Folder Name", "Folder ID"])
                .style(
                    Style::default()
                        .fg(COLOR_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                )
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(list_style)
                .title(" Available Destinations "),
        )
        .column_spacing(1);

        frame.render_widget(table, chunks[2]);
    }

    let hint = Paragraph::new(
        "Tab: Toggle Search/List | Enter on [MOVE HERE]: Move | Enter on Folder: Open | Esc: Cancel",
    )
    .style(Style::default().fg(COLOR_MUTED))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[3]);
}

fn draw_copy(
    frame: &mut Frame,
    old_name: &str,
    new_name: &InputBuffer,
    target_folder_id: &InputBuffer,
    active_field: usize,
) {
    let area = centered_rect(60, 45, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" 📋 Copy \"{}\" ", old_name));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let name_style = if active_field == 0 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let name_p = Paragraph::new(new_name.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(name_style)
            .title(" Copy Name "),
    );
    frame.render_widget(name_p, chunks[0]);

    let target_style = if active_field == 1 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let target_p = Paragraph::new(target_folder_id.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(target_style)
            .title(" Destination Folder ID (Optional, blank = same folder) "),
    );
    frame.render_widget(target_p, chunks[1]);

    let hint = Paragraph::new("Tab: Next field | Left/Right: Edit | Enter: Copy | Esc: Cancel")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[2]);
}

fn draw_upload(
    frame: &mut Frame,
    local_path: &InputBuffer,
    dest_name: &InputBuffer,
    convert_doc: bool,
    active_field: usize,
) {
    let area = centered_rect(65, 55, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 📤 Upload Local File to Google Drive ");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let path_style = if active_field == 0 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let path_p = Paragraph::new(local_path.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(path_style)
            .title(" Local File Path (e.g. /path/to/document.pdf) "),
    );
    frame.render_widget(path_p, chunks[0]);

    let dest_style = if active_field == 1 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let dest_p = Paragraph::new(dest_name.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(dest_style)
            .title(" Remote File Name (Optional, blank = keep original name) "),
    );
    frame.render_widget(dest_p, chunks[1]);

    let doc_style = if active_field == 2 {
        style_border_active()
    } else {
        style_border_inactive()
    };
    let doc_mark = if convert_doc {
        "[✔] Convert to native Google Docs (for .docx, .md, .txt)"
    } else {
        "[ ] Upload as raw file"
    };
    let doc_p = Paragraph::new(doc_mark)
        .style(if convert_doc {
            Style::default().fg(COLOR_SUCCESS)
        } else {
            Style::default().fg(COLOR_TEXT)
        })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(doc_style)
                .title(" Conversion "),
        );
    frame.render_widget(doc_p, chunks[2]);

    let hint =
        Paragraph::new("Tab: Next field | Space: Toggle Docs | Enter: Start Upload | Esc: Cancel")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[3]);
}

fn draw_download(
    frame: &mut Frame,
    file_name: &str,
    dest_path: &InputBuffer,
    downloaded: u64,
    total: Option<u64>,
    in_progress: bool,
) {
    let area = centered_rect(65, 45, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" 📥 Download \"{}\" ", file_name));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let p = Paragraph::new(dest_path.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_active())
            .title(" Destination Path or Directory (Left/Right to edit) "),
    );
    frame.render_widget(p, chunks[0]);

    if in_progress {
        let ratio = if let Some(tot) = total {
            if tot > 0 {
                (downloaded as f64 / tot as f64).min(1.0)
            } else {
                0.0
            }
        } else {
            0.5
        };
        let pct = (ratio * 100.0) as u16;
        let label = if let Some(tot) = total {
            format!(
                "{}/{} ({}%)",
                format_size(Some(&downloaded.to_string())),
                format_size(Some(&tot.to_string())),
                pct
            )
        } else {
            format!("{} downloaded", format_size(Some(&downloaded.to_string())))
        };
        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Download Progress "),
            )
            .gauge_style(Style::default().fg(COLOR_SUCCESS))
            .ratio(ratio)
            .label(label);
        frame.render_widget(gauge, chunks[1]);
    } else {
        let p2 =
            Paragraph::new("Press Enter to begin downloading file with live streaming progress.")
                .style(Style::default().fg(COLOR_MUTED));
        frame.render_widget(p2, chunks[1]);
    }

    let hint = Paragraph::new(
        "Enter: Start Download | Left/Right: Edit | Tab: Autocomplete path | Esc: Cancel",
    )
    .style(Style::default().fg(COLOR_MUTED))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[2]);
}
fn draw_batch_download(frame: &mut Frame, op: &FileOpState) {
    let (
        files,
        dest_dir,
        as_zip,
        current_file_name,
        downloaded_in_file,
        total_in_file,
        completed_files,
        in_progress,
    ) = match op {
        FileOpState::BatchDownload {
            files,
            dest_dir,
            as_zip,
            current_file_name,
            downloaded_in_file,
            total_in_file,
            completed_files,
            in_progress,
            ..
        } => (
            files.as_slice(),
            dest_dir,
            *as_zip,
            current_file_name.as_str(),
            *downloaded_in_file,
            *total_in_file,
            *completed_files,
            *in_progress,
        ),
        _ => return,
    };
    let area = centered_rect(65, 55, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(
            " 📥 Batch Download ({} files selected) ",
            files.len()
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(inner);

    let dest_p = Paragraph::new(dest_dir.display_string()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(style_border_active())
            .title(" Destination Directory (Tab: autocomplete) "),
    );
    frame.render_widget(dest_p, chunks[0]);

    if in_progress {
        let total = files.len().max(1);
        let batch_ratio = if completed_files >= total {
            1.0
        } else {
            (completed_files as f64 / total as f64).min(1.0)
        };
        let pct = (batch_ratio * 100.0) as u16;
        let batch_label = if completed_files >= total {
            format!(
                "Batch Progress: {}/{} files (100% - Finished!)",
                completed_files, total
            )
        } else {
            format!(
                "Batch Progress: {}/{} files ({}%)",
                completed_files, total, pct
            )
        };
        let batch_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Total Batch Progress "),
            )
            .gauge_style(Style::default().fg(COLOR_SUCCESS))
            .ratio(batch_ratio)
            .label(batch_label);
        frame.render_widget(batch_gauge, chunks[1]);

        let file_ratio = if let Some(tot) = total_in_file {
            if tot > 0 {
                (downloaded_in_file as f64 / tot as f64).min(1.0)
            } else {
                0.0
            }
        } else {
            0.5
        };
        let file_label = if let Some(tot) = total_in_file {
            format!(
                "{}: {}/{}",
                current_file_name,
                format_size(Some(&downloaded_in_file.to_string())),
                format_size(Some(&tot.to_string()))
            )
        } else {
            format!(
                "{}: {}",
                current_file_name,
                format_size(Some(&downloaded_in_file.to_string()))
            )
        };
        let file_gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Current File Streaming "),
            )
            .gauge_style(Style::default().fg(COLOR_PRIMARY))
            .ratio(file_ratio)
            .label(file_label);
        frame.render_widget(file_gauge, chunks[2]);
    } else {
        let zip_mark = if as_zip {
            "[✔] Compress as ZIP archive (.zip) [Google Drive Standard]"
        } else {
            "[ ] Download as individual files into directory"
        };
        let p_zip = Paragraph::new(zip_mark)
            .style(if as_zip {
                Style::default()
                    .fg(COLOR_SUCCESS)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(COLOR_TEXT)
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Archive Format (Space to toggle) "),
            );
        frame.render_widget(p_zip, chunks[1]);

        let p_info = Paragraph::new(format!(
            "Ready to download {} files to the destination directory above.\nPress Enter to begin batch download.",
            files.len()
        ))
        .style(Style::default().fg(COLOR_TEXT));
        frame.render_widget(p_info, chunks[2]);
    }

    let hint = Paragraph::new(
        "Enter: Start Batch Download | Space: Toggle ZIP | Tab: Autocomplete dir | Esc: Cancel",
    )
    .style(Style::default().fg(COLOR_MUTED))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[3]);
}
