use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenData {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: String,
}

#[allow(dead_code)]
pub struct AuthManager {
    client_id: String,
    client_secret: String,
}

impl AuthManager {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
        }
    }

    pub fn token_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find user config directory")?
            .join("gdrive-mcp");
        std::fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("token.json"))
    }

    pub fn load_saved_token() -> Result<TokenData> {
        let path = Self::token_path()?;
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Belum login. File token tidak ditemukan di {:?}", path))?;
        let token: TokenData = serde_json::from_str(&content)?;
        Ok(token)
    }

    pub fn save_token(token: &TokenData) -> Result<()> {
        let path = Self::token_path()?;
        let json = serde_json::to_string_pretty(token)?;
        std::fs::write(&path, json)?;
        Ok(())
    }
}
