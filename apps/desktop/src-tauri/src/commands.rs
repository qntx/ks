//! Typed commands exposed to the frontend over tauri-specta IPC.
//! Commands that carry secrets are whitelisted by ADR-18; `vault_status`
//! carries none.

use serde::Serialize;

/// Agent liveness as the hello handshake reports it.
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInfo {
    /// Who serves the endpoint: "cli" or "desktop".
    pub host: String,
    /// Seconds until the idle timeout fires.
    pub ttl_left: u32,
}

/// Vault state for the shell; never secret material.
#[derive(Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultStatus {
    /// Default vault path.
    pub path: String,
    /// Whether the directory exists.
    pub exists: bool,
    /// Whether it is a git vault.
    pub is_git: bool,
    /// Agent liveness when one is running.
    pub agent: Option<AgentInfo>,
}

/// Reads the default vault's state without unlocking it.
#[tauri::command]
#[specta::specta]
pub(crate) fn vault_status() -> Result<VaultStatus, String> {
    let path = ks::Vault::default_path().map_err(|e| e.to_string())?;
    let exists = path.is_dir();
    if !exists {
        return Ok(VaultStatus {
            path: path.display().to_string(),
            exists,
            is_git: false,
            agent: None,
        });
    }
    let vault = ks::Vault::open(&path).map_err(|e| e.to_string())?;
    let agent = ks::agent::status(&vault).ok().flatten().map(|a| AgentInfo {
        host: a.host,
        ttl_left: u32::try_from(a.ttl_left).unwrap_or(u32::MAX),
    });
    Ok(VaultStatus {
        path: path.display().to_string(),
        exists,
        is_git: vault.root().join(".git").exists(),
        agent,
    })
}
