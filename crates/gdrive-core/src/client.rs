use crate::auth::AuthManager;
use crate::models::{
    CreateFileOptions, DriveFile, DriveFileList, SearchFileOptions, UpdateFileOptions,
};
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures_util::Stream;
use std::sync::Arc;
use tokio::sync::RwLock;

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

/// Representasi stream download file beserta metadata
pub struct DownloadStream {
    pub name: String,
    pub size: Option<u64>,
    pub mime_type: String,
    pub stream: Box<dyn Stream<Item = reqwest::Result<Bytes>> + Unpin + Send>,
}

#[derive(Clone)]
pub struct GDriveClient {
    http: reqwest::Client,
    access_token: Arc<RwLock<String>>,
    auto_refresh: bool,
}

pub fn is_transient_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

impl GDriveClient {
    pub fn new(access_token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(access_token)),
            auto_refresh: false,
        }
    }

    pub fn with_auto_refresh(mut self, enabled: bool) -> Self {
        self.auto_refresh = enabled;
        self
    }

    /// Membuat instance client dari token yang tersimpan di disk (dengan proactive refresh)
    pub async fn from_saved_token() -> Result<Self> {
        let token = AuthManager::get_valid_token().await?;
        Ok(Self {
            http: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(token)),
            auto_refresh: true,
        })
    }

    /// Mengambil access token aktif saat ini (otomatis refresh proaktif jika sudah kedaluwarsa)
    pub async fn get_token(&self) -> String {
        if self.auto_refresh {
            if let Ok(valid) = AuthManager::get_valid_token().await {
                let mut lock = self.access_token.write().await;
                *lock = valid.clone();
                return valid;
            }
        }
        let lock = self.access_token.read().await;
        lock.clone()
    }

    /// Refresh reaktif jika mendeteksi HTTP 401 Unauthorized
    pub async fn force_refresh(&self) -> Result<String> {
        if self.auto_refresh {
            let refreshed = AuthManager::force_refresh().await?;
            let mut lock = self.access_token.write().await;
            *lock = refreshed.clone();
            Ok(refreshed)
        } else {
            bail!("401 Unauthorized dan auto-refresh dinonaktifkan.");
        }
    }

    /// Mengirim request dengan mekanisme retry otomatis:
    /// 1. HTTP 401 Unauthorized -> Refresh token dan coba kembali
    /// 2. Transient status (HTTP 429, 500, 502, 503, 504) atau network failure -> Exponential backoff hingga 3x
    pub async fn send_with_retry<F>(&self, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        const MAX_RETRIES: u32 = 3;
        let mut attempt = 0;
        let mut refreshed = false;
        let mut backoff = std::time::Duration::from_millis(500);

        loop {
            let token = self.get_token().await;
            let resp_result = build(&token).send().await;

            match resp_result {
                Ok(resp) => {
                    // Cek jika 401 Unauthorized (refresh token sekali dan lanjutkan retry loop)
                    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
                        && self.auto_refresh
                        && !refreshed
                    {
                        refreshed = true;
                        if self.force_refresh().await.is_ok() {
                            continue;
                        }
                    }

                    // Cek transient status (429, 5xx)
                    if is_transient_status(resp.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        tracing::warn!(
                            "Menerima status {} dari Google Drive API. Mencoba ulang ({}/{})...",
                            resp.status(),
                            attempt,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }

                    return Ok(resp);
                }
                Err(err) => {
                    // Network IO / timeout errors
                    if (err.is_connect() || err.is_timeout() || err.is_request())
                        && attempt < MAX_RETRIES
                    {
                        attempt += 1;
                        tracing::warn!(
                            "Gagal koneksi ke Google Drive API: {}. Mencoba ulang ({}/{})...",
                            err,
                            attempt,
                            MAX_RETRIES
                        );
                        tokio::time::sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }

    /// Membangun query string pencarian Google Drive yang aman dan resilient
    pub fn build_search_query(opts: &SearchFileOptions) -> String {
        let mut q_parts = Vec::new();
        q_parts.push("trashed = false".to_string());

        if let Some(folder) = &opts.folder_id {
            let clean_folder = Self::clean_file_id(folder);
            q_parts.push(format!("'{}' in parents", clean_folder));
        }

        if let Some(q) = &opts.query {
            let trimmed = q.trim();
            if !trimmed.is_empty() {
                let lower = trimmed.to_lowercase();
                let has_operator = lower.contains(" contains ")
                    || lower.contains(" = ")
                    || lower.contains(" != ")
                    || lower.contains(" in ")
                    || lower.contains(" and ")
                    || lower.contains(" or ")
                    || lower.contains("not ")
                    || lower.contains("has ")
                    || lower.starts_with("mimetype")
                    || lower.starts_with("name")
                    || lower.starts_with("fulltext")
                    || lower.starts_with("parents");

                if has_operator {
                    q_parts.push(trimmed.to_string());
                } else {
                    let escaped = trimmed.replace('\\', "\\\\").replace('\'', "\\'");
                    if opts.full_text {
                        q_parts.push(format!(
                            "(name contains '{escaped}' or fullText contains '{escaped}')"
                        ));
                    } else {
                        q_parts.push(format!("name contains '{escaped}'"));
                    }
                }
            }
        }

        q_parts.join(" and ")
    }

    /// Mencari file atau folder di Google Drive dengan opsi lengkap (pagination, sorting, fullText)
    pub async fn search_files_with_options(
        &self,
        opts: SearchFileOptions,
    ) -> Result<DriveFileList> {
        let full_q = Self::build_search_query(&opts);
        let limit = opts.page_size.unwrap_or(20);
        let order = opts
            .order_by
            .unwrap_or_else(|| "folder,name,modifiedTime desc".to_string());

        let url = format!("{}/files", DRIVE_API_BASE);
        let resp = self
            .send_with_retry(|t| {
                let mut req = self.http.get(&url).bearer_auth(t).query(&[
                    ("q", full_q.as_str()),
                    ("pageSize", &limit.to_string()),
                    (
                        "fields",
                        "nextPageToken, files(id, name, mimeType, webViewLink, parents, size, modifiedTime)",
                    ),
                    ("supportsAllDrives", "true"),
                    ("includeItemsFromAllDrives", "true"),
                    ("corpora", "allDrives"),
                    ("orderBy", order.as_str()),
                ]);
                if let Some(token) = &opts.page_token {
                    req = req.query(&[("pageToken", token.as_str())]);
                }
                req
            })
            .await
            .context("Gagal mengirim request search ke Google Drive")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::BAD_REQUEST
                && err_text.contains("\"location\": \"q\"")
            {
                bail!(
                    "Google Drive API query tidak valid: '{full_q}'.\n\
                    Tips: Jika menggunakan query khusus, pastikan sintaks operator sesuai format Google Drive \
                    (contoh: name contains 'file' atau mimeType = 'application/pdf').\n\
                    Detail error Google: {}",
                    err_text
                );
            }
            bail!("Google Drive API search error: {}", err_text);
        }

        let file_list: DriveFileList = resp
            .json()
            .await
            .context("Gagal mendecode response file list")?;
        Ok(file_list)
    }

    /// Mencari file atau folder di Google Drive (termasuk Shared Drives)
    pub async fn search_files(
        &self,
        query: Option<&str>,
        folder_id: Option<&str>,
        page_size: Option<u32>,
    ) -> Result<Vec<DriveFile>> {
        let res = self
            .search_files_with_options(SearchFileOptions {
                query: query.map(|s| s.to_string()),
                folder_id: folder_id.map(|s| s.to_string()),
                page_size,
                page_token: None,
                full_text: false,
                order_by: None,
            })
            .await?;
        Ok(res.files)
    }

    /// Membersihkan file ID jika pengguna atau LLM memasukkan URL Google Drive lengkap
    pub fn clean_file_id(input: &str) -> &str {
        let trimmed = input.trim();
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            if let Some(pos) = trimmed.find("/folders/") {
                let after = &trimmed[pos + 9..];
                let id = after
                    .split('/')
                    .next()
                    .unwrap_or(after)
                    .split('?')
                    .next()
                    .unwrap_or(after);
                return id;
            }
            if let Some(pos) = trimmed.find("/d/") {
                let after = &trimmed[pos + 3..];
                let id = after
                    .split('/')
                    .next()
                    .unwrap_or(after)
                    .split('?')
                    .next()
                    .unwrap_or(after);
                return id;
            }
            if let Some(pos) = trimmed.find("id=") {
                let after = &trimmed[pos + 3..];
                let id = after.split('&').next().unwrap_or(after);
                return id;
            }
        }
        trimmed
    }

    /// Membaca konten file dari Google Drive
    /// Auto-export Docs ke teks, Sheets ke CSV, Slides ke teks.
    /// Dilengkapi Binary Guard dan context window truncation limit (max 500 KB).
    pub async fn read_file(&self, raw_id: &str) -> Result<String> {
        let file_id = Self::clean_file_id(raw_id);
        // 1. Ambil metadata file dulu untuk cek mimeType
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    ("fields", "id, name, mimeType, size"),
                    ("supportsAllDrives", "true"),
                ])
            })
            .await
            .context("Gagal mengambil metadata file")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;

        // 2. Cek apakah folder
        if meta.mime_type == "application/vnd.google-apps.folder" {
            bail!(
                "ID '{}' adalah sebuah folder ('{}'), bukan file dokumen.\n\
                Tips: Gunakan 'gdrive search --folder {}' atau tool 'gdrive_list_folder' untuk melihat isi folder ini.",
                file_id, meta.name, file_id
            );
        }

        // 3. Auto-Export Google Workspace Docs, Sheets, & Slides
        let text_content = if meta.mime_type == "application/vnd.google-apps.document" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", "text/plain")])
                })
                .await?;
            resp.text().await?
        } else if meta.mime_type == "application/vnd.google-apps.spreadsheet" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", "text/csv")])
                })
                .await?;
            resp.text().await?
        } else if meta.mime_type == "application/vnd.google-apps.presentation" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", "text/plain")])
                })
                .await?;
            resp.text().await?
        } else {
            // 4. Binary File Guard
            let mime = meta.mime_type.to_lowercase();
            let is_binary = mime.starts_with("image/")
                || mime.starts_with("video/")
                || mime.starts_with("audio/")
                || mime == "application/pdf"
                || mime == "application/zip"
                || mime == "application/x-zip-compressed"
                || mime == "application/gzip"
                || mime == "application/x-tar"
                || mime == "application/octet-stream"
                || mime.contains("openxmlformats")
                || mime == "application/msword"
                || mime == "application/vnd.ms-excel"
                || mime == "application/vnd.ms-powerpoint";

            if is_binary {
                bail!(
                    "File '{}' adalah file binary/media ({}) dan tidak dapat dibaca langsung sebagai teks.\n\
                    Tips: Gunakan perintah 'gdrive download {}' untuk mengunduh file ini ke disk lokal.",
                    meta.name, meta.mime_type, file_id
                );
            }

            // Unduh regular text file (misal .md, .txt, .json, source code)
            let download_url = format!(
                "{}/files/{}?alt=media&supportsAllDrives=true",
                DRIVE_API_BASE, file_id
            );
            let resp = self
                .send_with_retry(|t| self.http.get(&download_url).bearer_auth(t))
                .await?;
            if !resp.status().is_success() {
                let err = resp.text().await.unwrap_or_default();
                bail!("Gagal mengunduh isi file: {}", err);
            }
            resp.text().await?
        };

        // 5. Context Window Safety Guard (Max 500 KB limit)
        const MAX_TEXT_PAYLOAD_BYTES: usize = 500 * 1024;
        if text_content.len() > MAX_TEXT_PAYLOAD_BYTES {
            let safe_end =
                match std::str::from_utf8(&text_content.as_bytes()[..MAX_TEXT_PAYLOAD_BYTES]) {
                    Ok(_) => MAX_TEXT_PAYLOAD_BYTES,
                    Err(e) => e.valid_up_to(),
                };
            return Ok(format!(
                "{}\n\n... [⚠️ Konten dipotong: ukuran file ({} bytes) melebihi batas aman 500 KB context window LLM. Unduh file lengkap dengan 'gdrive download {}']",
                &text_content[..safe_end], text_content.len(), file_id
            ));
        }

        Ok(text_content)
    }

    /// Membuat file baru di Google Drive (opsional konversi ke native Google Docs)
    pub async fn create_file(&self, opts: CreateFileOptions) -> Result<DriveFile> {
        let upload_url = format!(
            "{}/files?uploadType=multipart&supportsAllDrives=true",
            UPLOAD_API_BASE
        );

        let target_mime = if opts.as_google_doc {
            "application/vnd.google-apps.document"
        } else if opts.name.ends_with(".md") {
            "text/markdown"
        } else {
            "text/plain"
        };

        let mut metadata = serde_json::json!({
            "name": opts.name,
            "mimeType": target_mime,
        });

        if let Some(parent) = opts.parent_folder_id {
            metadata["parents"] = serde_json::json!([parent]);
        }

        let metadata_str = metadata.to_string();
        let content_str = opts.content;

        let build_form = || -> Result<reqwest::multipart::Form> {
            Ok(reqwest::multipart::Form::new()
                .part(
                    "metadata",
                    reqwest::multipart::Part::text(metadata_str.clone())
                        .mime_str("application/json; charset=UTF-8")?,
                )
                .part(
                    "media",
                    reqwest::multipart::Part::text(content_str.clone())
                        .mime_str("text/plain; charset=UTF-8")?,
                ))
        };

        let token = self.get_token().await;
        let mut resp = self
            .http
            .post(&upload_url)
            .bearer_auth(&token)
            .multipart(build_form()?)
            .send()
            .await
            .context("Gagal mengupload file ke Google Drive")?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED && self.auto_refresh {
            if let Ok(new_token) = self.force_refresh().await {
                resp = self
                    .http
                    .post(&upload_url)
                    .bearer_auth(&new_token)
                    .multipart(build_form()?)
                    .send()
                    .await
                    .context("Gagal mengupload file ke Google Drive (retry)")?;
            }
        }

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal create file di Google Drive: {}", err);
        }

        let created: DriveFile = resp.json().await?;
        Ok(created)
    }

    /// Mengupdate konten file yang sudah ada di Google Drive (overwrite atau append)
    pub async fn update_file(&self, opts: UpdateFileOptions) -> Result<DriveFile> {
        let final_content = if opts.append {
            let old_content = self.read_file(&opts.file_id).await.unwrap_or_default();
            if old_content.trim().is_empty() {
                opts.content
            } else {
                format!("{}\n{}", old_content, opts.content)
            }
        } else {
            opts.content
        };

        let upload_url = format!(
            "{}/files/{}?uploadType=media&supportsAllDrives=true",
            UPLOAD_API_BASE, opts.file_id
        );

        let content_bytes = final_content.into_bytes();
        let resp = self
            .send_with_retry(|t| {
                self.http
                    .patch(&upload_url)
                    .bearer_auth(t)
                    .header("Content-Type", "text/plain; charset=UTF-8")
                    .body(content_bytes.clone())
            })
            .await
            .context("Gagal mengupdate konten file ke Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal update file di Google Drive: {}", err);
        }

        let updated: DriveFile = resp.json().await?;
        Ok(updated)
    }

    /// Membuat folder baru di Google Drive (pribadi atau Shared Drives)
    pub async fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<DriveFile> {
        let url = format!("{}/files?supportsAllDrives=true", DRIVE_API_BASE);

        let mut body = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
        });

        if let Some(parent) = parent_id {
            body["parents"] = serde_json::json!([parent]);
        }

        let body_str = body.to_string();
        let resp = self
            .send_with_retry(|t| {
                self.http
                    .post(&url)
                    .bearer_auth(t)
                    .header("Content-Type", "application/json; charset=UTF-8")
                    .body(body_str.clone())
            })
            .await
            .context("Gagal membuat folder di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal create folder di Google Drive: {}", err);
        }

        let folder: DriveFile = resp.json().await?;
        Ok(folder)
    }

    /// Memindahkan file ke tempat sampah (Safe Trash - mencegah permanent data loss)
    pub async fn trash_file(&self, raw_id: &str) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!(
            "{}/files/{}?supportsAllDrives=true",
            DRIVE_API_BASE, file_id
        );

        let body = serde_json::json!({
            "trashed": true
        });

        let body_str = body.to_string();
        let resp = self
            .send_with_retry(|t| {
                self.http
                    .patch(&url)
                    .bearer_auth(t)
                    .header("Content-Type", "application/json; charset=UTF-8")
                    .body(body_str.clone())
            })
            .await
            .context("Gagal memindahkan file ke trash di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal trash file di Google Drive: {}", err);
        }

        let trashed: DriveFile = resp.json().await?;
        Ok(trashed)
    }

    /// Mengubah nama file atau folder di Google Drive
    pub async fn rename_file(&self, raw_id: &str, new_name: &str) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!(
            "{}/files/{}?supportsAllDrives=true&fields=id,name,mimeType,webViewLink,parents,modifiedTime",
            DRIVE_API_BASE, file_id
        );

        let body = serde_json::json!({
            "name": new_name
        });
        let body_str = body.to_string();

        let resp = self
            .send_with_retry(|t| {
                self.http
                    .patch(&url)
                    .bearer_auth(t)
                    .header("Content-Type", "application/json; charset=UTF-8")
                    .body(body_str.clone())
            })
            .await
            .context("Gagal merename file di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal rename file di Google Drive: {}", err);
        }

        let file: DriveFile = resp.json().await?;
        Ok(file)
    }

    /// Memindahkan file atau folder ke folder tujuan (addParents & removeParents)
    pub async fn move_file(
        &self,
        raw_id: &str,
        new_parent_id: &str,
        old_parent_id: Option<&str>,
    ) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let target_parent = Self::clean_file_id(new_parent_id);

        let remove_parents = match old_parent_id {
            Some(old_p) => Self::clean_file_id(old_p).to_string(),
            None => {
                // Ambil daftar parents saat ini agar bisa dilepas dari lokasi lama
                let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
                let meta_resp = self
                    .send_with_retry(|t| {
                        self.http
                            .get(&meta_url)
                            .bearer_auth(t)
                            .query(&[("fields", "parents"), ("supportsAllDrives", "true")])
                    })
                    .await
                    .context("Gagal mengambil metadata parents file")?;

                if meta_resp.status().is_success() {
                    let meta: serde_json::Value = meta_resp.json().await?;
                    if let Some(parents) = meta.get("parents").and_then(|p| p.as_array()) {
                        parents
                            .iter()
                            .filter_map(|p| p.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            }
        };

        let mut url = format!(
            "{}/files/{}?addParents={}&supportsAllDrives=true&fields=id,name,mimeType,webViewLink,parents,modifiedTime",
            DRIVE_API_BASE, file_id, target_parent
        );
        if !remove_parents.is_empty() {
            url.push_str(&format!("&removeParents={}", remove_parents));
        }

        let resp = self
            .send_with_retry(|t| {
                self.http
                    .patch(&url)
                    .bearer_auth(t)
                    .header("Content-Type", "application/json; charset=UTF-8")
                    .body("{}")
            })
            .await
            .context("Gagal memindahkan file di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal move file di Google Drive: {}", err);
        }

        let moved: DriveFile = resp.json().await?;
        Ok(moved)
    }

    /// Mengambil daftar Shared Drives (Team Drives) yang dapat diakses pengguna
    pub async fn list_drives(
        &self,
        page_size: Option<u32>,
    ) -> Result<Vec<crate::models::DriveItem>> {
        let limit = page_size.unwrap_or(50);
        let url = format!("{}/drives", DRIVE_API_BASE);

        let resp = self
            .send_with_retry(|t| {
                self.http
                    .get(&url)
                    .bearer_auth(t)
                    .query(&[("pageSize", &limit.to_string())])
            })
            .await
            .context("Gagal mengambil daftar Shared Drives")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal list Shared Drives: {}", err);
        }

        let drive_list: crate::models::DriveItemList = resp.json().await?;
        Ok(drive_list.drives)
    }

    /// Mengambil metadata file berdasarkan ID jika ada
    pub async fn get_file_metadata(&self, raw_id: &str) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    ("fields", "id, name, mimeType, size"),
                    ("supportsAllDrives", "true"),
                ])
            })
            .await
            .context("Gagal mengambil metadata file")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;
        Ok(meta)
    }

    /// Mengunduh raw bytes file dari Google Drive beserta metadata nama file aslinya
    pub async fn download_file_bytes(&self, raw_id: &str) -> Result<(String, Vec<u8>)> {
        let file_id = Self::clean_file_id(raw_id);
        // Ambil metadata dulu untuk nama dan mimeType
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    ("fields", "id, name, mimeType"),
                    ("supportsAllDrives", "true"),
                ])
            })
            .await
            .context("Gagal mengambil metadata file untuk download")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;

        // Jika Google Docs, ekspor ke PDF atau teks
        if meta.mime_type == "application/vnd.google-apps.document" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", "text/plain")])
                })
                .await?;
            let filename = format!("{}.txt", meta.name);
            return Ok((filename, resp.bytes().await?.to_vec()));
        }

        // Regular file media download
        let download_url = format!(
            "{}/files/{}?alt=media&supportsAllDrives=true",
            DRIVE_API_BASE, file_id
        );
        let resp = self
            .send_with_retry(|t| self.http.get(&download_url).bearer_auth(t))
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengunduh file media: {}", err);
        }

        Ok((meta.name, resp.bytes().await?.to_vec()))
    }

    /// Mengambil file sebagai stream byte untuk penghematan memori dan rendering progress bar
    pub async fn download_file_stream(&self, raw_id: &str) -> Result<DownloadStream> {
        let file_id = Self::clean_file_id(raw_id);
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    ("fields", "id, name, mimeType, size"),
                    ("supportsAllDrives", "true"),
                ])
            })
            .await
            .context("Gagal mengambil metadata file untuk download")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;
        let parsed_size = meta.size.as_deref().and_then(|s| s.parse::<u64>().ok());

        // Auto-Export Google Workspace (Docs, Sheets, Slides, Drawings)
        let workspace_export: Option<(&'static str, &'static str, &'static str)> =
            match meta.mime_type.as_str() {
                "application/vnd.google-apps.document" => Some(("text/plain", "txt", "text/plain")),
                "application/vnd.google-apps.spreadsheet" => Some(("text/csv", "csv", "text/csv")),
                "application/vnd.google-apps.presentation" => {
                    Some(("application/pdf", "pdf", "application/pdf"))
                }
                "application/vnd.google-apps.drawing" => Some(("image/png", "png", "image/png")),
                _ => None,
            };

        if let Some((export_mime, ext, out_mime)) = workspace_export {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .send_with_retry(|t| {
                    self.http
                        .get(&export_url)
                        .bearer_auth(t)
                        .query(&[("mimeType", export_mime)])
                })
                .await?;

            if !resp.status().is_success() {
                let err = resp.text().await.unwrap_or_default();
                bail!(
                    "Gagal mengekspor file Google Workspace ({}): {}",
                    meta.mime_type,
                    err
                );
            }

            let filename = format!("{}.{}", meta.name, ext);
            let len = resp.content_length();
            return Ok(DownloadStream {
                name: filename,
                size: len,
                mime_type: out_mime.to_string(),
                stream: Box::new(resp.bytes_stream()),
            });
        }

        // Regular file media download
        let download_url = format!(
            "{}/files/{}?alt=media&supportsAllDrives=true",
            DRIVE_API_BASE, file_id
        );
        let resp = self
            .send_with_retry(|t| self.http.get(&download_url).bearer_auth(t))
            .await?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengunduh file media: {}", err);
        }

        let total_size = parsed_size.or_else(|| resp.content_length());

        Ok(DownloadStream {
            name: meta.name,
            size: total_size,
            mime_type: meta.mime_type,
            stream: Box::new(resp.bytes_stream()),
        })
    }

    /// Mencari file berdasarkan nama (exact match atau contains)
    pub async fn find_files_by_name(&self, name_query: &str, limit: u32) -> Result<Vec<DriveFile>> {
        let escaped = name_query.trim().replace('\\', "\\\\").replace('\'', "\\'");
        let query_predicate = format!("name contains '{escaped}'");

        let res = self
            .search_files_with_options(SearchFileOptions {
                query: Some(query_predicate),
                folder_id: None,
                page_size: Some(limit),
                page_token: None,
                full_text: false,
                order_by: Some("folder,name,modifiedTime desc".to_string()),
            })
            .await?;
        Ok(res.files)
    }

    /// Mengupload file lokal dari disk ke Google Drive
    pub async fn upload_file_path(
        &self,
        path: &std::path::Path,
        name: Option<&str>,
        parent_id: Option<&str>,
        as_doc: bool,
    ) -> Result<DriveFile> {
        let file_name = match name {
            Some(n) => n.to_string(),
            None => path
                .file_name()
                .and_then(|os| os.to_str())
                .unwrap_or("uploaded_file")
                .to_string(),
        };

        let content = std::fs::read(path)
            .with_context(|| format!("Gagal membaca file lokal di {:?}", path))?;

        let upload_url = format!(
            "{}/files?uploadType=multipart&supportsAllDrives=true",
            UPLOAD_API_BASE
        );

        let target_mime = if as_doc {
            "application/vnd.google-apps.document"
        } else if file_name.ends_with(".md") {
            "text/markdown"
        } else if file_name.ends_with(".json") {
            "application/json"
        } else if file_name.ends_with(".txt") {
            "text/plain"
        } else {
            "application/octet-stream"
        };

        let mut metadata = serde_json::json!({
            "name": file_name,
            "mimeType": target_mime,
        });

        if let Some(parent) = parent_id {
            metadata["parents"] = serde_json::json!([parent]);
        }

        let metadata_str = metadata.to_string();

        let build_form = || -> Result<reqwest::multipart::Form> {
            Ok(reqwest::multipart::Form::new()
                .part(
                    "metadata",
                    reqwest::multipart::Part::text(metadata_str.clone())
                        .mime_str("application/json; charset=UTF-8")?,
                )
                .part(
                    "media",
                    reqwest::multipart::Part::bytes(content.clone()).mime_str(target_mime)?,
                ))
        };

        let token = self.get_token().await;
        let mut resp = self
            .http
            .post(&upload_url)
            .bearer_auth(&token)
            .multipart(build_form()?)
            .send()
            .await
            .context("Gagal mengupload file lokal ke Google Drive")?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED && self.auto_refresh {
            if let Ok(new_token) = self.force_refresh().await {
                resp = self
                    .http
                    .post(&upload_url)
                    .bearer_auth(&new_token)
                    .multipart(build_form()?)
                    .send()
                    .await
                    .context("Gagal mengupload file lokal ke Google Drive (retry)")?;
            }
        }

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal upload file ke Google Drive: {}", err);
        }

        let created: DriveFile = resp.json().await?;
        Ok(created)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_file_id() {
        assert_eq!(GDriveClient::clean_file_id("1abcXYZ456"), "1abcXYZ456");
        assert_eq!(
            GDriveClient::clean_file_id(
                "https://drive.google.com/file/d/1abcXYZ456/view?usp=sharing"
            ),
            "1abcXYZ456"
        );
        assert_eq!(
            GDriveClient::clean_file_id("https://docs.google.com/document/d/1docId789/edit"),
            "1docId789"
        );
        assert_eq!(
            GDriveClient::clean_file_id(
                "https://drive.google.com/drive/folders/1folderId123?usp=sharing"
            ),
            "1folderId123"
        );
        assert_eq!(
            GDriveClient::clean_file_id("https://drive.google.com/open?id=1openId999"),
            "1openId999"
        );
    }

    #[test]
    fn test_build_search_query_auto_wrap_and_escape() {
        let opts = SearchFileOptions {
            query: Some("it's a test".to_string()),
            ..Default::default()
        };
        let q = GDriveClient::build_search_query(&opts);
        assert_eq!(q, "trashed = false and name contains 'it\\'s a test'");
    }

    #[test]
    fn test_build_search_query_full_text() {
        let opts = SearchFileOptions {
            query: Some("financial report".to_string()),
            full_text: true,
            ..Default::default()
        };
        let q = GDriveClient::build_search_query(&opts);
        assert_eq!(
            q,
            "trashed = false and (name contains 'financial report' or fullText contains 'financial report')"
        );
    }

    #[test]
    fn test_build_search_query_preserves_raw_operators() {
        let opts = SearchFileOptions {
            query: Some("mimeType = 'application/pdf' and name contains 'RFC'".to_string()),
            ..Default::default()
        };
        let q = GDriveClient::build_search_query(&opts);
        assert_eq!(
            q,
            "trashed = false and mimeType = 'application/pdf' and name contains 'RFC'"
        );
    }

    #[test]
    fn test_build_search_query_name_predicate_with_and_operator() {
        let name_query = "Romeo and Juliet's notes.txt";
        let escaped = name_query.trim().replace('\\', "\\\\").replace('\'', "\\'");
        let query_predicate = format!("name contains '{escaped}'");
        let opts = SearchFileOptions {
            query: Some(query_predicate),
            ..Default::default()
        };
        let q = GDriveClient::build_search_query(&opts);
        assert_eq!(
            q,
            "trashed = false and name contains 'Romeo and Juliet\\'s notes.txt'"
        );
    }

    #[test]
    fn test_is_transient_status() {
        assert!(is_transient_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_transient_status(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(is_transient_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_transient_status(
            reqwest::StatusCode::SERVICE_UNAVAILABLE
        ));
        assert!(is_transient_status(reqwest::StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_transient_status(reqwest::StatusCode::NOT_FOUND));
        assert!(!is_transient_status(reqwest::StatusCode::FORBIDDEN));
    }
}
