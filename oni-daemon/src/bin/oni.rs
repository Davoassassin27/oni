use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "oni",
    version,
    about = "Oni CLI — dispatch and supervise agent tasks"
)]
struct Cli {
    /// Daemon base URL
    #[arg(
        long,
        global = true,
        env = "ONI_URL",
        default_value = "http://127.0.0.1:7766"
    )]
    url: String,

    /// Bearer token when the daemon runs with ONI_TOKEN
    #[arg(long, global = true, env = "ONI_TOKEN")]
    token: Option<String>,

    /// Print raw JSON instead of tables (ignored by the TUI)
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the interactive terminal UI (default when no command is given)
    Tui {
        /// Skip the TerminalTextEffects intro
        #[arg(long)]
        no_intro: bool,
        /// Refresh interval in seconds
        #[arg(long, default_value_t = 2)]
        interval: u64,
    },
    /// Check that the daemon is reachable
    Health,
    /// List registered projects
    Projects,
    /// List tasks, optionally filtered by state
    Tasks {
        #[arg(long)]
        state: Option<String>,
    },
    /// Show one task by id or id prefix
    Task { id: String },
    /// Print a task log
    Log {
        id: String,
        #[arg(long, default_value_t = 80)]
        tail: usize,
    },
    /// Dispatch a new task
    New {
        /// Project id or exact name
        #[arg(long)]
        project: String,
        /// Task title; defaults to the first line of the prompt
        #[arg(long)]
        title: Option<String>,
        /// Verification command; defaults to the project verify command
        #[arg(long)]
        acceptance: Option<String>,
        /// opencode, claude, or custom; defaults to the project preset
        #[arg(long)]
        agent: Option<String>,
        /// Command that reads the prompt from stdin (custom agent)
        #[arg(long)]
        custom_command: Option<String>,
        #[arg(long, default_value_t = 3)]
        max_iterations: u32,
        /// Wait until review/failed/stopped after creating
        #[arg(long)]
        watch: bool,
        /// Task prompt
        #[arg(required = true, num_args = 1.., trailing_var_arg = true)]
        prompt: Vec<String>,
    },
    /// Follow a task until it needs review or finishes
    Watch {
        id: String,
        #[arg(long, default_value_t = 2)]
        interval: u64,
    },
    /// Request a stop; kills the process tree
    Stop { id: String },
    /// Queue the task again after a failure or stop
    Retry { id: String },
    /// Merge the task branch into the base branch
    Merge { id: String },
    /// Accept verification-file changes so the task can be merged
    Accept { id: String },
    /// Discard the task and remove its worktree
    Discard { id: String },
    /// Print the unified patch, or just its stats
    Patch {
        id: String,
        #[arg(long)]
        stat: bool,
    },
}

#[derive(Clone)]
struct Client {
    base: String,
    token: Option<String>,
}

impl Client {
    fn new(base: String, token: Option<String>) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            token: token
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }

    fn request(&self, method: &str, path: &str) -> ureq::Request {
        let url = format!("{}/api{}", self.base, path);
        let request = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            _ => unreachable!("unsupported method"),
        };
        match &self.token {
            Some(token) => request.set("Authorization", &format!("Bearer {token}")),
            None => request,
        }
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send(self.request("GET", path), None)
    }

    fn get_text(&self, path: &str) -> Result<String> {
        match self.request("GET", path).call() {
            Ok(response) => response
                .into_string()
                .context("daemon returned non-UTF-8 log output"),
            Err(error) => Err(http_error(error)),
        }
    }

    fn post<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T> {
        self.send(self.request("POST", path), Some(body))
    }

    fn post_no_body<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send(self.request("POST", path), None)
    }

    fn post_empty(&self, path: &str) -> Result<()> {
        match self.request("POST", path).call() {
            Ok(_) => Ok(()),
            Err(error) => Err(http_error(error)),
        }
    }

    fn send<T: DeserializeOwned>(&self, request: ureq::Request, body: Option<Value>) -> Result<T> {
        let response = match body {
            Some(body) => request.send_json(body),
            None => request.call(),
        };

        match response {
            Ok(response) => response
                .into_json::<T>()
                .context("daemon returned invalid JSON"),
            Err(error) => Err(http_error(error)),
        }
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
            anyhow::anyhow!("cannot reach the Oni daemon: {error} (is it running?)")
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    ok: bool,
    version: String,
    projects: usize,
    tasks: usize,
    uptime_secs: u64,
    auth_required: bool,
    data_dir: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Project {
    id: String,
    name: String,
    path: String,
    base_branch: String,
    verify_command: String,
    agent: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
struct VerificationGuard {
    tampered: bool,
    files: Vec<String>,
    accepted_by_human: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Task {
    id: String,
    project_name: String,
    title: String,
    prompt: String,
    acceptance: String,
    state: String,
    branch: String,
    worktree_path: String,
    iteration: u32,
    max_iterations: u32,
    agent: String,
    base_branch: String,
    repo_path: String,
    created_at: String,
    updated_at: String,
    last_error: Option<String>,
    diff: Option<DiffSummary>,
    #[serde(default)]
    verification: VerificationGuard,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffSummary {
    files: Vec<FileChange>,
    insertions: u64,
    deletions: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileChange {
    path: String,
    insertions: u64,
    deletions: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct Patch {
    files: Vec<PatchFile>,
    truncated: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct PatchFile {
    path: String,
    patch: String,
    insertions: u64,
    deletions: u64,
}

fn main() {
    let cli = Cli::parse();
    if let Err(error) = run(cli) {
        eprintln!("oni: {error:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let Cli {
        url,
        token,
        json,
        command,
    } = cli;
    let command = match command {
        Some(Command::Tui { no_intro, interval }) => {
            return oni_tui::run(oni_tui::Options {
                url,
                token,
                interval,
                no_intro,
            });
        }
        Some(command) => command,
        None => {
            return oni_tui::run(oni_tui::Options {
                url,
                token,
                interval: 2,
                no_intro: false,
            });
        }
    };

    let client = Client::new(url, token);

    match command {
        Command::Tui { .. } => unreachable!("TUI is handled before the API client"),
        Command::Health => health(&client, json),
        Command::Projects => projects(&client, json),
        Command::Tasks { state } => tasks(&client, state.as_deref(), json),
        Command::Task { id } => task(&client, &id, json),
        Command::Log { id, tail } => log(&client, &id, tail),
        Command::New {
            project,
            title,
            acceptance,
            agent,
            custom_command,
            max_iterations,
            watch,
            prompt,
        } => new_task(
            &client,
            &project,
            title,
            acceptance,
            agent,
            custom_command,
            max_iterations,
            watch,
            &prompt.join(" "),
            json,
        ),
        Command::Watch { id, interval } => watch(&client, &id, interval, json),
        Command::Stop { id } => stop(&client, &id, json),
        Command::Retry { id } => retry(&client, &id, json),
        Command::Merge { id } => merge(&client, &id, json),
        Command::Accept { id } => accept_verification(&client, &id, json),
        Command::Discard { id } => discard(&client, &id),
        Command::Patch { id, stat } => patch(&client, &id, stat, json),
    }
}

fn health(client: &Client, json_output: bool) -> Result<()> {
    let health: Health = client.get("/health")?;
    if json_output {
        return print_json(&health);
    }

    println!(
        "ONI v{} · {}",
        health.version,
        if health.ok { "ok" } else { "not ok" }
    );
    println!(
        "projects {} · tasks {} · uptime {}s",
        health.projects, health.tasks, health.uptime_secs
    );
    println!("data {}", health.data_dir);
    println!(
        "auth {}",
        if health.auth_required {
            "token required"
        } else {
            "disabled (localhost only)"
        }
    );
    Ok(())
}

fn projects(client: &Client, json_output: bool) -> Result<()> {
    let projects: Vec<Project> = client.get("/projects")?;
    if json_output {
        return print_json(&projects);
    }

    if projects.is_empty() {
        println!("No projects yet. Add one from the PWA.");
        return Ok(());
    }

    println!(
        "{:<10} {:<24} {:<8} {:<10} VERIFY",
        "ID", "NAME", "BASE", "AGENT"
    );
    for project in projects {
        println!(
            "{:<10} {:<24} {:<8} {:<10} {}",
            short_id(&project.id),
            truncate(&project.name, 24),
            truncate(&project.base_branch, 8),
            project.agent,
            project.verify_command
        );
    }
    Ok(())
}

fn tasks(client: &Client, state: Option<&str>, json_output: bool) -> Result<()> {
    let tasks: Vec<Task> = client.get("/tasks")?;
    let filtered: Vec<&Task> = tasks
        .iter()
        .filter(|task| match state {
            Some(state) => task.state.eq_ignore_ascii_case(state),
            None => true,
        })
        .collect();

    if json_output {
        return print_json(&filtered);
    }

    if filtered.is_empty() {
        println!("No tasks.");
        return Ok(());
    }

    println!(
        "{:<10} {:<10} {:<6} {:<18} TITLE",
        "ID", "STATE", "ATT", "PROJECT"
    );
    for task in filtered {
        println!(
            "{:<10} {:<10} {:<6} {:<18} {}",
            short_id(&task.id),
            task.state,
            format!("{}/{}", task.iteration, task.max_iterations),
            truncate(&task.project_name, 18),
            truncate(&task.title, 60)
        );
    }
    Ok(())
}

fn task(client: &Client, id: &str, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let task: Task = client.get(&format!("/tasks/{id}"))?;
    if json_output {
        return print_json(&task);
    }
    print_task(&task);
    Ok(())
}

fn print_task(task: &Task) {
    println!("{}", task.title);
    println!("{:<12} {}", "id", task.id);
    println!("{:<12} {}", "state", task.state);
    println!("{:<12} {}", "project", task.project_name);
    println!("{:<12} {}", "branch", task.branch);
    println!("{:<12} {}", "worktree", task.worktree_path);
    println!("{:<12} {}", "agent", task.agent);
    println!(
        "{:<12} {}/{}",
        "attempt", task.iteration, task.max_iterations
    );
    println!("{:<12} {}", "acceptance", task.acceptance);
    println!("{:<12} {}", "updated", task.updated_at);
    if let Some(diff) = &task.diff {
        println!(
            "{:<12} {} files · +{} −{}",
            "changes",
            diff.files.len(),
            diff.insertions,
            diff.deletions
        );
    }
    if task.verification.tampered {
        println!(
            "{:<12} {}",
            "verification",
            if task.verification.accepted_by_human {
                "sensitive files changed (accepted by human)"
            } else {
                "SENSITIVE FILES CHANGED"
            }
        );
        for file in &task.verification.files {
            println!("             {file}");
        }
        if !task.verification.accepted_by_human {
            println!(
                "             merge blocked; run `oni accept {}`",
                short_id(&task.id)
            );
        }
    }
    if let Some(error) = &task.last_error {
        println!("{:<12} {}", "last error", first_line(error));
    }
}

fn log(client: &Client, id: &str, tail: usize) -> Result<()> {
    let id = resolve_task(client, id)?;
    let text = client.get_text(&format!("/tasks/{id}/log"))?;
    let lines: Vec<&str> = text.lines().collect();
    let start = if tail == 0 {
        0
    } else {
        lines.len().saturating_sub(tail)
    };
    for line in &lines[start..] {
        println!("{line}");
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn new_task(
    client: &Client,
    project: &str,
    title: Option<String>,
    acceptance: Option<String>,
    agent: Option<String>,
    custom_command: Option<String>,
    max_iterations: u32,
    watch_task: bool,
    prompt: &str,
    json_output: bool,
) -> Result<()> {
    let project = find_project(client, project)?;
    let title = title.unwrap_or_else(|| title_from_prompt(prompt));
    if title.trim().is_empty() {
        bail!("task title cannot be empty");
    }
    if prompt.trim().is_empty() {
        bail!("task prompt cannot be empty");
    }
    if let Some(agent) = &agent {
        if !matches!(agent.as_str(), "opencode" | "claude" | "custom") {
            bail!("agent must be opencode, claude, or custom");
        }
    }

    let mut body = json!({
        "projectId": project.id,
        "title": title,
        "prompt": prompt,
        "maxIterations": max_iterations,
    });
    if let Some(acceptance) = acceptance {
        body["acceptance"] = json!(acceptance);
    }
    if let Some(agent) = agent {
        body["agent"] = json!(agent);
    }
    if let Some(custom_command) = custom_command {
        body["customCommand"] = json!(custom_command);
    }

    let task: Task = client.post("/tasks", body)?;
    if json_output {
        print_json(&task)?;
    } else {
        println!("created task {}", task.id);
        println!("  state {} · branch {}", task.state, task.branch);
    }

    if watch_task {
        watch(client, &task.id, 2, json_output)?;
    }
    Ok(())
}

fn watch(client: &Client, id: &str, interval: u64, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let started = Instant::now();
    let mut last = String::new();

    loop {
        let task: Task = client.get(&format!("/tasks/{id}"))?;
        if task.state != last {
            if !json_output {
                println!(
                    "{:>4}s  {:<10} attempt {}/{}",
                    started.elapsed().as_secs(),
                    task.state,
                    task.iteration,
                    task.max_iterations
                );
            }
            last = task.state.clone();
        }

        if is_watch_done(&task.state) {
            if json_output {
                print_json(&task)?;
            } else if task.state == "review" {
                println!("ready for review: {}", task.id);
                if let Some(diff) = &task.diff {
                    println!(
                        "{} files · +{} −{}",
                        diff.files.len(),
                        diff.insertions,
                        diff.deletions
                    );
                }
                if task.verification.tampered && !task.verification.accepted_by_human {
                    println!(
                        "verification-sensitive files changed: {}",
                        task.verification.files.join(", ")
                    );
                    println!(
                        "next: oni patch {} then oni accept {} before merge",
                        short_id(&task.id),
                        short_id(&task.id)
                    );
                } else {
                    println!(
                        "next: oni patch {} or oni merge {}",
                        short_id(&task.id),
                        short_id(&task.id)
                    );
                }
            } else {
                println!("finished: {} ({})", task.state, task.id);
                if let Some(error) = &task.last_error {
                    println!("{}", first_line(error));
                }
            }
            return Ok(());
        }

        sleep(Duration::from_secs(interval.max(1)));
    }
}

fn stop(client: &Client, id: &str, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let task: Task = client.post_no_body(&format!("/tasks/{id}/stop"))?;
    if !json_output {
        println!("stop requested (state {})", task.state);
    }

    // The API returns the pre-stop snapshot while the runner is still winding
    // down. Poll briefly so the CLI reports the settled state.
    for _ in 0..20 {
        sleep(Duration::from_millis(500));
        let current: Task = client.get(&format!("/tasks/{id}"))?;
        if current.state == "stopped" {
            if json_output {
                print_json(&current)?;
            } else {
                println!("stopped: {}", current.id);
            }
            return Ok(());
        }
        if is_watch_done(&current.state) {
            if json_output {
                print_json(&current)?;
            } else {
                println!("task finished before stop: {}", current.state);
            }
            return Ok(());
        }
    }

    if json_output {
        print_json(&task)?;
    } else {
        println!("stop still in progress; check `oni task {id}`");
    }
    Ok(())
}

fn retry(client: &Client, id: &str, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let task: Task = client.post_no_body(&format!("/tasks/{id}/retry"))?;
    if json_output {
        print_json(&task)?;
    } else {
        println!("queued again: {} (state {})", task.id, task.state);
    }
    Ok(())
}

fn merge(client: &Client, id: &str, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let task: Task = client.post_no_body(&format!("/tasks/{id}/merge"))?;
    if json_output {
        print_json(&task)?;
    } else {
        println!("merged: {} → {}", task.branch, task.base_branch);
    }
    Ok(())
}

fn accept_verification(client: &Client, id: &str, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let task: Task = client.post_no_body(&format!("/tasks/{id}/accept-verification"))?;
    if json_output {
        print_json(&task)?;
    } else {
        println!("verification changes accepted: {}", task.id);
    }
    Ok(())
}

fn discard(client: &Client, id: &str) -> Result<()> {
    let id = resolve_task(client, id)?;
    client.post_empty(&format!("/tasks/{id}/discard"))?;
    println!("discarded: {id}");
    Ok(())
}

fn patch(client: &Client, id: &str, stat_only: bool, json_output: bool) -> Result<()> {
    let id = resolve_task(client, id)?;
    let patch: Patch = client.get(&format!("/tasks/{id}/patch"))?;
    if json_output {
        return print_json(&patch);
    }

    if patch.truncated {
        println!("warning: patch truncated at 512 KB");
    }
    if patch.files.is_empty() {
        println!("No changes.");
        return Ok(());
    }

    if stat_only {
        for file in &patch.files {
            println!(
                "{:<48} +{:<6} −{}",
                file.path, file.insertions, file.deletions
            );
        }
        println!(
            "{} files · +{} −{}",
            patch.files.len(),
            patch.files.iter().map(|file| file.insertions).sum::<u64>(),
            patch.files.iter().map(|file| file.deletions).sum::<u64>()
        );
    } else {
        for file in &patch.files {
            println!(
                "=== {} (+{} −{})",
                file.path, file.insertions, file.deletions
            );
            print!("{}", file.patch);
            if !file.patch.ends_with('\n') {
                println!();
            }
        }
    }
    Ok(())
}

fn find_project(client: &Client, needle: &str) -> Result<Project> {
    let projects: Vec<Project> = client.get("/projects")?;
    if let Some(project) = projects.iter().find(|project| project.id == needle) {
        return Ok(project.clone());
    }

    let matches: Vec<&Project> = projects
        .iter()
        .filter(|project| project.name.eq_ignore_ascii_case(needle))
        .collect();

    match matches.as_slice() {
        [project] => Ok((*project).clone()),
        [] => bail!("project not found: {needle} (try `oni projects`)"),
        _ => bail!("project name is ambiguous: {needle} (use the id)"),
    }
}

fn resolve_task(client: &Client, needle: &str) -> Result<String> {
    let needle = needle.trim().to_ascii_lowercase();
    if needle.is_empty() {
        bail!("task id cannot be empty");
    }

    let tasks: Vec<Task> = client.get("/tasks")?;
    if let Some(task) = tasks.iter().find(|task| task.id == needle) {
        return Ok(task.id.clone());
    }

    let matches: Vec<&Task> = tasks
        .iter()
        .filter(|task| task.id.to_ascii_lowercase().starts_with(&needle))
        .collect();

    match matches.as_slice() {
        [task] => Ok(task.id.clone()),
        [] => bail!("task not found: {needle} (try `oni tasks`)"),
        _ => bail!("task id prefix is ambiguous: {needle}"),
    }
}

fn is_watch_done(state: &str) -> bool {
    matches!(
        state,
        "review" | "failed" | "stopped" | "merged" | "discarded"
    )
}

fn title_from_prompt(prompt: &str) -> String {
    let first = prompt.lines().next().unwrap_or(prompt).trim();
    if first.is_empty() {
        return "Untitled task".into();
    }
    let mut title: String = first.chars().take(72).collect();
    if first.chars().count() > 72 {
        title.push('…');
    }
    title
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(max.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

fn first_line(value: &str) -> &str {
    value.lines().next().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_prompt_uses_first_line_and_truncates() {
        assert_eq!(title_from_prompt("Add pricing\nDetails"), "Add pricing");
        let long = "x".repeat(100);
        let title = title_from_prompt(&long);
        assert_eq!(title.chars().count(), 73);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn short_id_keeps_short_values() {
        assert_eq!(short_id("1234567890"), "12345678");
        assert_eq!(short_id("abc"), "abc");
    }

    #[test]
    fn truncate_respects_the_limit() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5).chars().count(), 5);
        assert!(truncate("hello world", 5).ends_with('…'));
    }
}
