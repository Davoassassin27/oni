use std::convert::Infallible;
use std::path::Path;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path as AxumPath, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post, put},
};
use chrono::Utc;
use futures_util::StreamExt;
use serde_json::{Value, json};
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

use crate::error::AppError;
use crate::git;
use crate::model::{
    AgentPreset, DiffPatch, DiffSummary, NewProject, NewTask, Project, ServerEvent, Task,
    TaskState, UpdateProject,
};
use crate::runner;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/{id}", put(update_project).delete(delete_project))
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/{id}", get(get_task))
        .route("/tasks/{id}/log", get(get_task_log))
        .route("/tasks/{id}/diff", get(get_task_diff))
        .route("/tasks/{id}/patch", get(get_task_patch))
        .route("/tasks/{id}/stop", post(stop_task))
        .route("/tasks/{id}/retry", post(retry_task))
        .route("/tasks/{id}/merge", post(merge_task))
        .route("/tasks/{id}/discard", post(discard_task))
        .route("/events", get(events))
        .with_state(state)
}

pub async fn auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(expected) = state.config.token.as_deref() else {
        return next.run(request).await;
    };

    let header_ok = bearer_token(request.headers()).is_some_and(|token| token == expected);
    let query_ok = request
        .uri()
        .query()
        .unwrap_or_default()
        .split('&')
        .any(|pair| pair == format!("token={expected}"));

    if header_ok || query_ok {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "unauthorized: missing or invalid Oni token" })),
        )
            .into_response()
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    if let Some(value) = headers
        .get("x-oni-token")
        .and_then(|value| value.to_str().ok())
    {
        return Some(value);
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let (projects, tasks) = state.store.counts().await;
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "uptimeSecs": state.started.elapsed().as_secs(),
        "projects": projects,
        "tasks": tasks,
        "dataDir": state.config.data_dir.display().to_string(),
        "authRequired": state.config.token.is_some(),
    }))
}

async fn list_projects(State(state): State<AppState>) -> Json<Vec<Project>> {
    Json(state.store.list_projects().await)
}

async fn create_project(
    State(state): State<AppState>,
    Json(body): Json<NewProject>,
) -> Result<(StatusCode, Json<Project>), AppError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::bad_request("project name cannot be empty"));
    }
    if body.verify_command.trim().is_empty() {
        return Err(AppError::bad_request(
            "verification command cannot be empty",
        ));
    }
    if body.path.trim().is_empty() {
        return Err(AppError::bad_request("repository path cannot be empty"));
    }
    if body.agent == AgentPreset::Custom
        && body
            .custom_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(AppError::bad_request(
            "a custom agent needs a command that reads the prompt from stdin",
        ));
    }

    let (path, base_branch) = git::validate_repo(Path::new(body.path.trim()), &body.base_branch)
        .await
        .map_err(|error| AppError::bad_request(format!("{error:#}")))?;

    let path_string = path.display().to_string();
    let existing = state.store.list_projects().await;
    if existing
        .iter()
        .any(|project| project.path == path_string && project.base_branch == base_branch)
    {
        return Err(AppError::conflict(
            "this repository and base branch are already registered",
        ));
    }

    let project = Project {
        id: Uuid::new_v4(),
        name,
        path: path_string,
        base_branch,
        verify_command: body.verify_command.trim().to_string(),
        agent: body.agent,
        custom_command: body
            .custom_command
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        created_at: Utc::now(),
    };

    state.store.create_project(project.clone()).await?;
    let _ = state.events.send(ServerEvent::Project {
        project: project.clone(),
    });

    Ok((StatusCode::CREATED, Json(project)))
}

async fn update_project(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
    Json(body): Json<UpdateProject>,
) -> Result<Json<Project>, AppError> {
    let existing_project = state
        .store
        .get_project(id)
        .await
        .ok_or_else(|| AppError::not_found("project not found"))?;

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::bad_request("project name cannot be empty"));
    }
    if body.verify_command.trim().is_empty() {
        return Err(AppError::bad_request(
            "verification command cannot be empty",
        ));
    }
    if body.path.trim().is_empty() {
        return Err(AppError::bad_request("repository path cannot be empty"));
    }
    if body.agent == AgentPreset::Custom
        && body
            .custom_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(AppError::bad_request(
            "a custom agent needs a command that reads the prompt from stdin",
        ));
    }

    let (path, base_branch) = git::validate_repo(Path::new(body.path.trim()), &body.base_branch)
        .await
        .map_err(|error| AppError::bad_request(format!("{error:#}")))?;

    let path_string = path.display().to_string();
    let projects = state.store.list_projects().await;
    if projects.iter().any(|project| {
        project.id != id && project.path == path_string && project.base_branch == base_branch
    }) {
        return Err(AppError::conflict(
            "another project already uses this repository and base branch",
        ));
    }

    let updated = state
        .store
        .update_project(id, |project| {
            project.name = name;
            project.path = path_string;
            project.base_branch = base_branch;
            project.verify_command = body.verify_command.trim().to_string();
            project.agent = body.agent;
            project.custom_command = body
                .custom_command
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        })
        .await?
        .ok_or_else(|| AppError::not_found("project not found"))?;

    debug_assert_eq!(updated.id, existing_project.id);
    let _ = state.events.send(ServerEvent::Project {
        project: updated.clone(),
    });
    Ok(Json(updated))
}

async fn delete_project(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<StatusCode, AppError> {
    if !state.store.delete_project(id).await? {
        return Err(AppError::not_found("project not found"));
    }
    let _ = state
        .events
        .send(ServerEvent::ProjectDeleted { project_id: id });
    Ok(StatusCode::NO_CONTENT)
}

async fn list_tasks(State(state): State<AppState>) -> Json<Vec<Task>> {
    Json(state.store.list_tasks().await)
}

async fn create_task(
    State(state): State<AppState>,
    Json(body): Json<NewTask>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    let project = state
        .store
        .get_project(body.project_id)
        .await
        .ok_or_else(|| AppError::not_found("project not found"))?;

    let title = body.title.trim().to_string();
    let prompt = body.prompt.trim().to_string();

    if title.is_empty() {
        return Err(AppError::bad_request("task title cannot be empty"));
    }
    if title.chars().count() > 160 {
        return Err(AppError::bad_request("task title is too long (max 160)"));
    }
    if prompt.is_empty() {
        return Err(AppError::bad_request("task prompt cannot be empty"));
    }
    if prompt.chars().count() > 12_000 {
        return Err(AppError::bad_request("task prompt is too long (max 12000)"));
    }

    let acceptance = body
        .acceptance
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| project.verify_command.clone());

    if acceptance.is_empty() {
        return Err(AppError::bad_request("acceptance command cannot be empty"));
    }

    let agent = body.agent.unwrap_or(project.agent);
    let custom_command = if agent == AgentPreset::Custom {
        body.custom_command
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or(project.custom_command.clone())
    } else {
        None
    };

    if agent == AgentPreset::Custom
        && custom_command
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err(AppError::bad_request(
            "a custom agent needs a command that reads the prompt from stdin",
        ));
    }

    let id = Uuid::new_v4();
    let short = id.simple().to_string();
    let short = &short[..8];
    let branch = git::branch_name(&title, id);
    let worktree = state
        .config
        .worktrees_dir
        .join(git::slugify(&project.name, 32))
        .join(short);

    let task = Task {
        id,
        project_id: project.id,
        project_name: project.name.clone(),
        repo_path: project.path.clone(),
        base_branch: project.base_branch.clone(),
        title,
        prompt,
        acceptance,
        state: TaskState::Queued,
        branch,
        worktree_path: worktree.display().to_string(),
        iteration: 0,
        max_iterations: body.max_iterations.unwrap_or(3).clamp(1, 6),
        agent,
        custom_command,
        verify_command: project.verify_command.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        finished_at: None,
        last_error: None,
        diff: None,
    };

    state.store.create_task(task.clone()).await?;
    state.emit_task(task.clone()).await;
    runner::launch(state.clone(), task.id).await;

    Ok((StatusCode::ACCEPTED, Json(task)))
}

async fn get_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;
    Ok(Json(task))
}

async fn get_task_log(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Response, AppError> {
    if state.store.get_task(id).await.is_none() {
        return Err(AppError::not_found("task not found"));
    }

    let path = state.log_path(id);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(AppError::internal(format!("cannot read log: {error}"))),
    };

    const MAX_LOG: usize = 1_500_000;
    let truncated = bytes.len() > MAX_LOG;
    let slice = if truncated {
        &bytes[bytes.len() - MAX_LOG..]
    } else {
        &bytes
    };
    let mut text = String::from_utf8_lossy(slice).to_string();
    if truncated {
        text = format!("… earlier output truncated …\n{text}");
    }

    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
}

async fn get_task_diff(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<DiffSummary>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if Path::new(&task.worktree_path).exists() {
        let diff =
            git::diff_summary(Path::new(&task.repo_path), &task.base_branch, &task.branch).await?;
        Ok(Json(diff))
    } else {
        Ok(Json(task.diff.unwrap_or_default()))
    }
}

async fn get_task_patch(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<DiffPatch>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if !Path::new(&task.worktree_path).exists() {
        return Err(AppError::conflict(
            "the patch is no longer available because the worktree was removed",
        ));
    }

    let patch =
        git::diff_patch(Path::new(&task.repo_path), &task.base_branch, &task.branch).await?;
    Ok(Json(patch))
}

async fn stop_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if task.state.is_terminal() {
        return Err(AppError::conflict("task is already finished"));
    }

    let handle = state.runners.lock().await.get(&id).cloned();
    if let Some(handle) = handle {
        handle.request_stop();
        return Ok(Json(task));
    }

    let stopped = state
        .store
        .update_task(id, |task| {
            task.state = TaskState::Stopped;
            task.finished_at = Some(Utc::now());
        })
        .await?
        .ok_or_else(|| AppError::not_found("task not found"))?;
    state.emit_task(stopped.clone()).await;
    Ok(Json(stopped))
}

async fn retry_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<(StatusCode, Json<Task>), AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if !matches!(
        task.state,
        TaskState::Failed | TaskState::Stopped | TaskState::Review
    ) {
        return Err(AppError::conflict(
            "only failed, stopped or review tasks can be retried",
        ));
    }

    if !Path::new(&task.worktree_path).exists() {
        return Err(AppError::conflict(
            "the worktree no longer exists; create a new task instead",
        ));
    }

    let retried = state
        .store
        .update_task(id, |task| {
            task.state = TaskState::Queued;
            task.last_error = None;
            task.finished_at = None;
            task.max_iterations = task.max_iterations.max(task.iteration + 1);
        })
        .await?
        .ok_or_else(|| AppError::not_found("task not found"))?;

    state.emit_task(retried.clone()).await;
    runner::launch(state.clone(), id).await;

    Ok((StatusCode::ACCEPTED, Json(retried)))
}

async fn merge_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if task.state != TaskState::Review {
        return Err(AppError::conflict("only tasks in review can be merged"));
    }

    git::merge_branch(Path::new(&task.repo_path), &task.base_branch, &task.branch)
        .await
        .map_err(|error| AppError::conflict(format!("{error:#}")))?;

    let _ = git::remove_worktree(Path::new(&task.repo_path), Path::new(&task.worktree_path)).await;
    let _ = git::delete_branch(Path::new(&task.repo_path), &task.branch).await;

    let merged = state
        .store
        .update_task(id, |task| {
            task.state = TaskState::Merged;
            task.finished_at = Some(Utc::now());
            task.last_error = None;
        })
        .await?
        .ok_or_else(|| AppError::not_found("task not found"))?;

    state.emit_task(merged.clone()).await;
    Ok(Json(merged))
}

async fn discard_task(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<Uuid>,
) -> Result<Json<Task>, AppError> {
    let task = state
        .store
        .get_task(id)
        .await
        .ok_or_else(|| AppError::not_found("task not found"))?;

    if !matches!(
        task.state,
        TaskState::Review | TaskState::Failed | TaskState::Stopped
    ) {
        return Err(AppError::conflict(
            "only review, failed or stopped tasks can be discarded",
        ));
    }

    git::remove_worktree(Path::new(&task.repo_path), Path::new(&task.worktree_path)).await?;
    let _ = git::delete_branch(Path::new(&task.repo_path), &task.branch).await;

    let discarded = state
        .store
        .update_task(id, |task| {
            task.state = TaskState::Discarded;
            task.finished_at = Some(Utc::now());
        })
        .await?
        .ok_or_else(|| AppError::not_found("task not found"))?;

    state.emit_task(discarded.clone()).await;
    Ok(Json(discarded))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.events.subscribe();
    let stream = BroadcastStream::new(receiver).filter_map(|message| async move {
        match message {
            Ok(event) => Event::default().json_data(&event).ok().map(Ok),
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("oni"),
    )
}
