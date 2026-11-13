use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::timeout;
use uuid::Uuid;

use crate::git;
use crate::model::{AgentPreset, ServerEvent, Task, TaskState};
use crate::state::{AppState, RunnerHandle, kill_tree};

const OUTPUT_TAIL_LIMIT: usize = 8_000;

#[derive(Debug)]
struct RunOutcome {
    success: bool,
    code: i32,
    timed_out: bool,
    tail: String,
}

pub async fn launch(state: AppState, task_id: Uuid) {
    let handle = RunnerHandle::default();
    state.runners.lock().await.insert(task_id, handle.clone());

    tokio::spawn(async move {
        if let Err(error) = run(&state, task_id, &handle).await {
            let message = format!("{error:#}");
            tracing::error!(task = %task_id, error = %message, "task runner failed");
            let _ = fail_active(&state, task_id, message).await;
        }
        state.runners.lock().await.remove(&task_id);
    });
}

async fn run(state: &AppState, task_id: Uuid, handle: &RunnerHandle) -> Result<()> {
    let mut task = state
        .store
        .get_task(task_id)
        .await
        .context("task not found")?;

    if task.state.is_terminal() {
        return Ok(());
    }

    if !Path::new(&task.worktree_path).exists() {
        append_log(
            state,
            task_id,
            &format!(
                "\n== oni ==\ncreating worktree\n  branch: {}\n  base:   {}\n  path:   {}\n",
                task.branch, task.base_branch, task.worktree_path
            ),
        )
        .await;

        git::create_worktree(
            Path::new(&task.repo_path),
            &task.base_branch,
            &task.branch,
            Path::new(&task.worktree_path),
        )
        .await
        .context("cannot create isolated worktree")?;
    }

    let mut retry_context = task.last_error.clone();

    loop {
        if handle.stop.load(Ordering::SeqCst) {
            mark_stopped(state, task_id).await?;
            return Ok(());
        }

        task = state
            .store
            .get_task(task_id)
            .await
            .context("task not found")?;

        if task.iteration >= task.max_iterations {
            let message = retry_context.unwrap_or_else(|| "Maximum iterations reached.".into());
            finish(state, task_id, TaskState::Failed, &message).await?;
            return Ok(());
        }

        let attempt = task.iteration + 1;
        let now = Utc::now();
        let active = save_and_emit(state, task_id, |task| {
            task.state = TaskState::Acting;
            task.iteration = attempt;
            task.started_at.get_or_insert(now);
            task.finished_at = None;
        })
        .await?;

        append_log(
            state,
            task_id,
            &format!(
                "\n== attempt {}/{} · {} ==\n",
                active.iteration,
                active.max_iterations,
                active.agent.label()
            ),
        )
        .await;

        let prompt = build_prompt(&active, retry_context.as_deref());
        let outcome = run_agent(state, &active, &prompt, handle).await?;

        if handle.stop.load(Ordering::SeqCst) {
            mark_stopped(state, task_id).await?;
            return Ok(());
        }

        if !outcome.success {
            let reason = if outcome.timed_out {
                format!(
                    "Agent timed out after {} minutes. Last output:\n\n{}",
                    state.config.agent_timeout_secs / 60,
                    outcome.tail
                )
            } else {
                format!(
                    "Agent exited with code {}. Last output:\n\n{}",
                    outcome.code, outcome.tail
                )
            };

            if attempt >= active.max_iterations {
                finish(state, task_id, TaskState::Failed, &reason).await?;
                return Ok(());
            }

            retry_context = Some(reason.clone());
            save_and_emit(state, task_id, |task| {
                task.state = TaskState::Retrying;
                task.last_error = Some(truncate(&reason, OUTPUT_TAIL_LIMIT));
            })
            .await?;
            append_log(state, task_id, "== retrying after agent failure ==\n").await;
            continue;
        }

        let verifying = save_and_emit(state, task_id, |task| {
            task.state = TaskState::Verifying;
        })
        .await?;

        append_log(
            state,
            task_id,
            &format!("\n== verify · {} ==\n", verifying.acceptance),
        )
        .await;

        let verification = run_verify(state, &verifying, handle).await?;

        if handle.stop.load(Ordering::SeqCst) {
            mark_stopped(state, task_id).await?;
            return Ok(());
        }

        if verification.success {
            let committed = git::commit_all(Path::new(&verifying.worktree_path), &verifying.title)
                .await
                .context("cannot commit the agent's changes")?;
            if committed {
                append_log(
                    state,
                    task_id,
                    "\n== oni ==\ncommitted the agent's changes\n",
                )
                .await;
            }

            let diff = match git::diff_summary(
                Path::new(&verifying.repo_path),
                &verifying.base_branch,
                &verifying.branch,
            )
            .await
            {
                Ok(diff) => Some(diff),
                Err(error) => {
                    append_log(
                        state,
                        task_id,
                        &format!("\n== warn ==\ncannot compute diff: {error:#}\n"),
                    )
                    .await;
                    None
                }
            };

            save_and_emit(state, task_id, |task| {
                task.state = TaskState::Review;
                task.diff = diff;
                task.last_error = None;
                task.finished_at = Some(Utc::now());
            })
            .await?;

            append_log(state, task_id, "\n== ready for review ==\n").await;
            return Ok(());
        }

        let reason = if verification.timed_out {
            format!(
                "Verification timed out after {} minutes. Last output:\n\n{}",
                state.config.verify_timeout_secs / 60,
                verification.tail
            )
        } else {
            format!(
                "Verification failed with code {}. Output:\n\n{}",
                verification.code, verification.tail
            )
        };

        if attempt >= active.max_iterations {
            finish(state, task_id, TaskState::Failed, &reason).await?;
            return Ok(());
        }

        retry_context = Some(reason.clone());
        save_and_emit(state, task_id, |task| {
            task.state = TaskState::Retrying;
            task.last_error = Some(truncate(&reason, OUTPUT_TAIL_LIMIT));
        })
        .await?;

        append_log(
            state,
            task_id,
            &format!(
                "\n== verification failed · retrying ({}/{}) ==\n",
                attempt + 1,
                active.max_iterations
            ),
        )
        .await;
    }
}

async fn run_agent(
    state: &AppState,
    task: &Task,
    prompt: &str,
    handle: &RunnerHandle,
) -> Result<RunOutcome> {
    let custom = task.agent == AgentPreset::Custom;
    let mut command = match task.agent {
        AgentPreset::Opencode => {
            let mut command = Command::new(opencode_program());
            command.arg("run").arg("--auto");
            if let Some(model) = std::env::var("ONI_OPENCODE_MODEL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
            {
                command.args(["--model", &model]);
            }
            command.arg(prompt);
            command
        }
        AgentPreset::Claude => {
            let mut command = Command::new("claude");
            command
                .arg("-p")
                .arg(prompt)
                .args(["--permission-mode", "acceptEdits"]);
            command
        }
        AgentPreset::Custom => {
            let raw = task
                .custom_command
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .context("custom agent has no command configured")?;
            shell_command(raw)
        }
    };

    command
        .current_dir(&task.worktree_path)
        .stdin(if custom {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start agent '{}'", task.agent.label()))?;

    if custom && let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(prompt.as_bytes())
            .await
            .context("cannot write prompt to the custom agent")?;
        let _ = stdin.flush().await;
        drop(stdin);
    }

    let pid = child.id().unwrap_or_default();
    handle.pid.store(pid, Ordering::SeqCst);
    if handle.stop.load(Ordering::SeqCst) {
        kill_tree(pid).await;
    }

    let tail = Arc::new(Mutex::new(String::new()));
    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| stream_reader(state.clone(), task.id, stdout, tail.clone()));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| stream_reader(state.clone(), task.id, stderr, tail.clone()));

    let duration = Duration::from_secs(state.config.agent_timeout_secs);
    let outcome = match timeout(duration, child.wait()).await {
        Ok(Ok(status)) => RunOutcome {
            success: status.success(),
            code: status.code().unwrap_or(-1),
            timed_out: false,
            tail: String::new(),
        },
        Ok(Err(error)) => {
            handle.pid.store(0, Ordering::SeqCst);
            return Err(error).context("cannot wait for the agent process");
        }
        Err(_) => {
            append_log(
                state,
                task.id,
                &format!(
                    "\n== oni ==\nagent timeout after {} minutes; killing process tree\n",
                    duration.as_secs() / 60
                ),
            )
            .await;
            kill_tree(pid).await;
            let _ = child.wait().await;
            RunOutcome {
                success: false,
                code: -1,
                timed_out: true,
                tail: String::new(),
            }
        }
    };

    handle.pid.store(0, Ordering::SeqCst);
    join_reader(stdout_task).await;
    join_reader(stderr_task).await;

    let mut outcome = outcome;
    outcome.tail = tail.lock().await.clone();
    Ok(outcome)
}

async fn run_verify(state: &AppState, task: &Task, handle: &RunnerHandle) -> Result<RunOutcome> {
    let mut command = shell_command(&task.acceptance);
    command
        .current_dir(&task.worktree_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = command
        .spawn()
        .with_context(|| format!("cannot start verification command: {}", task.acceptance))?;

    let pid = child.id().unwrap_or_default();
    handle.pid.store(pid, Ordering::SeqCst);
    if handle.stop.load(Ordering::SeqCst) {
        kill_tree(pid).await;
    }

    let tail = Arc::new(Mutex::new(String::new()));
    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| stream_reader(state.clone(), task.id, stdout, tail.clone()));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| stream_reader(state.clone(), task.id, stderr, tail.clone()));

    let duration = Duration::from_secs(state.config.verify_timeout_secs);
    let outcome = match timeout(duration, child.wait()).await {
        Ok(Ok(status)) => RunOutcome {
            success: status.success(),
            code: status.code().unwrap_or(-1),
            timed_out: false,
            tail: String::new(),
        },
        Ok(Err(error)) => {
            handle.pid.store(0, Ordering::SeqCst);
            return Err(error).context("cannot wait for the verification process");
        }
        Err(_) => {
            append_log(
                state,
                task.id,
                &format!(
                    "\n== oni ==\nverification timeout after {} minutes; killing process tree\n",
                    duration.as_secs() / 60
                ),
            )
            .await;
            kill_tree(pid).await;
            let _ = child.wait().await;
            RunOutcome {
                success: false,
                code: -1,
                timed_out: true,
                tail: String::new(),
            }
        }
    };

    handle.pid.store(0, Ordering::SeqCst);
    join_reader(stdout_task).await;
    join_reader(stderr_task).await;

    let mut outcome = outcome;
    outcome.tail = tail.lock().await.clone();
    Ok(outcome)
}

fn stream_reader<R>(
    state: AppState,
    task_id: Uuid,
    reader: R,
    tail: Arc<Mutex<String>>,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let clean = strip_ansi_escapes::strip(line.as_bytes());
            let clean = String::from_utf8_lossy(&clean);
            let clean = clean.trim_end_matches('\r');
            let chunk = format!("{clean}\n");

            {
                let mut tail = tail.lock().await;
                tail.push_str(&chunk);
                if tail.len() > OUTPUT_TAIL_LIMIT {
                    let drain = tail.len() - OUTPUT_TAIL_LIMIT;
                    let drain = next_char_boundary(tail.as_str(), drain);
                    tail.drain(..drain);
                }
            }

            append_log(&state, task_id, &chunk).await;
        }
    })
}

async fn join_reader(reader: Option<tokio::task::JoinHandle<()>>) {
    if let Some(reader) = reader {
        let _ = reader.await;
    }
}

fn next_char_boundary(value: &str, mut index: usize) -> usize {
    while index < value.len() && !value.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn shell_command(raw: &str) -> Command {
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        command.args(["/C", raw]);
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command.args(["-c", raw]);
        command
    }
}

/// Windows npm installs OpenCode as a `.cmd` shim, which `CreateProcessW`
/// cannot execute directly. Resolve the real `.exe` when possible; otherwise
/// fall back to the PATH lookup and let the spawn error be explicit.
fn opencode_program() -> PathBuf {
    if let Some(path) = std::env::var_os("ONI_OPENCODE_BIN") {
        return PathBuf::from(path);
    }

    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(appdata) = std::env::var_os("APPDATA") {
            candidates.push(
                PathBuf::from(appdata)
                    .join("npm")
                    .join("node_modules")
                    .join("@opencode")
                    .join("cli")
                    .join("bin")
                    .join("opencode.exe"),
            );
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local)
                    .join("npm")
                    .join("node_modules")
                    .join("@opencode")
                    .join("cli")
                    .join("bin")
                    .join("opencode.exe"),
            );
        }
        if let Some(home) = std::env::var_os("USERPROFILE") {
            candidates.push(
                PathBuf::from(home)
                    .join("AppData")
                    .join("Roaming")
                    .join("npm")
                    .join("node_modules")
                    .join("@opencode")
                    .join("cli")
                    .join("bin")
                    .join("opencode.exe"),
            );
        }
        if let Some(found) = candidates.into_iter().find(|path| path.is_file()) {
            return found;
        }
    }

    PathBuf::from("opencode")
}

pub async fn append_log(state: &AppState, task_id: Uuid, chunk: &str) {
    let path = state.log_path(task_id);
    if let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        let _ = file.write_all(chunk.as_bytes()).await;
    }

    let _ = state.events.send(ServerEvent::Log {
        task_id,
        chunk: chunk.to_string(),
    });
}

async fn save_and_emit<F>(state: &AppState, task_id: Uuid, update: F) -> Result<Task>
where
    F: FnOnce(&mut Task),
{
    let task = state
        .store
        .update_task(task_id, update)
        .await?
        .context("task not found")?;
    state.emit_task(task.clone()).await;
    Ok(task)
}

async fn finish(state: &AppState, task_id: Uuid, target: TaskState, message: &str) -> Result<()> {
    let existing = state
        .store
        .get_task(task_id)
        .await
        .context("task not found")?;
    if existing.state.is_terminal() {
        return Ok(());
    }

    save_and_emit(state, task_id, |task| {
        task.state = target;
        task.last_error = Some(truncate(message, OUTPUT_TAIL_LIMIT));
        task.finished_at = Some(Utc::now());
    })
    .await?;
    Ok(())
}

async fn fail_active(state: &AppState, task_id: Uuid, message: String) -> Result<()> {
    let existing = match state.store.get_task(task_id).await {
        Some(task) => task,
        None => return Ok(()),
    };
    if !existing.state.is_active() {
        return Ok(());
    }
    finish(state, task_id, TaskState::Failed, &message).await
}

async fn mark_stopped(state: &AppState, task_id: Uuid) -> Result<()> {
    let existing = match state.store.get_task(task_id).await {
        Some(task) => task,
        None => return Ok(()),
    };
    if existing.state.is_terminal() {
        return Ok(());
    }
    save_and_emit(state, task_id, |task| {
        task.state = TaskState::Stopped;
        task.finished_at = Some(Utc::now());
    })
    .await?;
    append_log(state, task_id, "\n== stopped by user ==\n").await;
    Ok(())
}

fn build_prompt(task: &Task, retry_context: Option<&str>) -> String {
    let mut prompt = format!(
        "You are an autonomous coding agent working inside an isolated git worktree managed by Oni.\n\n\
         Task: {}\n\n\
         {}\n\n\
         Acceptance criteria:\n\
         The following command must exit successfully:\n  {}\n\n\
         Rules:\n\
         - Work only inside the current directory (the worktree).\n\
         - Never modify or switch branches.\n\
         - Leave your changes in the working tree. Oni will review and commit them.\n\
         - Do not ask for confirmation; make the best decision and proceed.\n",
        task.title, task.prompt, task.acceptance
    );

    if let Some(context) = retry_context
        && !context.trim().is_empty()
    {
        prompt.push_str("\nThe previous attempt failed. Use this output to fix the problem:\n\n");
        prompt.push_str(context);
        prompt.push('\n');
    }

    prompt
}

fn truncate(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    let mut end = max;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… truncated", &value[..end])
}
