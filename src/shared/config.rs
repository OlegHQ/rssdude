use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self { theme: default_theme() }
    }
}

fn default_theme() -> String { "auto".into() }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Mark items older than this as read (e.g. "30d")
    pub auto_mark_read_after: Option<String>,
    /// Delete items older than this, except starred (e.g. "90d")
    pub auto_delete_after: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address to bind when running as server, or to connect to as client.
    pub address: Option<String>,
    /// Optional bearer token for authentication.
    pub token: Option<String>,
    /// Bind address override for `rssdude serve` (defaults to address).
    pub bind: Option<String>,
}

impl Config {
    /// Load config from ~/.rssdude/config.toml. Returns default if file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Whether a remote server address is configured (client mode).
    pub fn has_remote(&self) -> bool {
        self.server.address.is_some()
    }

    /// Get the full base URL for the remote server.
    pub fn remote_url(&self) -> Option<String> {
        self.server.address.as_ref().map(|addr| {
            if addr.starts_with("http://") || addr.starts_with("https://") {
                addr.clone()
            } else {
                format!("http://{addr}")
            }
        })
    }

    /// Get the bind address for serve mode.
    pub fn bind_address(&self) -> String {
        self.server
            .bind
            .clone()
            .or(self.server.address.clone())
            .unwrap_or_else(|| "127.0.0.1:8484".to_string())
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".rssdude").join("config.toml")
}
