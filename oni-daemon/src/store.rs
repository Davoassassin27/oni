use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::model::{Project, Task, TaskState};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Database {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    tasks: Vec<Task>,
}

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
    db: Arc<RwLock<Database>>,
}

impl Store {
    pub async fn load(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("cannot create data directory {}", parent.display()))?;
        }

        let db = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice::<Database>(&bytes)
                .with_context(|| format!("cannot parse {}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Database {
                version: 1,
                ..Database::default()
            },
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };

        Ok(Self {
            path,
            db: Arc::new(RwLock::new(db)),
        })
    }

    async fn persist(&self, db: &Database) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(db)?;
        let temp = self.path.with_extension("json.tmp");
        tokio::fs::write(&temp, &bytes)
            .await
            .with_context(|| format!("cannot write {}", temp.display()))?;

        #[cfg(windows)]
        if tokio::fs::try_exists(&self.path).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&self.path).await;
        }

        tokio::fs::rename(&temp, &self.path)
            .await
            .with_context(|| format!("cannot replace {}", self.path.display()))?;
        Ok(())
    }

    pub async fn list_projects(&self) -> Vec<Project> {
        let db = self.db.read().await;
        db.projects.clone()
    }

    pub async fn get_project(&self, id: Uuid) -> Option<Project> {
        let db = self.db.read().await;
        db.projects.iter().find(|project| project.id == id).cloned()
    }

    pub async fn create_project(&self, project: Project) -> Result<Project> {
        let mut db = self.db.write().await;
        db.projects.push(project.clone());
        self.persist(&db).await?;
        Ok(project)
    }

    pub async fn update_project<F>(&self, id: Uuid, update: F) -> Result<Option<Project>>
    where
        F: FnOnce(&mut Project),
    {
        let mut db = self.db.write().await;
        let Some(project) = db.projects.iter_mut().find(|project| project.id == id) else {
            return Ok(None);
        };

        update(project);
        let updated = project.clone();
        self.persist(&db).await?;
        Ok(Some(updated))
    }

    pub async fn delete_project(&self, id: Uuid) -> Result<bool> {
        let mut db = self.db.write().await;
        let before = db.projects.len();
        db.projects.retain(|project| project.id != id);
        if db.projects.len() == before {
            return Ok(false);
        }
        self.persist(&db).await?;
        Ok(true)
    }

    pub async fn list_tasks(&self) -> Vec<Task> {
        let mut tasks = self.db.read().await.tasks.clone();
        tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        tasks
    }

    pub async fn get_task(&self, id: Uuid) -> Option<Task> {
        let db = self.db.read().await;
        db.tasks.iter().find(|task| task.id == id).cloned()
    }

    pub async fn create_task(&self, task: Task) -> Result<Task> {
        let mut db = self.db.write().await;
        db.tasks.push(task.clone());
        self.persist(&db).await?;
        Ok(task)
    }

    pub async fn update_task<F>(&self, id: Uuid, update: F) -> Result<Option<Task>>
    where
        F: FnOnce(&mut Task),
    {
        let mut db = self.db.write().await;
        let Some(task) = db.tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };

        update(task);
        task.updated_at = chrono::Utc::now();
        let updated = task.clone();
        self.persist(&db).await?;
        Ok(Some(updated))
    }

    /// A daemon restart cannot resume in-flight agent processes. Surface that
    /// honestly instead of leaving tasks stuck in `acting` forever.
    pub async fn mark_interrupted_tasks(&self) -> Result<usize> {
        let mut db = self.db.write().await;
        let mut interrupted = 0;
        for task in &mut db.tasks {
            if task.state.is_active() {
                task.state = TaskState::Failed;
                task.finished_at = Some(chrono::Utc::now());
                task.updated_at = chrono::Utc::now();
                task.last_error = Some(
                    "The daemon restarted while this task was running. The worktree is still available for inspection or discard."
                        .into(),
                );
                interrupted += 1;
            }
        }
        if interrupted > 0 {
            self.persist(&db).await?;
        }
        Ok(interrupted)
    }

    pub async fn counts(&self) -> (usize, usize) {
        let db = self.db.read().await;
        (db.projects.len(), db.tasks.len())
    }
}

pub fn log_path(logs_dir: &Path, task_id: Uuid) -> PathBuf {
    logs_dir.join(format!("{task_id}.log"))
}
