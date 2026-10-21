use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesArgs {
    /// Kata kunci nama file atau query Google Drive (misal: "name contains 'notes'").
    pub query: Option<String>,
    /// ID folder target jika ingin membatasi pencarian di folder tertentu.
    pub folder_id: Option<String>,
    /// Batas jumlah hasil pencarian yang dikembalikan (default: 20).
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// ID unik file di Google Drive yang ingin dibaca.
    pub file_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFolderArgs {
    /// ID folder yang ingin dilihat isinya (opsional, kosongkan untuk root Drive).
    pub folder_id: Option<String>,
    /// Batas jumlah item yang ditampilkan (default: 50).
    pub page_size: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFileArgs {
    /// Nama file yang akan dibuat (contoh: 'Spesifikasi-Sistem.md').
    pub name: String,
    /// Isi konten dokumen teks atau markdown.
    pub content: String,
    /// ID folder tujuan di Drive (opsional, jika tidak diisi masuk ke root).
    pub parent_folder_id: Option<String>,
    /// Jika true, otomatis dikonversi menjadi native Google Docs agar bisa diedit di browser.
    pub as_google_doc: Option<bool>,
}
