use gdrive_core::{
    DriveAboutStorageQuota, DriveAboutUser, DriveAuthor, DriveComment, DriveFile, DrivePermission,
    DriveRevision,
};
use gdrive_tui::app::*;
use gdrive_tui::input::InputBuffer;
use gdrive_tui::ui::draw;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn buffer_to_string(terminal: &Terminal<TestBackend>) -> String {
    let mut s = String::new();
    let area = terminal.backend().buffer().area;
    for y in 0..area.height {
        for x in 0..area.width {
            if let Some(cell) = terminal.backend().buffer().cell((x, y)) {
                s.push_str(cell.symbol());
            }
        }
        s.push('\n');
    }
    s
}

fn sample_state() -> AppState {
    let mut state = AppState::new("test-profile".to_string(), None);
    state.current_folder_id = "root".to_string();
    state.folder_path_stack = vec![("My Drive".to_string(), "root".to_string())];
    state.folder_items = vec![
        DriveFile {
            id: "folder_1".to_string(),
            name: "Documents".to_string(),
            mime_type: "application/vnd.google-apps.folder".to_string(),
            web_view_link: None,
            parents: Some(vec!["root".to_string()]),
            size: None,
            modified_time: Some("2026-10-24T10:00:00Z".to_string()),
        },
        DriveFile {
            id: "file_1".to_string(),
            name: "Quarterly Report.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            web_view_link: None,
            parents: Some(vec!["root".to_string()]),
            size: Some("1048576".to_string()),
            modified_time: Some("2026-10-24T11:00:00Z".to_string()),
        },
    ];
    state.user_info = Some(DriveAboutUser {
        display_name: Some("Test User".to_string()),
        email_address: Some("tester@example.com".to_string()),
        permission_id: None,
        photo_link: None,
    });
    state.storage_quota = Some(DriveAboutStorageQuota {
        limit: Some("15000000000".to_string()),
        usage: Some("5000000000".to_string()),
        usage_in_drive: Some("4500000000".to_string()),
        usage_in_drive_trash: Some("500000000".to_string()),
    });
    state
}

#[test]
fn test_render_explorer_view() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = sample_state();

    terminal.draw(|f| draw(f, &mut state)).unwrap();
    let output = buffer_to_string(&terminal);
    assert!(
        output.contains("Documents"),
        "Output must contain folder name 'Documents'"
    );
    assert!(
        output.contains("Quarterly Report"),
        "Output must contain file name"
    );
    assert!(
        output.contains("My Drive"),
        "Output must contain breadcrumb 'My Drive'"
    );
}

#[test]
fn test_render_wizard_views() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // Step 0: Welcome / Profile selection
    let mut state_step0 = sample_state();
    state_step0.current_view = CurrentView::OnboardingWizard(WizardState {
        step: 0,
        profile_name: InputBuffer::new("test-profile"),
        ..Default::default()
    });
    terminal.draw(|f| draw(f, &mut state_step0)).unwrap();
    let out0 = buffer_to_string(&terminal);
    assert!(
        out0.contains("Setup") || out0.contains("Wizard") || out0.contains("Google Drive"),
        "Wizard step 0 must render setup dialog"
    );

    // Step 1: Credentials input
    let mut state_step1 = sample_state();
    state_step1.current_view = CurrentView::OnboardingWizard(WizardState {
        step: 1,
        client_id_input: InputBuffer::new("mock_client_id_xyz"),
        client_secret_input: InputBuffer::new("mock_client_secret_abc"),
        ..Default::default()
    });
    terminal.draw(|f| draw(f, &mut state_step1)).unwrap();
    let out1 = buffer_to_string(&terminal);
    assert!(
        out1.contains("Client ID") || out1.contains("Credentials"),
        "Wizard step 1 must render credentials form"
    );

    // Step 2: Browser waiting loop
    let mut state_step2 = sample_state();
    state_step2.current_view = CurrentView::OnboardingWizard(WizardState {
        step: 2,
        auth_url: Some("https://accounts.google.com/o/oauth2/v2/auth".to_string()),
        loading: true,
        ..Default::default()
    });
    terminal.draw(|f| draw(f, &mut state_step2)).unwrap();
    let out2 = buffer_to_string(&terminal);
    assert!(
        out2.contains("Browser")
            || out2.contains("Menunggu")
            || out2.contains("Waiting")
            || out2.contains("Otorisasi"),
        "Wizard step 2 must render browser wait info"
    );
}

#[test]
fn test_render_all_modals() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    // 1. ProfileModal
    let mut state = sample_state();
    state.current_view = CurrentView::ProfileModal;
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Profil")
            || buffer_to_string(&terminal).contains("Profile")
    );

    // 2. SearchModal
    state.current_view = CurrentView::SearchModal(SearchModalState {
        query: InputBuffer::new("financial forecast"),
        full_text: true,
        ..Default::default()
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Search")
            || buffer_to_string(&terminal).contains("Cari")
    );

    // 3. FileOpModal (Create, Rename, Move, Copy)
    state.current_view = CurrentView::FileOpModal(FileOpState::Create {
        name: InputBuffer::new("notes.md"),
        content: InputBuffer::new("# Hello World"),
        as_doc: false,
        active_field: 0,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("notes.md"));

    state.current_view = CurrentView::FileOpModal(FileOpState::CreateFolder {
        name: InputBuffer::new("Archive"),
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("Archive"));

    state.current_view = CurrentView::FileOpModal(FileOpState::Rename {
        file_id: "file_1".to_string(),
        old_name: "Quarterly Report.pdf".to_string(),
        new_name: InputBuffer::new("Annual Report.pdf"),
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("Annual Report.pdf"));

    state.current_view = CurrentView::FileOpModal(FileOpState::Move {
        items_to_move: vec![("file_1".to_string(), "Quarterly Report.pdf".to_string())],
        current_folder_id: "root".to_string(),
        current_folder_name: "My Drive".to_string(),
        path_stack: vec![("My Drive".to_string(), "root".to_string())],
        subfolders: vec![DriveFile {
            id: "folder_1".to_string(),
            name: "Projects".to_string(),
            mime_type: "application/vnd.google-apps.folder".to_string(),
            web_view_link: None,
            parents: None,
            size: None,
            modified_time: None,
        }],
        search_query: InputBuffer::default(),
        search_results: None,
        selected_index: 0,
        active_field: 1,
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("Quarterly Report.pdf"));

    state.current_view = CurrentView::FileOpModal(FileOpState::Copy {
        file_id: "file_1".to_string(),
        old_name: "Quarterly Report.pdf".to_string(),
        new_name: InputBuffer::new("Quarterly Report Copy.pdf"),
        target_folder_id: InputBuffer::default(),
        active_field: 0,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(buffer_to_string(&terminal).contains("Quarterly Report Copy.pdf"));

    // 4. PermissionsModal
    state.current_view = CurrentView::PermissionsModal(PermissionsState {
        file_id: "file_1".to_string(),
        file_name: "Quarterly Report.pdf".to_string(),
        permissions: vec![DrivePermission {
            id: "p1".to_string(),
            role: "owner".to_string(),
            permission_type: "user".to_string(),
            email_address: Some("owner@example.com".to_string()),
            display_name: Some("Owner".to_string()),
            domain: None,
        }],
        selected_index: 0,
        adding: false,
        add_role_index: 0,
        add_type_index: 0,
        add_email: InputBuffer::default(),
        active_field: 0,
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Permission")
            || buffer_to_string(&terminal).contains("Hak Akses")
    );

    // 5. CommentsModal
    state.current_view = CurrentView::CommentsModal(CommentsState {
        file_id: "file_1".to_string(),
        file_name: "Quarterly Report.pdf".to_string(),
        comments: vec![DriveComment {
            id: "c1".to_string(),
            content: "Approved for publication.".to_string(),
            author: Some(DriveAuthor {
                display_name: Some("Manager".to_string()),
                email_address: Some("manager@example.com".to_string()),
                me: None,
            }),
            created_time: Some("2026-10-24T12:00:00Z".to_string()),
            modified_time: None,
            resolved: Some(false),
        }],
        selected_index: 0,
        adding: false,
        new_comment_text: InputBuffer::default(),
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Comment")
            || buffer_to_string(&terminal).contains("Komentar")
    );

    // 6. RevisionsModal
    state.current_view = CurrentView::RevisionsModal(RevisionsState {
        file_id: "file_1".to_string(),
        file_name: "Quarterly Report.pdf".to_string(),
        revisions: vec![DriveRevision {
            id: "r1".to_string(),
            mime_type: Some("application/pdf".to_string()),
            modified_time: Some("2026-10-24T09:00:00Z".to_string()),
            keep_forever: Some(false),
            size: Some("1048576".to_string()),
            last_modifying_user: None,
        }],
        selected_index: 0,
        downloading: false,
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Revision")
            || buffer_to_string(&terminal).contains("Revisi")
    );

    // 7. TrashView
    state.current_view = CurrentView::TrashView(TrashViewState {
        items: vec![DriveFile {
            id: "trash_1".to_string(),
            name: "Old Draft.txt".to_string(),
            mime_type: "text/plain".to_string(),
            web_view_link: None,
            parents: None,
            size: Some("120".to_string()),
            modified_time: None,
        }],
        selected_index: 0,
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Trash")
            || buffer_to_string(&terminal).contains("Sampah")
    );

    // 8. AboutModal
    state.current_view = CurrentView::AboutModal;
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Storage")
            || buffer_to_string(&terminal).contains("Penyimpanan")
            || buffer_to_string(&terminal).contains("Akun")
    );

    // 9. SelfUpdateModal
    state.current_view = CurrentView::SelfUpdateModal(UpdateModalState::default());
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Update")
            || buffer_to_string(&terminal).contains("Versi")
    );

    // 10. HelpModal
    state.current_view = CurrentView::HelpModal;
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Bantuan")
            || buffer_to_string(&terminal).contains("Help")
            || buffer_to_string(&terminal).contains("Keybindings")
    );

    // 11. ConfirmModal
    state.current_view = CurrentView::ConfirmModal(ConfirmAction {
        title: "Konfirmasi Penghapusan".to_string(),
        prompt: "Apakah Anda yakin ingin memindahkan file ke trash?".to_string(),
        action_type: ConfirmType::TrashFile {
            id: "file_1".to_string(),
            name: "Quarterly Report.pdf".to_string(),
        },
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Konfirmasi")
            || buffer_to_string(&terminal).contains("trash")
    );

    // 12. DoctorModal
    state.current_view = CurrentView::DoctorModal(DoctorModalState {
        report: Some(gdrive_core::DoctorReport::new(vec![
            gdrive_core::DoctorCheckItem {
                component: "Config Directory".to_string(),
                status: gdrive_core::DoctorStatus::Ok,
                detail: "Dapat ditulisi".to_string(),
            },
        ])),
        loading: false,
    });
    terminal.draw(|f| draw(f, &mut state)).unwrap();
    assert!(
        buffer_to_string(&terminal).contains("Doctor")
            || buffer_to_string(&terminal).contains("Pemeriksaan")
    );
}

#[test]
fn test_render_small_screen_guard() {
    let backend = TestBackend::new(60, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = sample_state();

    terminal.draw(|f| draw(f, &mut state)).unwrap();
    let output = buffer_to_string(&terminal);

    assert!(
        output.contains("Screen Size Notice"),
        "Should render screen size notice block"
    );
    assert!(
        output.contains("Terminal too small!"),
        "Should render warning text"
    );
}

#[test]
fn test_folder_navigation_stack() {
    let mut state = sample_state();
    assert_eq!(state.current_folder_id, "root");
    assert_eq!(state.folder_path_stack.len(), 1);

    // Selected item is Documents folder (index 0)
    state.selected_index = 0;
    let target = state.enter_selected_folder();
    assert_eq!(target, Some("folder_1".to_string()));
    assert_eq!(state.current_folder_id, "folder_1");
    assert_eq!(state.folder_path_stack.len(), 2);

    // Navigate back up
    let parent = state.navigate_up();
    assert_eq!(parent, Some("root".to_string()));
    assert_eq!(state.current_folder_id, "root");
    assert_eq!(state.folder_path_stack.len(), 1);
}

#[test]
fn test_selection_bounds() {
    let mut state = sample_state();
    assert_eq!(state.selected_index, 0);

    state.select_next();
    assert_eq!(state.selected_index, 1);

    // Reaching the end wraps around to 0
    state.select_next();
    assert_eq!(state.selected_index, 0);

    // Reaching before first item wraps around to last item
    state.select_prev();
    assert_eq!(state.selected_index, 1);
}

#[test]
fn test_live_filter_matches() {
    let mut state = sample_state();
    assert_eq!(state.visible_items().len(), 2);

    // Filter by "rep" (matches "Quarterly Report.pdf")
    state.filtering = true;
    state.filter_query = InputBuffer::new("rep");
    assert_eq!(state.visible_items().len(), 1);
    assert_eq!(state.visible_items()[0].name, "Quarterly Report.pdf");

    // Filter by non-existent text
    state.filter_query = InputBuffer::new("nonexistent");
    assert_eq!(state.visible_items().len(), 0);
}
