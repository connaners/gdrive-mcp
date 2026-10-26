use schemars::JsonSchema;
use serde::Deserialize;

/// Parameter untuk tool gdrive_list_profiles
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProfilesArgs {}

/// Parameter untuk tool gdrive_switch_profile
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SwitchProfileArgs {
    /// Nama profil Google Drive yang ingin dijadikan aktif (contoh: 'work', 'personal', 'default').
    pub name: String,
}

/// Parameter untuk tool gdrive_get_about
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetAboutArgs {
    /// Nama profile akun Google Drive spesifik yang ingin diperiksa (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}
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
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RenameFileArgs {
    /// ID unik file atau folder di Google Drive yang ingin diubah namanya.
    pub file_id: String,
    /// Nama baru untuk file atau folder tersebut.
    pub new_name: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveFileArgs {
    /// ID unik file atau folder di Google Drive yang ingin dipindahkan.
    pub file_id: String,
    /// ID folder tujuan tempat file atau folder akan dipindahkan.
    pub new_parent_id: String,
    /// ID folder asal jika diketahui (opsional, jika tidak diberikan akan otomatis dideteksi).
    pub old_parent_id: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadFileArgs {
    /// ID unik file di Google Drive yang ingin dibaca.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_upload_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadFileArgs {
    /// Path ke file lokal di disk yang ingin diunggah ke Google Drive (bisa absolut atau relatif).
    pub local_path: String,
    /// Nama file yang akan disimpan di Google Drive (opsional, jika tidak diisi menggunakan nama asli file lokal).
    pub name: Option<String>,
    /// ID folder tujuan di Google Drive (opsional, jika tidak diisi disimpan di root Drive).
    pub parent_folder_id: Option<String>,
    /// Jika true, otomatis mengonversi file teks/markdown menjadi native Google Docs yang dapat diedit di web.
    pub as_google_doc: Option<bool>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListFolderArgs {
    /// ID folder yang ingin dilihat isinya (opsional, kosongkan untuk root Drive).
    pub folder_id: Option<String>,
    /// Batas jumlah item yang ditampilkan (default: 50).
    pub page_size: Option<u32>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_list_tree
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTreeArgs {
    /// ID folder awal atau 'root' (opsional, jika tidak diisi menggunakan root My Drive).
    pub folder_id: Option<String>,
    /// Kedalaman maksimal pohon folder (opsional, rentang 1-4, default: 2).
    pub max_depth: Option<u32>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_get_file_path
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFilePathArgs {
    /// ID file atau folder yang ingin diketahui jalur hierarki path lengkapnya.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
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
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateFileArgs {
    /// ID unik file di Google Drive yang ingin diperbarui isinya.
    pub file_id: String,
    /// Konten teks atau markdown baru.
    pub content: String,
    /// Jika true, konten baru akan ditambahkan di akhir file lama (append mode). Default false (overwrite).
    pub append: Option<bool>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateFolderArgs {
    /// Nama folder yang ingin dibuat (contoh: 'Dokumentasi-Projek').
    pub name: String,
    /// ID folder induk tempat folder baru akan diletakkan (opsional, jika kosong di root).
    pub parent_folder_id: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TrashFileArgs {
    /// ID unik file di Google Drive yang ingin dipindahkan ke tempat sampah (Safe Trash).
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDrivesArgs {
    /// Batas jumlah Shared Drives yang ditampilkan (default: 50).
    pub page_size: Option<u32>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_download_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadFileArgs {
    /// ID Google Drive, URL Google Drive, atau nama file yang ingin diunduh ke disk lokal.
    pub file_id_or_name: String,
    /// Path folder atau file tujuan penyimpanan lokal (opsional, default: direktori kerja saat ini).
    pub destination_path: Option<String>,
    /// Format ekspor jika file berupa Google Workspace docs (opsional: 'pdf', 'docx', 'xlsx', 'csv', 'txt', 'html').
    pub export_format: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_export_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExportFileArgs {
    /// ID file atau tautan Google Docs, Sheets, atau Slides yang ingin diekspor.
    pub file_id: String,
    /// Format tujuan ekspor: 'pdf', 'docx', 'xlsx', 'csv', 'txt', atau 'html'.
    pub format: String,
    /// Path lokasi penyimpanan file lokal hasil ekspor (opsional, jika kosong disimpan di direktori saat ini).
    pub destination_path: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_copy_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CopyFileArgs {
    /// ID unik file di Google Drive yang ingin disalin/diduplikasi.
    pub file_id: String,
    /// Nama baru untuk file hasil salinan (opsional, jika tidak diisi Drive akan memberi awalan 'Copy of').
    pub new_name: Option<String>,
    /// ID folder tujuan tempat file salinan disimpan (opsional, jika kosong disimpan di folder yang sama atau root).
    pub parent_folder_id: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_get_file_info
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileInfoArgs {
    /// ID unik atau URL Google Drive dari file/folder yang ingin diperiksa metadatanya.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_list_permissions
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListPermissionsArgs {
    /// ID unik atau URL Google Drive dari file atau folder yang ingin diperiksa daftar permission/hak aksesnya.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_share_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ShareFileArgs {
    /// ID unik atau URL Google Drive dari file atau folder yang akan dibagikan.
    pub file_id: String,
    /// Peran atau hak akses yang diberikan ('reader', 'commenter', 'writer', atau 'fileOrganizer').
    pub role: String,
    /// Tipe target pembagian ('anyone', 'user', 'group', atau 'domain').
    pub permission_type: String,
    /// Alamat email target (wajib jika permission_type adalah 'user' atau 'group').
    pub email_address: Option<String>,
    /// Kirim notifikasi email ke pengguna target saat dibagikan. Default false.
    pub send_notification: Option<bool>,
    /// Nama domain target jika permission_type adalah 'domain' (contoh: 'example.com').
    pub domain: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_delete_permission
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeletePermissionArgs {
    /// ID unik atau URL Google Drive dari file atau folder.
    pub file_id: String,
    /// ID permission yang ingin dihapus (dapat diperoleh dari gdrive_list_permissions).
    pub permission_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_get_shareable_link
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetShareableLinkArgs {
    /// ID unik atau URL Google Drive dari file atau folder yang ingin diambil link-nya.
    pub file_id: String,
    /// Jika true, otomatis membuat permission publik (anyone with link as reader) jika belum publik. Default false.
    pub make_public: Option<bool>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_list_comments
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCommentsArgs {
    /// ID unik atau URL Google Drive dari file yang ingin dilihat komentarnya.
    pub file_id: String,
    /// Batas jumlah komentar yang diambil (default: 50, maksimal: 100).
    pub page_size: Option<u32>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_add_comment
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentArgs {
    /// ID unik atau URL Google Drive dari file tempat komentar akan ditambahkan.
    pub file_id: String,
    /// Isi teks komentar atau feedback.
    pub content: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_resolve_comment
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ResolveCommentArgs {
    /// ID unik atau URL Google Drive dari file.
    pub file_id: String,
    /// ID komentar yang ingin ditandai terselesaikan (resolved).
    pub comment_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_list_trash
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTrashArgs {
    /// Batas jumlah file di tempat sampah yang ingin ditampilkan (default: 50, maksimal: 100).
    pub page_size: Option<u32>,
    /// Token halaman untuk melanjutkan pagination jika hasil sebelumnya terpotong.
    pub page_token: Option<String>,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_restore_file
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RestoreFileArgs {
    /// ID unik atau URL Google Drive dari file yang ingin dipulihkan dari tempat sampah.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_empty_trash
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmptyTrashArgs {
    /// Konfirmasi eksplisit untuk mengosongkan tempat sampah secara permanen (harus true).
    pub confirm: bool,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_list_revisions
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListRevisionsArgs {
    /// ID unik atau URL Google Drive dari file yang ingin diperiksa riwayat versinya.
    pub file_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_get_revision
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRevisionArgs {
    /// ID unik atau URL Google Drive dari file.
    pub file_id: String,
    /// ID spesifik revisi/versi yang ingin diambil.
    pub revision_id: String,
    /// Nama profile akun Google Drive yang digunakan untuk operasi ini (opsional, jika tidak diisi menggunakan profile aktif).
    pub profile: Option<String>,
}

/// Parameter untuk tool gdrive_check_update
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckUpdateArgs {
    /// Tampilkan catatan rilis (changelog / release notes) lengkap dari versi terbaru jika tersedia (default: false).
    pub include_changelog: Option<bool>,
}

/// Parameter untuk tool gdrive_self_update
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SelfUpdateArgs {
    /// Jika true, proses MCP server akan keluar secara graceful (exit code 0) sesaat setelah pembaruan selesai agar host dapat me-restart proses dengan binary baru. Jika false (default: false), proses tetap berjalan dan dapat di-restart manual.
    pub restart: Option<bool>,
}

/// Parameter untuk tool gdrive_doctor
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunDoctorArgs {
    /// Nama profil Google Drive yang ingin diperiksa (opsional, jika tidak diisi menggunakan profil aktif).
    pub profile: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::schema_for;

    fn assert_no_raw_bool<T: JsonSchema>() {
        let schema = schema_for!(T);
        let val = serde_json::to_value(&schema).unwrap();
        assert_val_no_raw_bool(&val);
    }

    fn assert_val_no_raw_bool(val: &serde_json::Value) {
        if let serde_json::Value::Object(map) = val {
            if let Some(props) = map.get("properties").and_then(|p| p.as_object()) {
                for (prop_name, prop_val) in props {
                    assert!(
                        !prop_val.is_boolean(),
                        "Property '{}' generated a bare boolean schema: {:?}",
                        prop_name,
                        prop_val
                    );
                }
            }
            for v in map.values() {
                assert_val_no_raw_bool(v);
            }
        } else if let serde_json::Value::Array(arr) = val {
            for v in arr {
                assert_val_no_raw_bool(v);
            }
        }
    }

    #[test]
    fn test_all_mcp_schemas_are_safe() {
        assert_no_raw_bool::<SearchFilesArgs>();
        assert_no_raw_bool::<RenameFileArgs>();
        assert_no_raw_bool::<MoveFileArgs>();
        assert_no_raw_bool::<ReadFileArgs>();
        assert_no_raw_bool::<ListFolderArgs>();
        assert_no_raw_bool::<CreateFileArgs>();
        assert_no_raw_bool::<UpdateFileArgs>();
        assert_no_raw_bool::<CreateFolderArgs>();
        assert_no_raw_bool::<TrashFileArgs>();
        assert_no_raw_bool::<ListDrivesArgs>();
        assert_no_raw_bool::<DownloadFileArgs>();
        assert_no_raw_bool::<CopyFileArgs>();
        assert_no_raw_bool::<GetFileInfoArgs>();
        assert_no_raw_bool::<ListPermissionsArgs>();
        assert_no_raw_bool::<ShareFileArgs>();
        assert_no_raw_bool::<DeletePermissionArgs>();
        assert_no_raw_bool::<GetShareableLinkArgs>();
        assert_no_raw_bool::<ListCommentsArgs>();
        assert_no_raw_bool::<AddCommentArgs>();
        assert_no_raw_bool::<ResolveCommentArgs>();
        assert_no_raw_bool::<ListTrashArgs>();
        assert_no_raw_bool::<RestoreFileArgs>();
        assert_no_raw_bool::<EmptyTrashArgs>();
        assert_no_raw_bool::<ListRevisionsArgs>();
        assert_no_raw_bool::<GetRevisionArgs>();
        assert_no_raw_bool::<CheckUpdateArgs>();
        assert_no_raw_bool::<SelfUpdateArgs>();
        assert_no_raw_bool::<ListProfilesArgs>();
        assert_no_raw_bool::<SwitchProfileArgs>();
        assert_no_raw_bool::<GetAboutArgs>();
        assert_no_raw_bool::<RunDoctorArgs>();
    }
}
