pub mod about_update;
pub mod comments;
pub mod confirm;
pub mod doctor;
pub mod explorer;
pub mod help;
pub mod operations;
pub mod permissions;
pub mod profiles;
pub mod revisions;
pub mod search;
pub mod theme;
pub mod trash;
pub mod wizard;

use ratatui::layout::Alignment;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{AppState, CurrentView};
use crate::ui::theme::*;

pub fn draw(frame: &mut Frame, state: &mut AppState) {
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
        let center_box = theme::centered_rect_exact(50, 10, size);
        frame.render_widget(warning_p, center_box);
        return;
    }

    // Always draw base explorer in the background
    explorer::draw_explorer(frame, state);

    // Overlay modal views if active
    match state.current_view {
        CurrentView::DriveExplorer => {}
        CurrentView::OnboardingWizard(ref wiz) => {
            wizard::draw_wizard(frame, wiz);
        }
        CurrentView::ProfileModal => {
            profiles::draw_profiles(frame, state);
        }
        CurrentView::SearchModal(ref s) => {
            search::draw_search_modal(frame, s);
        }
        CurrentView::FileOpModal(ref op) => {
            operations::draw_file_op(frame, op);
        }
        CurrentView::PermissionsModal(ref p) => {
            permissions::draw_permissions(frame, p);
        }
        CurrentView::CommentsModal(ref c) => {
            comments::draw_comments(frame, c);
        }
        CurrentView::RevisionsModal(ref r) => {
            revisions::draw_revisions(frame, r);
        }
        CurrentView::TrashView(ref t) => {
            trash::draw_trash(frame, t);
        }
        CurrentView::AboutModal => {
            about_update::draw_about_modal(frame, state);
        }
        CurrentView::SelfUpdateModal(ref u) => {
            about_update::draw_update_modal(frame, u);
        }
        CurrentView::ConfirmModal(ref c) => {
            confirm::draw_confirm(frame, c);
        }
        CurrentView::HelpModal => {
            help::draw_help(frame);
        }
        CurrentView::DoctorModal(ref d) => {
            doctor::draw_doctor(frame, d);
        }
    }
}
