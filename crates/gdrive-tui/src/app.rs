use gdrive_core::{
    DriveAbout, DriveAboutStorageQuota, DriveAboutUser, DriveComment, DriveFile, DriveItem,
    DrivePermission, DriveRevision, GDriveClient, ProfileStatusInfo, TokenData, UpdateInfo,
    UpdateSuccess,
};
use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::input::InputBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Default,
    NameAsc,
    NameDesc,
    DateDesc,
    DateAsc,
    SizeDesc,
    SizeAsc,
}

impl SortMode {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Default => Self::NameAsc,
            Self::NameAsc => Self::NameDesc,
            Self::NameDesc => Self::DateDesc,
            Self::DateDesc => Self::DateAsc,
            Self::DateAsc => Self::SizeDesc,
            Self::SizeDesc => Self::SizeAsc,
            Self::SizeAsc => Self::Default,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::NameAsc => "Name (A-Z)",
            Self::NameDesc => "Name (Z-A)",
            Self::DateDesc => "Date (Newest)",
            Self::DateAsc => "Date (Oldest)",
            Self::SizeDesc => "Size (Largest)",
            Self::SizeAsc => "Size (Smallest)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriveCategory {
    MyDrive,
    SharedWithMe,
    Starred,
    SharedDrive { id: String, name: String },
}

#[derive(Debug, Clone)]
pub enum ConfirmType {
    TrashFile {
        id: String,
        name: String,
    },
    RestoreFile {
        id: String,
        name: String,
    },
    EmptyTrash,
    DeletePermission {
        file_id: String,
        permission_id: String,
        grantee: String,
    },
    SwitchProfile {
        name: String,
    },
    DeleteProfile {
        name: String,
    },
    LogoutProfile,
}

#[derive(Debug, Clone)]
pub struct ConfirmAction {
    pub title: String,
    pub prompt: String,
    pub action_type: ConfirmType,
}

#[derive(Debug, Clone, Default)]
pub struct WizardState {
    pub step: usize, // 0: enter name & select mode, 1: manual credentials form, 2: waiting for browser/auth, 3: finished
    pub profile_name: InputBuffer,
    pub client_id_input: InputBuffer,
    pub client_secret_input: InputBuffer,
    pub credentials_path_input: InputBuffer,
    pub has_existing_credentials: bool,
    pub auth_url: Option<String>,
    pub active_field: usize, // 0: client_id, 1: client_secret, 2: file_path
    pub error_message: Option<String>,
    pub loading: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SearchModalState {
    pub query: InputBuffer,
    pub full_text: bool,
    pub order_by_index: usize, // 0: default, 1: modifiedTime desc, 2: name asc, 3: quotaBytesUsed desc
    pub active_field: usize,   // 0: query, 1: full_text checkbox, 2: order_by
}

#[derive(Debug, Clone)]
pub enum FileOpState {
    Create {
        name: InputBuffer,
        content: InputBuffer,
        as_doc: bool,
        active_field: usize, // 0: name, 1: content, 2: as_doc checkbox
    },
    CreateFolder {
        name: InputBuffer,
    },
    Rename {
        file_id: String,
        old_name: String,
        new_name: InputBuffer,
    },
    Move {
        items_to_move: Vec<(String, String)>,
        current_folder_id: String,
        current_folder_name: String,
        path_stack: Vec<(String, String)>,
        subfolders: Vec<DriveFile>,
        search_query: InputBuffer,
        search_results: Option<Vec<DriveFile>>,
        selected_index: usize,
        active_field: usize,
        loading: bool,
    },
    Copy {
        file_id: String,
        old_name: String,
        new_name: InputBuffer,
        target_folder_id: InputBuffer,
        active_field: usize, // 0: new_name, 1: target_folder_id
    },
    Upload {
        local_path: InputBuffer,
        dest_name: InputBuffer,
        convert_doc: bool,
        active_field: usize, // 0: local_path, 1: dest_name, 2: convert_doc
    },
    Download {
        file_id: String,
        file_name: String,
        dest_path: InputBuffer,
        downloaded: u64,
        total: Option<u64>,
        in_progress: bool,
    },
    BatchDownload {
        files: Vec<DriveFile>,
        dest_dir: InputBuffer,
        as_zip: bool,
        current_file_idx: usize,
        current_file_name: String,
        downloaded_in_file: u64,
        total_in_file: Option<u64>,
        completed_files: usize,
        in_progress: bool,
    },
}

#[derive(Debug, Clone)]
pub struct PermissionsState {
    pub file_id: String,
    pub file_name: String,
    pub permissions: Vec<DrivePermission>,
    pub selected_index: usize,
    pub adding: bool,
    pub add_role_index: usize, // 0: reader, 1: commenter, 2: writer
    pub add_type_index: usize, // 0: user, 1: group, 2: domain, 3: anyone
    pub add_email: InputBuffer,
    pub active_field: usize,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub struct CommentsState {
    pub file_id: String,
    pub file_name: String,
    pub comments: Vec<DriveComment>,
    pub selected_index: usize,
    pub adding: bool,
    pub new_comment_text: InputBuffer,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub struct RevisionsState {
    pub file_id: String,
    pub file_name: String,
    pub revisions: Vec<DriveRevision>,
    pub selected_index: usize,
    pub downloading: bool,
    pub loading: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TrashViewState {
    pub items: Vec<DriveFile>,
    pub selected_index: usize,
    pub loading: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateModalState {
    pub checking: bool,
    pub updating: bool,
    pub info: Option<UpdateInfo>,
    pub success: Option<UpdateSuccess>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DoctorModalState {
    pub report: Option<gdrive_core::DoctorReport>,
    pub loading: bool,
}

#[derive(Debug, Clone)]
pub enum CurrentView {
    DriveExplorer,
    OnboardingWizard(WizardState),
    ProfileModal,
    SearchModal(SearchModalState),
    FileOpModal(FileOpState),
    PermissionsModal(PermissionsState),
    CommentsModal(CommentsState),
    RevisionsModal(RevisionsState),
    TrashView(TrashViewState),
    AboutModal,
    SelfUpdateModal(UpdateModalState),
    ConfirmModal(ConfirmAction),
    HelpModal,
    DoctorModal(DoctorModalState),
}

#[derive(Debug)]
pub enum AppEvent {
    Key(crossterm::event::KeyEvent),
    Tick,
    FilesLoaded {
        folder_id: Option<String>,
        result: Result<Vec<DriveFile>, String>,
    },
    SearchResultsLoaded {
        query_desc: String,
        result: Result<Vec<DriveFile>, String>,
    },
    PreviewLoaded {
        file_id: String,
        result: Result<String, String>,
    },
    FolderPeekLoaded {
        folder_id: String,
        files: Vec<DriveFile>,
    },
    MoveFoldersLoaded {
        folder_id: String,
        subfolders: Vec<DriveFile>,
    },
    MoveSearchLoaded {
        query: String,
        results: Vec<DriveFile>,
    },
    BatchDownloadProgress {
        completed_files: usize,
        total_files: usize,
        current_file_name: String,
        current_downloaded: u64,
        current_total: Option<u64>,
    },
    AboutLoaded(Result<DriveAbout, String>),
    DrivesLoaded(Result<Vec<DriveItem>, String>),
    ProfilesLoaded(Result<Vec<ProfileStatusInfo>, String>),
    DoctorReportLoaded(gdrive_core::DoctorReport),
    OperationComplete(Result<String, String>),
    DownloadProgress {
        downloaded: u64,
        total: Option<u64>,
    },
    TrashLoaded(Result<Vec<DriveFile>, String>),
    PermissionsLoaded(Result<Vec<DrivePermission>, String>),
    CommentsLoaded(Result<Vec<DriveComment>, String>),
    RevisionsLoaded(Result<Vec<DriveRevision>, String>),
    AuthUrlReady(String),
    AuthFinished(Result<TokenData, String>),
    UpdateCheckFinished(Result<UpdateInfo, String>),
    UpdateFinished(Result<UpdateSuccess, String>),
}

pub struct AppState {
    pub client: Option<GDriveClient>,
    pub current_profile: String,
    pub available_profiles: Vec<ProfileStatusInfo>,
    pub profile_modal_selected_index: usize,
    pub current_folder_id: String,
    pub folder_path_stack: Vec<(String, String)>, // (name, id)
    pub folder_items: Vec<DriveFile>,
    pub filter_query: InputBuffer,
    pub filtering: bool,
    pub search_results: Option<Vec<DriveFile>>,
    pub search_query_desc: Option<String>,
    pub selected_index: usize,
    pub table_scroll: usize,
    pub sort_mode: SortMode,
    pub preview_content: Option<String>,
    pub preview_file_id: Option<String>,
    pub preview_scroll: u16,
    pub preview_loading: bool,
    pub folder_peek_cache: HashMap<String, Vec<DriveFile>>,
    pub folder_peek_loading: Option<String>,
    pub loading: bool,
    pub status_toast: Option<(String, bool, Instant)>, // message, is_error, timestamp
    pub storage_quota: Option<DriveAboutStorageQuota>,
    pub user_info: Option<DriveAboutUser>,
    pub drives_list: Vec<DriveItem>,
    pub selected_drive_tab: usize, // 0 = My Drive, 1 = Shared with Me, 2 = Starred, 3.. = Shared Drives
    pub current_view: CurrentView,
    pub should_exit: bool,
    pub tick_count: usize,
    pub tagged_files: HashSet<String>,
}

impl AppState {
    pub fn new(profile: String, initial_folder: Option<String>) -> Self {
        let initial_folder_id = initial_folder.unwrap_or_else(|| "root".to_string());
        let initial_name = if initial_folder_id == "root" {
            "My Drive".to_string()
        } else {
            format!("Folder ({})", initial_folder_id)
        };

        Self {
            client: None,
            current_profile: profile,
            available_profiles: Vec::new(),
            profile_modal_selected_index: 0,
            current_folder_id: initial_folder_id.clone(),
            folder_path_stack: vec![(initial_name, initial_folder_id)],
            folder_items: Vec::new(),
            filter_query: InputBuffer::default(),
            filtering: false,
            search_results: None,
            search_query_desc: None,
            selected_index: 0,
            table_scroll: 0,
            sort_mode: SortMode::Default,
            preview_content: None,
            preview_file_id: None,
            preview_scroll: 0,
            preview_loading: false,
            folder_peek_cache: HashMap::new(),
            folder_peek_loading: None,
            loading: false,
            status_toast: None,
            storage_quota: None,
            user_info: None,
            drives_list: Vec::new(),
            selected_drive_tab: 0,
            current_view: CurrentView::DriveExplorer,
            should_exit: false,
            tick_count: 0,
            tagged_files: HashSet::new(),
        }
    }

    pub fn categories(&self) -> Vec<DriveCategory> {
        let mut cats = vec![
            DriveCategory::MyDrive,
            DriveCategory::SharedWithMe,
            DriveCategory::Starred,
        ];
        for d in &self.drives_list {
            cats.push(DriveCategory::SharedDrive {
                id: d.id.clone(),
                name: d.name.clone(),
            });
        }
        cats
    }

    pub fn visible_items(&self) -> Vec<&DriveFile> {
        let items: &[DriveFile] = match self.search_results {
            Some(ref list) => list.as_slice(),
            None => self.folder_items.as_slice(),
        };

        let filter_str = self.filter_query.as_str().to_lowercase();
        let mut filtered: Vec<&DriveFile> = if filter_str.trim().is_empty() {
            items.iter().collect()
        } else {
            items
                .iter()
                .filter(|f| f.name.to_lowercase().contains(&filter_str))
                .collect()
        };

        match self.sort_mode {
            SortMode::Default => {}
            SortMode::NameAsc => {
                filtered.sort_by_key(|a| a.name.to_lowercase());
            }
            SortMode::NameDesc => {
                filtered.sort_by_key(|b| std::cmp::Reverse(b.name.to_lowercase()));
            }
            SortMode::DateDesc => {
                filtered.sort_by(|a, b| b.modified_time.cmp(&a.modified_time));
            }
            SortMode::DateAsc => {
                filtered.sort_by(|a, b| a.modified_time.cmp(&b.modified_time));
            }
            SortMode::SizeDesc => {
                filtered.sort_by(|a, b| {
                    let sz_a = a
                        .size
                        .as_deref()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    let sz_b = b
                        .size
                        .as_deref()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    sz_b.cmp(&sz_a)
                });
            }
            SortMode::SizeAsc => {
                filtered.sort_by(|a, b| {
                    let sz_a = a
                        .size
                        .as_deref()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    let sz_b = b
                        .size
                        .as_deref()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    sz_a.cmp(&sz_b)
                });
            }
        }

        filtered
    }

    pub fn update_table_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected_index < self.table_scroll {
            self.table_scroll = self.selected_index;
        } else if self.selected_index >= self.table_scroll + visible_height {
            self.table_scroll = self
                .selected_index
                .saturating_sub(visible_height.saturating_sub(1));
        }
    }

    pub fn current_selected_file(&self) -> Option<&DriveFile> {
        let items = self.visible_items();
        items.get(self.selected_index).copied()
    }

    pub fn set_toast(&mut self, msg: String, is_error: bool) {
        self.status_toast = Some((msg, is_error, Instant::now()));
    }
    pub fn toggle_tag(&mut self, file_id: String) {
        if self.tagged_files.contains(&file_id) {
            self.tagged_files.remove(&file_id);
        } else {
            self.tagged_files.insert(file_id);
        }
    }

    pub fn toggle_tag_current(&mut self) {
        if let Some(file) = self.current_selected_file() {
            let id = file.id.clone();
            self.toggle_tag(id);
        }
    }

    pub fn clear_tags(&mut self) {
        self.tagged_files.clear();
    }

    pub fn tagged_file_objects(&self) -> Vec<DriveFile> {
        let visible = self.visible_items();
        visible
            .into_iter()
            .filter(|f| self.tagged_files.contains(&f.id))
            .cloned()
            .collect()
    }

    pub fn clear_search_and_filter(&mut self) {
        self.search_results = None;
        self.search_query_desc = None;
        self.filter_query.clear();
        self.filtering = false;
        self.selected_index = 0;
        self.table_scroll = 0;
    }

    pub fn clear_expired_toast(&mut self) {
        if let Some((_, _, time)) = self.status_toast {
            if time.elapsed().as_secs() >= 5 {
                self.status_toast = None;
            }
        }
    }
    pub fn on_tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        self.clear_expired_toast();
    }

    pub fn spinner_frame(&self) -> char {
        const BRAILLE: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        BRAILLE[self.tick_count % BRAILLE.len()]
    }

    pub fn pulsing_color(&self) -> ratatui::style::Color {
        const COLORS: &[ratatui::style::Color] = &[
            ratatui::style::Color::Cyan,
            ratatui::style::Color::LightCyan,
            ratatui::style::Color::Blue,
            ratatui::style::Color::LightBlue,
            ratatui::style::Color::LightMagenta,
            ratatui::style::Color::Magenta,
        ];
        COLORS[self.tick_count % COLORS.len()]
    }

    pub fn select_next(&mut self) {
        let count = self.visible_items().len();
        if count > 0 {
            if self.selected_index + 1 < count {
                self.selected_index += 1;
            } else {
                self.selected_index = 0;
            }
            self.preview_scroll = 0;
        }
    }

    pub fn select_prev(&mut self) {
        let count = self.visible_items().len();
        if count > 0 {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = count - 1;
            }
            self.preview_scroll = 0;
        }
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
        self.preview_scroll = 0;
    }

    pub fn select_last(&mut self) {
        let count = self.visible_items().len();
        if count > 0 {
            self.selected_index = count - 1;
            self.preview_scroll = 0;
        }
    }

    pub fn scroll_preview_down(&mut self, amount: u16) {
        self.preview_scroll = self.preview_scroll.saturating_add(amount);
    }

    pub fn scroll_preview_up(&mut self, amount: u16) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount);
    }

    pub fn enter_selected_folder(&mut self) -> Option<String> {
        if let Some(file) = self.current_selected_file() {
            if file.mime_type == "application/vnd.google-apps.folder" {
                let id = file.id.clone();
                let name = file.name.clone();
                self.folder_path_stack.push((name, id.clone()));
                self.current_folder_id = id.clone();
                self.filter_query.clear();
                self.filtering = false;
                self.search_results = None;
                self.search_query_desc = None;
                self.selected_index = 0;
                self.table_scroll = 0;
                self.preview_content = None;
                self.preview_file_id = None;
                self.preview_scroll = 0;
                return Some(id);
            }
        }
        None
    }

    pub fn navigate_up(&mut self) -> Option<String> {
        if self.folder_path_stack.len() > 1 {
            self.folder_path_stack.pop();
            let (_, parent_id) = self.folder_path_stack.last().cloned().unwrap();
            self.current_folder_id = parent_id.clone();
            self.filter_query.clear();
            self.filtering = false;
            self.search_results = None;
            self.search_query_desc = None;
            self.selected_index = 0;
            self.table_scroll = 0;
            self.preview_content = None;
            self.preview_file_id = None;
            self.preview_scroll = 0;
            return Some(parent_id);
        } else if self.current_folder_id != "root" && self.selected_drive_tab == 0 {
            // Already at top of stack but not root -> reset to root
            self.reset_to_folder("My Drive".to_string(), "root".to_string());
            return Some("root".to_string());
        }
        None
    }

    pub fn reset_to_folder(&mut self, name: String, id: String) {
        self.folder_path_stack = vec![(name, id.clone())];
        self.current_folder_id = id;
        self.filter_query.clear();
        self.filtering = false;
        self.search_results = None;
        self.search_query_desc = None;
        self.selected_index = 0;
        self.table_scroll = 0;
        self.preview_content = None;
        self.preview_file_id = None;
        self.preview_scroll = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_file(
        id: &str,
        name: &str,
        mime: &str,
        size: Option<&str>,
        date: Option<&str>,
    ) -> DriveFile {
        DriveFile {
            id: id.to_string(),
            name: name.to_string(),
            mime_type: mime.to_string(),
            web_view_link: None,
            parents: None,
            size: size.map(ToString::to_string),
            modified_time: date.map(ToString::to_string),
            starred: None,
        }
    }

    #[test]
    fn test_app_state_navigation() {
        let mut state = AppState::new("default".to_string(), None);
        assert_eq!(state.current_folder_id, "root");
        assert_eq!(state.folder_path_stack.len(), 1);

        state.folder_items = vec![
            dummy_file(
                "f1",
                "Folder 1",
                "application/vnd.google-apps.folder",
                None,
                None,
            ),
            dummy_file(
                "doc1",
                "Document 1",
                "application/vnd.google-apps.document",
                Some("1024"),
                None,
            ),
        ];

        assert_eq!(state.visible_items().len(), 2);
        assert_eq!(state.current_selected_file().unwrap().id, "f1");

        // Enter folder
        let entered = state.enter_selected_folder();
        assert_eq!(entered, Some("f1".to_string()));
        assert_eq!(state.current_folder_id, "f1");
        assert_eq!(state.folder_path_stack.len(), 2);

        // Navigate up
        let up = state.navigate_up();
        assert_eq!(up, Some("root".to_string()));
        assert_eq!(state.current_folder_id, "root");
        assert_eq!(state.folder_path_stack.len(), 1);
    }

    #[test]
    fn test_app_state_filtering_and_sorting() {
        let mut state = AppState::new("default".to_string(), None);
        state.folder_items = vec![
            dummy_file(
                "1",
                "Architecture.md",
                "text/markdown",
                Some("500"),
                Some("2026-03-29T10:00:00Z"),
            ),
            dummy_file(
                "2",
                "Notes.txt",
                "text/plain",
                Some("100"),
                Some("2026-03-29T12:00:00Z"),
            ),
            dummy_file(
                "3",
                "Budget.xlsx",
                "application/vnd.ms-excel",
                Some("1000"),
                Some("2026-03-29T08:00:00Z"),
            ),
        ];

        state.filter_query = InputBuffer::new("arch");
        assert_eq!(state.visible_items().len(), 1);
        assert_eq!(state.visible_items()[0].name, "Architecture.md");

        state.filter_query = InputBuffer::default();
        assert_eq!(state.visible_items().len(), 3);

        // Test sorting by name asc
        state.sort_mode = SortMode::NameAsc;
        let sorted = state.visible_items();
        assert_eq!(sorted[0].name, "Architecture.md");
        assert_eq!(sorted[1].name, "Budget.xlsx");
        assert_eq!(sorted[2].name, "Notes.txt");

        // Test sorting by size desc
        state.sort_mode = SortMode::SizeDesc;
        let sorted_sz = state.visible_items();
        assert_eq!(sorted_sz[0].name, "Budget.xlsx"); // 1000
        assert_eq!(sorted_sz[1].name, "Architecture.md"); // 500
        assert_eq!(sorted_sz[2].name, "Notes.txt"); // 100
    }

    #[test]
    fn test_toast_notification() {
        let mut state = AppState::new("default".to_string(), None);
        assert!(state.status_toast.is_none());

        state.set_toast("Operation successful".to_string(), false);
        assert!(state.status_toast.is_some());
        let (msg, is_err, _) = state.status_toast.as_ref().unwrap();
        assert_eq!(msg, "Operation successful");
        assert!(!is_err);
    }
}
