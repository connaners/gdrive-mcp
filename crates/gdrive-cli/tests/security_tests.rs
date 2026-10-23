use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_gitignore_blocks_secrets() {
    let gitignore_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".gitignore");

    assert!(
        gitignore_path.exists(),
        ".gitignore must exist at workspace root"
    );

    let content = fs::read_to_string(&gitignore_path).expect("Failed to read .gitignore");

    let required_patterns = [
        "token.json",
        "credentials.json",
        ".env",
        "client_secret*.json",
    ];

    for pattern in required_patterns {
        assert!(
            content.contains(pattern),
            ".gitignore is missing critical secret protection pattern: '{}'",
            pattern
        );
    }
}

#[test]
fn test_no_secrets_in_tracked_files() {
    // Check if git is available
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                assert!(
                    !trimmed.ends_with("token.json"),
                    "Tracked file '{}' violates Rule 4 (Zero Secret Leaks): token.json must not be tracked in git!",
                    trimmed
                );
                assert!(
                    !trimmed.ends_with("credentials.json"),
                    "Tracked file '{}' violates Rule 4 (Zero Secret Leaks): credentials.json must not be tracked in git!",
                    trimmed
                );
                assert!(
                    !trimmed.starts_with(".env") && !trimmed.ends_with(".env"),
                    "Tracked file '{}' violates Rule 4 (Zero Secret Leaks): .env files must not be tracked in git!",
                    trimmed
                );
            }
        }
    }
}

#[test]
fn test_no_hardcoded_google_tokens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let crates_dir = root.join("crates");
    let token_prefix = format!("{}.{}", "ya29", "");
    fn scan_dir(dir: &Path, prefix: &str) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Skip target directory if any
                    if path.file_name().and_then(|s| s.to_str()) != Some("target") {
                        scan_dir(&path, prefix);
                    }
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    // Skip the security_tests.rs file itself
                    if path.file_name().and_then(|s| s.to_str()) == Some("security_tests.rs") {
                        continue;
                    }
                    let content = fs::read_to_string(&path).unwrap_or_default();
                    assert!(
                        !content.contains(prefix),
                        "Source file {:?} contains potential real Google access token ({})",
                        path,
                        prefix
                    );
                }
            }
        }
    }

    scan_dir(&crates_dir, &token_prefix);
}
