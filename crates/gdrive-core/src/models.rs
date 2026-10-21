use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(rename = "webViewLink", default)]
    pub web_view_link: Option<String>,
    #[serde(default)]
    pub parents: Option<Vec<String>>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(rename = "modifiedTime", default)]
    pub modified_time: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFileList {
    #[serde(default)]
    pub files: Vec<DriveFile>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchFileOptions {
    pub query: Option<String>,
    pub folder_id: Option<String>,
    pub page_size: Option<u32>,
    pub page_token: Option<String>,
    pub full_text: bool,
    pub order_by: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateFileOptions {
    pub name: String,
    pub content: String,
    pub parent_folder_id: Option<String>,
    pub as_google_doc: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateFileOptions {
    pub file_id: String,
    pub content: String,
    pub append: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveItemList {
    #[serde(default)]
    pub drives: Vec<DriveItem>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}
