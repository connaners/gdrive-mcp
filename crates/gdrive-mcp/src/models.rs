use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchFilesArgs {
    /// Kata kunci nama file atau query Google Drive (misal: "notes" atau "name contains 'notes'").
    pub query: Option<String>,
    /// ID folder target jika ingin membatasi pencarian di folder tertentu.
    pub folder_id: Option<String>,
    /// Batas jumlah hasil pencarian yang dikembalikan (default: 20).
    pub page_size: Option<u32>,
    /// Token halaman untuk melanjutkan pagination jika hasil sebelumnya terpotong.
    pub page_token: Option<String>,
    /// Jika true, mencari di dalam seluruh teks/isi dokumen, bukan hanya nama file. Default false.
    pub full_text: Option<bool>,
    /// Pengurutan hasil pencarian (contoh: 'modifiedTime desc', 'name', 'folder,name').
    pub order_by: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameFileArgs {
    /// ID unik file atau folder di Google Drive yang ingin diubah namanya.
    pub file_id: String,
    /// Nama baru untuk file atau folder tersebut.
    pub new_name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveFileArgs {
    /// ID unik file atau folder di Google Drive yang ingin dipindahkan.
    pub file_id: String,
    /// ID folder tujuan tempat file atau folder akan dipindahkan.
    pub new_parent_id: String,
    /// ID folder asal jika diketahui (opsional, jika tidak diberikan akan otomatis dideteksi).
    pub old_parent_id: Option<String>,
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateFileArgs {
    /// ID unik file di Google Drive yang ingin diperbarui isinya.
    pub file_id: String,
    /// Konten teks atau markdown baru.
    pub content: String,
    /// Jika true, konten baru akan ditambahkan di akhir file lama (append mode). Default false (overwrite).
    pub append: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFolderArgs {
    /// Nama folder yang ingin dibuat (contoh: 'Dokumentasi-Projek').
    pub name: String,
    /// ID folder induk tempat folder baru akan diletakkan (opsional, jika kosong di root).
    pub parent_folder_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrashFileArgs {
    /// ID unik file di Google Drive yang ingin dipindahkan ke tempat sampah (Safe Trash).
    pub file_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDrivesArgs {
    /// Batas jumlah Shared Drives yang ditampilkan (default: 50).
    pub page_size: Option<u32>,
}
