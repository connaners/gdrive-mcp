use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::PermissionsState;
use crate::ui::theme::*;

pub fn draw_permissions(frame: &mut Frame, state: &PermissionsState) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(style_border_active())
        .title(format!(" 👥 Permissions for \"{}\" ", state.file_name));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.adding {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Email/Target
                Constraint::Length(3), // Role
                Constraint::Length(3), // Type
                Constraint::Length(2), // Hints
            ])
            .split(inner);

        let email_style = if state.active_field == 0 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let email_p = Paragraph::new(state.add_email.display_string()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(email_style)
                .title(" Email / Grantee (or blank for 'anyone') "),
        );
        frame.render_widget(email_p, chunks[0]);

        let roles = ["reader", "commenter", "writer"];
        let current_role = roles.get(state.add_role_index).unwrap_or(&"reader");
        let role_style = if state.active_field == 1 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let role_p = Paragraph::new(format!("◀ {} ▶ (Space to change)", current_role)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(role_style)
                .title(" Role "),
        );
        frame.render_widget(role_p, chunks[1]);

        let types = ["user", "group", "domain", "anyone"];
        let current_type = types.get(state.add_type_index).unwrap_or(&"user");
        let type_style = if state.active_field == 2 {
            style_border_active()
        } else {
            style_border_inactive()
        };
        let type_p = Paragraph::new(format!("◀ {} ▶ (Space to change)", current_type)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(type_style)
                .title(" Type "),
        );
        frame.render_widget(type_p, chunks[2]);

        let hint = Paragraph::new(
            "Tab: Next field | Space: Cycle options | Enter: Add Permission | Esc: Cancel",
        )
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
        frame.render_widget(hint, chunks[3]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(3)])
        .split(inner);

    if state.permissions.is_empty() {
        let msg = if state.loading {
            "Loading permissions..."
        } else {
            "No permissions found."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(COLOR_MUTED))
            .alignment(Alignment::Center);
        frame.render_widget(p, chunks[0]);
    } else {
        let rows: Vec<Row> = state
            .permissions
            .iter()
            .enumerate()
            .map(|(idx, perm)| {
                let grantee = perm
                    .email_address
                    .as_deref()
                    .or(perm.display_name.as_deref())
                    .or(perm.domain.as_deref())
                    .unwrap_or(if perm.permission_type == "anyone" {
                        "Anyone with link"
                    } else {
                        "-"
                    });

                let is_selected = idx == state.selected_index;
                let style = if is_selected {
                    style_selected()
                } else {
                    Style::default().fg(COLOR_TEXT)
                };

                Row::new(vec![
                    perm.id.clone(),
                    perm.role.clone(),
                    perm.permission_type.clone(),
                    grantee.to_string(),
                ])
                .style(style)
            })
            .collect();

        let header_row = Row::new(vec!["ID", "Role", "Type", "Grantee / Email"])
            .style(
                Style::default()
                    .fg(COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);

        let widths = [
            Constraint::Percentage(25),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(45),
        ];

        let table = Table::new(rows, widths)
            .header(header_row)
            .column_spacing(1);

        frame.render_widget(table, chunks[0]);
    }

    let footer = Paragraph::new("a: Add permission | d: Delete selected permission | Esc: Back")
        .style(Style::default().fg(COLOR_MUTED))
        .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
