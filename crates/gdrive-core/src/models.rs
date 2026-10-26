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
    #[serde(default)]
    pub starred: Option<bool>,
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
    pub file_type: Option<String>,
    pub starred_only: Option<bool>,
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

/// Node representasi struktur pohon folder Google Drive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveTreeNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub is_folder: bool,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub children: Vec<DriveTreeNode>,
}

/// Item breadcrumb untuk hierarki folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveBreadcrumb {
    pub id: String,
    pub name: String,
}

/// Hasil resolusi jalur lengkap file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFilePathResult {
    pub file_id: String,
    pub name: String,
    pub full_path: String,
    pub breadcrumbs: Vec<DriveBreadcrumb>,
}

/// Satu aksi dalam batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveBatchAction {
    pub action: String,
    pub file_id: String,
    #[serde(default)]
    pub target_folder: Option<String>,
    #[serde(default)]
    pub new_name: Option<String>,
    #[serde(default)]
    pub starred: Option<bool>,
}

/// Hasil dari satu aksi batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveBatchItemResult {
    pub file_id: String,
    pub action: String,
    pub status: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Laporan ringkasan batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveBatchReport {
    pub total_operations: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<DriveBatchItemResult>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrivePermission {
    pub id: String,
    #[serde(rename = "type")]
    pub permission_type: String,
    pub role: String,
    #[serde(rename = "emailAddress", default)]
    pub email_address: Option<String>,
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrivePermissionList {
    #[serde(default)]
    pub permissions: Vec<DrivePermission>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreatePermissionOptions {
    pub role: String,
    pub permission_type: String,
    pub email_address: Option<String>,
    pub domain: Option<String>,
    pub send_notification_email: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveAuthor {
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(rename = "emailAddress", default)]
    pub email_address: Option<String>,
    #[serde(default)]
    pub me: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveComment {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub author: Option<DriveAuthor>,
    #[serde(rename = "createdTime", default)]
    pub created_time: Option<String>,
    #[serde(rename = "modifiedTime", default)]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub resolved: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveCommentList {
    #[serde(default)]
    pub comments: Vec<DriveComment>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveRevision {
    pub id: String,
    #[serde(rename = "mimeType", default)]
    pub mime_type: Option<String>,
    #[serde(rename = "modifiedTime", default)]
    pub modified_time: Option<String>,
    #[serde(rename = "keepForever", default)]
    pub keep_forever: Option<bool>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(rename = "lastModifyingUser", default)]
    pub last_modifying_user: Option<DriveAuthor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveRevisionList {
    #[serde(default)]
    pub revisions: Vec<DriveRevision>,
    #[serde(rename = "nextPageToken", default)]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveAboutUser {
    #[serde(rename = "displayName", default)]
    pub display_name: Option<String>,
    #[serde(rename = "emailAddress", default)]
    pub email_address: Option<String>,
    #[serde(rename = "permissionId", default)]
    pub permission_id: Option<String>,
    #[serde(rename = "photoLink", default)]
    pub photo_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveAboutStorageQuota {
    #[serde(default)]
    pub limit: Option<String>,
    #[serde(default)]
    pub usage: Option<String>,
    #[serde(rename = "usageInDrive", default)]
    pub usage_in_drive: Option<String>,
    #[serde(rename = "usageInDriveTrash", default)]
    pub usage_in_drive_trash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveAbout {
    #[serde(default)]
    pub user: Option<DriveAboutUser>,
    #[serde(rename = "storageQuota", default)]
    pub storage_quota: Option<DriveAboutStorageQuota>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_drive_permission_list() {
        let json_data = r#"{
            "permissions": [
                {
                    "id": "anyoneWithLink",
                    "type": "anyone",
                    "role": "reader"
                },
                {
                    "id": "user-12345",
                    "type": "user",
                    "role": "writer",
                    "emailAddress": "alice@example.com",
                    "displayName": "Alice Smith"
                },
                {
                    "id": "domain-xyz",
                    "type": "domain",
                    "role": "commenter",
                    "domain": "example.com"
                }
            ],
            "nextPageToken": "token-next-page"
        }"#;

        let list: DrivePermissionList = serde_json::from_str(json_data).unwrap();
        assert_eq!(list.permissions.len(), 3);
        assert_eq!(list.next_page_token.as_deref(), Some("token-next-page"));

        let p0 = &list.permissions[0];
        assert_eq!(p0.id, "anyoneWithLink");
        assert_eq!(p0.permission_type, "anyone");
        assert_eq!(p0.role, "reader");
        assert_eq!(p0.email_address, None);

        let p1 = &list.permissions[1];
        assert_eq!(p1.id, "user-12345");
        assert_eq!(p1.permission_type, "user");
        assert_eq!(p1.role, "writer");
        assert_eq!(p1.email_address.as_deref(), Some("alice@example.com"));
        assert_eq!(p1.display_name.as_deref(), Some("Alice Smith"));

        let p2 = &list.permissions[2];
        assert_eq!(p2.id, "domain-xyz");
        assert_eq!(p2.permission_type, "domain");
        assert_eq!(p2.role, "commenter");
        assert_eq!(p2.domain.as_deref(), Some("example.com"));
    }

    #[test]
    fn test_deserialize_drive_comment_list() {
        let json_data = r#"{
            "comments": [
                {
                    "id": "comment-1",
                    "content": "Please review this section",
                    "author": {
                        "displayName": "Bob Builder",
                        "emailAddress": "bob@example.com",
                        "me": true
                    },
                    "createdTime": "2026-03-30T10:00:00.000Z",
                    "resolved": false
                }
            ],
            "nextPageToken": "comment-token"
        }"#;

        let list: DriveCommentList = serde_json::from_str(json_data).unwrap();
        assert_eq!(list.comments.len(), 1);
        assert_eq!(list.next_page_token.as_deref(), Some("comment-token"));
        let c = &list.comments[0];
        assert_eq!(c.id, "comment-1");
        assert_eq!(c.content, "Please review this section");
        assert_eq!(c.resolved, Some(false));
        let author = c.author.as_ref().unwrap();
        assert_eq!(author.display_name.as_deref(), Some("Bob Builder"));
        assert_eq!(author.me, Some(true));
    }

    #[test]
    fn test_deserialize_drive_revision_list() {
        let json_data = r#"{
            "revisions": [
                {
                    "id": "rev-1",
                    "mimeType": "application/pdf",
                    "modifiedTime": "2026-03-29T12:00:00.000Z",
                    "keepForever": true,
                    "size": "1048576",
                    "lastModifyingUser": {
                        "displayName": "Alice Admin",
                        "emailAddress": "alice@admin.org"
                    }
                }
            ]
        }"#;

        let list: DriveRevisionList = serde_json::from_str(json_data).unwrap();
        assert_eq!(list.revisions.len(), 1);
        let r = &list.revisions[0];
        assert_eq!(r.id, "rev-1");
        assert_eq!(r.keep_forever, Some(true));
        assert_eq!(r.size.as_deref(), Some("1048576"));
        let user = r.last_modifying_user.as_ref().unwrap();
        assert_eq!(user.display_name.as_deref(), Some("Alice Admin"));
    }

    #[test]
    fn test_about_model_deserialization() {
        let json_data = r#"{
            "user": {
                "displayName": "Ada Lovelace",
                "emailAddress": "ada@example.com",
                "permissionId": "perm-123",
                "photoLink": "https://example.com/photo.jpg"
            },
            "storageQuota": {
                "limit": "16106127360",
                "usage": "5368709120",
                "usageInDrive": "4294967296",
                "usageInDriveTrash": "1073741824"
            }
        }"#;

        let about: DriveAbout = serde_json::from_str(json_data).unwrap();
        let user = about.user.as_ref().unwrap();
        assert_eq!(user.display_name.as_deref(), Some("Ada Lovelace"));
        assert_eq!(user.email_address.as_deref(), Some("ada@example.com"));
        assert_eq!(user.permission_id.as_deref(), Some("perm-123"));

        let quota = about.storage_quota.as_ref().unwrap();
        assert_eq!(quota.limit.as_deref(), Some("16106127360"));
        assert_eq!(quota.usage.as_deref(), Some("5368709120"));
        assert_eq!(quota.usage_in_drive.as_deref(), Some("4294967296"));
        assert_eq!(quota.usage_in_drive_trash.as_deref(), Some("1073741824"));
    }
}
