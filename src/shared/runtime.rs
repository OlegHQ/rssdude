use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub address: String,
    pub pid: u32,
}

pub fn runtime_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".rssdude").join("server.run")
}

pub fn load() -> Option<RuntimeInfo> {
    let path = runtime_path();
    let contents = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

pub fn clear() {
    let _ = std::fs::remove_file(runtime_path());
}

/// Write the runtime file. Returns a guard that removes the file on drop.
pub fn write(bind: &str) -> Result<RuntimeGuard> {
    let path = runtime_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let info = RuntimeInfo {
        address: client_address(bind),
        pid: std::process::id(),
    };
    std::fs::write(&path, serde_json::to_string(&info)?)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(RuntimeGuard { path })
}

/// Convert a bind address into a client-reachable address.
/// Wildcard binds (0.0.0.0, [::]) become loopback for local clients.
fn client_address(bind: &str) -> String {
    if let Some(port) = bind.strip_prefix("0.0.0.0:") {
        format!("127.0.0.1:{port}")
    } else if let Some(port) = bind.strip_prefix("[::]:") {
        format!("[::1]:{port}")
    } else {
        bind.to_string()
    }
}

/// Quick TCP probe — used by the CLI to decide if the runtime file points at
/// a live server. Returns false on stale files left by SIGKILL/panic.
pub async fn is_reachable(address: &str) -> bool {
    let host_port = address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .unwrap_or(address);
    tokio::time::timeout(
        Duration::from_millis(200),
        tokio::net::TcpStream::connect(host_port),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

pub struct RuntimeGuard {
    path: PathBuf,
}

impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
