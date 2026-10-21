use crate::models::{CreateFileOptions, DriveFile, DriveFileList};
use anyhow::{bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

const DRIVE_API_BASE: &str = "https://www.googleapis.com/drive/v3";
const UPLOAD_API_BASE: &str = "https://www.googleapis.com/upload/drive/v3";

#[derive(Clone)]
pub struct GDriveClient {
    http: reqwest::Client,
    #[allow(dead_code)]
    access_token: String,
}

impl GDriveClient {
    pub fn new(access_token: String) -> Self {
        let mut headers = HeaderMap::new();
        let auth_val = format!("Bearer {}", access_token);
        if let Ok(mut hv) = HeaderValue::from_str(&auth_val) {
            hv.set_sensitive(true);
            headers.insert(AUTHORIZATION, hv);
        }

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_default();

        Self {
            http,
            access_token,
        }
    }

    /// Mencari file atau folder di Google Drive (termasuk Shared Drives)
    pub async fn search_files(
        &self,
        query: Option<&str>,
        folder_id: Option<&str>,
        page_size: Option<u32>,
    ) -> Result<Vec<DriveFile>> {
        let mut q_parts = Vec::new();
        q_parts.push("trashed = false".to_string());

        if let Some(folder) = folder_id {
            q_parts.push(format!("'{}' in parents", folder));
        }

        if let Some(q) = query {
            if !q.trim().is_empty() {
                q_parts.push(q.to_string());
            }
        }

        let full_q = q_parts.join(" and ");
        let limit = page_size.unwrap_or(20);

        let url = format!("{}/files", DRIVE_API_BASE);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("q", full_q.as_str()),
                ("pageSize", &limit.to_string()),
                ("fields", "files(id, name, mimeType, webViewLink, parents, size, modifiedTime)"),
                ("supportsAllDrives", "true"),
                ("includeItemsFromAllDrives", "true"),
                ("corpora", "allDrives"),
            ])
            .send()
            .await
            .context("Gagal mengirim request search ke Google Drive")?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            bail!("Google Drive API search error: {}", err_text);
        }

        let file_list: DriveFileList = resp.json().await.context("Gagal mendecode response file list")?;
        Ok(file_list.files)
    }

    /// Membaca konten file dari Google Drive
    /// Jika berupa Google Docs/Sheets, otomatis diekspor ke Plain Text / Markdown
    pub async fn read_file(&self, file_id: &str) -> Result<String> {
        // 1. Ambil metadata file dulu untuk cek mimeType
        let meta_url = format!("{}/files/{}", DRIVE_API_BASE, file_id);
        let meta_resp = self
            .http
            .get(&meta_url)
            .query(&[
                ("fields", "id, name, mimeType"),
                ("supportsAllDrives", "true"),
            ])
            .send()
            .await
            .context("Gagal mengambil metadata file")?;

        if !meta_resp.status().is_success() {
            let err = meta_resp.text().await.unwrap_or_default();
            bail!("Gagal mengambil metadata file: {}", err);
        }

        let meta: DriveFile = meta_resp.json().await?;

        // 2. Jika Google Workspace Docs, gunakan endpoint export
        if meta.mime_type == "application/vnd.google-apps.document" {
            let export_url = format!("{}/files/{}/export", DRIVE_API_BASE, file_id);
            let resp = self
                .http
                .get(&export_url)
                .query(&[("mimeType", "text/plain")])
                .send()
                .await?;
            return Ok(resp.text().await?);
        }

        // 3. Untuk file biasa (misal .md, .txt, .json), unduh raw content via alt=media
        let download_url = format!("{}/files/{}?alt=media&supportsAllDrives=true", DRIVE_API_BASE, file_id);
        let resp = self.http.get(&download_url).send().await?;
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal mengunduh file media: {}", err);
        }

        Ok(resp.text().await?)
    }

    /// Membuat file baru di Google Drive (opsional konversi ke native Google Docs)
    pub async fn create_file(&self, opts: CreateFileOptions) -> Result<DriveFile> {
        let upload_url = format!("{}/files?uploadType=multipart&supportsAllDrives=true", UPLOAD_API_BASE);

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

        let form = reqwest::multipart::Form::new()
            .part(
                "metadata",
                reqwest::multipart::Part::text(metadata.to_string())
                    .mime_str("application/json; charset=UTF-8")?,
            )
            .part(
                "media",
                reqwest::multipart::Part::text(opts.content)
                    .mime_str("text/plain; charset=UTF-8")?,
            );

        let resp = self
            .http
            .post(&upload_url)
            .multipart(form)
            .send()
            .await
            .context("Gagal mengupload file ke Google Drive")?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            bail!("Gagal create file di Google Drive: {}", err);
        }

        let created: DriveFile = resp.json().await?;
        Ok(created)
    }
}
