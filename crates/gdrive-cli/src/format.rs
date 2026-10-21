use comfy_table::{presets::UTF8_FULL, Attribute, Cell, Color, ContentArrangement, Table};
use gdrive_core::DriveFile;

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
}
