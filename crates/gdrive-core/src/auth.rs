use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn default_token_type() -> String {
    "Bearer".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    /// Unix timestamp dalam detik kapan token kedaluwarsa
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default = "default_token_type")]
    pub token_type: String,
}

impl TokenData {
    /// Hitung unix timestamp saat ini dalam detik
    pub fn current_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Set `expires_at` otomatis berdasarkan `expires_in` jika belum ada
    pub fn with_computed_expiry(mut self) -> Self {
        if self.expires_at.is_none() {
            if let Some(exp_in) = self.expires_in {
                self.expires_at = Some(Self::current_timestamp_secs() + exp_in);
            }
        }
        self
    }

    /// Cek apakah token sudah kedaluwarsa atau sisa waktu < 300 detik (5 menit buffer)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = Self::current_timestamp_secs();
            // Jika waktu sekarang >= expiry - 300 detik, anggap expired untuk auto-refresh
            now + 300 >= expires_at
        } else {
            false
        }
    }

    /// Menghitung sisa waktu token dalam detik (None jika tidak ada info expiry atau sudah lewat)
    pub fn seconds_remaining(&self) -> Option<u64> {
        let expires_at = self.expires_at?;
        let now = Self::current_timestamp_secs();
        if expires_at > now {
            Some(expires_at - now)
        } else {
            Some(0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilesConfig {
    #[serde(default = "default_active_profile")]
    pub active_profile: String,
    #[serde(default = "default_profiles_list")]
    pub profiles: Vec<String>,
}

fn default_active_profile() -> String {
    "default".to_string()
}

fn default_profiles_list() -> Vec<String> {
    vec!["default".to_string()]
}

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const DRIVE_SCOPES: &str =
    "https://www.googleapis.com/auth/drive https://www.googleapis.com/auth/drive.file";

#[derive(Clone)]
pub struct AuthManager {
    credentials: OAuthCredentials,
}

impl AuthManager {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            credentials: OAuthCredentials {
                client_id,
                client_secret,
            },
        }
    }

    /// Daftar direktori config yang dicek (termasuk ~/.config/gdrive-mcp sebagai fallback)
    pub fn get_possible_config_dirs() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".config").join("gdrive-mcp"));
        }
        if let Some(cfg) = dirs::config_dir() {
            let p = cfg.join("gdrive-mcp");
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        paths
    }

    /// Direktori konfigurasi utama
    pub fn config_dir() -> Result<PathBuf> {
        for dir in Self::get_possible_config_dirs() {
            if dir.exists() {
                return Ok(dir);
            }
        }
        let target_dir = Self::get_possible_config_dirs()
            .into_iter()
            .next()
            .context("Could not determine user config directory")?;
        std::fs::create_dir_all(&target_dir)?;
        Ok(target_dir)
    }

    /// Path ke file profiles.json
    pub fn profiles_config_path() -> Result<PathBuf> {
        let dir = Self::config_dir()?;
        Ok(dir.join("profiles.json"))
    }

    /// Membaca konfigurasi profil (profiles.json)
    pub fn load_profiles_config() -> ProfilesConfig {
        if let Ok(path) = Self::profiles_config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<ProfilesConfig>(&content) {
                    return cfg;
                }
            }
        }
        ProfilesConfig {
            active_profile: "default".to_string(),
            profiles: vec!["default".to_string()],
        }
    }

    /// Menulis file secara atomic dengan temporary file dan set owner-only permissions di Unix
    pub fn write_secure_file(path: &std::path::Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }

        let tmp_path = path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp_path, content)
            .with_context(|| format!("Gagal menulis temporary file ke {:?}", tmp_path))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600));
        }

        std::fs::rename(&tmp_path, path)
            .with_context(|| format!("Gagal merename temporary file ke {:?}", path))?;
        Ok(())
    }

    /// Menyimpan konfigurasi profil ke profiles.json
    pub fn save_profiles_config(cfg: &ProfilesConfig) -> Result<()> {
        let path = Self::profiles_config_path()?;
        let json = serde_json::to_string_pretty(cfg)?;
        Self::write_secure_file(&path, &json)?;
        Ok(())
    }

    /// Mendapatkan nama profile aktif (prioritas: GDRIVE_PROFILE env var > profiles.json > 'default')
    pub fn get_active_profile() -> String {
        if let Ok(env_profile) = std::env::var("GDRIVE_PROFILE") {
            let trimmed = env_profile.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
        let cfg = Self::load_profiles_config();
        if cfg.active_profile.trim().is_empty() {
            "default".to_string()
        } else {
            cfg.active_profile
        }
    }

    /// Memvalidasi nama profile (hanya alfanumerik, underscore, dan hyphen)
    pub fn validate_profile_name(name: &str) -> Result<&str> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Nama profile tidak boleh kosong.");
        }
        if trimmed.contains("..")
            || trimmed.contains('/')
            || trimmed.contains('\\')
            || !trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            anyhow::bail!(
                "Nama profile '{}' tidak valid. Hanya karakter alfanumerik, underscore, dan hyphen yang diperbolehkan.",
                trimmed
            );
        }
        Ok(trimmed)
    }

    /// Mengubah profile aktif di profiles.json
    pub fn set_active_profile(name: &str) -> Result<()> {
        let trimmed = Self::validate_profile_name(name)?;
        let mut cfg = Self::load_profiles_config();
        if !cfg.profiles.iter().any(|p| p == trimmed) {
            cfg.profiles.push(trimmed.to_string());
        }
        cfg.active_profile = trimmed.to_string();
        Self::save_profiles_config(&cfg)?;
        Ok(())
    }

    /// Mengambil daftar semua profile yang terdaftar
    pub fn list_profiles() -> Result<Vec<String>> {
        let cfg = Self::load_profiles_config();
        let mut list = cfg.profiles;
        if !list.iter().any(|p| p == "default") {
            list.insert(0, "default".to_string());
        }
        Ok(list)
    }

    /// Mendaftarkan nama profile baru ke profiles.json
    pub fn register_profile(name: &str) -> Result<()> {
        let trimmed = Self::validate_profile_name(name)?;
        if trimmed == "default" {
            return Ok(());
        }
        let mut cfg = Self::load_profiles_config();
        if !cfg.profiles.iter().any(|p| p == trimmed) {
            cfg.profiles.push(trimmed.to_string());
            Self::save_profiles_config(&cfg)?;
        }
        Ok(())
    }

    /// Menghapus profile beserta file tokennya
    pub fn remove_profile(name: &str) -> Result<()> {
        let trimmed = Self::validate_profile_name(name)?;
        if trimmed == "default" {
            anyhow::bail!("Profile 'default' tidak dapat dihapus. Anda dapat menggunakan 'gdrive auth logout' untuk menghapus token default.");
        }
        let mut cfg = Self::load_profiles_config();
        cfg.profiles.retain(|p| p != trimmed);
        if cfg.active_profile == trimmed {
            cfg.active_profile = "default".to_string();
        }
        Self::save_profiles_config(&cfg)?;

        if let Ok(token_path) = Self::token_path_for_profile(Some(trimmed)) {
            if token_path.exists() {
                let _ = std::fs::remove_file(&token_path);
            }
            if let Some(parent) = token_path.parent() {
                let _ = std::fs::remove_dir(parent);
            }
        }
        Ok(())
    }

    /// Menentukan path file token untuk profile tertentu (None = active profile)
    pub fn token_path_for_profile(profile: Option<&str>) -> Result<PathBuf> {
        let active_name = match profile {
            Some(p) => p.to_string(),
            None => Self::get_active_profile(),
        };
        let active = Self::validate_profile_name(&active_name)?;

        if active == "default" {
            Self::default_token_path()
        } else {
            let base_dir = Self::config_dir()?;
            let prof_dir = base_dir.join("profiles").join(active);
            Ok(prof_dir.join("token.json"))
        }
    }

    /// Membaca token untuk profile tertentu
    pub fn load_saved_token_for_profile(profile: Option<&str>) -> Result<TokenData> {
        let path = Self::token_path_for_profile(profile)?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Belum login. File token tidak ditemukan di {:?}", path))?;
        let token: TokenData = serde_json::from_str(&content)?;
        Ok(token)
    }

    /// Menyimpan token untuk profile tertentu
    pub fn save_token_for_profile(profile: Option<&str>, token: &TokenData) -> Result<()> {
        let path = Self::token_path_for_profile(profile)?;
        let token_to_save = token.clone().with_computed_expiry();
        let json = serde_json::to_string_pretty(&token_to_save)?;
        Self::write_secure_file(&path, &json)?;
        if let Some(p) = profile {
            let _ = Self::register_profile(p);
        }
        Ok(())
    }

    /// Menghapus token untuk profile tertentu
    pub fn logout_for_profile(profile: Option<&str>) -> Result<()> {
        let path = Self::token_path_for_profile(profile)?;
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Gagal menghapus token di {:?}", path))?;
        }
        Ok(())
    }

    /// Mendapatkan token yang valid untuk profile tertentu (proactive refresh)
    pub async fn get_valid_token_for_profile(profile: Option<&str>) -> Result<String> {
        let saved = Self::load_saved_token_for_profile(profile)?;
        if !saved.is_expired() {
            return Ok(saved.access_token);
        }

        if let Some(ref rf) = saved.refresh_token {
            let auth = Self::from_env_or_config()?;
            let refreshed = auth.refresh_access_token(rf).await?;
            Self::save_token_for_profile(profile, &refreshed)?;
            Ok(refreshed.access_token)
        } else {
            Ok(saved.access_token)
        }
    }

    /// Memaksa refresh token untuk profile tertentu
    pub async fn force_refresh_for_profile(profile: Option<&str>) -> Result<String> {
        let saved = Self::load_saved_token_for_profile(profile)?;
        if let Some(ref rf) = saved.refresh_token {
            let auth = Self::from_env_or_config()?;
            let refreshed = auth.refresh_access_token(rf).await?;
            Self::save_token_for_profile(profile, &refreshed)?;
            Ok(refreshed.access_token)
        } else {
            anyhow::bail!("Tidak ada refresh_token yang tersimpan untuk melakukan token refresh.");
        }
    }

    /// Ambil credentials dari Environment Variables (`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`),
    /// atau file `credentials.json` (dicek di `~/.config/gdrive-mcp` dan OS config dir).
    pub fn resolve_credentials() -> Result<OAuthCredentials> {
        // 1. Cek Environment variables
        if let (Ok(client_id), Ok(client_secret)) = (
            std::env::var("GOOGLE_CLIENT_ID"),
            std::env::var("GOOGLE_CLIENT_SECRET"),
        ) {
            if !client_id.trim().is_empty() && !client_secret.trim().is_empty() {
                return Ok(OAuthCredentials {
                    client_id: client_id.trim().to_string(),
                    client_secret: client_secret.trim().to_string(),
                });
            }
        }

        // 2. Cek file credentials.json di list config dirs
        for dir in Self::get_possible_config_dirs() {
            let cred_path = dir.join("credentials.json");
            if cred_path.exists() {
                let content = std::fs::read_to_string(&cred_path)
                    .with_context(|| format!("Gagal membaca {:?}", cred_path))?;

                let v: serde_json::Value = serde_json::from_str(&content)?;
                let id = v
                    .get("installed")
                    .and_then(|inst| inst.get("client_id"))
                    .or_else(|| v.get("client_id"))
                    .and_then(|s| s.as_str());
                let secret = v
                    .get("installed")
                    .and_then(|inst| inst.get("client_secret"))
                    .or_else(|| v.get("client_secret"))
                    .and_then(|s| s.as_str());

                if let (Some(id), Some(secret)) = (id, secret) {
                    return Ok(OAuthCredentials {
                        client_id: id.to_string(),
                        client_secret: secret.to_string(),
                    });
                }
            }
        }

        let default_dir = Self::get_possible_config_dirs()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("."));
        let cred_path = default_dir.join("credentials.json");

        anyhow::bail!(
            "Google OAuth Credentials belum ditemukan!\n\
            Silakan atur salah satu opsi berikut:\n\
            1. Set environment variable: GOOGLE_CLIENT_ID dan GOOGLE_CLIENT_SECRET\n\
            2. Atau letakkan file credentials OAuth Desktop di: {:?}\n\
            Panduan pembuatan OAuth Client ID dapat dilihat di README.md.",
            cred_path
        );
    }

    pub fn from_env_or_config() -> Result<Self> {
        let credentials = Self::resolve_credentials()?;
        Ok(Self { credentials })
    }

    pub fn credentials_path() -> Result<PathBuf> {
        for dir in Self::get_possible_config_dirs() {
            let p = dir.join("credentials.json");
            if p.exists() {
                return Ok(p);
            }
        }
        let target_dir = Self::get_possible_config_dirs()
            .into_iter()
            .next()
            .context("Could not determine user config directory")?;
        std::fs::create_dir_all(&target_dir)?;
        Ok(target_dir.join("credentials.json"))
    }

    pub fn save_credentials(client_id: &str, client_secret: &str) -> Result<PathBuf> {
        let path = Self::credentials_path()?;
        let json_content = serde_json::json!({
            "installed": {
                "client_id": client_id.trim(),
                "client_secret": client_secret.trim()
            }
        });
        std::fs::write(&path, serde_json::to_string_pretty(&json_content)?)?;
        Ok(path)
    }

    /// Menentukan path file token default (~/.config/gdrive-mcp/token.json)
    pub fn default_token_path() -> Result<PathBuf> {
        for dir in Self::get_possible_config_dirs() {
            let p = dir.join("token.json");
            if p.exists() {
                return Ok(p);
            }
        }
        let target_dir = Self::get_possible_config_dirs()
            .into_iter()
            .next()
            .context("Could not determine user config directory")?;
        std::fs::create_dir_all(&target_dir)?;
        Ok(target_dir.join("token.json"))
    }

    /// Menentukan path file token untuk profile aktif saat ini
    pub fn token_path() -> Result<PathBuf> {
        Self::token_path_for_profile(None)
    }
    pub fn load_saved_token() -> Result<TokenData> {
        Self::load_saved_token_for_profile(None)
    }

    pub fn save_token(token: &TokenData) -> Result<()> {
        Self::save_token_for_profile(None, token)
    }

    pub fn logout() -> Result<()> {
        Self::logout_for_profile(None)
    }

    /// Buat URL otorisasi Google OAuth2
    pub fn build_auth_url(&self, redirect_uri: &str) -> Result<String> {
        let mut u = url::Url::parse(GOOGLE_AUTH_URL)?;
        u.query_pairs_mut()
            .append_pair("client_id", &self.credentials.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", DRIVE_SCOPES)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent");
        Ok(u.to_string())
    }

    /// Eksekusi alur login interaktif menggunakan loopback HTTP server lokal
    pub async fn login_for_profile(
        &self,
        profile: Option<&str>,
        no_browser: bool,
    ) -> Result<TokenData> {
        // Coba bind ke port 8085, jika sibuk bind ke port dinamis apa saja
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:8085").await {
            Ok(l) => l,
            Err(_) => tokio::net::TcpListener::bind("127.0.0.1:0").await?,
        };

        let port = listener.local_addr()?.port();
        let redirect_uri = format!("http://127.0.0.1:{}", port);
        let auth_url = self.build_auth_url(&redirect_uri)?;

        eprintln!("\n🔗 Buka tautan berikut untuk otorisasi Google Drive:");
        eprintln!("{}\n", auth_url);

        let mut browser_opened = false;
        if !no_browser && open::that(&auth_url).is_ok() {
            eprintln!("Browser otomatis dibuka. Menunggu persetujuan otorisasi...");
            browser_opened = true;
        }

        if !browser_opened {
            eprintln!("Silakan buka URL di atas di browser Anda.");
        }

        eprintln!(
            "Menunggu respon otomatis dari browser pada {} ...\n\
            (Atau jika di server remote/SSH, salin URL redirect/kode dan tempelkan di terminal ini):",
            redirect_uri
        );

        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        let mut stdin_reader = BufReader::new(tokio::io::stdin());
        let mut stdin_line = String::new();

        // Tunggu koneksi masuk dari loopback ATAU input manual dari stdin dengan batas timeout 120 detik
        let code = tokio::select! {
            incoming = listener.accept() => {
                let (mut stream, _) = incoming?;
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).await?;
                let req_text = String::from_utf8_lossy(&buf[..n]);

                let code_opt = Self::parse_code_from_http_request(&req_text);

                let body = if code_opt.is_some() {
                    "<!DOCTYPE html><html><body style=\"font-family:sans-serif;text-align:center;padding:50px;\">\
                     <h2 style=\"color:#2e7d32;\">Login Berhasil!</h2>\
                     <p>Autentikasi Google Drive MCP berhasil. Anda dapat menutup tab ini dan kembali ke terminal.</p>\
                     </body></html>"
                } else {
                    "<!DOCTYPE html><html><body style=\"font-family:sans-serif;text-align:center;padding:50px;\">\
                     <h2 style=\"color:#c62828;\">Gagal Mendapatkan Kode</h2>\
                     <p>Silakan coba kembali di terminal.</p>\
                     </body></html>"
                };

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );

                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;

                code_opt.ok_or_else(|| anyhow::anyhow!("Tidak menemukan query parameter 'code' dari callback redirect."))?
            }
            res = stdin_reader.read_line(&mut stdin_line) => {
                res?;
                let parsed = Self::parse_code_from_input(&stdin_line);
                if parsed.is_empty() {
                    anyhow::bail!("Input dari terminal kosong.");
                }
                parsed
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {
                anyhow::bail!("Timeout menunggu otorisasi dari browser atau terminal (120 detik).");
            }
        };

        eprintln!("Kode otorisasi berhasil ditangkap. Menukarkan dengan access token...");
        let token = self.exchange_code(&code, &redirect_uri).await?;
        Self::save_token_for_profile(profile, &token)?;
        let path = Self::token_path_for_profile(profile)?;
        eprintln!("Token berhasil disimpan di {:?}", path);
        Ok(token)
    }

    pub async fn login(&self, no_browser: bool) -> Result<TokenData> {
        self.login_for_profile(None, no_browser).await
    }

    /// Tukarkan authorization code menjadi access token & refresh token
    pub async fn exchange_code(&self, code: &str, redirect_uri: &str) -> Result<TokenData> {
        let client = reqwest::Client::new();
        let params = [
            ("client_id", self.credentials.client_id.as_str()),
            ("client_secret", self.credentials.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri),
        ];

        let resp = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await
            .context("Gagal mengirim request penukaran token ke Google OAuth")?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gagal menukarkan authorization code: {}", err_text);
        }

        let token: TokenData = resp.json().await.context("Gagal parsing response token")?;
        Ok(token.with_computed_expiry())
    }

    /// Refresh access token menggunakan refresh_token yang tersimpan
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<TokenData> {
        let client = reqwest::Client::new();
        let params = [
            ("client_id", self.credentials.client_id.as_str()),
            ("client_secret", self.credentials.client_secret.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let resp = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await
            .context("Gagal mengirim request refresh token ke Google OAuth")?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Gagal me-refresh token Google Drive: {}", err_text);
        }

        let mut token: TokenData = resp
            .json()
            .await
            .context("Gagal parsing response refresh token")?;
        // Google OAuth response refresh seringkali tidak mengirim refresh_token lagi jika sama;
        // pertahankan refresh_token yang sudah ada!
        if token.refresh_token.is_none() {
            token.refresh_token = Some(refresh_token.to_string());
        }
        Ok(token.with_computed_expiry())
    }

    /// Mendapatkan access token yang valid secara proaktif:
    /// Jika token tersimpan sudah expired atau sisa waktu < 5 menit, otomatis lakukan refresh & simpan
    pub async fn get_valid_token() -> Result<String> {
        Self::get_valid_token_for_profile(None).await
    }

    pub async fn force_refresh() -> Result<String> {
        Self::force_refresh_for_profile(None).await
    }

    /// Helper untuk mem-parsing 'code' dari raw HTTP GET request line
    pub fn parse_code_from_http_request(req: &str) -> Option<String> {
        let first_line = req.lines().next()?;
        let path = first_line.split_whitespace().nth(1)?;
        let query_str = path.split('?').nth(1)?;

        for (k, v) in url::form_urlencoded::parse(query_str.as_bytes()) {
            if k == "code" {
                return Some(v.into_owned());
            }
        }
        None
    }

    /// Helper untuk mem-parsing code jika user menempelkan kode langsung atau seluruh URL redirect
    pub fn parse_code_from_input(input: &str) -> String {
        let trimmed = input.trim();
        if let Ok(u) = url::Url::parse(trimmed) {
            for (k, v) in u.query_pairs() {
                if k == "code" {
                    return v.into_owned();
                }
            }
        }
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_expiry_calculation() {
        let now = TokenData::current_timestamp_secs();
        let valid_token = TokenData {
            access_token: "test".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_in: Some(3600),
            expires_at: Some(now + 3600),
            token_type: "Bearer".to_string(),
        };
        assert!(!valid_token.is_expired());
        assert!(valid_token.seconds_remaining().unwrap() > 3000);

        // Token yang tinggal 200 detik lagi (< 300 detik buffer) dianggap expired untuk refresh
        let expiring_token = TokenData {
            access_token: "test".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_in: Some(200),
            expires_at: Some(now + 200),
            token_type: "Bearer".to_string(),
        };
        assert!(expiring_token.is_expired());

        // Token yang sudah lewat
        let expired_token = TokenData {
            access_token: "test".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_in: Some(3600),
            expires_at: Some(now - 10),
            token_type: "Bearer".to_string(),
        };
        assert!(expired_token.is_expired());
        assert_eq!(expired_token.seconds_remaining(), Some(0));
    }

    #[test]
    fn test_token_with_computed_expiry() {
        let token = TokenData {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_in: Some(3600),
            expires_at: None,
            token_type: "Bearer".to_string(),
        }
        .with_computed_expiry();

        assert!(token.expires_at.is_some());
        assert!(!token.is_expired());
    }

    #[test]
    fn test_build_auth_url() {
        let manager = AuthManager::new("mock_client_id".to_string(), "mock_secret".to_string());
        let url = manager.build_auth_url("http://127.0.0.1:8085").unwrap();
        assert!(url.contains("client_id=mock_client_id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8085"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope="));
    }

    #[test]
    fn test_parse_code_from_http_request() {
        let req = "GET /?code=4%2F0AY0e-gtestcode123&scope=email HTTP/1.1\r\nHost: 127.0.0.1:8085\r\n\r\n";
        let code = AuthManager::parse_code_from_http_request(req);
        assert_eq!(code, Some("4/0AY0e-gtestcode123".to_string()));

        let invalid_req = "GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1:8085\r\n\r\n";
        assert_eq!(AuthManager::parse_code_from_http_request(invalid_req), None);
    }

    #[test]
    fn test_parse_code_from_input() {
        // User pastes full redirect URL
        let full_url = "http://127.0.0.1:8085/?code=my_auth_code_xyz&state=123";
        assert_eq!(
            AuthManager::parse_code_from_input(full_url),
            "my_auth_code_xyz"
        );

        // User pastes just the code
        let raw_code = "my_auth_code_xyz";
        assert_eq!(
            AuthManager::parse_code_from_input(raw_code),
            "my_auth_code_xyz"
        );
    }

    #[test]
    fn test_token_refresh_preserves_existing_refresh_token() {
        let initial_token = TokenData {
            access_token: "old_access".to_string(),
            refresh_token: Some("my_refresh_secret".to_string()),
            expires_in: Some(3600),
            expires_at: Some(TokenData::current_timestamp_secs() + 3600),
            token_type: "Bearer".to_string(),
        };

        let mut refreshed_response: TokenData = serde_json::from_str(
            r#"{"access_token":"new_access","expires_in":3600,"token_type":"Bearer"}"#,
        )
        .unwrap();

        if refreshed_response.refresh_token.is_none() {
            refreshed_response.refresh_token = initial_token.refresh_token.clone();
        }

        assert_eq!(refreshed_response.access_token, "new_access");
        assert_eq!(
            refreshed_response.refresh_token,
            Some("my_refresh_secret".to_string())
        );
    }

    #[tokio::test]
    async fn test_gdrive_client_token_accessor() {
        use crate::client::GDriveClient;
        let client = GDriveClient::new("test_token_123".to_string());
        assert_eq!(client.get_token().await, "test_token_123");
    }

    #[test]
    fn test_profiles_config_defaults() {
        let default_cfg = ProfilesConfig {
            active_profile: default_active_profile(),
            profiles: default_profiles_list(),
        };
        assert_eq!(default_cfg.active_profile, "default");
        assert_eq!(default_cfg.profiles, vec!["default"]);

        let serialized = serde_json::to_string(&default_cfg).unwrap();
        let deserialized: ProfilesConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.active_profile, "default");
        assert_eq!(deserialized.profiles.len(), 1);
    }

    #[test]
    fn test_active_profile_and_token_path_with_env_var() {
        std::env::set_var("GDRIVE_PROFILE", "test_env_work");
        assert_eq!(AuthManager::get_active_profile(), "test_env_work");
        let path = AuthManager::token_path().unwrap();
        assert!(path
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("profiles/test_env_work/token.json"));

        std::env::set_var("GDRIVE_PROFILE", "../bad_path");
        assert!(AuthManager::token_path().is_err());

        std::env::remove_var("GDRIVE_PROFILE");
    }

    #[test]
    fn test_validate_profile_name() {
        assert!(AuthManager::validate_profile_name("work").is_ok());
        assert!(AuthManager::validate_profile_name("personal_1").is_ok());
        assert!(AuthManager::validate_profile_name("my-account").is_ok());

        assert!(AuthManager::validate_profile_name("").is_err());
        assert!(AuthManager::validate_profile_name("   ").is_err());
        assert!(AuthManager::validate_profile_name("../etc").is_err());
        assert!(AuthManager::validate_profile_name("sub/dir").is_err());
        assert!(AuthManager::validate_profile_name("sub\\dir").is_err());
        assert!(AuthManager::validate_profile_name("work:1").is_err());
    }

    #[test]
    fn test_remove_profile_rejects_empty_and_default() {
        assert!(AuthManager::remove_profile("").is_err());
        assert!(AuthManager::remove_profile("   ").is_err());
        assert!(AuthManager::remove_profile("default").is_err());
    }
}
