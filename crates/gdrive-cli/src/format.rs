use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use gdrive_core::{DriveComment, DriveFile, DrivePermission, DriveRevision};

pub fn format_size(bytes_str: Option<&str>) -> String {
    let bytes = match bytes_str.and_then(|s| s.parse::<u64>().ok()) {
        Some(b) => b,
        None => return "-".to_string(),
    };

    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_mime(mime: &str) -> (&'static str, &'static str) {
    match mime {
        "application/vnd.google-apps.folder" => ("📁", "Folder"),
        "application/vnd.google-apps.document" => ("📝", "Google Doc"),
        "application/vnd.google-apps.spreadsheet" => ("📊", "Google Sheet"),
        "application/vnd.google-apps.presentation" => ("📽️", "Google Slides"),
        "application/pdf" => ("📄", "PDF"),
        m if m.starts_with("image/") => ("🖼️", "Image"),
        m if m.starts_with("video/") => ("🎬", "Video"),
        m if m.starts_with("audio/") => ("🎵", "Audio"),
        m if m.starts_with("text/") => ("📜", "Text"),
        "application/zip" | "application/x-tar" | "application/gzip" => ("📦", "Archive"),
        _ => ("📎", "File"),
    }
}

pub fn format_date(iso_date: Option<&str>) -> String {
    match iso_date {
        Some(d) if d.len() >= 16 => {
            let date = &d[..10];
            let time = &d[11..16];
            format!("{} {}", date, time)
        }
        Some(d) => d.to_string(),
        None => "-".to_string(),
    }
}

pub fn render_files_table(files: &[DriveFile]) -> String {
    if files.is_empty() {
        return "Tidak ada file atau folder yang ditemukan.".to_string();
    }

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
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
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

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
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
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
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
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
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
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
}
