use crate::auth::AuthManager;
use crate::models::{
    CreateFileOptions, CreatePermissionOptions, DriveAbout, DriveFile, DriveFileList,
    DrivePermission, DrivePermissionList, SearchFileOptions, UpdateFileOptions,
};
use anyhow::{bail, Context, Result};
use bytes::Bytes;
use futures_util::Stream;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn drive_api_base() -> String {
    let raw = std::env::var("GDRIVE_API_BASE_URL").unwrap_or_default();
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://www.googleapis.com/drive/v3".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn upload_api_base() -> String {
    let raw = std::env::var("GDRIVE_UPLOAD_API_BASE_URL").unwrap_or_default();
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        "https://www.googleapis.com/upload/drive/v3".to_string()
    } else {
        trimmed.to_string()
    }
}

pub type DownloadByteStream = Box<dyn Stream<Item = reqwest::Result<Bytes>> + Unpin + Send>;

/// Representasi stream download file beserta metadata
pub struct DownloadStream {
    pub name: String,
    pub size: Option<u64>,
    pub mime_type: String,
    pub stream: DownloadByteStream,
}

#[derive(Clone)]
pub struct GDriveClient {
    http: reqwest::Client,
    access_token: Arc<RwLock<String>>,
    auto_refresh: bool,
    profile: Option<String>,
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
            profile: None,
        }
    }

    pub fn with_auto_refresh(mut self, enabled: bool) -> Self {
        self.auto_refresh = enabled;
        self
    }

    pub fn with_profile(mut self, profile: Option<String>) -> Self {
        self.profile = profile;
        self
    }

    /// Membuat instance client dari profile tertentu (None = active profile)
    pub async fn from_profile(profile: Option<&str>) -> Result<Self> {
        let concrete_profile = match profile {
            Some(p) => AuthManager::validate_profile_name(p)?.to_string(),
            None => AuthManager::get_active_profile(),
        };
        let token = AuthManager::get_valid_token_for_profile(Some(&concrete_profile)).await?;
        Ok(Self {
            http: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(token)),
            auto_refresh: true,
            profile: Some(concrete_profile),
        })
    }

    /// Membuat instance client dari token yang tersimpan di disk (dengan proactive refresh)
    pub async fn from_saved_token() -> Result<Self> {
        Self::from_profile(None).await
    }

    /// Mengambil access token aktif saat ini (otomatis refresh proaktif jika sudah kedaluwarsa)
    pub async fn get_token(&self) -> String {
        if self.auto_refresh {
            if let Ok(valid) =
                AuthManager::get_valid_token_for_profile(self.profile.as_deref()).await
            {
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
            let refreshed = AuthManager::force_refresh_for_profile(self.profile.as_deref()).await?;
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

    /// Mengirim request mutasi non-idempotent dengan bearer auth dan token refresh (pada 401),
    /// tanpa retry spekulatif pada network drop atau 5xx untuk mencegah duplikasi (misal create folder)
    /// atau menimpa konten yang diperbarui writer lain (misal update file).
    pub async fn send_mutation<F>(&self, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&str) -> reqwest::RequestBuilder,
    {
        let token = self.get_token().await;
        let resp = build(&token).send().await;

        match resp {
            Ok(resp) => {
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED && self.auto_refresh {
                    if let Ok(new_token) = self.force_refresh().await {
                        return Ok(build(&new_token).send().await?);
                    }
                }
                Ok(resp)
            }
            Err(err) => Err(err.into()),
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

        let url = format!("{}/files", drive_api_base());
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

    /// Validasi ID Drive agar tidak mengandung path traversal atau karakter ilegal
    pub fn validate_drive_id(raw_id: &str) -> Result<&str> {
        let cleaned = Self::clean_file_id(raw_id);
        if cleaned.is_empty() {
            bail!("ID Drive tidak boleh kosong.");
        }
        if cleaned.contains('/') || cleaned.contains('\\') || cleaned.contains("..") {
            bail!(
                "ID Drive '{}' tidak valid: mengandung karakter traversal.",
                raw_id
            );
        }
        Ok(cleaned)
    }

    /// Membaca konten file dari Google Drive
    /// Auto-export Docs ke teks, Sheets ke CSV, Slides ke teks.
    /// Dilengkapi Binary Guard dan context window truncation limit (max 500 KB).
    pub async fn read_file(&self, raw_id: &str) -> Result<String> {
        let file_id = Self::clean_file_id(raw_id);
        // 1. Ambil metadata file dulu untuk cek mimeType
        let meta_url = format!("{}/files/{}", drive_api_base(), file_id);
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
            let export_url = format!("{}/files/{}/export", drive_api_base(), file_id);
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
            let export_url = format!("{}/files/{}/export", drive_api_base(), file_id);
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
            let export_url = format!("{}/files/{}/export", drive_api_base(), file_id);
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
                drive_api_base(),
                file_id
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
            upload_api_base()
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
            upload_api_base(),
            opts.file_id
        );

        let content_bytes = final_content.into_bytes();
        let resp = self
            .send_mutation(|t| {
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
        let url = format!("{}/files?supportsAllDrives=true", drive_api_base());

        let mut body = serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
        });

        if let Some(parent) = parent_id {
            body["parents"] = serde_json::json!([parent]);
        }

        let body_str = body.to_string();
        let resp = self
            .send_mutation(|t| {
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
            drive_api_base(),
            file_id
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

    /// Memulihkan file dari tempat sampah (untrash)
    pub async fn restore_trash_file(&self, raw_id: &str) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!(
            "{}/files/{}?supportsAllDrives=true&fields=id,name,mimeType,webViewLink,parents,size,modifiedTime",
            drive_api_base(), file_id
        );

        let body = serde_json::json!({
            "trashed": false
        });

        let resp = self
            .send_mutation(|t| self.http.patch(&url).bearer_auth(t).json(&body))
            .await
            .context("Gagal memulihkan file dari tempat sampah di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!(
                "Gagal memulihkan file dari tempat sampah di Google Drive: {}",
                err
            );
        }

        let restored: DriveFile = resp.json().await?;
        Ok(restored)
    }

    /// Mengambil daftar file dan folder yang berada di tempat sampah
    pub async fn list_trash(
        &self,
        page_size: Option<u32>,
        page_token: Option<&str>,
    ) -> Result<DriveFileList> {
        let limit = page_size.unwrap_or(50).clamp(1, 100);
        let limit_str = limit.to_string();
        let url = format!("{}/files", drive_api_base());

        let resp = self
            .send_with_retry(|t| {
                let mut req = self.http.get(&url).bearer_auth(t).query(&[
                    ("q", "trashed = true"),
                    ("pageSize", limit_str.as_str()),
                    (
                        "fields",
                        "nextPageToken, files(id, name, mimeType, webViewLink, parents, size, modifiedTime)",
                    ),
                    ("supportsAllDrives", "true"),
                    ("includeItemsFromAllDrives", "true"),
                    ("corpora", "allDrives"),
                ]);
                if let Some(token) = page_token {
                    req = req.query(&[("pageToken", token)]);
                }
                req
            })
            .await
            .context("Gagal mengambil daftar file di trash")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil daftar trash: {}", err);
        }

        let list: DriveFileList = resp.json().await?;
        Ok(list)
    }

    /// Mengosongkan tempat sampah Google Drive (permanently purge trashed files)
    pub async fn empty_trash(&self) -> Result<()> {
        let url = format!("{}/files/trash?supportsAllDrives=true", drive_api_base());
        let resp = self
            .send_mutation(|t| self.http.delete(&url).bearer_auth(t))
            .await
            .context("Gagal mengosongkan tempat sampah di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengosongkan tempat sampah di Google Drive: {}", err);
        }

        Ok(())
    }

    /// Mengubah nama file atau folder di Google Drive
    pub async fn rename_file(&self, raw_id: &str, new_name: &str) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!(
            "{}/files/{}?supportsAllDrives=true&fields=id,name,mimeType,webViewLink,parents,modifiedTime",
            drive_api_base(), file_id
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

    /// Menyalin (copy/duplicate) file di Google Drive (opsional: nama baru dan folder tujuan baru)
    pub async fn copy_file(
        &self,
        raw_id: &str,
        new_name: Option<&str>,
        parent_id: Option<&str>,
    ) -> Result<DriveFile> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!(
            "{}/files/{}/copy?supportsAllDrives=true&fields=id,name,mimeType,webViewLink,parents,size,modifiedTime",
            drive_api_base(), file_id
        );

        let mut body = serde_json::Map::new();
        if let Some(name) = new_name {
            body.insert(
                "name".to_string(),
                serde_json::Value::String(name.to_string()),
            );
        }
        if let Some(pid) = parent_id {
            body.insert(
                "parents".to_string(),
                serde_json::Value::Array(vec![serde_json::Value::String(
                    Self::clean_file_id(pid).to_string(),
                )]),
            );
        }

        let body_str = serde_json::Value::Object(body).to_string();

        let resp = self
            .send_with_retry(|t| {
                self.http
                    .post(&url)
                    .bearer_auth(t)
                    .header("Content-Type", "application/json; charset=UTF-8")
                    .body(body_str.clone())
            })
            .await
            .context("Gagal menyalin file di Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal menyalin file di Google Drive: {}", err);
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
                let meta_url = format!("{}/files/{}", drive_api_base(), file_id);
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
            drive_api_base(), file_id, target_parent
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
        let url = format!("{}/drives", drive_api_base());

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
        let meta_url = format!("{}/files/{}", drive_api_base(), file_id);
        let meta_resp = self
            .send_with_retry(|t| {
                self.http.get(&meta_url).bearer_auth(t).query(&[
                    (
                        "fields",
                        "id, name, mimeType, size, webViewLink, parents, modifiedTime",
                    ),
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

    /// Mengambil daftar permission (hak akses) dari suatu file atau folder di Google Drive
    pub async fn list_permissions(&self, raw_id: &str) -> Result<Vec<DrivePermission>> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!("{}/files/{}/permissions", drive_api_base(), file_id);
        let resp = self
            .send_with_retry(|t| {
                self.http.get(&url).bearer_auth(t).query(&[
                    ("supportsAllDrives", "true"),
                    (
                        "fields",
                        "permissions(id,type,role,emailAddress,displayName,domain),nextPageToken",
                    ),
                ])
            })
            .await
            .context("Gagal mengambil daftar permission file")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil daftar permission file: {}", err);
        }

        let list: DrivePermissionList = resp.json().await?;
        Ok(list.permissions)
    }

    /// Membuat permission baru (membagikan file/folder ke pengguna, grup, domain, atau publik)
    pub async fn create_permission(
        &self,
        raw_id: &str,
        opts: CreatePermissionOptions,
    ) -> Result<DrivePermission> {
        let file_id = Self::clean_file_id(raw_id);
        let url = format!("{}/files/{}/permissions", drive_api_base(), file_id);

        let mut body = serde_json::json!({
            "role": opts.role,
            "type": opts.permission_type,
        });
        if let Some(email) = &opts.email_address {
            body["emailAddress"] = serde_json::Value::String(email.clone());
        }
        if let Some(domain) = &opts.domain {
            body["domain"] = serde_json::Value::String(domain.clone());
        }

        let send_notif = if opts.send_notification_email {
            "true"
        } else {
            "false"
        };
        let resp = self
            .send_mutation(|t| {
                self.http
                    .post(&url)
                    .bearer_auth(t)
                    .query(&[
                        ("supportsAllDrives", "true"),
                        ("sendNotificationEmail", send_notif),
                        ("fields", "id,type,role,emailAddress,displayName,domain"),
                    ])
                    .json(&body)
            })
            .await
            .context("Gagal mengirim request pembuatan permission")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal membuat permission file: {}", err);
        }

        let perm: DrivePermission = resp.json().await?;
        Ok(perm)
    }

    /// Menghapus permission dari suatu file atau folder di Google Drive
    pub async fn delete_permission(&self, raw_id: &str, permission_id: &str) -> Result<()> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let perm_id = Self::validate_drive_id(permission_id)?;
        let url = format!(
            "{}/files/{}/permissions/{}",
            drive_api_base(),
            file_id,
            perm_id
        );
        let resp = self
            .send_mutation(|t| {
                self.http
                    .delete(&url)
                    .bearer_auth(t)
                    .query(&[("supportsAllDrives", "true")])
            })
            .await
            .context("Gagal mengirim request penghapusan permission")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal menghapus permission file: {}", err);
        }

        Ok(())
    }

    /// Mengambil daftar komentar pada file Google Drive
    pub async fn list_comments(
        &self,
        raw_id: &str,
        page_size: Option<u32>,
    ) -> Result<Vec<crate::models::DriveComment>> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let limit = page_size.unwrap_or(50).clamp(1, 100);
        let limit_str = limit.to_string();
        let url = format!("{}/files/{}/comments", drive_api_base(), file_id);

        let resp = self
            .send_with_retry(|t| {
                self.http.get(&url).bearer_auth(t).query(&[
                    ("supportsAllDrives", "true"),
                    ("pageSize", limit_str.as_str()),
                    (
                        "fields",
                        "comments(id,content,author,createdTime,modifiedTime,resolved),nextPageToken",
                    ),
                ])
            })
            .await
            .context("Gagal mengambil daftar komentar file")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil daftar komentar file: {}", err);
        }

        let list: crate::models::DriveCommentList = resp.json().await?;
        Ok(list.comments)
    }

    /// Menambahkan komentar baru ke file Google Drive
    pub async fn create_comment(
        &self,
        raw_id: &str,
        content: &str,
    ) -> Result<crate::models::DriveComment> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let url = format!("{}/files/{}/comments", drive_api_base(), file_id);

        let body = serde_json::json!({
            "content": content
        });

        let resp = self
            .send_mutation(|t| {
                self.http
                    .post(&url)
                    .bearer_auth(t)
                    .query(&[
                        ("supportsAllDrives", "true"),
                        (
                            "fields",
                            "id,content,author,createdTime,modifiedTime,resolved",
                        ),
                    ])
                    .json(&body)
            })
            .await
            .context("Gagal membuat komentar di file")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal membuat komentar file: {}", err);
        }

        let comment: crate::models::DriveComment = resp.json().await?;
        Ok(comment)
    }

    /// Menandai komentar sebagai terselesaikan (resolved) via replies endpoint
    pub async fn resolve_comment(
        &self,
        raw_id: &str,
        comment_id: &str,
    ) -> Result<crate::models::DriveComment> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let comm_id = Self::validate_drive_id(comment_id)?;
        let replies_url = format!(
            "{}/files/{}/comments/{}/replies",
            drive_api_base(),
            file_id,
            comm_id
        );

        let body = serde_json::json!({
            "action": "resolve"
        });

        let resp = self
            .send_mutation(|t| {
                self.http
                    .post(&replies_url)
                    .bearer_auth(t)
                    .query(&[("supportsAllDrives", "true")])
                    .json(&body)
            })
            .await
            .context("Gagal mengirim aksi resolve komentar")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal menyelesaikan komentar: {}", err);
        }

        // Ambil status comment terbaru setelah di-resolve
        let comment_url = format!(
            "{}/files/{}/comments/{}",
            drive_api_base(),
            file_id,
            comm_id
        );
        let comment_resp = self
            .send_with_retry(|t| {
                self.http.get(&comment_url).bearer_auth(t).query(&[
                    ("supportsAllDrives", "true"),
                    (
                        "fields",
                        "id,content,author,createdTime,modifiedTime,resolved",
                    ),
                ])
            })
            .await
            .context("Gagal mengambil status komentar setelah resolve")?;

        if !comment_resp.status().is_success() {
            return Ok(crate::models::DriveComment {
                id: comment_id.to_string(),
                content: String::new(),
                author: None,
                created_time: None,
                modified_time: None,
                resolved: Some(true),
            });
        }

        let comment: crate::models::DriveComment = comment_resp.json().await?;
        Ok(comment)
    }

    /// Mengambil tautan yang dapat dibagikan (shareable link / webViewLink)
    pub async fn get_shareable_link(&self, raw_id: &str, make_public: bool) -> Result<String> {
        if make_public {
            let opts = CreatePermissionOptions {
                role: "reader".to_string(),
                permission_type: "anyone".to_string(),
                email_address: None,
                domain: None,
                send_notification_email: false,
            };
            self.create_permission(raw_id, opts)
                .await
                .context("Gagal membuat permission publik untuk file")?;
        }

        let meta = self.get_file_metadata(raw_id).await?;
        let link = meta.web_view_link.unwrap_or_else(|| {
            format!(
                "https://drive.google.com/file/d/{}/view",
                Self::clean_file_id(raw_id)
            )
        });
        Ok(link)
    }

    /// Mengambil daftar riwayat revisi (versi) dari file
    pub async fn list_revisions(&self, raw_id: &str) -> Result<Vec<crate::models::DriveRevision>> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let url = format!("{}/files/{}/revisions", drive_api_base(), file_id);

        let resp = self
            .send_with_retry(|t| {
                self.http.get(&url).bearer_auth(t).query(&[
                    ("supportsAllDrives", "true"),
                    (
                        "fields",
                        "revisions(id,mimeType,modifiedTime,keepForever,size,lastModifyingUser),nextPageToken",
                    ),
                ])
            })
            .await
            .context("Gagal mengambil daftar revisi file")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil daftar revisi file: {}", err);
        }

        let list: crate::models::DriveRevisionList = resp.json().await?;
        Ok(list.revisions)
    }

    /// Mengunduh raw bytes dari revisi file tertentu
    pub async fn download_revision_bytes(
        &self,
        raw_id: &str,
        revision_id: &str,
    ) -> Result<(String, Vec<u8>)> {
        let file_id = Self::validate_drive_id(raw_id)?;
        let rev_id = Self::validate_drive_id(revision_id)?;
        let meta = self.get_file_metadata(file_id).await?;
        let filename = meta.name;

        let url = format!(
            "{}/files/{}/revisions/{}?alt=media&supportsAllDrives=true",
            drive_api_base(),
            file_id,
            rev_id
        );

        let resp = self
            .send_with_retry(|t| self.http.get(&url).bearer_auth(t))
            .await
            .context("Gagal mengunduh konten revisi file")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengunduh konten revisi: {}", err);
        }

        let bytes = resp.bytes().await?.to_vec();
        Ok((filename, bytes))
    }

    /// Mengunduh raw bytes file dari Google Drive beserta metadata nama file aslinya
    pub async fn download_file_bytes(&self, raw_id: &str) -> Result<(String, Vec<u8>)> {
        let file_id = Self::clean_file_id(raw_id);
        // Ambil metadata dulu untuk nama dan mimeType
        let meta_url = format!("{}/files/{}", drive_api_base(), file_id);
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
            let export_url = format!("{}/files/{}/export", drive_api_base(), file_id);
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
            drive_api_base(),
            file_id
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
        let meta_url = format!("{}/files/{}", drive_api_base(), file_id);
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
            let export_url = format!("{}/files/{}/export", drive_api_base(), file_id);
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
            drive_api_base(),
            file_id
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
            upload_api_base()
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

    /// Mengambil informasi akun Google Drive aktif dan kuota penyimpanan (storage quota)
    pub async fn get_about(&self) -> Result<DriveAbout> {
        let url = format!("{}/about", drive_api_base());
        let resp = self
            .send_with_retry(|t| {
                self.http
                    .get(&url)
                    .bearer_auth(t)
                    .query(&[("fields", "user,storageQuota")])
            })
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil informasi akun Drive: {}", text);
        }

        let about: DriveAbout = resp
            .json()
            .await
            .context("Gagal mem-parsing informasi akun Drive")?;

        // Jika token belum menyimpan email / display_name dan client terikat ke profil terkelola,
        // perbarui token.json profil secara transparan
        if self.auto_refresh && self.profile.is_some() {
            if let Some(ref user) = about.user {
                if let Ok(mut saved) =
                    AuthManager::load_saved_token_for_profile(self.profile.as_deref())
                {
                    let mut changed = false;
                    if saved.account_email.is_none() && user.email_address.is_some() {
                        saved.account_email = user.email_address.clone();
                        changed = true;
                    }
                    if saved.display_name.is_none() && user.display_name.is_some() {
                        saved.display_name = user.display_name.clone();
                        changed = true;
                    }
                    if changed {
                        let _ =
                            AuthManager::save_token_for_profile(self.profile.as_deref(), &saved);
                    }
                }
            }
        }
        Ok(about)
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
