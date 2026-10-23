use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::app::*;
use crate::input::InputBuffer;
use gdrive_core::{
    check_for_update, execute_self_update, AuthManager, CreateFileOptions, CreatePermissionOptions,
    GDriveClient, SearchFileOptions,
};

pub async fn run_tui(
    profile: Option<String>,
    initial_folder: Option<String>,
    demo: bool,
) -> Result<()> {
    let is_demo = demo
        || std::env::var("GDRIVE_TUI_DEMO")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
    let active_profile = profile.unwrap_or_else(|| {
        if is_demo {
            "demo".to_string()
        } else {
            AuthManager::get_active_profile()
        }
    });

    // Terminal raw mode & alternate screen setup
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
            crossterm::cursor::Show
        );
        prev_hook(panic_info);
    }));

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let res = run_app(&mut terminal, active_profile, initial_folder, is_demo).await;

    // Teardown
    let _ = terminal.show_cursor();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    );
    let _ = crossterm::terminal::disable_raw_mode();

    res
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    profile: String,
    initial_folder: Option<String>,
    is_demo: bool,
) -> Result<()> {
    let mut state = AppState::new(profile.clone(), initial_folder);
    let (event_tx, mut event_rx) = unbounded_channel::<AppEvent>();

    if is_demo {
        state.loading = false;
        state.folder_path_stack = vec![("My Drive (Demo)".to_string(), "root".to_string())];
        state.folder_items = vec![
            gdrive_core::DriveFile {
                id: "demo_folder_1".to_string(),
                name: "Projects".to_string(),
                mime_type: "application/vnd.google-apps.folder".to_string(),
                web_view_link: None,
                parents: Some(vec!["root".to_string()]),
                size: None,
                modified_time: Some("2026-10-24T08:00:00Z".to_string()),
            },
            gdrive_core::DriveFile {
                id: "demo_folder_2".to_string(),
                name: "Invoices".to_string(),
                mime_type: "application/vnd.google-apps.folder".to_string(),
                web_view_link: None,
                parents: Some(vec!["root".to_string()]),
                size: None,
                modified_time: Some("2026-10-23T14:30:00Z".to_string()),
            },
            gdrive_core::DriveFile {
                id: "demo_file_1".to_string(),
                name: "System Architecture.md".to_string(),
                mime_type: "text/markdown".to_string(),
                web_view_link: None,
                parents: Some(vec!["root".to_string()]),
                size: Some("12450".to_string()),
                modified_time: Some("2026-10-24T09:15:00Z".to_string()),
            },
            gdrive_core::DriveFile {
                id: "demo_file_2".to_string(),
                name: "Q4 Financial Plan.xlsx".to_string(),
                mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                    .to_string(),
                web_view_link: None,
                parents: Some(vec!["root".to_string()]),
                size: Some("542000".to_string()),
                modified_time: Some("2026-10-22T16:45:00Z".to_string()),
            },
            gdrive_core::DriveFile {
                id: "demo_file_3".to_string(),
                name: "Meeting Notes.gdoc".to_string(),
                mime_type: "application/vnd.google-apps.document".to_string(),
                web_view_link: None,
                parents: Some(vec!["root".to_string()]),
                size: Some("8500".to_string()),
                modified_time: Some("2026-10-24T11:00:00Z".to_string()),
            },
        ];
        state.user_info = Some(gdrive_core::DriveAboutUser {
            display_name: Some("demo-user".to_string()),
            email_address: Some("demo-user@workplace.com".to_string()),
            permission_id: None,
            photo_link: None,
        });
        state.storage_quota = Some(gdrive_core::DriveAboutStorageQuota {
            limit: Some("15000000000".to_string()),
            usage: Some("7500000000".to_string()),
            usage_in_drive: Some("7000000000".to_string()),
            usage_in_drive_trash: Some("500000000".to_string()),
        });
        state.available_profiles = vec![gdrive_core::ProfileStatusInfo {
            name: "demo".to_string(),
            is_active: true,
            is_authenticated: true,
            email: Some("demo-user@workplace.com".to_string()),
            display_name: Some("demo-user".to_string()),
            expires_in_seconds: Some(3600),
            token_path: std::path::PathBuf::from("/mock/demo/token.json"),
        }];
        state.set_toast(
            "Running in interactive Demo Mode with mock data.".to_string(),
            false,
        );
    } else {
        match GDriveClient::from_profile(Some(&profile)).await {
            Ok(client) => {
                state.client = Some(client.clone());
                state.loading = true;

                spawn_load_folder(
                    client.clone(),
                    state.current_folder_id.clone(),
                    event_tx.clone(),
                );
                spawn_load_drives(client.clone(), event_tx.clone());
                spawn_load_about(client.clone(), event_tx.clone());
                spawn_load_profiles(event_tx.clone());
            }
            Err(_) => {
                let has_creds = AuthManager::from_env_or_config().is_ok();
                state.current_view = CurrentView::OnboardingWizard(WizardState {
                    step: 0,
                    profile_name: InputBuffer::new(&profile),
                    client_id_input: InputBuffer::default(),
                    client_secret_input: InputBuffer::default(),
                    credentials_path_input: InputBuffer::default(),
                    has_existing_credentials: has_creds,
                    auth_url: None,
                    active_field: 0,
                    error_message: None,
                    loading: false,
                });
                state.set_toast(
                    "Authentication required. Press Enter to launch browser login.".to_string(),
                    false,
                );
            }
        }
    }

    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(std::time::Duration::from_millis(250));

    loop {
        terminal.draw(|frame| crate::ui::draw(frame, &mut state))?;

        if state.should_exit {
            break;
        }

        tokio::select! {
            _ = tick_interval.tick() => {
                state.on_tick();
            }
            maybe_event = event_stream.next() => {
                if let Some(Ok(Event::Key(key))) = maybe_event {
                    if key.kind == KeyEventKind::Press {
                        handle_key_event(&mut state, key, &event_tx).await;
                    }
                }
            }
            Some(app_event) = event_rx.recv() => {
                handle_app_event(&mut state, app_event, &event_tx).await;
            }
        }
    }

    Ok(())
}

async fn handle_key_event(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        state.should_exit = true;
        return;
    }

    if matches!(state.current_view, CurrentView::DriveExplorer) {
        handle_explorer_keys(state, key, tx).await;
    } else {
        handle_modal_key_event(state, key, tx).await;
    }
}

async fn handle_modal_key_event(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    match state.current_view {
        CurrentView::DriveExplorer => {}
        CurrentView::OnboardingWizard(_) => handle_wizard_keys(state, key, tx).await,
        CurrentView::ProfileModal => handle_profile_modal_keys(state, key, tx).await,
        CurrentView::SearchModal(_) => handle_search_modal_keys(state, key, tx).await,
        CurrentView::FileOpModal(_) => handle_file_op_keys(state, key, tx).await,
        CurrentView::PermissionsModal(_) => handle_permissions_keys(state, key, tx).await,
        CurrentView::CommentsModal(_) => handle_comments_keys(state, key, tx).await,
        CurrentView::RevisionsModal(_) => handle_revisions_keys(state, key, tx).await,
        CurrentView::TrashView(_) => handle_trash_view_keys(state, key, tx).await,
        CurrentView::AboutModal => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                state.current_view = CurrentView::DriveExplorer;
            }
        }
        CurrentView::SelfUpdateModal(_) => handle_update_modal_keys(state, key, tx).await,
        CurrentView::ConfirmModal(_) => handle_confirm_keys(state, key, tx).await,
        CurrentView::HelpModal => {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                state.current_view = CurrentView::DriveExplorer;
            }
        }
        CurrentView::DoctorModal(_) => {
            if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                state.current_view = CurrentView::DriveExplorer;
            } else if matches!(key.code, KeyCode::Char('r')) {
                if let CurrentView::DoctorModal(d) = &mut state.current_view {
                    d.loading = true;
                }
                let tx_c = tx.clone();
                let profile = state.current_profile.clone();
                tokio::spawn(async move {
                    let term_size = crossterm::terminal::size().ok();
                    let report =
                        gdrive_core::run_doctor_checks(Some(&profile), Some((true, term_size)))
                            .await;
                    let _ = tx_c.send(AppEvent::DoctorReportLoaded(report));
                });
            }
        }
    }
}

async fn handle_explorer_keys(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    if state.filtering {
        handle_filtering_keys(state, key);
        return;
    }

    if handle_explorer_ctrl_keys(state, key) {
        return;
    }

    if handle_explorer_nav_keys(state, key.code, tx) {
        return;
    }

    handle_explorer_action_keys(state, key.code, tx);
}

fn handle_filtering_keys(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => state.filtering = false,
        KeyCode::Esc => {
            state.filtering = false;
            state.filter_query.clear();
            state.selected_index = 0;
            state.table_scroll = 0;
        }
        _ => {
            if state.filter_query.handle_key(key) {
                state.selected_index = 0;
                state.table_scroll = 0;
            }
        }
    }
}

fn handle_explorer_ctrl_keys(state: &mut AppState, key: KeyEvent) -> bool {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('f') => {
                state.current_view = CurrentView::SearchModal(SearchModalState::default());
                return true;
            }
            KeyCode::Char('d') => {
                state.scroll_preview_down(10);
                return true;
            }
            KeyCode::Char('u') => {
                state.scroll_preview_up(10);
                return true;
            }
            _ => {}
        }
    }
    false
}

fn handle_explorer_nav_keys(
    state: &mut AppState,
    code: KeyCode,
    tx: &UnboundedSender<AppEvent>,
) -> bool {
    match code {
        KeyCode::Down | KeyCode::Char('j') => {
            state.select_next();
            trigger_preview_load_if_needed(state, tx);
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.select_prev();
            trigger_preview_load_if_needed(state, tx);
            true
        }
        KeyCode::Home | KeyCode::Char('g') => {
            state.select_first();
            trigger_preview_load_if_needed(state, tx);
            true
        }
        KeyCode::End | KeyCode::Char('G') => {
            state.select_last();
            trigger_preview_load_if_needed(state, tx);
            true
        }
        KeyCode::PageDown => {
            state.scroll_preview_down(15);
            true
        }
        KeyCode::PageUp => {
            state.scroll_preview_up(15);
            true
        }
        KeyCode::Enter => {
            handle_explorer_enter(state, tx);
            true
        }
        // Universal Back Navigation: Backspace, Left arrow, 'h', or 'b'
        KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') | KeyCode::Char('b') => {
            handle_explorer_backspace(state, tx);
            true
        }
        KeyCode::Char('~') | KeyCode::Char('H') => {
            state.reset_to_folder("My Drive".to_string(), "root".to_string());
            state.selected_drive_tab = 0;
            state.set_toast("Returned to My Drive (Root)".to_string(), false);
            if let Some(ref client) = state.client {
                spawn_load_folder(client.clone(), "root".to_string(), tx.clone());
            }
            true
        }
        KeyCode::Tab => {
            let total_tabs = state.categories().len();
            state.selected_drive_tab = (state.selected_drive_tab + 1) % total_tabs;
            switch_drive_tab(state, tx);
            true
        }
        KeyCode::BackTab => {
            let total_tabs = state.categories().len();
            state.selected_drive_tab = (state.selected_drive_tab + total_tabs - 1) % total_tabs;
            switch_drive_tab(state, tx);
            true
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let digit = c.to_digit(10).unwrap() as usize;
            let total_tabs = state.categories().len();
            if digit < total_tabs {
                state.selected_drive_tab = digit;
                switch_drive_tab(state, tx);
            }
            true
        }
        KeyCode::Char('/') => {
            state.filtering = true;
            true
        }
        KeyCode::Char(' ') => {
            state.current_view = CurrentView::SearchModal(SearchModalState::default());
            true
        }
        KeyCode::Esc => {
            if state.search_results.is_some() {
                state.search_results = None;
                state.search_query_desc = None;
                state.selected_index = 0;
                state.table_scroll = 0;
                trigger_preview_load_if_needed(state, tx);
            }
            true
        }
        _ => false,
    }
}

fn handle_explorer_enter(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    if let Some(folder_id) = state.enter_selected_folder() {
        state.loading = true;
        if let Some(ref client) = state.client {
            spawn_load_folder(client.clone(), folder_id, tx.clone());
        }
    } else if let Some(file) = state.current_selected_file() {
        if let Some(ref link) = file.web_view_link {
            let _ = open::that(link);
            state.set_toast("Opened file in browser.".to_string(), false);
        }
    }
}

fn handle_explorer_backspace(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    if let Some(parent_id) = state.navigate_up() {
        state.loading = true;
        state.set_toast(format!("Navigated up to: {}", parent_id), false);
        if let Some(ref client) = state.client {
            spawn_load_folder(client.clone(), parent_id, tx.clone());
        }
    }
}

fn handle_explorer_action_keys(
    state: &mut AppState,
    code: KeyCode,
    tx: &UnboundedSender<AppEvent>,
) {
    match code {
        KeyCode::Char('q') => state.should_exit = true,
        // Cycle sort mode
        KeyCode::Char('s') => {
            state.sort_mode = state.sort_mode.cycle();
            state.selected_index = 0;
            state.table_scroll = 0;
            state.set_toast(format!("Sorted by: {}", state.sort_mode.label()), false);
        }
        KeyCode::Char('c') => {
            state.current_view = CurrentView::FileOpModal(FileOpState::Create {
                name: InputBuffer::default(),
                content: InputBuffer::default(),
                as_doc: false,
                active_field: 0,
            });
        }
        KeyCode::Char('F') => {
            state.current_view = CurrentView::FileOpModal(FileOpState::CreateFolder {
                name: InputBuffer::default(),
            });
        }
        KeyCode::Char('r') => open_rename_modal(state),
        KeyCode::Char('m') => open_move_modal(state, tx),
        KeyCode::Char('v') => {
            state.toggle_tag_current();
        }
        KeyCode::Char('x') => {
            if state.search_results.is_some() || !state.filter_query.is_empty() {
                state.clear_search_and_filter();
                state.set_toast("Search and filters cleared.".to_string(), false);
            } else {
                state.toggle_tag_current();
            }
        }
        KeyCode::Char('y') => open_copy_modal(state),
        KeyCode::Char('u') => open_upload_modal(state),
        KeyCode::Char('d') => open_download_modal(state),
        KeyCode::Char('o') => open_in_browser(state),
        KeyCode::Char('P') => open_permissions_modal(state, tx),
        KeyCode::Char('C') => open_comments_modal(state, tx),
        KeyCode::Char('R') => open_revisions_modal(state, tx),
        KeyCode::Char('t') => open_trash_confirm(state),
        KeyCode::Char('T') => open_trash_view(state, tx),
        KeyCode::Char('p') => {
            state.current_view = CurrentView::ProfileModal;
            spawn_load_profiles(tx.clone());
        }
        KeyCode::Char('A') => {
            state.current_view = CurrentView::AboutModal;
            if let Some(ref client) = state.client {
                spawn_load_about(client.clone(), tx.clone());
            }
        }
        KeyCode::Char('S') => {
            state.current_view = CurrentView::SelfUpdateModal(UpdateModalState::default());
            spawn_check_update(tx.clone());
        }
        KeyCode::Char('D') => {
            state.current_view = CurrentView::DoctorModal(DoctorModalState {
                report: None,
                loading: true,
            });
            let tx_c = tx.clone();
            let profile = state.current_profile.clone();
            tokio::spawn(async move {
                let term_size = crossterm::terminal::size().ok();
                let report =
                    gdrive_core::run_doctor_checks(Some(&profile), Some((true, term_size))).await;
                let _ = tx_c.send(AppEvent::DoctorReportLoaded(report));
            });
        }
        KeyCode::Char('?') => state.current_view = CurrentView::HelpModal,
        _ => {}
    }
}

fn open_rename_modal(state: &mut AppState) {
    if let Some(file) = state.current_selected_file() {
        state.current_view = CurrentView::FileOpModal(FileOpState::Rename {
            file_id: file.id.clone(),
            old_name: file.name.clone(),
            new_name: InputBuffer::new(&file.name),
        });
    }
}

fn open_move_modal(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    let items: Vec<(String, String)> = if !state.tagged_files.is_empty() {
        state
            .tagged_file_objects()
            .into_iter()
            .map(|f| (f.id, f.name))
            .collect()
    } else if let Some(file) = state.current_selected_file() {
        vec![(file.id.clone(), file.name.clone())]
    } else {
        return;
    };

    let (current_id, current_name) = state
        .folder_path_stack
        .last()
        .cloned()
        .unwrap_or_else(|| ("My Drive".to_string(), "root".to_string()));

    let moving_ids: std::collections::HashSet<String> =
        items.iter().map(|(id, _)| id.clone()).collect();
    let initial_subfolders: Vec<gdrive_core::DriveFile> = state
        .folder_items
        .iter()
        .filter(|f| {
            f.mime_type == "application/vnd.google-apps.folder" && !moving_ids.contains(&f.id)
        })
        .cloned()
        .collect();

    state.current_view = CurrentView::FileOpModal(FileOpState::Move {
        items_to_move: items,
        current_folder_id: current_id.clone(),
        current_folder_name: current_name,
        path_stack: state.folder_path_stack.clone(),
        subfolders: initial_subfolders,
        search_query: InputBuffer::default(),
        search_results: None,
        selected_index: 0,
        active_field: 1,
        loading: false,
    });

    if let Some(ref client) = state.client {
        spawn_load_move_folders(client.clone(), current_id, tx.clone());
    }
}

fn open_copy_modal(state: &mut AppState) {
    if let Some(file) = state.current_selected_file() {
        state.current_view = CurrentView::FileOpModal(FileOpState::Copy {
            file_id: file.id.clone(),
            old_name: file.name.clone(),
            new_name: InputBuffer::new(format!("Copy of {}", file.name)),
            target_folder_id: InputBuffer::default(),
            active_field: 0,
        });
    }
}

fn open_upload_modal(state: &mut AppState) {
    state.current_view = CurrentView::FileOpModal(FileOpState::Upload {
        local_path: InputBuffer::default(),
        dest_name: InputBuffer::default(),
        convert_doc: false,
        active_field: 0,
    });
}

fn open_download_modal(state: &mut AppState) {
    let default_dir = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));
    if !state.tagged_files.is_empty() {
        let tagged = state.tagged_file_objects();
        state.current_view = CurrentView::FileOpModal(FileOpState::BatchDownload {
            files: tagged,
            dest_dir: InputBuffer::new(default_dir.display().to_string()),
            as_zip: true,
            current_file_idx: 0,
            current_file_name: String::new(),
            downloaded_in_file: 0,
            total_in_file: None,
            completed_files: 0,
            in_progress: false,
        });
    } else if let Some(file) = state.current_selected_file() {
        let default_dest = default_dir.join(&file.name);
        state.current_view = CurrentView::FileOpModal(FileOpState::Download {
            file_id: file.id.clone(),
            file_name: file.name.clone(),
            dest_path: InputBuffer::new(default_dest.display().to_string()),
            downloaded: 0,
            total: file.size.as_deref().and_then(|s| s.parse::<u64>().ok()),
            in_progress: false,
        });
    }
}

fn open_in_browser(state: &mut AppState) {
    if let Some(file) = state.current_selected_file() {
        if let Some(ref link) = file.web_view_link {
            let _ = open::that(link);
            state.set_toast("Opened item in browser.".to_string(), false);
        } else {
            state.set_toast("Item has no web link.".to_string(), true);
        }
    }
}

fn open_permissions_modal(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    if let Some(file) = state.current_selected_file() {
        let file_id = file.id.clone();
        let file_name = file.name.clone();
        state.current_view = CurrentView::PermissionsModal(PermissionsState {
            file_id: file_id.clone(),
            file_name,
            permissions: Vec::new(),
            selected_index: 0,
            adding: false,
            add_role_index: 0,
            add_type_index: 0,
            add_email: InputBuffer::default(),
            active_field: 0,
            loading: true,
        });
        if let Some(ref client) = state.client {
            spawn_load_permissions(client.clone(), file_id, tx.clone());
        }
    }
}

fn open_comments_modal(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    if let Some(file) = state.current_selected_file() {
        let file_id = file.id.clone();
        let file_name = file.name.clone();
        state.current_view = CurrentView::CommentsModal(CommentsState {
            file_id: file_id.clone(),
            file_name,
            comments: Vec::new(),
            selected_index: 0,
            adding: false,
            new_comment_text: InputBuffer::default(),
            loading: true,
        });
        if let Some(ref client) = state.client {
            spawn_load_comments(client.clone(), file_id, tx.clone());
        }
    }
}

fn open_revisions_modal(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    if let Some(file) = state.current_selected_file() {
        let file_id = file.id.clone();
        let file_name = file.name.clone();
        state.current_view = CurrentView::RevisionsModal(RevisionsState {
            file_id: file_id.clone(),
            file_name,
            revisions: Vec::new(),
            selected_index: 0,
            downloading: false,
            loading: true,
        });
        if let Some(ref client) = state.client {
            spawn_load_revisions(client.clone(), file_id, tx.clone());
        }
    }
}

fn open_trash_confirm(state: &mut AppState) {
    if let Some(file) = state.current_selected_file() {
        state.current_view = CurrentView::ConfirmModal(ConfirmAction {
            title: "Trash File".to_string(),
            prompt: format!("Are you sure you want to move \"{}\" to trash?", file.name),
            action_type: ConfirmType::TrashFile {
                id: file.id.clone(),
                name: file.name.clone(),
            },
        });
    }
}

fn open_trash_view(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    state.current_view = CurrentView::TrashView(TrashViewState {
        items: Vec::new(),
        selected_index: 0,
        loading: true,
    });
    if let Some(ref client) = state.client {
        spawn_load_trash(client.clone(), tx.clone());
    }
}

fn switch_drive_tab(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    state.loading = true;
    let categories = state.categories();
    let cat = categories
        .get(state.selected_drive_tab)
        .cloned()
        .unwrap_or(DriveCategory::MyDrive);

    match cat {
        DriveCategory::MyDrive => {
            state.reset_to_folder("My Drive".to_string(), "root".to_string());
            if let Some(ref client) = state.client {
                spawn_load_folder(client.clone(), "root".to_string(), tx.clone());
            }
        }
        DriveCategory::SharedWithMe => {
            state.reset_to_folder("Shared with Me".to_string(), "shared_with_me".to_string());
            if let Some(ref client) = state.client {
                spawn_load_category(
                    client.clone(),
                    "sharedWithMe = true".to_string(),
                    "shared_with_me".to_string(),
                    tx.clone(),
                );
            }
        }
        DriveCategory::Starred => {
            state.reset_to_folder("Starred".to_string(), "starred".to_string());
            if let Some(ref client) = state.client {
                spawn_load_category(
                    client.clone(),
                    "starred = true".to_string(),
                    "starred".to_string(),
                    tx.clone(),
                );
            }
        }
        DriveCategory::SharedDrive { id, name } => {
            state.reset_to_folder(name, id.clone());
            if let Some(ref client) = state.client {
                spawn_load_folder(client.clone(), id, tx.clone());
            }
        }
    }
}

fn trigger_preview_load_if_needed(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    let file_info = state
        .current_selected_file()
        .map(|f| (f.id.clone(), f.mime_type.clone(), f.name.clone()));

    if let Some((file_id, mime_type, file_name)) = file_info {
        if mime_type == "application/vnd.google-apps.folder" {
            state.preview_content = None;
            state.preview_file_id = None;
            state.preview_loading = false;

            // Peek contents of selected folder
            if !state.folder_peek_cache.contains_key(&file_id)
                && state.folder_peek_loading.as_deref() != Some(&file_id)
            {
                state.folder_peek_loading = Some(file_id.clone());
                if let Some(ref client) = state.client {
                    spawn_load_folder_peek(client.clone(), file_id, tx.clone());
                }
            }
            return;
        }

        let is_textual = mime_type.starts_with("text/")
            || mime_type == "application/json"
            || mime_type == "application/vnd.google-apps.document"
            || mime_type == "application/vnd.google-apps.spreadsheet"
            || mime_type == "application/vnd.google-apps.presentation"
            || file_name.ends_with(".md")
            || file_name.ends_with(".txt")
            || file_name.ends_with(".csv")
            || file_name.ends_with(".rs")
            || file_name.ends_with(".json");

        if is_textual {
            state.preview_file_id = Some(file_id.clone());
            state.preview_loading = true;
            state.preview_content = None;
            state.preview_scroll = 0;
            if let Some(ref client) = state.client {
                spawn_load_preview(client.clone(), file_id, tx.clone());
            }
        } else {
            state.preview_content = None;
            state.preview_file_id = None;
            state.preview_loading = false;
        }
    } else {
        state.preview_content = None;
        state.preview_file_id = None;
        state.preview_loading = false;
    }
}

async fn handle_wizard_keys(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let step = match &state.current_view {
        CurrentView::OnboardingWizard(ref w) => w.step,
        _ => return,
    };

    if step == 0 {
        handle_wizard_step0(state, key, tx).await;
    } else if step == 1 {
        handle_wizard_step1(state, key, tx).await;
    } else if step == 2 {
        handle_wizard_step2(state, key.code);
    }
}

async fn handle_wizard_step0(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let has_creds = match &state.current_view {
        CurrentView::OnboardingWizard(ref w) => w.has_existing_credentials,
        _ => return,
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Char('m') => {
            if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.step = 1;
            }
        }
        KeyCode::Enter => {
            if has_creds {
                trigger_wizard_login_with_existing(state, tx).await;
            } else if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.step = 1;
            }
        }
        _ => {
            if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.profile_name.handle_key(key);
            }
        }
    }
}

async fn trigger_wizard_login_with_existing(state: &mut AppState, tx: &UnboundedSender<AppEvent>) {
    let prof_name = match &state.current_view {
        CurrentView::OnboardingWizard(ref w) => {
            let s = w.profile_name.as_str().trim().to_string();
            if s.is_empty() {
                "default".to_string()
            } else {
                s
            }
        }
        _ => return,
    };

    if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
        w.step = 2;
        w.loading = true;
        w.error_message = None;
    }

    state.current_profile = prof_name.clone();
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        let (url_tx, url_rx) = tokio::sync::oneshot::channel();
        let auth_mgr = match AuthManager::from_env_or_config() {
            Ok(m) => m,
            Err(e) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(e.to_string())));
                return;
            }
        };

        let login_task = tokio::spawn(async move {
            auth_mgr
                .login_loopback_silent(Some(&prof_name), url_tx)
                .await
        });

        if let Ok(url) = url_rx.await {
            let _ = tx_clone.send(AppEvent::AuthUrlReady(url.clone()));
            let _ = open::that(&url);
        }

        match login_task.await {
            Ok(Ok(token)) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Ok(token)));
            }
            Ok(Err(e)) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(e.to_string())));
            }
            Err(join_err) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(join_err.to_string())));
            }
        }
    });
}

async fn handle_wizard_step1(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let wiz = match &mut state.current_view {
        CurrentView::OnboardingWizard(ref mut w) => w,
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            wiz.step = 0;
        }
        KeyCode::Tab => {
            wiz.active_field = (wiz.active_field + 1) % 3;
        }
        KeyCode::BackTab => {
            wiz.active_field = (wiz.active_field + 2) % 3;
        }
        KeyCode::Enter => {
            trigger_wizard_login_with_manual_creds(state, tx).await;
        }
        _ => match wiz.active_field {
            0 => {
                wiz.client_id_input.handle_key(key);
            }
            1 => {
                wiz.client_secret_input.handle_key(key);
            }
            2 => {
                wiz.credentials_path_input.handle_key(key);
            }
            _ => {}
        },
    }
}

async fn trigger_wizard_login_with_manual_creds(
    state: &mut AppState,
    tx: &UnboundedSender<AppEvent>,
) {
    let (cid, sec, path) = match &state.current_view {
        CurrentView::OnboardingWizard(ref w) => (
            w.client_id_input.as_str().trim().to_string(),
            w.client_secret_input.as_str().trim().to_string(),
            w.credentials_path_input.as_str().trim().to_string(),
        ),
        _ => return,
    };

    if !path.is_empty() {
        let p = PathBuf::from(&path);
        if let Err(e) = AuthManager::save_credentials_from_file(&p) {
            if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.error_message = Some(format!("Gagal membaca file credentials: {}", e));
            }
            return;
        }
    } else if !cid.is_empty() && !sec.is_empty() {
        if let Err(e) = AuthManager::save_credentials(&cid, &sec) {
            if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.error_message = Some(format!("Gagal menyimpan credentials: {}", e));
            }
            return;
        }
    } else {
        if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
            w.error_message = Some(
                "Silakan masukkan Client ID & Secret atau path file credentials.json".to_string(),
            );
        }
        return;
    }

    if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
        w.step = 2;
        w.loading = true;
        w.error_message = None;
    }

    let profile_name = state.current_profile.clone();
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        let (url_tx, url_rx) = tokio::sync::oneshot::channel();
        let auth_mgr = match AuthManager::from_env_or_config() {
            Ok(m) => m,
            Err(e) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(e.to_string())));
                return;
            }
        };

        let login_task = tokio::spawn(async move {
            auth_mgr
                .login_loopback_silent(Some(&profile_name), url_tx)
                .await
        });

        if let Ok(url) = url_rx.await {
            let _ = tx_clone.send(AppEvent::AuthUrlReady(url.clone()));
            let _ = open::that(&url);
        }

        match login_task.await {
            Ok(Ok(token)) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Ok(token)));
            }
            Ok(Err(e)) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(e.to_string())));
            }
            Err(join_err) => {
                let _ = tx_clone.send(AppEvent::AuthFinished(Err(join_err.to_string())));
            }
        }
    });
}

fn handle_wizard_step2(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            if let CurrentView::OnboardingWizard(ref mut w) = state.current_view {
                w.step = 0;
                w.loading = false;
            }
        }
        KeyCode::Char('o') => {
            if let CurrentView::OnboardingWizard(ref w) = state.current_view {
                if let Some(ref url) = w.auth_url {
                    let _ = open::that(url);
                    state.set_toast("Opened authorization URL in browser.".to_string(), false);
                }
            }
        }
        _ => {}
    }
}

async fn handle_profile_modal_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let count = state.available_profiles.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                state.profile_modal_selected_index =
                    (state.profile_modal_selected_index + 1) % count;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if count > 0 {
                if state.profile_modal_selected_index > 0 {
                    state.profile_modal_selected_index -= 1;
                } else {
                    state.profile_modal_selected_index = count - 1;
                }
            }
        }
        KeyCode::Enter => {
            if let Some(p) = state
                .available_profiles
                .get(state.profile_modal_selected_index)
            {
                let name = p.name.clone();
                let _ = AuthManager::set_active_profile(&name);
                state.current_profile = name.clone();
                state.set_toast(format!("Switched to profile: {}", name), false);
                state.current_view = CurrentView::DriveExplorer;

                if let Ok(client) = GDriveClient::from_profile(Some(&name)).await {
                    state.client = Some(client.clone());
                    state.loading = true;
                    spawn_load_folder(client.clone(), "root".to_string(), tx.clone());
                    spawn_load_drives(client.clone(), tx.clone());
                    spawn_load_about(client.clone(), tx.clone());
                }
            }
        }
        KeyCode::Char('a') => {
            let has_creds = AuthManager::from_env_or_config().is_ok();
            state.current_view = CurrentView::OnboardingWizard(WizardState {
                step: 0,
                profile_name: InputBuffer::new("work"),
                client_id_input: InputBuffer::default(),
                client_secret_input: InputBuffer::default(),
                credentials_path_input: InputBuffer::default(),
                has_existing_credentials: has_creds,
                auth_url: None,
                active_field: 0,
                error_message: None,
                loading: false,
            });
        }
        KeyCode::Char('d') => {
            if let Some(p) = state
                .available_profiles
                .get(state.profile_modal_selected_index)
            {
                let name = p.name.clone();
                state.current_view = CurrentView::ConfirmModal(ConfirmAction {
                    title: "Delete Profile".to_string(),
                    prompt: format!("Delete profile \"{}\" and remove its stored token?", name),
                    action_type: ConfirmType::DeleteProfile { name },
                });
            }
        }
        _ => {}
    }
}

async fn handle_search_modal_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let s = match &mut state.current_view {
        CurrentView::SearchModal(ref mut s) => s,
        _ => return,
    };

    match key.code {
        KeyCode::Esc => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Tab => {
            s.active_field = (s.active_field + 1) % 3;
        }
        KeyCode::BackTab => {
            s.active_field = (s.active_field + 2) % 3;
        }
        KeyCode::Char(' ') if s.active_field == 1 => {
            s.full_text = !s.full_text;
        }
        KeyCode::Char(' ') if s.active_field == 2 => {
            s.order_by_index = (s.order_by_index + 1) % 4;
        }
        KeyCode::Enter => {
            let q = s.query.as_str().trim().to_string();
            let full_text = s.full_text;
            let order_by = match s.order_by_index {
                1 => Some("modifiedTime desc".to_string()),
                2 => Some("name asc".to_string()),
                3 => Some("quotaBytesUsed desc".to_string()),
                _ => None,
            };

            if q.is_empty() {
                state.set_toast("Search query cannot be empty.".to_string(), true);
                return;
            }

            state.loading = true;
            state.current_view = CurrentView::DriveExplorer;
            if let Some(ref client) = state.client {
                spawn_search(client.clone(), q, full_text, order_by, tx.clone());
            }
        }
        _ => {
            if s.active_field == 0 {
                s.query.handle_key(key);
            }
        }
    }
}

async fn handle_file_op_keys(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let mut execute_action: Option<FileOpState> = None;
    let mut load_subfolder_id: Option<String> = None;

    if let CurrentView::FileOpModal(ref mut op) = state.current_view {
        handle_file_op_input(op, key, &mut execute_action, &mut load_subfolder_id);
    }

    if let Some(target) = load_subfolder_id {
        if let Some(ref client) = state.client {
            if let Some(query) = target.strip_prefix("search:") {
                spawn_search_folders(client.clone(), query.to_string(), tx.clone());
            } else {
                spawn_load_move_folders(client.clone(), target, tx.clone());
            }
        }
    }

    if key.code == KeyCode::Esc {
        state.current_view = CurrentView::DriveExplorer;
        return;
    }

    if let Some(action) = execute_action {
        execute_file_op(state, action, tx).await;
    }
}

fn handle_file_op_input(
    op: &mut FileOpState,
    key: KeyEvent,
    exec: &mut Option<FileOpState>,
    load_subfolder_id: &mut Option<String>,
) {
    if key.code == KeyCode::Enter && !matches!(op, FileOpState::Move { .. }) {
        *exec = Some(op.clone());
        return;
    }
    match op {
        FileOpState::Create {
            name,
            content,
            as_doc,
            active_field,
        } => match key.code {
            KeyCode::Tab => *active_field = (*active_field + 1) % 3,
            KeyCode::BackTab => *active_field = (*active_field + 2) % 3,
            KeyCode::Char(' ') if *active_field == 2 => *as_doc = !*as_doc,
            _ => match *active_field {
                0 => {
                    name.handle_key(key);
                }
                1 => {
                    content.handle_key(key);
                }
                _ => {}
            },
        },
        FileOpState::CreateFolder { name } => {
            name.handle_key(key);
        }
        FileOpState::Rename { new_name, .. } => {
            new_name.handle_key(key);
        }
        FileOpState::Move {
            ref mut current_folder_id,
            ref mut current_folder_name,
            ref mut path_stack,
            ref subfolders,
            ref mut search_query,
            ref search_results,
            ref mut selected_index,
            ref mut active_field,
            ref mut loading,
            ..
        } => {
            if key.code == KeyCode::Tab {
                *active_field = (*active_field + 1) % 2;
                return;
            }

            if *active_field == 0 {
                let old_q = search_query.as_str();
                if search_query.handle_key(key) {
                    let new_q = search_query.as_str();
                    if new_q != old_q {
                        *selected_index = 0;
                        if new_q.trim().is_empty() {
                            *load_subfolder_id = Some(current_folder_id.clone());
                        } else {
                            *loading = true;
                            *load_subfolder_id = Some(format!("search:{}", new_q.trim()));
                        }
                    }
                }
                return;
            }

            let total_rows = if let Some(ref s_res) = search_results {
                s_res.len()
            } else {
                let mut base = 1 + subfolders.len();
                if path_stack.len() > 1 {
                    base += 1;
                }
                if current_folder_id != "root" {
                    base += 1;
                }
                base
            };

            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    if *selected_index > 0 {
                        *selected_index -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *selected_index + 1 < total_rows {
                        *selected_index += 1;
                    }
                }
                KeyCode::Enter => {
                    if let Some(ref s_res) = search_results {
                        if let Some(target) = s_res.get(*selected_index) {
                            *current_folder_id = target.id.clone();
                            *current_folder_name = target.name.clone();
                            *exec = Some(op.clone());
                        }
                    } else {
                        let mut row = 0;
                        if *selected_index == row {
                            *exec = Some(op.clone());
                            return;
                        }
                        if path_stack.len() > 1 {
                            row += 1;
                            if *selected_index == row {
                                path_stack.pop();
                                let (p_name, p_id) = path_stack.last().cloned().unwrap();
                                *current_folder_id = p_id.clone();
                                *current_folder_name = p_name;
                                *selected_index = 0;
                                *loading = true;
                                *load_subfolder_id = Some(p_id);
                                return;
                            }
                        }
                        if current_folder_id != "root" {
                            row += 1;
                            if *selected_index == row {
                                *path_stack = vec![("My Drive".to_string(), "root".to_string())];
                                *current_folder_id = "root".to_string();
                                *current_folder_name = "My Drive".to_string();
                                *selected_index = 0;
                                *loading = true;
                                *load_subfolder_id = Some("root".to_string());
                                return;
                            }
                        }
                        let sub_idx = *selected_index - row - 1;
                        if let Some(sub) = subfolders.get(sub_idx) {
                            let next_id = sub.id.clone();
                            let next_name = sub.name.clone();
                            path_stack.push((next_name.clone(), next_id.clone()));
                            *current_folder_id = next_id.clone();
                            *current_folder_name = next_name;
                            *selected_index = 0;
                            *loading = true;
                            *load_subfolder_id = Some(next_id);
                        }
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if search_results.is_none() {
                        let mut row = 0;
                        if path_stack.len() > 1 {
                            row += 1;
                        }
                        if current_folder_id != "root" {
                            row += 1;
                        }
                        if *selected_index > row {
                            let sub_idx = *selected_index - row - 1;
                            if let Some(sub) = subfolders.get(sub_idx) {
                                let next_id = sub.id.clone();
                                let next_name = sub.name.clone();
                                path_stack.push((next_name.clone(), next_id.clone()));
                                *current_folder_id = next_id.clone();
                                *current_folder_name = next_name;
                                *selected_index = 0;
                                *loading = true;
                                *load_subfolder_id = Some(next_id);
                            }
                        }
                    }
                }
                KeyCode::Left | KeyCode::Backspace | KeyCode::Char('h') if path_stack.len() > 1 => {
                    path_stack.pop();
                    let (p_name, p_id) = path_stack.last().cloned().unwrap();
                    *current_folder_id = p_id.clone();
                    *current_folder_name = p_name;
                    *selected_index = 0;
                    *loading = true;
                    *load_subfolder_id = Some(p_id);
                }
                _ => {}
            }
        }
        FileOpState::Copy {
            new_name,
            target_folder_id,
            active_field,
            ..
        } => match key.code {
            KeyCode::Tab => *active_field = (*active_field + 1) % 2,
            KeyCode::BackTab => *active_field = (*active_field + 1) % 2,
            _ => match *active_field {
                0 => {
                    new_name.handle_key(key);
                }
                1 => {
                    target_folder_id.handle_key(key);
                }
                _ => {}
            },
        },
        FileOpState::Upload {
            local_path,
            dest_name,
            convert_doc,
            active_field,
        } => match key.code {
            KeyCode::Tab => *active_field = (*active_field + 1) % 3,
            KeyCode::BackTab => *active_field = (*active_field + 2) % 3,
            KeyCode::Char(' ') if *active_field == 2 => *convert_doc = !*convert_doc,
            _ => match *active_field {
                0 => {
                    local_path.handle_key(key);
                }
                1 => {
                    dest_name.handle_key(key);
                }
                _ => {}
            },
        },
        FileOpState::Download {
            dest_path,
            in_progress,
            ..
        } => {
            if !*in_progress {
                if key.code == KeyCode::Tab {
                    dest_path.autocomplete_local_path();
                } else {
                    dest_path.handle_key(key);
                }
            }
        }
        FileOpState::BatchDownload {
            dest_dir,
            as_zip,
            in_progress,
            ..
        } => {
            if !*in_progress {
                if key.code == KeyCode::Char(' ') {
                    *as_zip = !*as_zip;
                } else if key.code == KeyCode::Tab {
                    dest_dir.autocomplete_local_path();
                } else {
                    dest_dir.handle_key(key);
                }
            }
        }
    }
}

async fn execute_file_op(
    state: &mut AppState,
    action: FileOpState,
    tx: &UnboundedSender<AppEvent>,
) {
    match action {
        FileOpState::Create {
            name,
            content,
            as_doc,
            ..
        } => {
            let name_str = name.as_str().trim().to_string();
            if name_str.is_empty() {
                state.set_toast("File name cannot be empty.".to_string(), true);
                return;
            }
            let create_opts = CreateFileOptions {
                name: name_str,
                content: content.as_str(),
                parent_folder_id: Some(state.current_folder_id.clone()),
                as_google_doc: as_doc,
            };
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c.create_file(create_opts).await;
                    let msg = res
                        .map(|f| format!("Created file \"{}\"", f.name))
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        FileOpState::CreateFolder { name } => {
            let folder_name = name.as_str().trim().to_string();
            if folder_name.is_empty() {
                state.set_toast("Folder name cannot be empty.".to_string(), true);
                return;
            }
            let parent_id = state.current_folder_id.clone();
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c.create_folder(&folder_name, Some(&parent_id)).await;
                    let msg = res
                        .map(|f| format!("Created folder \"{}\"", f.name))
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        FileOpState::Rename {
            file_id, new_name, ..
        } => {
            let target_name = new_name.as_str().trim().to_string();
            if target_name.is_empty() {
                state.set_toast("Name cannot be empty.".to_string(), true);
                return;
            }
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c.rename_file(&file_id, &target_name).await;
                    let msg = res
                        .map(|f| format!("Renamed to \"{}\"", f.name))
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        FileOpState::Move {
            items_to_move,
            current_folder_id,
            current_folder_name,
            ..
        } => {
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let total = items_to_move.len();
                    for (id, _) in &items_to_move {
                        let _ = client_c.move_file(id, &current_folder_id, None).await;
                    }
                    let msg = if total > 1 {
                        format!("Batch moved {} items into '{}'", total, current_folder_name)
                    } else if let Some((_, name)) = items_to_move.first() {
                        format!("Moved \"{}\" into '{}'", name, current_folder_name)
                    } else {
                        format!("Moved items into '{}'", current_folder_name)
                    };
                    let _ = tx_c.send(AppEvent::OperationComplete(Ok(msg)));
                });
            }
        }
        FileOpState::Copy {
            file_id,
            new_name,
            target_folder_id,
            ..
        } => {
            let n_name = if new_name.as_str().trim().is_empty() {
                None
            } else {
                Some(new_name.as_str().trim().to_string())
            };
            let t_folder = if target_folder_id.as_str().trim().is_empty() {
                None
            } else {
                Some(target_folder_id.as_str().trim().to_string())
            };
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c
                        .copy_file(&file_id, n_name.as_deref(), t_folder.as_deref())
                        .await;
                    let msg = res
                        .map(|f| format!("Copied to \"{}\"", f.name))
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        FileOpState::Upload {
            local_path,
            dest_name,
            convert_doc,
            ..
        } => {
            let l_path = PathBuf::from(local_path.as_str().trim());
            if !l_path.exists() {
                state.set_toast(
                    format!("File '{}' not found on disk.", local_path.as_str()),
                    true,
                );
                return;
            }
            let d_name = if dest_name.as_str().trim().is_empty() {
                None
            } else {
                Some(dest_name.as_str().trim().to_string())
            };
            let parent_id = state.current_folder_id.clone();
            state.current_view = CurrentView::DriveExplorer;
            state.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c
                        .upload_file_path(&l_path, d_name.as_deref(), Some(&parent_id), convert_doc)
                        .await;
                    let msg = res
                        .map(|f| format!("Uploaded \"{}\"", f.name))
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        FileOpState::Download {
            file_id, dest_path, ..
        } => {
            let out_path = dest_path.as_str().trim().to_string();
            if let CurrentView::FileOpModal(FileOpState::Download {
                ref mut in_progress,
                ..
            }) = state.current_view
            {
                *in_progress = true;
            }
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    match client_c.download_file_stream(&file_id).await {
                        Ok(dl_stream) => {
                            let total = dl_stream.size;
                            let target_path =
                                if out_path.ends_with('/') || Path::new(&out_path).is_dir() {
                                    Path::new(&out_path).join(&dl_stream.name)
                                } else {
                                    PathBuf::from(&out_path)
                                };

                            match tokio::fs::File::create(&target_path).await {
                                Ok(mut file) => {
                                    use tokio::io::AsyncWriteExt;
                                    let mut downloaded: u64 = 0;
                                    let mut stream = dl_stream.stream;
                                    let mut write_err = None;

                                    while let Some(chunk_res) = stream.next().await {
                                        match chunk_res {
                                            Ok(chunk) => {
                                                if let Err(e) = file.write_all(&chunk).await {
                                                    write_err = Some(e.to_string());
                                                    break;
                                                }
                                                downloaded += chunk.len() as u64;
                                                let _ = tx_c.send(AppEvent::DownloadProgress {
                                                    downloaded,
                                                    total,
                                                });
                                            }
                                            Err(e) => {
                                                write_err = Some(e.to_string());
                                                break;
                                            }
                                        }
                                    }

                                    if let Some(e) = write_err {
                                        let _ = tx_c.send(AppEvent::OperationComplete(Err(
                                            format!("Download interrupted: {}", e),
                                        )));
                                    } else {
                                        let _ = file.flush().await;
                                        let _ = tx_c.send(AppEvent::OperationComplete(Ok(
                                            format!("Downloaded to {:?}", target_path),
                                        )));
                                    }
                                }
                                Err(e) => {
                                    let _ = tx_c.send(AppEvent::OperationComplete(Err(format!(
                                        "Failed creating destination file: {}",
                                        e
                                    ))));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx_c.send(AppEvent::OperationComplete(Err(e.to_string())));
                        }
                    }
                });
            }
        }
        FileOpState::BatchDownload {
            files,
            dest_dir,
            as_zip,
            ..
        } => {
            let out_dir = dest_dir.as_str().trim().to_string();
            if let CurrentView::FileOpModal(FileOpState::BatchDownload {
                ref mut in_progress,
                ..
            }) = state.current_view
            {
                *in_progress = true;
            }
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let total_files = files.len();
                    let dir_path = PathBuf::from(&out_dir);
                    let _ = tokio::fs::create_dir_all(&dir_path).await;

                    if as_zip {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let zip_path = dir_path.join(format!("gdrive_batch_{}.zip", ts));

                        match std::fs::File::create(&zip_path) {
                            Ok(zip_file) => {
                                let mut zip_writer = zip::ZipWriter::new(zip_file);
                                for (completed, file) in files.iter().enumerate() {
                                    let _ = tx_c.send(AppEvent::BatchDownloadProgress {
                                        completed_files: completed,
                                        total_files,
                                        current_file_name: file.name.clone(),
                                        current_downloaded: 0,
                                        current_total: file
                                            .size
                                            .as_deref()
                                            .and_then(|s| s.parse::<u64>().ok()),
                                    });

                                    if let Ok(dl_stream) =
                                        client_c.download_file_stream(&file.id).await
                                    {
                                        let options = zip::write::SimpleFileOptions::default()
                                            .compression_method(zip::CompressionMethod::Deflated);
                                        let _ = zip_writer.start_file(&dl_stream.name, options);

                                        let mut dl_bytes: u64 = 0;
                                        let mut stream = dl_stream.stream;
                                        use std::io::Write;
                                        while let Some(chunk_res) = stream.next().await {
                                            if let Ok(chunk) = chunk_res {
                                                let _ = zip_writer.write_all(&chunk);
                                                dl_bytes += chunk.len() as u64;
                                                let _ =
                                                    tx_c.send(AppEvent::BatchDownloadProgress {
                                                        completed_files: completed,
                                                        total_files,
                                                        current_file_name: file.name.clone(),
                                                        current_downloaded: dl_bytes,
                                                        current_total: dl_stream.size,
                                                    });
                                            }
                                        }
                                    }
                                }

                                let _ = zip_writer.finish();
                                let _ = tx_c.send(AppEvent::BatchDownloadProgress {
                                    completed_files: total_files,
                                    total_files,
                                    current_file_name: "Complete".to_string(),
                                    current_downloaded: 0,
                                    current_total: None,
                                });

                                let _ = tx_c.send(AppEvent::OperationComplete(Ok(format!(
                                    "Batch download complete: {} files compressed into {:?}",
                                    total_files, zip_path
                                ))));
                            }
                            Err(e) => {
                                let _ = tx_c.send(AppEvent::OperationComplete(Err(format!(
                                    "Failed creating zip archive: {}",
                                    e
                                ))));
                            }
                        }
                    } else {
                        for (completed, file) in files.iter().enumerate() {
                            let _ = tx_c.send(AppEvent::BatchDownloadProgress {
                                completed_files: completed,
                                total_files,
                                current_file_name: file.name.clone(),
                                current_downloaded: 0,
                                current_total: file
                                    .size
                                    .as_deref()
                                    .and_then(|s| s.parse::<u64>().ok()),
                            });

                            if let Ok(dl_stream) = client_c.download_file_stream(&file.id).await {
                                let target_path = dir_path.join(&dl_stream.name);
                                if let Ok(mut f) = tokio::fs::File::create(&target_path).await {
                                    use tokio::io::AsyncWriteExt;
                                    let mut dl_bytes: u64 = 0;
                                    let mut stream = dl_stream.stream;
                                    while let Some(chunk_res) = stream.next().await {
                                        if let Ok(chunk) = chunk_res {
                                            let _ = f.write_all(&chunk).await;
                                            dl_bytes += chunk.len() as u64;
                                            let _ = tx_c.send(AppEvent::BatchDownloadProgress {
                                                completed_files: completed,
                                                total_files,
                                                current_file_name: file.name.clone(),
                                                current_downloaded: dl_bytes,
                                                current_total: dl_stream.size,
                                            });
                                        }
                                    }
                                    let _ = f.flush().await;
                                }
                            }
                        }

                        let _ = tx_c.send(AppEvent::BatchDownloadProgress {
                            completed_files: total_files,
                            total_files,
                            current_file_name: "Complete".to_string(),
                            current_downloaded: 0,
                            current_total: None,
                        });

                        let _ = tx_c.send(AppEvent::OperationComplete(Ok(format!(
                            "Batch download complete: {} files saved to {:?}",
                            total_files, dir_path
                        ))));
                    }
                });
            }
        }
    }
}

async fn handle_permissions_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let adding = match &state.current_view {
        CurrentView::PermissionsModal(ref p) => p.adding,
        _ => return,
    };

    if adding {
        handle_permissions_adding_keys(state, key, tx);
    } else {
        handle_permissions_list_keys(state, key.code);
    }
}

fn handle_permissions_adding_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let perm_st = match &mut state.current_view {
        CurrentView::PermissionsModal(ref mut p) => p,
        _ => return,
    };

    match key.code {
        KeyCode::Esc => perm_st.adding = false,
        KeyCode::Tab => perm_st.active_field = (perm_st.active_field + 1) % 3,
        KeyCode::BackTab => perm_st.active_field = (perm_st.active_field + 2) % 3,
        KeyCode::Char(' ') if perm_st.active_field == 1 => {
            perm_st.add_role_index = (perm_st.add_role_index + 1) % 3;
        }
        KeyCode::Char(' ') if perm_st.active_field == 2 => {
            perm_st.add_type_index = (perm_st.add_type_index + 1) % 4;
        }
        KeyCode::Enter => {
            let roles = ["reader", "commenter", "writer"];
            let types = ["user", "group", "domain", "anyone"];
            let role = roles[perm_st.add_role_index].to_string();
            let p_type = types[perm_st.add_type_index].to_string();
            let email_str = perm_st.add_email.as_str().trim().to_string();
            let email = if email_str.is_empty() {
                None
            } else {
                Some(email_str)
            };

            let opts = CreatePermissionOptions {
                role,
                permission_type: p_type,
                email_address: email,
                domain: None,
                send_notification_email: false,
            };
            let f_id = perm_st.file_id.clone();
            perm_st.adding = false;
            perm_st.loading = true;

            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c.create_permission(&f_id, opts).await;
                    let msg = res
                        .map(|_| "Permission added successfully.".to_string())
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        _ => {
            if perm_st.active_field == 0 {
                perm_st.add_email.handle_key(key);
            }
        }
    }
}

fn handle_permissions_list_keys(state: &mut AppState, code: KeyCode) {
    let (count, sel, file_id) = match &state.current_view {
        CurrentView::PermissionsModal(ref p) => {
            (p.permissions.len(), p.selected_index, p.file_id.clone())
        }
        _ => return,
    };

    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                if let CurrentView::PermissionsModal(ref mut p) = state.current_view {
                    p.selected_index = (sel + 1) % count;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if count > 0 {
                if let CurrentView::PermissionsModal(ref mut p) = state.current_view {
                    p.selected_index = if sel > 0 { sel - 1 } else { count - 1 };
                }
            }
        }
        KeyCode::Char('a') => {
            if let CurrentView::PermissionsModal(ref mut p) = state.current_view {
                p.adding = true;
                p.add_email.clear();
                p.active_field = 0;
            }
        }
        KeyCode::Char('d') => {
            if let CurrentView::PermissionsModal(ref p) = state.current_view {
                if let Some(perm) = p.permissions.get(p.selected_index) {
                    let permission_id = perm.id.clone();
                    let grantee = perm
                        .email_address
                        .as_deref()
                        .unwrap_or(&perm.id)
                        .to_string();
                    state.current_view = CurrentView::ConfirmModal(ConfirmAction {
                        title: "Revoke Permission".to_string(),
                        prompt: format!("Revoke access for \"{}\"?", grantee),
                        action_type: ConfirmType::DeletePermission {
                            file_id,
                            permission_id,
                            grantee,
                        },
                    });
                }
            }
        }
        _ => {}
    }
}

async fn handle_comments_keys(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let adding = match &state.current_view {
        CurrentView::CommentsModal(ref c) => c.adding,
        _ => return,
    };

    if adding {
        handle_comments_adding_keys(state, key, tx);
    } else {
        handle_comments_list_keys(state, key.code, tx);
    }
}

fn handle_comments_adding_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let comm_st = match &mut state.current_view {
        CurrentView::CommentsModal(ref mut c) => c,
        _ => return,
    };

    match key.code {
        KeyCode::Esc => comm_st.adding = false,
        KeyCode::Enter => {
            let text = comm_st.new_comment_text.as_str().trim().to_string();
            if text.is_empty() {
                return;
            }
            let f_id = comm_st.file_id.clone();
            comm_st.adding = false;
            comm_st.loading = true;
            if let Some(ref client) = state.client {
                let client_c = client.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = client_c.create_comment(&f_id, &text).await;
                    let msg = res
                        .map(|_| "Comment posted.".to_string())
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
        }
        _ => {
            comm_st.new_comment_text.handle_key(key);
        }
    }
}

fn handle_comments_list_keys(state: &mut AppState, code: KeyCode, tx: &UnboundedSender<AppEvent>) {
    let (count, sel, f_id, comment_id) = match &state.current_view {
        CurrentView::CommentsModal(ref c) => (
            c.comments.len(),
            c.selected_index,
            c.file_id.clone(),
            c.comments.get(c.selected_index).map(|com| com.id.clone()),
        ),
        _ => return,
    };

    match code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                if let CurrentView::CommentsModal(ref mut c) = state.current_view {
                    c.selected_index = (sel + 1) % count;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if count > 0 {
                if let CurrentView::CommentsModal(ref mut c) = state.current_view {
                    c.selected_index = if sel > 0 { sel - 1 } else { count - 1 };
                }
            }
        }
        KeyCode::Char('a') => {
            if let CurrentView::CommentsModal(ref mut c) = state.current_view {
                c.adding = true;
                c.new_comment_text.clear();
            }
        }
        KeyCode::Char('r') => {
            if let Some(c_id) = comment_id {
                if let CurrentView::CommentsModal(ref mut c) = state.current_view {
                    c.loading = true;
                }
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.resolve_comment(&f_id, &c_id).await;
                        let msg = res
                            .map(|_| "Comment resolved.".to_string())
                            .map_err(|e| e.to_string());
                        let _ = tx_c.send(AppEvent::OperationComplete(msg));
                    });
                }
            }
        }
        _ => {}
    }
}

async fn handle_revisions_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    let rev_st = match &mut state.current_view {
        CurrentView::RevisionsModal(ref mut r) => r,
        _ => return,
    };

    let count = rev_st.revisions.len();
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                rev_st.selected_index = (rev_st.selected_index + 1) % count;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if count > 0 {
                if rev_st.selected_index > 0 {
                    rev_st.selected_index -= 1;
                } else {
                    rev_st.selected_index = count - 1;
                }
            }
        }
        KeyCode::Char('d') => {
            if let Some(rev) = rev_st.revisions.get(rev_st.selected_index) {
                let f_id = rev_st.file_id.clone();
                let rev_id = rev.id.clone();
                let file_name = rev_st.file_name.clone();
                rev_st.downloading = true;
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.download_revision_bytes(&f_id, &rev_id).await;
                        match res {
                            Ok((_mime, bytes)) => {
                                let out_name = format!("{}_rev_{}", file_name, rev_id);
                                let _ = std::fs::write(&out_name, &bytes);
                                let _ = tx_c.send(AppEvent::OperationComplete(Ok(format!(
                                    "Revision saved to {}",
                                    out_name
                                ))));
                            }
                            Err(e) => {
                                let _ = tx_c.send(AppEvent::OperationComplete(Err(e.to_string())));
                            }
                        }
                    });
                }
            }
        }
        _ => {}
    }
}

async fn handle_trash_view_keys(
    state: &mut AppState,
    key: KeyEvent,
    _tx: &UnboundedSender<AppEvent>,
) {
    let (count, selected_item) = match &state.current_view {
        CurrentView::TrashView(ref trash_st) => (
            trash_st.items.len(),
            trash_st.items.get(trash_st.selected_index).cloned(),
        ),
        _ => return,
    };

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if count > 0 {
                if let CurrentView::TrashView(ref mut trash_st) = state.current_view {
                    trash_st.selected_index = (trash_st.selected_index + 1) % count;
                }
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if count > 0 {
                if let CurrentView::TrashView(ref mut trash_st) = state.current_view {
                    if trash_st.selected_index > 0 {
                        trash_st.selected_index -= 1;
                    } else {
                        trash_st.selected_index = count - 1;
                    }
                }
            }
        }
        KeyCode::Char('r') => {
            if let Some(item) = selected_item {
                state.current_view = CurrentView::ConfirmModal(ConfirmAction {
                    title: "Restore File".to_string(),
                    prompt: format!("Restore \"{}\" from trash back to Drive?", item.name),
                    action_type: ConfirmType::RestoreFile {
                        id: item.id,
                        name: item.name,
                    },
                });
            }
        }
        KeyCode::Char('e') => {
            state.current_view = CurrentView::ConfirmModal(ConfirmAction {
                title: "Empty Trash".to_string(),
                prompt: "Are you sure you want to permanently delete all items in trash?"
                    .to_string(),
                action_type: ConfirmType::EmptyTrash,
            });
        }
        _ => {}
    }
}

async fn handle_update_modal_keys(
    state: &mut AppState,
    key: KeyEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            state.current_view = CurrentView::DriveExplorer;
        }
        KeyCode::Char('u') => {
            let should_update = match &state.current_view {
                CurrentView::SelfUpdateModal(ref u) => {
                    u.info.as_ref().map(|i| i.update_available).unwrap_or(false) && !u.updating
                }
                _ => false,
            };

            if should_update {
                if let CurrentView::SelfUpdateModal(ref mut u) = state.current_view {
                    u.updating = true;
                }
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = execute_self_update(None, None)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx_c.send(AppEvent::UpdateFinished(res));
                });
            }
        }
        _ => {}
    }
}

async fn handle_confirm_keys(state: &mut AppState, key: KeyEvent, tx: &UnboundedSender<AppEvent>) {
    let confirmed = matches!(
        key.code,
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter
    );
    let cancelled = matches!(
        key.code,
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc
    );

    if cancelled {
        state.current_view = CurrentView::DriveExplorer;
        return;
    }

    if confirmed {
        let action = match &state.current_view {
            CurrentView::ConfirmModal(ref a) => a.clone(),
            _ => return,
        };

        state.current_view = CurrentView::DriveExplorer;
        state.loading = true;
        match action.action_type {
            ConfirmType::TrashFile { id, .. } => {
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.trash_file(&id).await;
                        let msg = res
                            .map(|f| format!("Moved \"{}\" to trash", f.name))
                            .map_err(|e| e.to_string());
                        let _ = tx_c.send(AppEvent::OperationComplete(msg));
                    });
                }
            }
            ConfirmType::RestoreFile { id, .. } => {
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.restore_trash_file(&id).await;
                        let msg = res
                            .map(|f| format!("Restored \"{}\"", f.name))
                            .map_err(|e| e.to_string());
                        let _ = tx_c.send(AppEvent::OperationComplete(msg));
                    });
                }
            }
            ConfirmType::EmptyTrash => {
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.empty_trash().await;
                        let msg = res
                            .map(|_| "Trash emptied successfully.".to_string())
                            .map_err(|e| e.to_string());
                        let _ = tx_c.send(AppEvent::OperationComplete(msg));
                    });
                }
            }
            ConfirmType::DeletePermission {
                file_id,
                permission_id,
                grantee,
            } => {
                if let Some(ref client) = state.client {
                    let client_c = client.clone();
                    let tx_c = tx.clone();
                    tokio::spawn(async move {
                        let res = client_c.delete_permission(&file_id, &permission_id).await;
                        let msg = res
                            .map(|_| format!("Revoked permission for {}", grantee))
                            .map_err(|e| e.to_string());
                        let _ = tx_c.send(AppEvent::OperationComplete(msg));
                    });
                }
            }
            ConfirmType::DeleteProfile { name } => {
                let prof_name = name.clone();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = AuthManager::remove_profile(&prof_name).map_err(|e| e.to_string());
                    let msg = res.map(|_| format!("Deleted profile {}", prof_name));
                    let _ = tx_c.send(AppEvent::OperationComplete(msg));
                });
            }
            _ => {}
        }
    }
}

async fn handle_app_event(
    state: &mut AppState,
    app_event: AppEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    if handle_loaded_event(state, &app_event, tx) {
        return;
    }

    if handle_modal_update_event(state, &app_event) {
        return;
    }

    handle_lifecycle_event(state, app_event, tx).await;
}

fn handle_loaded_event(
    state: &mut AppState,
    app_event: &AppEvent,
    tx: &UnboundedSender<AppEvent>,
) -> bool {
    match app_event {
        AppEvent::FilesLoaded { folder_id, result } => {
            state.loading = false;
            if folder_id.as_deref() == Some(&state.current_folder_id) {
                match result {
                    Ok(files) => {
                        state.folder_items = files.clone();
                        state.selected_index = 0;
                        state.table_scroll = 0;
                        trigger_preview_load_if_needed(state, tx);
                    }
                    Err(e) => {
                        state.set_toast(format!("Error loading folder: {}", e), true);
                    }
                }
            }
            true
        }
        AppEvent::SearchResultsLoaded { query_desc, result } => {
            state.loading = false;
            match result {
                Ok(files) => {
                    state.search_results = Some(files.clone());
                    state.search_query_desc = Some(query_desc.clone());
                    state.selected_index = 0;
                    state.table_scroll = 0;
                    trigger_preview_load_if_needed(state, tx);
                }
                Err(e) => {
                    state.set_toast(format!("Search failed: {}", e), true);
                }
            }
            true
        }
        AppEvent::PreviewLoaded { file_id, result } => {
            if state.current_selected_file().map(|f| &f.id) == Some(file_id) {
                state.preview_loading = false;
                match result {
                    Ok(text) => state.preview_content = Some(text.clone()),
                    Err(e) => state.preview_content = Some(format!("Error loading preview: {}", e)),
                }
            }
            true
        }
        AppEvent::FolderPeekLoaded { folder_id, files } => {
            state.folder_peek_loading = None;
            state
                .folder_peek_cache
                .insert(folder_id.clone(), files.clone());
            true
        }
        _ => false,
    }
}

fn handle_modal_update_event(state: &mut AppState, app_event: &AppEvent) -> bool {
    match app_event {
        AppEvent::AboutLoaded(res) => {
            if let Ok(about) = res {
                state.storage_quota = about.storage_quota.clone();
                state.user_info = about.user.clone();
            }
            true
        }
        AppEvent::DrivesLoaded(res) => {
            if let Ok(drives) = res {
                state.drives_list = drives.clone();
            }
            true
        }
        AppEvent::ProfilesLoaded(res) => {
            if let Ok(profiles) = res {
                state.available_profiles = profiles.clone();
            }
            true
        }
        AppEvent::DoctorReportLoaded(report) => {
            if let CurrentView::DoctorModal(d) = &mut state.current_view {
                d.report = Some(report.clone());
                d.loading = false;
            }
            true
        }
        AppEvent::MoveFoldersLoaded {
            folder_id,
            subfolders,
        } => {
            if let CurrentView::FileOpModal(FileOpState::Move {
                ref current_folder_id,
                subfolders: ref mut subs,
                ref mut loading,
                ..
            }) = state.current_view
            {
                if current_folder_id == folder_id {
                    *subs = subfolders.clone();
                    *loading = false;
                }
            }
            true
        }
        AppEvent::MoveSearchLoaded { query: _, results } => {
            if let CurrentView::FileOpModal(FileOpState::Move {
                search_results: ref mut s_res,
                ref mut loading,
                ..
            }) = state.current_view
            {
                *s_res = Some(results.clone());
                *loading = false;
            }
            true
        }
        AppEvent::BatchDownloadProgress {
            completed_files,
            total_files: _,
            current_file_name,
            current_downloaded,
            current_total,
        } => {
            if let CurrentView::FileOpModal(FileOpState::BatchDownload {
                completed_files: ref mut comp,
                current_file_name: ref mut name,
                downloaded_in_file: ref mut dl,
                total_in_file: ref mut tot,
                ..
            }) = state.current_view
            {
                *comp = *completed_files;
                *name = current_file_name.clone();
                *dl = *current_downloaded;
                *tot = *current_total;
            }
            true
        }
        AppEvent::DownloadProgress { downloaded, total } => {
            if let CurrentView::FileOpModal(FileOpState::Download {
                downloaded: ref mut d,
                total: ref mut t,
                ..
            }) = state.current_view
            {
                *d = *downloaded;
                *t = *total;
            }
            true
        }
        AppEvent::TrashLoaded(res) => {
            if let CurrentView::TrashView(ref mut trash_st) = state.current_view {
                trash_st.loading = false;
                match res {
                    Ok(items) => {
                        trash_st.items = items.clone();
                        trash_st.selected_index = 0;
                    }
                    Err(e) => {
                        state.set_toast(format!("Error loading trash: {}", e), true);
                    }
                }
            }
            true
        }
        AppEvent::PermissionsLoaded(res) => {
            if let CurrentView::PermissionsModal(ref mut perm_st) = state.current_view {
                perm_st.loading = false;
                match res {
                    Ok(perms) => {
                        perm_st.permissions = perms.clone();
                        perm_st.selected_index = 0;
                    }
                    Err(e) => {
                        state.set_toast(format!("Error loading permissions: {}", e), true);
                    }
                }
            }
            true
        }
        AppEvent::CommentsLoaded(res) => {
            if let CurrentView::CommentsModal(ref mut comm_st) = state.current_view {
                comm_st.loading = false;
                match res {
                    Ok(comments) => {
                        comm_st.comments = comments.clone();
                        comm_st.selected_index = 0;
                    }
                    Err(e) => {
                        state.set_toast(format!("Error loading comments: {}", e), true);
                    }
                }
            }
            true
        }
        AppEvent::RevisionsLoaded(res) => {
            if let CurrentView::RevisionsModal(ref mut rev_st) = state.current_view {
                rev_st.loading = false;
                match res {
                    Ok(revisions) => {
                        rev_st.revisions = revisions.clone();
                        rev_st.selected_index = 0;
                    }
                    Err(e) => {
                        state.set_toast(format!("Error loading revisions: {}", e), true);
                    }
                }
            }
            true
        }
        _ => false,
    }
}

async fn handle_lifecycle_event(
    state: &mut AppState,
    app_event: AppEvent,
    tx: &UnboundedSender<AppEvent>,
) {
    match app_event {
        AppEvent::OperationComplete(res) => {
            state.loading = false;
            match res {
                Ok(msg) => {
                    state.set_toast(msg, false);
                    state.clear_tags();
                    state.folder_peek_cache.clear();
                    state.folder_peek_loading = None;
                    if let Some(ref client) = state.client {
                        spawn_load_folder(
                            client.clone(),
                            state.current_folder_id.clone(),
                            tx.clone(),
                        );
                    }
                }
                Err(e) => {
                    state.set_toast(format!("Error: {}", e), true);
                }
            }
        }
        AppEvent::AuthUrlReady(url) => {
            if let CurrentView::OnboardingWizard(ref mut wiz) = state.current_view {
                wiz.auth_url = Some(url);
            }
        }
        AppEvent::AuthFinished(res) => match res {
            Ok(_) => {
                state.set_toast(
                    "Authentication successful! Welcome to Google Drive.".to_string(),
                    false,
                );
                state.current_view = CurrentView::DriveExplorer;
                if let Ok(client) = GDriveClient::from_profile(Some(&state.current_profile)).await {
                    state.client = Some(client.clone());
                    state.loading = true;
                    spawn_load_folder(client.clone(), state.current_folder_id.clone(), tx.clone());
                    spawn_load_drives(client.clone(), tx.clone());
                    spawn_load_about(client.clone(), tx.clone());
                    spawn_load_profiles(tx.clone());
                }
            }
            Err(e) => {
                if let CurrentView::OnboardingWizard(ref mut wiz) = state.current_view {
                    wiz.step = 0;
                    wiz.loading = false;
                    wiz.error_message = Some(format!("Authentication failed: {}", e));
                }
            }
        },
        AppEvent::UpdateCheckFinished(res) => {
            if let CurrentView::SelfUpdateModal(ref mut update_st) = state.current_view {
                update_st.checking = false;
                match res {
                    Ok(info) => update_st.info = Some(info),
                    Err(e) => update_st.error = Some(e),
                }
            }
        }
        AppEvent::UpdateFinished(res) => {
            if let CurrentView::SelfUpdateModal(ref mut update_st) = state.current_view {
                update_st.updating = false;
                match res {
                    Ok(success) => update_st.success = Some(success),
                    Err(e) => update_st.error = Some(e),
                }
            }
        }
        _ => {}
    }
}

// Background Task Spawners

fn spawn_load_folder(client: GDriveClient, folder_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let opts = SearchFileOptions {
            query: None,
            folder_id: Some(folder_id.clone()),
            page_size: Some(100),
            page_token: None,
            full_text: false,
            order_by: Some("folder,name,modifiedTime desc".to_string()),
        };
        let res = client
            .search_files_with_options(opts)
            .await
            .map(|l| l.files)
            .map_err(|e| e.to_string());

        let _ = tx.send(AppEvent::FilesLoaded {
            folder_id: Some(folder_id),
            result: res,
        });
    });
}

fn spawn_load_category(
    client: GDriveClient,
    query_str: String,
    folder_id: String,
    tx: UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        let opts = SearchFileOptions {
            query: Some(query_str),
            folder_id: None,
            page_size: Some(100),
            page_token: None,
            full_text: false,
            order_by: Some("folder,name,modifiedTime desc".to_string()),
        };
        let res = client
            .search_files_with_options(opts)
            .await
            .map(|l| l.files)
            .map_err(|e| e.to_string());

        let _ = tx.send(AppEvent::FilesLoaded {
            folder_id: Some(folder_id),
            result: res,
        });
    });
}

fn spawn_load_folder_peek(client: GDriveClient, folder_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let opts = SearchFileOptions {
            query: None,
            folder_id: Some(folder_id.clone()),
            page_size: Some(15),
            page_token: None,
            full_text: false,
            order_by: Some("folder,name,modifiedTime desc".to_string()),
        };
        if let Ok(res) = client.search_files_with_options(opts).await {
            let _ = tx.send(AppEvent::FolderPeekLoaded {
                folder_id,
                files: res.files,
            });
        }
    });
}

fn spawn_load_preview(client: GDriveClient, file_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client.read_file(&file_id).await.map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::PreviewLoaded {
            file_id,
            result: res,
        });
    });
}

fn spawn_load_drives(client: GDriveClient, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client.list_drives(None).await.map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::DrivesLoaded(res));
    });
}

fn spawn_load_about(client: GDriveClient, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client.get_about().await.map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::AboutLoaded(res));
    });
}

fn spawn_load_profiles(tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = AuthManager::get_all_profiles_status().map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::ProfilesLoaded(res));
    });
}

fn spawn_load_trash(client: GDriveClient, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client
            .list_trash(Some(100), None)
            .await
            .map(|l| l.files)
            .map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::TrashLoaded(res));
    });
}

fn spawn_load_permissions(client: GDriveClient, file_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client
            .list_permissions(&file_id)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::PermissionsLoaded(res));
    });
}

fn spawn_load_comments(client: GDriveClient, file_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client
            .list_comments(&file_id, Some(50))
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::CommentsLoaded(res));
    });
}

fn spawn_load_revisions(client: GDriveClient, file_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = client
            .list_revisions(&file_id)
            .await
            .map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::RevisionsLoaded(res));
    });
}

fn spawn_search(
    client: GDriveClient,
    query: String,
    full_text: bool,
    order_by: Option<String>,
    tx: UnboundedSender<AppEvent>,
) {
    tokio::spawn(async move {
        let opts = SearchFileOptions {
            query: Some(query.clone()),
            folder_id: None,
            page_size: Some(100),
            page_token: None,
            full_text,
            order_by,
        };
        let res = client.search_files_with_options(opts).await;
        match res {
            Ok(list) => {
                let desc = format!(
                    "\"{}\"{}",
                    query,
                    if full_text { " (fullText)" } else { "" }
                );
                let _ = tx.send(AppEvent::SearchResultsLoaded {
                    query_desc: desc,
                    result: Ok(list.files),
                });
            }
            Err(e) => {
                let desc = format!("\"{}\"", query);
                let _ = tx.send(AppEvent::SearchResultsLoaded {
                    query_desc: desc,
                    result: Err(e.to_string()),
                });
            }
        }
    });
}

fn spawn_check_update(tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let res = check_for_update(None).await.map_err(|e| e.to_string());
        let _ = tx.send(AppEvent::UpdateCheckFinished(res));
    });
}
fn spawn_load_move_folders(client: GDriveClient, folder_id: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let opts = SearchFileOptions {
            query: Some("mimeType = 'application/vnd.google-apps.folder'".to_string()),
            folder_id: Some(folder_id.clone()),
            page_size: Some(100),
            page_token: None,
            full_text: false,
            order_by: Some("name asc".to_string()),
        };
        match client.search_files_with_options(opts).await {
            Ok(res) => {
                let _ = tx.send(AppEvent::MoveFoldersLoaded {
                    folder_id,
                    subfolders: res.files,
                });
            }
            Err(_) => {
                let _ = tx.send(AppEvent::MoveFoldersLoaded {
                    folder_id,
                    subfolders: Vec::new(),
                });
            }
        }
    });
}
fn spawn_search_folders(client: GDriveClient, query_name: String, tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let escaped = query_name.trim().replace('\\', "\\\\").replace('\'', "\\'");
        let q = format!("mimeType = 'application/vnd.google-apps.folder' and name contains '{}' and trashed = false", escaped);
        let opts = SearchFileOptions {
            query: Some(q),
            folder_id: None,
            page_size: Some(50),
            page_token: None,
            full_text: false,
            order_by: Some("name asc".to_string()),
        };
        let folders = client
            .search_files_with_options(opts)
            .await
            .map(|l| l.files)
            .unwrap_or_default();
        let _ = tx.send(AppEvent::MoveSearchLoaded {
            query: query_name,
            results: folders,
        });
    });
}
