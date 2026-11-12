use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentPreset {
    Opencode,
    Claude,
    Custom,
}

impl AgentPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Opencode => "opencode",
            Self::Claude => "claude",
            Self::Custom => "custom",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub base_branch: String,
    pub verify_command: String,
    pub agent: AgentPreset,
    #[serde(default)]
    pub custom_command: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProject {
    pub name: String,
    pub path: String,
    #[serde(default = "default_branch")]
    pub base_branch: String,
    pub verify_command: String,
    #[serde(default = "default_agent")]
    pub agent: AgentPreset,
    #[serde(default)]
    pub custom_command: Option<String>,
}

fn default_branch() -> String {
    "main".into()
}

fn default_agent() -> AgentPreset {
    AgentPreset::Opencode
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Queued,
    Acting,
    Verifying,
    Retrying,
    Review,
    Merged,
    Discarded,
    Failed,
    Stopped,
}

impl TaskState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Merged | Self::Discarded | Self::Failed | Self::Stopped
        )
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Acting | Self::Verifying | Self::Retrying
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    pub path: String,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSummary {
    pub stat: String,
    pub files: Vec<FileChange>,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub repo_path: String,
    pub base_branch: String,
    pub title: String,
    pub prompt: String,
    pub acceptance: String,
    pub state: TaskState,
    pub branch: String,
    pub worktree_path: String,
    pub iteration: u32,
    pub max_iterations: u32,
    pub agent: AgentPreset,
    #[serde(default)]
    pub custom_command: Option<String>,
    pub verify_command: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub diff: Option<DiffSummary>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub project_id: Uuid,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub acceptance: Option<String>,
    #[serde(default)]
    pub agent: Option<AgentPreset>,
    #[serde(default)]
    pub custom_command: Option<String>,
    #[serde(default)]
    pub max_iterations: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Task {
        task: Task,
    },
    #[serde(rename_all = "camelCase")]
    Log {
        task_id: Uuid,
        chunk: String,
    },
    Project {
        project: Project,
    },
}
