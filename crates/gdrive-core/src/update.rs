use serde::{Deserialize, Serialize};

/// Informasi rilis dari GitHub Releases API.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub assets: Vec<GitHubReleaseAsset>,
}

/// Asset file yang terlampir pada rilis GitHub.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Informasi status ketersediaan pembaruan binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_name: String,
    pub release_url: String,
    pub published_at: Option<String>,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
}

/// Hasil dari eksekusi pembaruan mandiri (self-update).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSuccess {
    pub current_version: String,
    pub updated_to: String,
    pub binary_path: String,
    pub message: String,
}

/// Representasi versi Semantic Versioning (SemVer) sederhana untuk perbandingan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    /// Mem-parse string versi menjadi `SemVer`.
    /// Mengabaikan prefix 'v'/'V', dan mengabaikan sufiks pre-release/build (misal `-beta.1`, `+build`).
    pub fn parse(v: &str) -> Option<Self> {
        let s = v.trim().trim_start_matches(['v', 'V']);
        let core = s.split(['-', '+']).next().unwrap_or(s);
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        let patch = parts.next()?.parse::<u64>().ok()?;
        Some(SemVer {
            major,
            minor,
            patch,
        })
    }
}

/// Mengembalikan true jika versi `latest` lebih baru daripada `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (SemVer::parse(current), SemVer::parse(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Mencari asset rilis GitHub yang cocok dengan sistem operasi dan arsitektur mesin saat ini.
pub fn find_matching_asset<'a>(
    assets: &'a [GitHubReleaseAsset],
    os: &str,
    arch: &str,
) -> Option<&'a GitHubReleaseAsset> {
    let os_lower = os.to_ascii_lowercase();
    let arch_lower = arch.to_ascii_lowercase();

    let os_patterns: &[&str] = match os_lower.as_str() {
        "macos" | "darwin" => &["apple-darwin", "darwin", "macos"],
        "linux" => &["linux"],
        "windows" => &["windows", "win64", "win32"],
        _ => &[os_lower.as_str()],
    };

    let arch_patterns: &[&str] = match arch_lower.as_str() {
        "x86_64" | "amd64" | "x64" => &["x86_64", "amd64", "x64"],
        "aarch64" | "arm64" => &["aarch64", "arm64"],
        _ => &[arch_lower.as_str()],
    };

    assets.iter().find(|asset| {
        let name = asset.name.to_ascii_lowercase();
        let is_valid_ext = name.ends_with(".tar.gz") || name.ends_with(".tgz");
        if !is_valid_ext {
            return false;
        }
        if name.ends_with(".sha256")
            || name.ends_with(".sig")
            || name.ends_with(".asc")
            || name.ends_with(".txt")
        {
            return false;
        }
        let matches_os = os_patterns.iter().any(|pat| name.contains(pat));
        let matches_arch = arch_patterns.iter().any(|pat| name.contains(pat));
        matches_os && matches_arch
    })
}

/// Mengekstrak konten binary dari archive .tar.gz ke memori.
pub fn extract_binary_from_tar_gz<R: std::io::Read>(
    reader: R,
    binary_name: &str,
) -> anyhow::Result<Vec<u8>> {
    let gz = flate2::read::GzDecoder::new(reader);
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_file = entry.header().entry_type().is_file();
        if is_file && file_name == binary_name {
            let mut buf = Vec::new();
            use std::io::Read;
            entry.read_to_end(&mut buf)?;
            if !buf.is_empty() {
                return Ok(buf);
            }
        }
    }
    anyhow::bail!(
        "Binary '{}' tidak ditemukan di dalam archive tar.gz",
        binary_name
    )
}

/// Memeriksa ketersediaan versi terbaru dari GitHub Releases.
pub async fn check_for_update(repo: Option<&str>) -> anyhow::Result<UpdateInfo> {
    let repo = repo.unwrap_or("connaners/gdrive-mcp");
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let client = reqwest::Client::builder().build()?;
    let mut req = client
        .get(&url)
        .header(
            reqwest::header::USER_AGENT,
            format!("gdrive-mcp/{}", env!("CARGO_PKG_VERSION")),
        )
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");

    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            req = req.header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", trimmed),
            );
        }
    }

    let resp = req.send().await.map_err(|e| {
        anyhow::anyhow!(
            "Gagal menghubungi GitHub Releases API: {}. Periksa koneksi internet Anda.",
            e
        )
    })?;

    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "Batas panggilan GitHub API tercapai (HTTP 403 Rate Limit). Set environment variable GITHUB_TOKEN atau GH_TOKEN untuk menaikkan batas kuota: {}",
            text
        );
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API error (HTTP {}): {}", status, text);
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Gagal mem-parse respon JSON dari GitHub Releases: {}", e))?;

    let current_ver = env!("CARGO_PKG_VERSION");
    let latest_ver = release.tag_name.trim_start_matches(['v', 'V']);
    let update_available = is_newer_version(current_ver, &release.tag_name);

    let matching_asset = find_matching_asset(
        &release.assets,
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let download_url = matching_asset.map(|a| a.browser_download_url.clone());

    Ok(UpdateInfo {
        current_version: current_ver.to_string(),
        latest_version: latest_ver.to_string(),
        update_available,
        release_name: release.name.unwrap_or_else(|| release.tag_name.clone()),
        release_url: release.html_url,
        published_at: release.published_at,
        release_notes: release.body,
        download_url,
    })
}

/// Menjalankan pembaruan mandiri (self-update) binary saat ini dengan binary terbaru dari GitHub Releases.
pub async fn execute_self_update(
    repo: Option<&str>,
    target_exe: Option<std::path::PathBuf>,
) -> anyhow::Result<UpdateSuccess> {
    let exe_path = match target_exe {
        Some(p) => p,
        None => {
            let current = std::env::current_exe()?;
            current.canonicalize().unwrap_or(current)
        }
    };

    let info = check_for_update(repo).await?;

    if !info.update_available {
        return Ok(UpdateSuccess {
            current_version: info.current_version.clone(),
            updated_to: info.current_version.clone(),
            binary_path: exe_path.display().to_string(),
            message: format!(
                "gdrive sudah berada pada versi terbaru (v{}). Tidak ada pembaruan yang diperlukan.",
                info.current_version
            ),
        });
    }

    let download_url = info.download_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "Tidak ditemukan asset binary yang cocok untuk platform {} ({}) pada rilis {}. Silakan unduh secara manual dari: {}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            info.latest_version,
            info.release_url
        )
    })?;

    let client = reqwest::Client::builder().build()?;
    let req = client.get(download_url).header(
        reqwest::header::USER_AGENT,
        format!("gdrive-mcp/{}", env!("CARGO_PKG_VERSION")),
    );

    let resp = req.send().await.map_err(|e| {
        anyhow::anyhow!(
            "Gagal mengunduh asset pembaruan dari '{}': {}",
            download_url,
            e
        )
    })?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Gagal mengunduh asset pembaruan (HTTP {}). URL: {}",
            resp.status(),
            download_url
        );
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("Gagal membaca bytes dari unduhan asset: {}", e))?;

    let binary_name = if cfg!(windows) {
        "gdrive.exe"
    } else {
        "gdrive"
    };
    let binary_bytes = extract_binary_from_tar_gz(std::io::Cursor::new(&bytes), binary_name)?;

    let parent_dir = exe_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let tmp_path = parent_dir.join(format!(".gdrive.update.{}.tmp", std::process::id()));

    let write_res = (|| -> anyhow::Result<()> {
        std::fs::write(&tmp_path, &binary_bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&tmp_path, perms)?;
        }

        #[cfg(target_os = "macos")]
        {
            let output = std::process::Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&tmp_path)
                .output();
            match output {
                Ok(out) if !out.status.success() => {
                    let err = String::from_utf8_lossy(&out.stderr);
                    tracing::warn!(
                        "codesign gagal dengan status {}: {}",
                        out.status,
                        err.trim()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Gagal menjalankan ad-hoc codesign pada file sementara: {}",
                        e
                    );
                }
                _ => {}
            }
        }

        #[cfg(not(windows))]
        {
            std::fs::rename(&tmp_path, &exe_path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    anyhow::anyhow!(
                        "Akses ditolak saat mengganti binary di '{}'. Coba jalankan dengan hak akses elevated (misal: sudo) atau periksa izin direktori: {}",
                        exe_path.display(),
                        e
                    )
                } else {
                    anyhow::anyhow!("Gagal mengganti binary di '{}': {}", exe_path.display(), e)
                }
            })?;
        }

        #[cfg(windows)]
        {
            let old_path = exe_path.with_extension("exe.old");
            let _ = std::fs::remove_file(&old_path);
            std::fs::rename(&exe_path, &old_path)?;
            if let Err(e) = std::fs::rename(&tmp_path, &exe_path) {
                let _ = std::fs::rename(&old_path, &exe_path);
                return Err(anyhow::anyhow!("Gagal mengganti binary di Windows: {}", e));
            }
            let _ = std::fs::remove_file(&old_path);
        }

        Ok(())
    })();

    if write_res.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    write_res?;

    Ok(UpdateSuccess {
        current_version: info.current_version,
        updated_to: info.latest_version.clone(),
        binary_path: exe_path.display().to_string(),
        message: format!(
            "Berhasil memperbarui gdrive ke versi v{} di '{}'.",
            info.latest_version,
            exe_path.display()
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parsing_and_comparison() {
        assert_eq!(
            SemVer::parse("0.3.0"),
            Some(SemVer {
                major: 0,
                minor: 3,
                patch: 0
            })
        );
        assert_eq!(
            SemVer::parse("v0.3.1"),
            Some(SemVer {
                major: 0,
                minor: 3,
                patch: 1
            })
        );
        assert_eq!(
            SemVer::parse("V1.0.0-rc.1"),
            Some(SemVer {
                major: 1,
                minor: 0,
                patch: 0
            })
        );
        assert_eq!(
            SemVer::parse("0.3.1+build20261023"),
            Some(SemVer {
                major: 0,
                minor: 3,
                patch: 1
            })
        );

        assert!(is_newer_version("0.3.0", "0.3.1"));
        assert!(is_newer_version("0.3.0", "v0.3.1"));
        assert!(is_newer_version("0.3.0", "1.0.0"));
        assert!(is_newer_version("0.9.9", "v1.0.0"));
        assert!(!is_newer_version("0.3.0", "0.3.0"));
        assert!(!is_newer_version("v0.3.0", "0.3.0"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.3.1", "0.3.0"));
    }

    #[test]
    fn test_asset_matching_os_arch() {
        let assets = vec![
            GitHubReleaseAsset {
                name: "gdrive-linux-x86_64.tar.gz".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-linux-x86_64.tar.gz".to_string(),
                size: 10_000_000,
            },
            GitHubReleaseAsset {
                name: "gdrive-aarch64-unknown-linux-gnu.tar.gz".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-aarch64-unknown-linux-gnu.tar.gz".to_string(),
                size: 10_000_000,
            },
            GitHubReleaseAsset {
                name: "gdrive-aarch64-apple-darwin.tar.gz".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-aarch64-apple-darwin.tar.gz".to_string(),
                size: 11_000_000,
            },
            GitHubReleaseAsset {
                name: "gdrive-x86_64-apple-darwin.tar.gz".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-x86_64-apple-darwin.tar.gz".to_string(),
                size: 11_000_000,
            },
            GitHubReleaseAsset {
                name: "gdrive-x86_64-apple-darwin.tar.gz.sha256".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-x86_64-apple-darwin.tar.gz.sha256".to_string(),
                size: 64,
            },
            GitHubReleaseAsset {
                name: "gdrive-windows-x86_64.tar.gz".to_string(),
                browser_download_url: "https://github.com/connaners/gdrive-mcp/releases/download/v0.3.1/gdrive-windows-x86_64.tar.gz".to_string(),
                size: 12_000_000,
            },
        ];

        let linux_x64 = find_matching_asset(&assets, "linux", "x86_64");
        assert!(linux_x64.is_some());
        assert_eq!(linux_x64.unwrap().name, "gdrive-linux-x86_64.tar.gz");

        let linux_arm = find_matching_asset(&assets, "linux", "aarch64");
        assert!(linux_arm.is_some());
        assert_eq!(
            linux_arm.unwrap().name,
            "gdrive-aarch64-unknown-linux-gnu.tar.gz"
        );

        let macos_arm = find_matching_asset(&assets, "macos", "aarch64");
        assert!(macos_arm.is_some());
        assert_eq!(
            macos_arm.unwrap().name,
            "gdrive-aarch64-apple-darwin.tar.gz"
        );

        let darwin_x64 = find_matching_asset(&assets, "darwin", "x86_64");
        assert!(darwin_x64.is_some());
        assert_eq!(
            darwin_x64.unwrap().name,
            "gdrive-x86_64-apple-darwin.tar.gz"
        );

        let windows_x64 = find_matching_asset(&assets, "windows", "x86_64");
        assert!(windows_x64.is_some());
        assert_eq!(windows_x64.unwrap().name, "gdrive-windows-x86_64.tar.gz");

        // Non-existent platform
        let freebsd = find_matching_asset(&assets, "freebsd", "mips");
        assert!(freebsd.is_none());
    }

    #[test]
    fn test_tar_gz_extraction() -> anyhow::Result<()> {
        let dummy_binary_content = b"#!/usr/bin/env bash\necho 'gdrive mock binary'\n";

        let mut gz_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        {
            let mut tar_builder = tar::Builder::new(&mut gz_encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(dummy_binary_content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar_builder.append_data(&mut header, "gdrive", &dummy_binary_content[..])?;
            tar_builder.finish()?;
        }
        let archive_bytes = gz_encoder.finish()?;

        let extracted = extract_binary_from_tar_gz(std::io::Cursor::new(&archive_bytes), "gdrive")?;
        assert_eq!(extracted, dummy_binary_content);

        let not_found =
            extract_binary_from_tar_gz(std::io::Cursor::new(&archive_bytes), "nonexistent");
        assert!(not_found.is_err());

        Ok(())
    }
}
