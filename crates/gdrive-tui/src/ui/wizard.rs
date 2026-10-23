use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::WizardState;
use crate::ui::theme::*;

pub fn draw_wizard(frame: &mut Frame, state: &WizardState) {
    let area = centered_rect(70, 75, frame.area());
    frame.render_widget(Clear, area);

    let title = match state.step {
        0 => " 🔑 Add Google Drive Profile / Account ",
        1 => " 🛠️ Configure Custom OAuth Credentials ",
        2 => " 🌐 Authorizing in Browser... ",
        _ => " ✅ Setup Complete ",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(title);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Description
            Constraint::Min(8),    // Form or Auth Status
            Constraint::Length(3), // Footer hints
        ])
        .split(inner_area);

    if state.step == 0 {
        let desc = Paragraph::new(
            "Connect an existing or new Google Drive profile to this workstation.\n\
            Enter profile name and press Enter to launch browser login.",
        )
        .style(Style::default().fg(COLOR_TEXT))
        .wrap(Wrap { trim: true });
        frame.render_widget(desc, chunks[0]);

        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Profile name
                Constraint::Length(4), // Status info box
            ])
            .split(chunks[1]);

        let prof_p = Paragraph::new(state.profile_name.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style_border_active())
                .title(" Profile Name (e.g. work, personal) "),
        );
        frame.render_widget(prof_p, form_chunks[0]);

        let info_lines = if state.has_existing_credentials {
            vec![
                Line::from(Span::styled(
                    "✔ Existing OAuth credentials found.",
                    Style::default()
                        .fg(COLOR_SUCCESS)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    "Press 'Enter' to open browser and authorize directly.",
                    Style::default().fg(COLOR_TEXT),
                )),
                Line::from(Span::styled(
                    "Press 'm' to configure custom Client ID / Secret manually.",
                    Style::default().fg(COLOR_MUTED),
                )),
            ]
        } else {
            vec![
                Line::from(Span::styled(
                    "⚠️ No existing credentials found.",
                    Style::default().fg(COLOR_ACCENT),
                )),
                Line::from(Span::styled(
                    "Press 'Enter' to enter Client ID & Secret or import credentials.json.",
                    Style::default().fg(COLOR_TEXT),
                )),
            ]
        };
        let info_p = Paragraph::new(info_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Credentials Status "),
        );
        frame.render_widget(info_p, form_chunks[1]);

        if let Some(ref err) = state.error_message {
            let err_p = Paragraph::new(format!("⚠️  {}", err)).style(
                Style::default()
                    .fg(COLOR_ERROR)
                    .add_modifier(Modifier::BOLD),
            );
            frame.render_widget(err_p, chunks[2]);
        } else {
            let hint_p = Paragraph::new("Enter: Continue | m: Manual Credentials | Esc: Cancel")
                .style(Style::default().fg(COLOR_MUTED))
                .alignment(Alignment::Center);
            frame.render_widget(hint_p, chunks[2]);
        }
    } else if state.step == 1 {
        // Manual credentials input form
        let desc = Paragraph::new(
            "Enter your Google OAuth credentials or path to downloaded credentials.json file.\n\
            Tab to switch fields, Enter to submit, Esc to return.",
        )
        .style(Style::default().fg(COLOR_TEXT))
        .wrap(Wrap { trim: true });
        frame.render_widget(desc, chunks[0]);

        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Client ID
                Constraint::Length(3), // Client Secret
                Constraint::Length(1), // Separator
                Constraint::Length(3), // File path
            ])
            .split(chunks[1]);

        let id_style = if state.active_field == 0 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let id_p = Paragraph::new(state.client_id_input.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(id_style)
                .title(" Client ID "),
        );
        frame.render_widget(id_p, form_chunks[0]);

        let secret_style = if state.active_field == 1 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let secret_p = Paragraph::new(state.client_secret_input.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(secret_style)
                .title(" Client Secret "),
        );
        frame.render_widget(secret_p, form_chunks[1]);

        let sep_p = Paragraph::new("--- OR import from credentials.json ---")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(sep_p, form_chunks[2]);

        let path_style = if state.active_field == 2 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let path_p = Paragraph::new(state.credentials_path_input.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(path_style)
                .title(" credentials.json path "),
        );
        frame.render_widget(path_p, form_chunks[3]);

        let footer = Paragraph::new("Tab: Next field | Enter: Start Login | Esc: Back")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
    } else if state.step == 2 {
        let auth_url_text = state
            .auth_url
            .as_deref()
            .unwrap_or("Generating authorization link...");

        let lines = vec![
            Line::from(Span::styled(
                "A local authorization listener has started.",
                Style::default().fg(COLOR_SUCCESS),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Please authorize in your web browser:",
                Style::default().fg(COLOR_TEXT),
            )),
            Line::from(""),
            Line::from(Span::styled(
                auth_url_text,
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::UNDERLINED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "👉 Press 'o' to re-open link in your default browser.",
                Style::default()
                    .fg(COLOR_ACCENT)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Awaiting OAuth callback on http://127.0.0.1:8085 ...",
                Style::default().fg(COLOR_MUTED),
            )),
        ];

        let p = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(p, chunks[1]);

        let footer = Paragraph::new("Press 'o' to open browser, or Esc/q to cancel.")
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);
    }
}
