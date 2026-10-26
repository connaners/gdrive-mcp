use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
pub use gdrive_core::{format_date, format_mime, format_size};
use gdrive_core::{
    DriveAbout, DriveComment, DriveFile, DrivePermission, DriveRevision, ProfileStatusInfo,
};

fn apply_table_arrangement(table: &mut Table) {
    if table.width().unwrap_or(0) >= 60 {
        table.set_content_arrangement(ContentArrangement::Dynamic);
    }
}

pub fn render_files_table(files: &[DriveFile]) -> String {
    if files.is_empty() {
        return "Tidak ada file atau folder yang ditemukan.".to_string();
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Tipe")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new("Nama")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
        Cell::new("Ukuran")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
        Cell::new("Terakhir Diubah")
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
    ]);

    for f in files {
        let (icon, label) = format_mime(&f.mime_type);
        let type_display = format!("{} {}", icon, label);
        let size_display = format_size(f.size.as_deref());
        let date_display = format_date(f.modified_time.as_deref());

        table.add_row(vec![
            Cell::new(&f.id).fg(Color::DarkGrey),
            Cell::new(type_display),
            Cell::new(&f.name),
            Cell::new(size_display),
            Cell::new(date_display),
        ]);
    }

    table.to_string()
}

/// Membersihkan nama file download dari path traversal, separator direktori, dan karakter berbahaya
pub fn sanitize_download_filename(raw_name: &str) -> String {
    let clean = raw_name.replace('\\', "/");
    let basename = std::path::Path::new(&clean)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.trim_start_matches('.'))
        .filter(|n| !n.is_empty())
        .unwrap_or("downloaded_file");

    basename.to_string()
}

pub fn render_file_info(f: &DriveFile) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);

    let (icon, label) = format_mime(&f.mime_type);
    let type_display = format!("{} {} ({})", icon, label, f.mime_type);
    let size_display = format_size(f.size.as_deref());
    let date_display = format_date(f.modified_time.as_deref());
    let parents_display = f
        .parents
        .as_ref()
        .map(|p| p.join(", "))
        .unwrap_or_else(|| "-".to_string());
    let link_display = f.web_view_link.as_deref().unwrap_or("-");

    table.add_row(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new(&f.id),
    ]);
    table.add_row(vec![
        Cell::new("Nama")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
        Cell::new(&f.name),
    ]);
    table.add_row(vec![
        Cell::new("Tipe")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new(type_display),
    ]);
    table.add_row(vec![
        Cell::new("Ukuran")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
        Cell::new(size_display),
    ]);
    table.add_row(vec![
        Cell::new("Terakhir Diubah")
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
        Cell::new(date_display),
    ]);
    table.add_row(vec![
        Cell::new("Folder Induk (Parents)").add_attribute(Attribute::Bold),
        Cell::new(parents_display),
    ]);
    table.add_row(vec![
        Cell::new("Web View Link").add_attribute(Attribute::Bold),
        Cell::new(link_display),
    ]);

    table.to_string()
}

pub fn render_permissions_table(perms: &[DrivePermission]) -> String {
    if perms.is_empty() {
        return "Tidak ada permission yang ditemukan.".to_string();
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Tipe")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new("Role")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
        Cell::new("Nama / Email / Domain")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
    ]);

    for p in perms {
        let target_display = if let Some(email) = &p.email_address {
            if let Some(name) = &p.display_name {
                format!("{} ({})", name, email)
            } else {
                email.clone()
            }
        } else if let Some(domain) = &p.domain {
            format!("Domain: {}", domain)
        } else if let Some(name) = &p.display_name {
            name.clone()
        } else if p.permission_type == "anyone" {
            "Siapa saja yang memiliki link (Anyone)".to_string()
        } else {
            "-".to_string()
        };

        table.add_row(vec![
            Cell::new(&p.id),
            Cell::new(&p.permission_type),
            Cell::new(&p.role),
            Cell::new(target_display),
        ]);
    }

    table.to_string()
}

pub fn render_comments_table(comments: &[DriveComment]) -> String {
    if comments.is_empty() {
        return "Tidak ada komentar pada file ini.".to_string();
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Author")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
        Cell::new("Status")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new("Created")
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
        Cell::new("Content")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
    ]);

    for c in comments {
        let author_str = if let Some(a) = &c.author {
            if let Some(name) = &a.display_name {
                name.clone()
            } else if let Some(email) = &a.email_address {
                email.clone()
            } else {
                "-".to_string()
            }
        } else {
            "-".to_string()
        };

        let status_str = match c.resolved {
            Some(true) => "Resolved",
            _ => "Open",
        };

        table.add_row(vec![
            Cell::new(&c.id),
            Cell::new(author_str),
            Cell::new(status_str),
            Cell::new(format_date(c.created_time.as_deref())),
            Cell::new(&c.content),
        ]);
    }

    table.to_string()
}

pub fn render_revisions_table(revs: &[DriveRevision]) -> String {
    if revs.is_empty() {
        return "Tidak ada riwayat revisi pada file ini.".to_string();
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("Revision ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("User")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
        Cell::new("Modified")
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
        Cell::new("Size")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new("KeepForever")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
    ]);

    for r in revs {
        let user_str = if let Some(u) = &r.last_modifying_user {
            if let Some(name) = &u.display_name {
                name.clone()
            } else if let Some(email) = &u.email_address {
                email.clone()
            } else {
                "-".to_string()
            }
        } else {
            "-".to_string()
        };

        let keep_forever_str = match r.keep_forever {
            Some(true) => "Yes",
            Some(false) => "No",
            None => "-",
        };

        table.add_row(vec![
            Cell::new(&r.id),
            Cell::new(user_str),
            Cell::new(format_date(r.modified_time.as_deref())),
            Cell::new(format_size(r.size.as_deref())),
            Cell::new(keep_forever_str),
        ]);
    }

    table.to_string()
}

pub fn render_storage_bar(usage: u64, limit: u64) -> String {
    if limit == 0 {
        return "-".to_string();
    }
    let pct = (usage as f64 / limit as f64) * 100.0;
    let clamped_pct = pct.clamp(0.0, 100.0);
    let total_slots: usize = 20;
    let filled_slots = ((clamped_pct / 100.0) * total_slots as f64).round() as usize;
    let empty_slots = total_slots.saturating_sub(filled_slots);
    let bar: String = "█".repeat(filled_slots) + &"░".repeat(empty_slots);
    format!("[{}] {:.1}%", bar, pct)
}

pub fn render_about(about: &DriveAbout) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);

    table.set_header(vec![
        Cell::new("Informasi Akun")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Detail")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
    ]);

    let user_name = about
        .user
        .as_ref()
        .and_then(|u| u.display_name.as_deref())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("-");
    let user_email = about
        .user
        .as_ref()
        .and_then(|u| u.email_address.as_deref())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("-");

    table.add_row(vec![Cell::new("Nama Pengguna"), Cell::new(user_name)]);
    table.add_row(vec![Cell::new("Alamat Email"), Cell::new(user_email)]);

    if let Some(quota) = &about.storage_quota {
        let limit_str = if let Some(lim) = quota.limit.as_deref() {
            format_size(Some(lim))
        } else {
            "Tak terbatas".to_string()
        };
        let usage_str = format_size(quota.usage.as_deref());
        let drive_str = format_size(quota.usage_in_drive.as_deref());
        let trash_str = format_size(quota.usage_in_drive_trash.as_deref());

        table.add_row(vec![Cell::new("Total Penggunaan"), Cell::new(usage_str)]);
        table.add_row(vec![Cell::new("Batas Penyimpanan"), Cell::new(limit_str)]);
        table.add_row(vec![Cell::new("Penggunaan Drive"), Cell::new(drive_str)]);
        table.add_row(vec![Cell::new("Penggunaan Trash"), Cell::new(trash_str)]);

        if let (Some(u), Some(l)) = (
            quota.usage.as_deref().and_then(|s| s.parse::<u64>().ok()),
            quota.limit.as_deref().and_then(|s| s.parse::<u64>().ok()),
        ) {
            let bar = render_storage_bar(u, l);
            table.add_row(vec![Cell::new("Kapasitas Terpakai"), Cell::new(bar)]);
        }
    }

    table.to_string()
}

pub fn render_profiles_table(profiles: &[ProfileStatusInfo]) -> String {
    if profiles.is_empty() {
        return "Tidak ada profil yang terdaftar.".to_string();
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("Profile")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Status")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
        Cell::new("Akun Google")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
        Cell::new("Masa Berlaku")
            .add_attribute(Attribute::Bold)
            .fg(Color::Magenta),
        Cell::new("Lokasi Token")
            .add_attribute(Attribute::Bold)
            .fg(Color::Blue),
    ]);

    for p in profiles {
        let status_cell = if p.is_active {
            Cell::new("* Aktif")
                .fg(Color::Green)
                .add_attribute(Attribute::Bold)
        } else {
            Cell::new("-").fg(Color::DarkGrey)
        };

        let name = p.display_name.as_deref().filter(|s| !s.trim().is_empty());
        let email = p.email.as_deref().filter(|s| !s.trim().is_empty());
        let account_str = if p.is_authenticated {
            match (name, email) {
                (Some(name), Some(email)) => format!("{} ({})", name, email),
                (None, Some(email)) => email.to_string(),
                (Some(name), None) => name.to_string(),
                (None, None) => "Terhubung (tanpa detail email)".to_string(),
            }
        } else {
            "Belum terhubung".to_string()
        };

        let expiry_str = if p.is_authenticated {
            if let Some(secs) = p.expires_in_seconds {
                if secs >= 3600 {
                    format!("{}j {}m tersisa", secs / 3600, (secs % 3600) / 60)
                } else if secs >= 60 {
                    format!("{}m tersisa", secs / 60)
                } else if secs > 0 {
                    format!("{}d tersisa", secs)
                } else {
                    "Kedaluwarsa (auto-refresh)".to_string()
                }
            } else {
                "Auto-refresh aktif".to_string()
            }
        } else {
            "Belum login".to_string()
        };

        let path_str = p.token_path.to_string_lossy().to_string();

        table.add_row(vec![
            Cell::new(&p.name).add_attribute(Attribute::Bold),
            status_cell,
            Cell::new(account_str),
            Cell::new(expiry_str),
            Cell::new(path_str).fg(Color::DarkGrey),
        ]);
    }

    table.to_string()
}

pub fn render_doctor_table(report: &gdrive_core::DoctorReport) -> String {
    use gdrive_core::DoctorStatus;
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    apply_table_arrangement(&mut table);
    table.set_header(vec![
        Cell::new("Komponen")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Status")
            .add_attribute(Attribute::Bold)
            .fg(Color::Green),
        Cell::new("Detail / Rekomendasi Solusi")
            .add_attribute(Attribute::Bold)
            .fg(Color::Yellow),
    ]);

    for c in &report.checks {
        let status_cell = match c.status {
            DoctorStatus::Ok => Cell::new("[OK]")
                .fg(Color::Green)
                .add_attribute(Attribute::Bold),
            DoctorStatus::Warn => Cell::new("[WARN]")
                .fg(Color::Yellow)
                .add_attribute(Attribute::Bold),
            DoctorStatus::Fail => Cell::new("[FAIL]")
                .fg(Color::Red)
                .add_attribute(Attribute::Bold),
        };
        table.add_row(vec![
            Cell::new(&c.component).fg(Color::White),
            status_cell,
            Cell::new(&c.detail),
        ]);
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(None), "-");
        assert_eq!(format_size(Some("500")), "500 B");
        assert_eq!(format_size(Some("2048")), "2.0 KB");
        assert_eq!(format_size(Some("10485760")), "10.00 MB");
    }

    #[test]
    fn test_format_mime() {
        let (icon, label) = format_mime("application/vnd.google-apps.folder");
        assert_eq!(icon, "📁");
        assert_eq!(label, "Folder");
    }

    #[test]
    fn test_format_date() {
        assert_eq!(
            format_date(Some("2026-10-22T01:38:55.000Z")),
            "2026-10-22 01:38"
        );
        assert_eq!(format_date(None), "-");
    }

    #[test]
    fn test_sanitize_download_filename() {
        assert_eq!(sanitize_download_filename("normal.pdf"), "normal.pdf");
        assert_eq!(sanitize_download_filename("../../etc/passwd"), "passwd");
        assert_eq!(
            sanitize_download_filename("..\\..\\windows\\system32\\calc.exe"),
            "calc.exe"
        );
        assert_eq!(sanitize_download_filename(".hidden"), "hidden");
        assert_eq!(sanitize_download_filename("..."), "downloaded_file");
        assert_eq!(sanitize_download_filename(""), "downloaded_file");
    }

    #[test]
    fn test_render_file_info() {
        let file = DriveFile {
            id: "test123".to_string(),
            name: "Document.docx".to_string(),
            mime_type: "application/vnd.google-apps.document".to_string(),
            web_view_link: Some("https://drive.google.com/open?id=test123".to_string()),
            parents: Some(vec!["parent_root".to_string()]),
            size: Some("1024".to_string()),
            modified_time: Some("2026-10-22T01:38:55.000Z".to_string()),
            starred: None,
        };
        let rendered = render_file_info(&file);
        assert!(rendered.contains("test123"));
        assert!(rendered.contains("Document.docx"));
        assert!(rendered.contains("Google Doc"));
        assert!(rendered.contains("1.0 KB"));
        assert!(rendered.contains("parent_root"));
    }

    #[test]
    fn test_render_permissions_table() {
        let perms = vec![
            DrivePermission {
                id: "anyoneWithLink".to_string(),
                permission_type: "anyone".to_string(),
                role: "reader".to_string(),
                email_address: None,
                display_name: None,
                domain: None,
            },
            DrivePermission {
                id: "user-1".to_string(),
                permission_type: "user".to_string(),
                role: "writer".to_string(),
                email_address: Some("bob@example.com".to_string()),
                display_name: Some("Bob".to_string()),
                domain: None,
            },
        ];

        let rendered = render_permissions_table(&perms);
        assert!(rendered.contains("anyoneWithLink"));
        assert!(rendered.contains("anyone"));
        assert!(rendered.contains("reader"));
        assert!(rendered.contains("Siapa saja yang memiliki link (Anyone)"));
        assert!(rendered.contains("user-1"));
        assert!(rendered.contains("writer"));
        assert!(rendered.contains("Bob (bob@example.com)"));

        let empty_rendered = render_permissions_table(&[]);
        assert!(empty_rendered.contains("Tidak ada permission yang ditemukan."));
    }

    #[test]
    fn test_render_comments_table() {
        let comments = vec![DriveComment {
            id: "c1".to_string(),
            content: "Fix typo here".to_string(),
            author: Some(gdrive_core::DriveAuthor {
                display_name: Some("Alice".to_string()),
                email_address: Some("alice@example.com".to_string()),
                me: Some(true),
            }),
            created_time: Some("2026-03-30T10:00:00.000Z".to_string()),
            modified_time: None,
            resolved: Some(false),
        }];
        let out = render_comments_table(&comments);
        assert!(out.contains("c1"));
        assert!(out.contains("Alice"));
        assert!(out.contains("Open"));
        assert!(out.contains("Fix typo here"));

        let empty_out = render_comments_table(&[]);
        assert_eq!(empty_out, "Tidak ada komentar pada file ini.");
    }

    #[test]
    fn test_render_revisions_table() {
        let revs = vec![DriveRevision {
            id: "rev123".to_string(),
            mime_type: Some("application/pdf".to_string()),
            modified_time: Some("2026-03-30T11:00:00.000Z".to_string()),
            keep_forever: Some(true),
            size: Some("2048".to_string()),
            last_modifying_user: Some(gdrive_core::DriveAuthor {
                display_name: Some("Charlie".to_string()),
                email_address: Some("charlie@example.com".to_string()),
                me: None,
            }),
        }];
        let out = render_revisions_table(&revs);
        assert!(out.contains("rev123"));
        assert!(out.contains("Charlie"));
        assert!(out.contains("2.0 KB"));
        assert!(out.contains("Yes"));

        let empty_out = render_revisions_table(&[]);
        assert_eq!(empty_out, "Tidak ada riwayat revisi pada file ini.");
    }

    #[test]
    fn test_render_storage_bar() {
        let bar = render_storage_bar(5, 10);
        assert!(bar.contains("50.0%"));
        assert!(bar.contains("██████████░░░░░░░░░░"));
    }

    #[test]
    fn test_render_profiles_table() {
        let list = vec![
            ProfileStatusInfo {
                name: "default".to_string(),
                is_active: true,
                is_authenticated: true,
                email: Some("me@gmail.com".to_string()),
                display_name: Some("Me".to_string()),
                expires_in_seconds: Some(3500),
                token_path: std::path::PathBuf::from("/path/to/token.json"),
            },
            ProfileStatusInfo {
                name: "work".to_string(),
                is_active: false,
                is_authenticated: false,
                email: None,
                display_name: None,
                expires_in_seconds: None,
                token_path: std::path::PathBuf::from("/path/to/work/token.json"),
            },
        ];

        let out = render_profiles_table(&list);
        assert!(out.contains("default"));
        assert!(out.contains("* Aktif"));
        assert!(out.contains("Me (me@gmail.com)"));
        assert!(out.contains("58m tersisa"));
        assert!(out.contains("work"));
        assert!(out.contains("Belum terhubung"));
        assert!(out.contains("Belum login"));
    }

    #[test]
    fn test_render_about() {
        let about = DriveAbout {
            user: Some(gdrive_core::DriveAboutUser {
                display_name: Some("Ada Lovelace".to_string()),
                email_address: Some("ada@example.com".to_string()),
                permission_id: None,
                photo_link: None,
            }),
            storage_quota: Some(gdrive_core::DriveAboutStorageQuota {
                limit: Some("10737418240".to_string()), // 10 GB
                usage: Some("5368709120".to_string()),  // 5 GB
                usage_in_drive: Some("4294967296".to_string()),
                usage_in_drive_trash: Some("1073741824".to_string()),
            }),
        };

        let out = render_about(&about);
        assert!(out.contains("Ada Lovelace"));
        assert!(out.contains("ada@example.com"));
        assert!(out.contains("10.00 GB"));
        assert!(out.contains("5.00 GB"));
        assert!(out.contains("50.0%"));
    }
}
