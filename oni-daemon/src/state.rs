use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

use tokio::sync::{Mutex, broadcast};
use uuid::Uuid;

use crate::config::Config;
use crate::model::ServerEvent;
use crate::store::Store;

#[derive(Clone, Default)]
pub struct RunnerHandle {
    pub stop: Arc<AtomicBool>,
    pub pid: Arc<AtomicU32>,
}

impl RunnerHandle {
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        let pid = self.pid.load(Ordering::SeqCst);
        if pid > 0 {
            tokio::spawn(kill_tree(pid));
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub events: broadcast::Sender<ServerEvent>,
    pub runners: Arc<Mutex<HashMap<Uuid, RunnerHandle>>>,
    pub config: Arc<Config>,
    pub started: Instant,
}

impl AppState {
    pub fn log_path(&self, task_id: Uuid) -> PathBuf {
        crate::store::log_path(&self.config.logs_dir, task_id)
    }

    pub async fn emit_task(&self, task: crate::model::Task) {
        let _ = self.events.send(ServerEvent::Task { task });
    }
}

#[cfg(windows)]
pub async fn kill_tree(pid: u32) {
    let _ = tokio::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .await;
}

#[cfg(not(windows))]
pub async fn kill_tree(pid: u32) {
    let _ = tokio::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()
        .await;
}
