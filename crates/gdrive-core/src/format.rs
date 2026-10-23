/// Format ukuran bytes ke representasi terbaca manusia (B, KB, MB, GB)
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

/// Format MIME type menjadi icon emoji dan label singkat
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

/// Format tanggal ISO 8601 (2026-03-29T10:00:00.000Z) ke bentuk ringkas "YYYY-MM-DD HH:MM"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(None), "-");
        assert_eq!(format_size(Some("invalid")), "-");
        assert_eq!(format_size(Some("500")), "500 B");
        assert_eq!(format_size(Some("2048")), "2.0 KB");
        assert_eq!(format_size(Some("5242880")), "5.00 MB");
        assert_eq!(format_size(Some("1073741824")), "1.00 GB");
    }

    #[test]
    fn test_format_mime() {
        assert_eq!(
            format_mime("application/vnd.google-apps.folder"),
            ("📁", "Folder")
        );
        assert_eq!(format_mime("application/pdf"), ("📄", "PDF"));
        assert_eq!(format_mime("unknown/type"), ("📎", "File"));
    }

    #[test]
    fn test_format_date() {
        assert_eq!(format_date(None), "-");
        assert_eq!(
            format_date(Some("2026-03-29T10:00:00.000Z")),
            "2026-03-29 10:00"
        );
    }
}
