use std::path::{Path, PathBuf};

/// Cleans a path string by:
/// 1. Trimming leading and trailing whitespace.
/// 2. Stripping `file://` scheme if present (e.g. `file:///path/to/file` -> `/path/to/file`).
/// 3. Stripping enclosing matching or stray double/single quotes (`"` or `'`).
/// 4. Unescaping shell-escaped characters (such as `\ ` to ` `) on Unix platforms.
/// 5. Expanding tilde prefix (`~` or `~/...`) to the user's home directory.
pub fn clean_path_str(raw: &str) -> String {
    let mut s = raw.trim();

    // 1. Strip file:// scheme (e.g., from dragged browser URLs or file managers)
    if s.starts_with("file://") {
        if let Ok(url) = url::Url::parse(s) {
            if let Ok(file_path) = url.to_file_path() {
                return file_path.to_string_lossy().to_string();
            }
        }
        if let Some(rest) = s.strip_prefix("file://") {
            s = rest;
        }
    }

    // 2. Strip matching surrounding quotes iteratively (handles nested quotes or quotes with spaces)
    loop {
        let trimmed = s.trim();
        if ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
            && trimmed.len() >= 2
        {
            s = &trimmed[1..trimmed.len() - 1];
            continue;
        }
        s = trimmed;
        break;
    }

    // 3. Strip any stray leading or trailing quote (e.g. accidental partial copy-paste)
    s = s.strip_prefix('"').unwrap_or(s);
    s = s.strip_suffix('"').unwrap_or(s);
    s = s.strip_prefix('\'').unwrap_or(s);
    s = s.strip_suffix('\'').unwrap_or(s);
    let s = s.trim();

    // 4. Handle shell backslash escapes (e.g., /path/to/my\ file.jpg -> /path/to/my file.jpg)
    let unescaped = unescape_shell_escapes(s);

    // 5. Expand tilde `~` or `~/...`
    expand_tilde_str(&unescaped)
}

/// Resolves a raw user input string into a cleaned, absolute or canonical `PathBuf`.
pub fn clean_local_path(raw: &str) -> PathBuf {
    let cleaned = clean_path_str(raw);
    let pb = PathBuf::from(&cleaned);

    // If path is relative, resolve against current directory
    if pb.is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(&pb))
            .unwrap_or(pb)
    } else {
        pb
    }
}

/// Unescapes shell-escaped characters (e.g. `\ ` -> ` `) on non-Windows platforms.
/// If the path with literal backslashes already exists on disk, it is preserved.
fn unescape_shell_escapes(s: &str) -> String {
    #[cfg(windows)]
    {
        s.to_string()
    }
    #[cfg(not(windows))]
    {
        // If the path with literal backslashes already exists as a file or dir on disk, keep it
        if Path::new(s).exists() {
            return s.to_string();
        }

        if !s.contains('\\') {
            return s.to_string();
        }

        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    // Common characters escaped by shells: space, brackets, parens, quotes, etc.
                    if matches!(
                        next,
                        ' ' | '\t'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | '&'
                            | ';'
                            | '|'
                            | '<'
                            | '>'
                            | '\''
                            | '"'
                            | '$'
                            | '`'
                            | '\\'
                    ) {
                        out.push(next);
                        chars.next();
                        continue;
                    }
                }
            }
            out.push(c);
        }
        out
    }
}

/// Expands leading `~` or `~/` to the user's home directory.
fn expand_tilde_str(s: &str) -> String {
    if s == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().to_string();
        }
    } else if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().to_string();
        }
    } else if cfg!(windows) {
        if let Some(rest) = s.strip_prefix("~\\") {
            if let Some(home) = dirs::home_dir() {
                return home.join(rest).to_string_lossy().to_string();
            }
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_path_double_quotes_and_trailing_spaces() {
        let raw = "\"/Users/rohmnsa/On My Own/Own Business/LKPP_Abdul Rohman Syafii_0.jpg\" ";
        let cleaned = clean_path_str(raw);
        assert_eq!(
            cleaned,
            "/Users/rohmnsa/On My Own/Own Business/LKPP_Abdul Rohman Syafii_0.jpg"
        );
    }

    #[test]
    fn test_clean_path_single_quotes() {
        let raw = " '/path/to/my document.pdf' ";
        let cleaned = clean_path_str(raw);
        assert_eq!(cleaned, "/path/to/my document.pdf");
    }

    #[test]
    fn test_clean_path_nested_quotes() {
        let raw = "\"'/path/to/nested.txt'\"";
        let cleaned = clean_path_str(raw);
        assert_eq!(cleaned, "/path/to/nested.txt");
    }

    #[test]
    fn test_clean_path_stray_quotes() {
        let raw = "\"/path/to/unclosed.txt";
        let cleaned = clean_path_str(raw);
        assert_eq!(cleaned, "/path/to/unclosed.txt");

        let raw2 = "/path/to/trailing_quote.txt\"";
        let cleaned2 = clean_path_str(raw2);
        assert_eq!(cleaned2, "/path/to/trailing_quote.txt");
    }

    #[test]
    fn test_clean_path_tilde_expansion() {
        if let Some(home) = dirs::home_dir() {
            let raw = "~/Documents/report.docx";
            let cleaned = clean_path_str(raw);
            assert_eq!(
                cleaned,
                home.join("Documents/report.docx")
                    .to_string_lossy()
                    .to_string()
            );

            // Tilde wrapped in quotes
            let raw_quoted = "\"~/Documents/report.docx\" ";
            let cleaned_quoted = clean_path_str(raw_quoted);
            assert_eq!(
                cleaned_quoted,
                home.join("Documents/report.docx")
                    .to_string_lossy()
                    .to_string()
            );
        }
    }

    #[test]
    fn test_clean_path_file_url() {
        let raw = "file:///tmp/test.txt";
        let cleaned = clean_path_str(raw);
        assert_eq!(cleaned, "/tmp/test.txt");
    }

    #[test]
    #[cfg(not(windows))]
    fn test_clean_path_shell_escapes() {
        let raw = "/Users/rohmnsa/On\\ My\\ Own/Own\\ Business/test.jpg";
        let cleaned = clean_path_str(raw);
        assert_eq!(cleaned, "/Users/rohmnsa/On My Own/Own Business/test.jpg");
    }

    #[test]
    fn test_clean_local_path_absolute_resolution() {
        let raw = "\"/tmp/some_file.pdf\"";
        let pb = clean_local_path(raw);
        assert!(pb.is_absolute());
        assert_eq!(pb, PathBuf::from("/tmp/some_file.pdf"));
    }
}
