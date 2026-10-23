use gdrive_core::DoctorStatus;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::DoctorModalState;
use crate::ui::theme::*;

pub fn draw_doctor(frame: &mut Frame, state: &DoctorModalState) {
    let area = centered_rect(80, 75, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(" 🩺 Google Drive MCP & CLI Doctor ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(inner);

    if state.loading {
        let p = Paragraph::new(
            "\n\n  ⏳ Sedang menjalankan pemeriksaan diagnostik lingkungan kerja...\n  (Memeriksa konfigurasi, kredensial, token, dan koneksi API Google Drive)",
        )
        .style(Style::default().fg(COLOR_ACCENT))
        .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else if let Some(report) = &state.report {
        let header = Row::new(vec!["Komponen", "Status", "Detail / Rekomendasi Solusi"])
            .style(Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD))
            .bottom_margin(1);

        let rows: Vec<Row> = report
            .checks
            .iter()
            .map(|check| {
                let (status_str, status_color) = match check.status {
                    DoctorStatus::Ok => ("[OK]", COLOR_SUCCESS),
                    DoctorStatus::Warn => ("[WARN]", COLOR_WARNING),
                    DoctorStatus::Fail => ("[FAIL]", COLOR_ERROR),
                };

                Row::new(vec![
                    ratatui::widgets::Cell::from(check.component.clone()),
                    ratatui::widgets::Cell::from(status_str).style(
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    ratatui::widgets::Cell::from(check.detail.clone()),
                ])
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(18),
                Constraint::Length(8),
                Constraint::Min(30),
            ],
        )
        .header(header)
        .column_spacing(1);

        frame.render_widget(table, chunks[0]);

        // Summary row
        let summary_text = format!(
            "Hasil Pemeriksaan: {} OK  |  {} Warning  |  {} Failure",
            report.ok_count, report.warn_count, report.fail_count
        );
        let summary_color = if report.fail_count > 0 {
            COLOR_ERROR
        } else if report.warn_count > 0 {
            COLOR_WARNING
        } else {
            COLOR_SUCCESS
        };

        let summary_p = Paragraph::new(summary_text)
            .style(
                Style::default()
                    .fg(summary_color)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);
        frame.render_widget(summary_p, chunks[1]);
    } else {
        let p = Paragraph::new("\n\n  Tekan 'r' untuk menjalankan pemeriksaan.")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    }

    // Footer actions
    let hint = Paragraph::new(" [r] Refresh / Rerun check  |  [Esc / q] Tutup ")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[2]);
}
