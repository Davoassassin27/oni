use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct Client {
    base: String,
    token: Option<String>,
    timeout: Duration,
}

impl Client {
    pub fn new(base: impl Into<String>, token: Option<String>) -> Self {
        Self {
            base: base.into().trim_end_matches('/').to_string(),
            token: token
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            timeout: Duration::from_secs(5),
        }
    }

    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let url = format!("{}/api{}", self.base, path);
        let request = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            _ => unreachable!("unsupported method"),
        }
        .timeout(self.timeout);
        match &self.token {
            Some(token) => request.set("Authorization", &format!("Bearer {token}")),
            None => request,
        }
    }

    fn send<T: DeserializeOwned>(&self, request: ureq::Request) -> Result<T> {
        match request.call() {
            Ok(response) => response
                .into_json::<T>()
                .context("daemon returned invalid JSON"),
            Err(error) => Err(http_error(error)),
        }
    }

    pub fn health(&self) -> Result<Health> {
        self.send(self.request("GET", "/health"))
    }

    pub fn projects(&self) -> Result<Vec<Project>> {
        self.send(self.request("GET", "/projects"))
    }

    pub fn tasks(&self) -> Result<Vec<Task>> {
        self.send(self.request("GET", "/tasks"))
    }

    pub fn task(&self, id: &str) -> Result<Task> {
        self.send(self.request("GET", &format!("/tasks/{id}")))
    }

    pub fn log(&self, id: &str) -> Result<String> {
        let request = self.request("GET", &format!("/tasks/{id}/log"));
        match request.call() {
            Ok(response) => response
                .into_string()
                .context("daemon returned non-UTF-8 log output"),
            Err(error) => Err(http_error(error)),
        }
    }

    pub fn patch(&self, id: &str) -> Result<Patch> {
        self.send(self.request("GET", &format!("/tasks/{id}/patch")))
    }

    pub fn stop(&self, id: &str) -> Result<Task> {
        self.send(self.request("POST", &format!("/tasks/{id}/stop")))
    }

    pub fn retry(&self, id: &str) -> Result<Task> {
        self.send(self.request("POST", &format!("/tasks/{id}/retry")))
    }

    pub fn merge(&self, id: &str) -> Result<Task> {
        self.send(self.request("POST", &format!("/tasks/{id}/merge")))
    }

    pub fn accept_verification(&self, id: &str) -> Result<Task> {
        self.send(self.request("POST", &format!("/tasks/{id}/accept-verification")))
    }

    pub fn discard(&self, id: &str) -> Result<Task> {
        self.send(self.request("POST", &format!("/tasks/{id}/discard")))
    }
}

fn http_error(error: ureq::Error) -> anyhow::Error {
    match error {
        ureq::Error::Status(code, response) => {
            let body = response.into_string().unwrap_or_default();
            let body = body.trim();
            if body.is_empty() {
                anyhow::anyhow!("daemon returned HTTP {code}")
            } else {
                anyhow::anyhow!("daemon returned HTTP {code}: {body}")
            }
        }
        ureq::Error::Transport(error) => {
            anyhow::anyhow!("cannot reach the Oni daemon: {error}")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub version: String,
    pub projects: usize,
    pub tasks: usize,
    pub uptime_secs: u64,
    pub auth_required: bool,
    pub data_dir: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub base_branch: String,
    pub verify_command: String,
    pub agent: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationGuard {
    #[serde(default)]
    pub tampered: bool,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub accepted_by_human: bool,
}

impl Default for VerificationGuard {
    fn default() -> Self {
        Self {
            tampered: false,
            files: Vec::new(),
            accepted_by_human: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub repo_path: String,
    pub base_branch: String,
    pub title: String,
    pub prompt: String,
    pub acceptance: String,
    pub state: String,
    pub branch: String,
    pub worktree_path: String,
    pub iteration: u32,
    pub max_iterations: u32,
    pub agent: String,
    pub custom_command: Option<String>,
    pub verify_command: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub last_error: Option<String>,
    pub diff: Option<DiffSummary>,
    #[serde(default)]
    pub verification: VerificationGuard,
}

impl Task {
    pub fn is_active(&self) -> bool {
        matches!(
            self.state.as_str(),
            "queued" | "acting" | "verifying" | "retrying"
        )
    }

    pub fn is_review(&self) -> bool {
        self.state == "review"
    }

    pub fn is_done(&self) -> bool {
        matches!(self.state.as_str(), "merged" | "discarded")
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.state.as_str(), "failed" | "stopped")
    }

    pub fn verification_blocked(&self) -> bool {
        self.verification.tampered && !self.verification.accepted_by_human
    }

    pub fn can_merge(&self) -> bool {
        self.is_review() && !self.verification_blocked()
    }

    pub fn can_discard(&self) -> bool {
        self.is_review() || self.is_failed()
    }

    pub fn can_stop(&self) -> bool {
        self.is_active()
    }

    pub fn can_retry(&self) -> bool {
        self.is_review() || self.is_failed()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub stat: String,
    pub files: Vec<FileChange>,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileChange {
    pub path: String,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Patch {
    pub files: Vec<PatchFile>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PatchFile {
    pub path: String,
    pub patch: String,
    pub insertions: u64,
    pub deletions: u64,
}
