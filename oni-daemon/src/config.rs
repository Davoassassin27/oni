use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub token: Option<String>,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub worktrees_dir: PathBuf,
    pub pwa_dir: Option<PathBuf>,
    pub agent_timeout_secs: u64,
    pub verify_timeout_secs: u64,
    /// Optional local Juniperus sidecar. Empty means deterministic fallback.
    pub juniperus_url: Option<String>,
    pub juniperus_token: Option<String>,
    pub juniperus_timeout_ms: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let cwd = std::env::current_dir().context("cannot resolve current directory")?;

        let data_dir = std::env::var_os("ONI_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let home = std::env::var_os("USERPROFILE")
                    .or_else(|| std::env::var_os("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| cwd.clone());
                home.join(".oni")
            });

        let logs_dir = data_dir.join("logs");
        let worktrees_dir = data_dir.join("worktrees");

        let pwa_dir = std::env::var_os("ONI_PWA_DIR").map(PathBuf::from);

        let host = std::env::var("ONI_HOST").unwrap_or_else(|_| "127.0.0.1".into());
        let port = std::env::var("ONI_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(7766);

        let token = std::env::var("ONI_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let agent_timeout_secs = std::env::var("ONI_AGENT_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(30 * 60);
        let verify_timeout_secs = std::env::var("ONI_VERIFY_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(10 * 60);

        let juniperus_url = std::env::var("ONI_JUNIPERUS_URL")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let juniperus_token = std::env::var("ONI_JUNIPERUS_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let juniperus_timeout_ms = std::env::var("ONI_JUNIPERUS_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1_500);

        Ok(Self {
            host,
            port,
            token,
            data_dir,
            logs_dir,
            worktrees_dir,
            pwa_dir,
            agent_timeout_secs,
            verify_timeout_secs,
            juniperus_url,
            juniperus_token,
            juniperus_timeout_ms,
        })
    }
}
