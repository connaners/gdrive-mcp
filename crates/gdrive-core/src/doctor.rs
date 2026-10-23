use serde::{Deserialize, Serialize};

use crate::auth::AuthManager;
use crate::client::drive_api_base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheckItem {
    pub component: String,
    pub status: DoctorStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub checks: Vec<DoctorCheckItem>,
    pub ok_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
}

impl DoctorReport {
    pub fn new(checks: Vec<DoctorCheckItem>) -> Self {
        let ok_count = checks
            .iter()
            .filter(|c| c.status == DoctorStatus::Ok)
            .count();
        let warn_count = checks
            .iter()
            .filter(|c| c.status == DoctorStatus::Warn)
            .count();
        let fail_count = checks
            .iter()
            .filter(|c| c.status == DoctorStatus::Fail)
            .count();

        Self {
            checks,
            ok_count,
            warn_count,
            fail_count,
        }
    }
}

pub async fn run_doctor_checks(
    target_profile: Option<&str>,
    terminal_info: Option<(bool, Option<(u16, u16)>)>,
) -> DoctorReport {
    let mut checks = Vec::new();

    // 1. Config directory & writeability
    check_config_dir(&mut checks);

    // 2. OAuth Credentials
    check_credentials(&mut checks);

    // 3. Active Profile & Token
    check_token(&mut checks, target_profile);

    // 4. Network reachability
    check_network(&mut checks).await;

    // 5. Terminal / TTY capabilities
    check_terminal(&mut checks, terminal_info);

    DoctorReport::new(checks)
}

fn check_config_dir(checks: &mut Vec<DoctorCheckItem>) {
    match AuthManager::config_dir() {
        Ok(dir) => {
            let probe_file = dir.join(".doctor_probe");
            match std::fs::write(&probe_file, "ok") {
                Ok(_) => {
                    let _ = std::fs::remove_file(&probe_file);
                    checks.push(DoctorCheckItem {
                        component: "Config Directory".to_string(),
                        status: DoctorStatus::Ok,
                        detail: format!("Dapat ditulisi: {:?}", dir),
                    });
                }
                Err(e) => {
                    checks.push(DoctorCheckItem {
                        component: "Config Directory".to_string(),
                        status: DoctorStatus::Fail,
                        detail: format!("Tidak dapat menulis ke {:?}: {}", dir, e),
                    });
                }
            }
        }
        Err(e) => {
            checks.push(DoctorCheckItem {
                component: "Config Directory".to_string(),
                status: DoctorStatus::Fail,
                detail: format!("Gagal menentukan user config directory: {}", e),
            });
        }
    }
}

fn check_credentials(checks: &mut Vec<DoctorCheckItem>) {
    let path = match AuthManager::credentials_path() {
        Ok(p) => p,
        Err(e) => {
            checks.push(DoctorCheckItem {
                component: "OAuth Credentials".to_string(),
                status: DoctorStatus::Fail,
                detail: format!("Gagal menentukan path credentials: {}", e),
            });
            return;
        }
    };

    if !path.exists() {
        checks.push(DoctorCheckItem {
            component: "OAuth Credentials".to_string(),
            status: DoctorStatus::Warn,
            detail: "credentials.json belum ada. Jalankan 'gdrive auth setup'".to_string(),
        });
        return;
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match AuthManager::parse_credentials_json(&content) {
            Ok(_) => checks.push(DoctorCheckItem {
                component: "OAuth Credentials".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("Format valid ({:?})", path),
            }),
            Err(e) => checks.push(DoctorCheckItem {
                component: "OAuth Credentials".to_string(),
                status: DoctorStatus::Fail,
                detail: format!("File credentials corrupt ({})", e),
            }),
        },
        Err(e) => checks.push(DoctorCheckItem {
            component: "OAuth Credentials".to_string(),
            status: DoctorStatus::Fail,
            detail: format!("Tidak dapat membaca credentials: {}", e),
        }),
    }
}

fn check_token(checks: &mut Vec<DoctorCheckItem>, target_profile: Option<&str>) {
    let active_profile = target_profile
        .map(|s| s.to_string())
        .unwrap_or_else(AuthManager::get_active_profile);

    match AuthManager::load_saved_token_for_profile(Some(&active_profile)) {
        Ok(token) => {
            let email = token
                .account_email
                .as_deref()
                .unwrap_or("email tidak diketahui");
            if !token.is_expired() {
                let mins = token.seconds_remaining().map(|s| s / 60).unwrap_or(0);
                checks.push(DoctorCheckItem {
                    component: "OAuth Token".to_string(),
                    status: DoctorStatus::Ok,
                    detail: format!(
                        "Profil '{}' ({}) - Aktif (berlaku ~{} menit)",
                        active_profile, email, mins
                    ),
                });
            } else if token.refresh_token.is_some() {
                checks.push(DoctorCheckItem {
                    component: "OAuth Token".to_string(),
                    status: DoctorStatus::Ok,
                    detail: format!(
                        "Profil '{}' ({}) - Token kedaluwarsa tapi auto-refresh tersedia",
                        active_profile, email
                    ),
                });
            } else {
                checks.push(DoctorCheckItem {
                    component: "OAuth Token".to_string(),
                    status: DoctorStatus::Warn,
                    detail: format!(
                        "Profil '{}' ({}) - Token kedaluwarsa tanpa refresh token. Jalankan 'gdrive auth login'",
                        active_profile, email
                    ),
                });
            }
        }
        Err(_) => {
            checks.push(DoctorCheckItem {
                component: "OAuth Token".to_string(),
                status: DoctorStatus::Warn,
                detail: format!(
                    "Profil '{}' belum login. Jalankan 'gdrive auth login'",
                    active_profile
                ),
            });
        }
    }
}

async fn check_network(checks: &mut Vec<DoctorCheckItem>) {
    let base_url = drive_api_base();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    match client {
        Ok(http) => match http.get(format!("{}/about", base_url)).send().await {
            Ok(resp) => checks.push(DoctorCheckItem {
                component: "Network API".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("Terhubung ke Google Drive API (status: {})", resp.status()),
            }),
            Err(e) => checks.push(DoctorCheckItem {
                component: "Network API".to_string(),
                status: DoctorStatus::Warn,
                detail: format!("Koneksi ke {} terhambat/offline: {}", base_url, e),
            }),
        },
        Err(e) => checks.push(DoctorCheckItem {
            component: "Network API".to_string(),
            status: DoctorStatus::Fail,
            detail: format!("Gagal inisialisasi HTTP client: {}", e),
        }),
    }
}

fn check_terminal(
    checks: &mut Vec<DoctorCheckItem>,
    terminal_info: Option<(bool, Option<(u16, u16)>)>,
) {
    let (is_term, term_size) = match terminal_info {
        Some((t, size)) => (t, size),
        None => (false, None),
    };

    match (is_term, term_size) {
        (true, Some((w, h))) if w >= 80 && h >= 24 => {
            checks.push(DoctorCheckItem {
                component: "Terminal / TTY".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("Ukuran {}x{} (memenuhi syarat TUI minimum 80x24)", w, h),
            });
        }
        (true, Some((w, h))) => {
            checks.push(DoctorCheckItem {
                component: "Terminal / TTY".to_string(),
                status: DoctorStatus::Warn,
                detail: format!("Ukuran {}x{} kurang dari batas nyaman TUI (>= 80x24)", w, h),
            });
        }
        (true, None) => {
            checks.push(DoctorCheckItem {
                component: "Terminal / TTY".to_string(),
                status: DoctorStatus::Ok,
                detail: "TTY terdeteksi (ukuran tidak diketahui)".to_string(),
            });
        }
        (false, _) => {
            checks.push(DoctorCheckItem {
                component: "Terminal / TTY".to_string(),
                status: DoctorStatus::Ok,
                detail: "Non-interactive / piped stdout (mode CLI & MCP stdio optimal)".to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doctor_report_structure() {
        let report = run_doctor_checks(Some("test-profile"), Some((true, Some((100, 30))))).await;
        assert_eq!(report.checks.len(), 5);
        assert_eq!(report.ok_count + report.warn_count + report.fail_count, 5);
    }
}
