//! Juniperus router integration.
//!
//! The daemon never executes a router decision directly. `POST /api/router/route`
//! returns a validated, preview-only decision for the PWA/CLI. The deterministic
//! fallback is always available; the SLM sidecar is optional (`ONI_JUNIPERUS_URL`).

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::config::Config;
use crate::git;
use crate::model::{Project, Task, TaskState};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterIntent {
    SpawnAgent,
    QueryStatus,
    MergeWorktree,
    RetryTask,
    CancelTask,
    DiscardTask,
    ListProjects,
    Help,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentTarget {
    Opencode,
    Claude,
    Custom,
    #[default]
    Auto,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStrategy {
    #[default]
    IsolatedBranch,
    ExistingBranch,
    /// Only accepted to be rejected and downgraded to `isolated_branch`.
    MainDirect,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopStatus {
    #[default]
    SingleShot,
    ContinueLoop,
    Completed,
    NeedsHuman,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RouterDecision {
    pub intent: RouterIntent,
    pub agent_target: AgentTarget,
    pub model_override: Option<String>,
    pub worktree_strategy: WorktreeStrategy,
    pub branch_name: Option<String>,
    pub project_ref: Option<String>,
    pub task_ref: Option<String>,
    pub task_prompt: Option<String>,
    pub loop_status: LoopStatus,
    pub max_iterations: u32,
    pub success_criterion: Option<String>,
    pub reasoning: String,
}

impl Default for RouterDecision {
    fn default() -> Self {
        Self {
            intent: RouterIntent::Unknown,
            agent_target: AgentTarget::Auto,
            model_override: None,
            worktree_strategy: WorktreeStrategy::IsolatedBranch,
            branch_name: None,
            project_ref: None,
            task_ref: None,
            task_prompt: None,
            loop_status: LoopStatus::NeedsHuman,
            max_iterations: 3,
            success_criterion: None,
            reasoning: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingMeta {
    pub source: String,
    pub latency_ms: u64,
    pub degraded: bool,
    pub error: Option<String>,
    pub backend: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteResponse {
    pub decision: RouterDecision,
    pub routing: RoutingMeta,
    pub context: RouteContextSummary,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteContextSummary {
    pub projects: usize,
    pub tasks: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct SidecarProject {
    pub id: String,
    pub name: String,
    pub base_branch: String,
    pub verify_command: String,
    pub agent: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SidecarTask {
    pub id: String,
    pub title: String,
    pub state: String,
    pub project: String,
    pub branch: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SidecarContext {
    pub projects: Vec<SidecarProject>,
    pub tasks: Vec<SidecarTask>,
    pub selected_project_id: Option<String>,
    pub selected_task_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct SidecarMeta {
    backend: String,
    model: String,
    latency_ms: f64,
    degraded: bool,
    error: Option<String>,
}

impl Default for SidecarMeta {
    fn default() -> Self {
        Self {
            backend: String::new(),
            model: String::new(),
            latency_ms: 0.0,
            degraded: false,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct SidecarResponse {
    decision: RouterDecision,
    #[serde(default)]
    meta: SidecarMeta,
}

/// Resolve a natural-language command into a preview decision.
pub async fn route(
    state: &crate::state::AppState,
    text: String,
    project_id: Option<Uuid>,
    task_id: Option<Uuid>,
) -> RouteResponse {
    let started = Instant::now();
    let projects = state.store.list_projects().await;
    let tasks = state.store.list_tasks().await;
    let context = build_context(&projects, &tasks, project_id, task_id);
    let context_summary = RouteContextSummary {
        projects: context.projects.len(),
        tasks: context.tasks.len(),
    };

    let mut degraded = false;
    let mut error: Option<String> = None;
    let mut backend: Option<String> = None;
    let mut model: Option<String> = None;

    if state.config.juniperus_url.is_some() {
        let config = state.config.clone();
        let prompt = text.clone();
        let sidecar_context = context.clone();
        let call =
            tokio::task::spawn_blocking(move || call_sidecar(&config, &prompt, &sidecar_context))
                .await;

        match call {
            Ok(Ok(response)) => {
                degraded = response.meta.degraded;
                error = response.meta.error.clone();
                backend = Some(response.meta.backend.clone()).filter(|value| !value.is_empty());
                model = Some(response.meta.model.clone()).filter(|value| !value.is_empty());
                if !degraded {
                    let decision = sanitize_decision(response.decision, &text, &context);
                    return RouteResponse {
                        decision,
                        routing: RoutingMeta {
                            source: "juniperus".into(),
                            latency_ms: started.elapsed().as_millis() as u64,
                            degraded: false,
                            error: None,
                            backend,
                            model,
                        },
                        context: context_summary,
                    };
                }
                if error.is_none() {
                    error = Some("juniperus returned a degraded decision".into());
                }
            }
            Ok(Err(message)) => {
                degraded = true;
                error = Some(message);
            }
            Err(join_error) => {
                degraded = true;
                error = Some(format!("juniperus task failed: {join_error}"));
            }
        }
    }

    let decision = sanitize_decision(fallback_decision(&text, &context), &text, &context);
    RouteResponse {
        decision,
        routing: RoutingMeta {
            source: "fallback".into(),
            latency_ms: started.elapsed().as_millis() as u64,
            degraded,
            error,
            backend,
            model,
        },
        context: context_summary,
    }
}

/// Health probe for `GET /api/router/status`. It never fails the daemon.
pub fn probe_sidecar(config: &Config) -> serde_json::Value {
    let Some(base) = config.juniperus_url.as_deref() else {
        return json!({
            "enabled": false,
            "url": null,
            "reachable": false,
            "detail": "ONI_JUNIPERUS_URL is not set; using the deterministic fallback",
        });
    };

    let url = format!("{}/health", base.trim_end_matches('/'));
    let mut request = ureq::get(&url).timeout(Duration::from_millis(config.juniperus_timeout_ms));
    if let Some(token) = &config.juniperus_token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    match request.call() {
        Ok(response) => match response.into_json::<serde_json::Value>() {
            Ok(body) => json!({
                "enabled": true,
                "url": base,
                "reachable": true,
                "sidecar": body,
            }),
            Err(error) => json!({
                "enabled": true,
                "url": base,
                "reachable": false,
                "detail": format!("invalid health response: {error}"),
            }),
        },
        Err(error) => json!({
            "enabled": true,
            "url": base,
            "reachable": false,
            "detail": format!("{error}"),
        }),
    }
}

fn call_sidecar(
    config: &Config,
    text: &str,
    context: &SidecarContext,
) -> Result<SidecarResponse, String> {
    let base = config
        .juniperus_url
        .as_deref()
        .ok_or_else(|| "ONI_JUNIPERUS_URL is not set".to_string())?;
    let url = format!("{}/route", base.trim_end_matches('/'));
    let mut request = ureq::post(&url).timeout(Duration::from_millis(config.juniperus_timeout_ms));
    if let Some(token) = &config.juniperus_token {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let response = request
        .set("Content-Type", "application/json")
        .send_json(json!({ "text": text, "context": context }))
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!("juniperus returned HTTP {code}"),
            ureq::Error::Transport(transport) => format!("juniperus unreachable: {transport}"),
        })?;
    response
        .into_json::<SidecarResponse>()
        .map_err(|error| format!("juniperus returned invalid JSON: {error}"))
}

fn build_context(
    projects: &[Project],
    tasks: &[Task],
    project_id: Option<Uuid>,
    task_id: Option<Uuid>,
) -> SidecarContext {
    let selected_project_id = project_id.or_else(|| {
        if projects.len() == 1 {
            Some(projects[0].id)
        } else {
            None
        }
    });

    let mut ordered_projects: Vec<&Project> = Vec::new();
    if let Some(selected) = selected_project_id
        && let Some(project) = projects.iter().find(|project| project.id == selected)
    {
        ordered_projects.push(project);
    }
    for project in projects {
        if ordered_projects.len() >= 8 {
            break;
        }
        if !ordered_projects.iter().any(|item| item.id == project.id) {
            ordered_projects.push(project);
        }
    }

    let mut ordered_tasks: Vec<&Task> = Vec::new();
    if let Some(selected) = task_id
        && let Some(task) = tasks.iter().find(|task| task.id == selected)
    {
        ordered_tasks.push(task);
    }
    for task in tasks {
        if ordered_tasks.len() >= 12 {
            break;
        }
        let interesting = task.state.is_active()
            || matches!(
                task.state,
                TaskState::Review | TaskState::Failed | TaskState::Stopped
            );
        if interesting && !ordered_tasks.iter().any(|item| item.id == task.id) {
            ordered_tasks.push(task);
        }
    }

    SidecarContext {
        projects: ordered_projects
            .into_iter()
            .map(|project| SidecarProject {
                id: project.id.to_string(),
                name: project.name.clone(),
                base_branch: project.base_branch.clone(),
                verify_command: project.verify_command.clone(),
                agent: project.agent.label().into(),
            })
            .collect(),
        tasks: ordered_tasks
            .into_iter()
            .map(|task| SidecarTask {
                id: task.id.to_string(),
                title: task.title.clone(),
                state: state_label(task.state).into(),
                project: task.project_name.clone(),
                branch: task.branch.clone(),
            })
            .collect(),
        selected_project_id: selected_project_id.map(|id| id.to_string()),
        selected_task_id: task_id.map(|id| id.to_string()),
    }
}

fn state_label(state: TaskState) -> &'static str {
    match state {
        TaskState::Queued => "queued",
        TaskState::Acting => "acting",
        TaskState::Verifying => "verifying",
        TaskState::Retrying => "retrying",
        TaskState::Review => "review",
        TaskState::Merged => "merged",
        TaskState::Discarded => "discarded",
        TaskState::Failed => "failed",
        TaskState::Stopped => "stopped",
    }
}

/// Deterministic fallback. It is intentionally conservative: when it cannot
/// identify a project or task it asks for a human instead of guessing.
pub(crate) fn fallback_decision(text: &str, context: &SidecarContext) -> RouterDecision {
    let lowered = text.to_lowercase();
    let task = find_task(&lowered, context);
    let project = find_project(&lowered, context);
    let mentions_main = lowered.contains("main")
        || lowered.contains("master")
        || lowered.contains("rama principal")
        || lowered.contains("base branch");

    if contains_any(&lowered, &["merge", "integr", "fusion", "ship"]) {
        return task_action(
            RouterIntent::MergeWorktree,
            LoopStatus::Completed,
            1,
            task.as_ref(),
            "Se pidió integrar una tarea.",
        );
    }
    if contains_any(
        &lowered,
        &[
            "retry",
            "reintent",
            "otra oportunidad",
            "volvé a correr",
            "volve a correr",
            "run again",
            "again",
        ],
    ) {
        return task_action(
            RouterIntent::RetryTask,
            LoopStatus::ContinueLoop,
            3,
            task.as_ref(),
            "Se pidió reintentar una tarea.",
        );
    }
    if contains_any(
        &lowered,
        &[
            "cancel",
            "stop",
            "frena",
            "frená",
            "detene",
            "detené",
            "mata",
            "matá",
            "pará",
            "para la tarea",
            "kill",
        ],
    ) {
        return task_action(
            RouterIntent::CancelTask,
            LoopStatus::Completed,
            1,
            task.as_ref(),
            "Se pidió detener una tarea.",
        );
    }
    if contains_any(
        &lowered,
        &[
            "discard",
            "descart",
            "tira la tarea",
            "tirá la tarea",
            "borra la tarea",
            "borrá la tarea",
            "delete",
            "throw away",
        ],
    ) {
        return task_action(
            RouterIntent::DiscardTask,
            LoopStatus::Completed,
            1,
            task.as_ref(),
            "Se pidió descartar una tarea.",
        );
    }
    if contains_any(
        &lowered,
        &[
            "help",
            "ayuda",
            "que podes",
            "qué podés",
            "para que serv",
            "para qué serv",
            "como funciona",
            "cómo funciona",
            "what can you do",
        ],
    ) {
        return simple_decision(
            RouterIntent::Help,
            LoopStatus::Completed,
            "El usuario pide ayuda.",
        );
    }
    if contains_any(
        &lowered,
        &[
            "projects",
            "proyectos",
            "repos",
            "repositorios",
            "repositorios",
        ],
    ) {
        return simple_decision(
            RouterIntent::ListProjects,
            LoopStatus::Completed,
            "Consulta de proyectos.",
        );
    }
    let strong_spawn = contains_any(
        &lowered,
        &[
            "add",
            "agrega",
            "agregá",
            "crea",
            "creá",
            "implementa",
            "implementá",
            "build",
            "arregla",
            "arreglá",
            "fix",
            "refactor",
            "actualiza",
            "actualizá",
            "update",
            "migra",
            "migrá",
            "documenta",
            "documentá",
            "escribi",
            "escribí",
            "write",
            "test",
            "hace",
            "hacé",
            "bumpea",
            "bumpeá",
            "optimiza",
            "optimizá",
            "limpia",
            "limpiá",
            "clean",
            "separa",
            "separá",
            "split",
            "protege",
            "protegé",
            "protect",
            "valida",
            "validá",
            "validate",
            "revisa",
            "revisá",
            "elimina",
            "eliminá",
            "remove",
            "renombra",
            "renombrá",
            "rename",
            "corregi",
            "corregí",
            "correct",
            "dockeriza",
            "dockerizá",
            "dockerize",
            "export",
            "exportá",
            "spike",
            "diff",
        ],
    );

    if contains_any(
        &lowered,
        &[
            "status",
            "estado",
            "como va",
            "cómo va",
            "que paso",
            "qué pasó",
            "log",
            "review",
            "tareas",
            "tasks",
            "stuck",
            "trabad",
            "lista",
            "listo",
            "ready",
            "terminada",
            "done",
        ],
    ) && !strong_spawn
    {
        let task_ref = task.as_ref().map(|task| short_task_ref(task, &lowered));
        return RouterDecision {
            intent: RouterIntent::QueryStatus,
            project_ref: task.as_ref().map(|task| task.project.clone()),
            task_ref,
            loop_status: LoopStatus::Completed,
            max_iterations: 1,
            reasoning: "Consulta de estado.".into(),
            ..Default::default()
        };
    }
    if contains_any(
        &lowered,
        &[
            "add",
            "agrega",
            "agregá",
            "crea",
            "creá",
            "implementa",
            "implementá",
            "build",
            "arregla",
            "arreglá",
            "fix",
            "refactor",
            "actualiza",
            "actualizá",
            "update",
            "migra",
            "migrá",
            "documenta",
            "documentá",
            "escribi",
            "escribí",
            "test",
            "hace",
            "hacé",
            "bumpea",
            "bumpeá",
            "optimiza",
            "optimizá",
            "limpia",
            "limpiá",
            "separa",
            "separá",
            "protege",
            "protegé",
            "valida",
            "validá",
            "revisa",
            "revisá",
            "elimina",
            "eliminá",
            "renombra",
            "renombrá",
            "corregi",
            "corregí",
            "dockeriza",
            "dockerizá",
            "dockerize",
            "export",
            "exportá",
            "spike",
            "write",
            "clean",
            "split",
            "protect",
            "validate",
            "remove",
            "rename",
            "correct",
            "diff",
        ],
    ) {
        let Some(project) = project.or_else(|| selected_project(context)) else {
            return RouterDecision {
                intent: RouterIntent::SpawnAgent,
                task_prompt: Some(text.to_string()),
                loop_status: LoopStatus::NeedsHuman,
                max_iterations: 3,
                reasoning: "Falta elegir el proyecto; no se inventa uno.".into(),
                ..Default::default()
            };
        };
        return RouterDecision {
            intent: RouterIntent::SpawnAgent,
            agent_target: agent_target(&project.agent),
            worktree_strategy: WorktreeStrategy::IsolatedBranch,
            branch_name: Some(format!("oni/{}", git::slugify(text, 60))),
            project_ref: Some(project.name.clone()),
            task_prompt: Some(text.to_string()),
            loop_status: if mentions_main {
                LoopStatus::NeedsHuman
            } else {
                LoopStatus::SingleShot
            },
            max_iterations: 3,
            success_criterion: non_empty(&project.verify_command),
            reasoning: if mentions_main {
                "La rama base está prohibida; se usará un worktree aislado.".into()
            } else {
                "Pedido de implementación delegado a un agente.".into()
            },
            ..Default::default()
        };
    }

    RouterDecision {
        intent: RouterIntent::Unknown,
        loop_status: LoopStatus::NeedsHuman,
        max_iterations: 1,
        reasoning: "No se pudo interpretar el pedido.".into(),
        ..Default::default()
    }
}

fn task_action(
    intent: RouterIntent,
    loop_status: LoopStatus,
    max_iterations: u32,
    task: Option<&SidecarTask>,
    reasoning: &str,
) -> RouterDecision {
    let Some(task) = task else {
        return RouterDecision {
            intent,
            loop_status: LoopStatus::NeedsHuman,
            max_iterations,
            reasoning: "Falta identificar la tarea.".into(),
            ..Default::default()
        };
    };
    RouterDecision {
        intent,
        project_ref: Some(task.project.clone()),
        task_ref: Some(task.id.chars().take(8).collect()),
        loop_status,
        max_iterations,
        reasoning: reasoning.into(),
        ..Default::default()
    }
}

fn simple_decision(
    intent: RouterIntent,
    loop_status: LoopStatus,
    reasoning: &str,
) -> RouterDecision {
    RouterDecision {
        intent,
        loop_status,
        max_iterations: 1,
        reasoning: reasoning.into(),
        ..Default::default()
    }
}

fn find_task(lowered: &str, context: &SidecarContext) -> Option<SidecarTask> {
    for task in &context.tasks {
        let id = task.id.to_lowercase();
        let short: String = id.chars().take(8).collect();
        if lowered.contains(&id) || (!short.is_empty() && lowered.contains(&short)) {
            return Some(task.clone());
        }
        let title = task.title.to_lowercase();
        if !title.is_empty() && lowered.contains(&title) {
            return Some(task.clone());
        }
    }
    context
        .selected_task_id
        .as_deref()
        .and_then(|selected| {
            context
                .tasks
                .iter()
                .find(|task| task.id == selected || task.id.starts_with(selected))
        })
        .cloned()
}

fn find_project(lowered: &str, context: &SidecarContext) -> Option<SidecarProject> {
    for project in &context.projects {
        if contains_name(lowered, &project.name.to_lowercase()) {
            return Some(project.clone());
        }
    }
    selected_project(context)
}

/// Substring match with word-ish boundaries, so project `api` does not match
/// `rapid` or `capital`.
fn contains_name(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.match_indices(needle).any(|(index, _)| {
        let before = haystack[..index].chars().next_back();
        let after = haystack[index + needle.len()..].chars().next();
        before.is_none_or(|character| !character.is_alphanumeric())
            && after.is_none_or(|character| !character.is_alphanumeric())
    })
}

fn selected_project(context: &SidecarContext) -> Option<SidecarProject> {
    context
        .selected_project_id
        .as_deref()
        .and_then(|selected| {
            context
                .projects
                .iter()
                .find(|project| project.id == selected || project.id.starts_with(selected))
        })
        .cloned()
}

fn short_task_ref(task: &SidecarTask, lowered: &str) -> String {
    if !task.title.is_empty() && lowered.contains(&task.title.to_lowercase()) {
        task.title.clone()
    } else {
        task.id.chars().take(8).collect()
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn agent_target(agent: &str) -> AgentTarget {
    match agent {
        "opencode" => AgentTarget::Opencode,
        "claude" => AgentTarget::Claude,
        "custom" => AgentTarget::Custom,
        _ => AgentTarget::Auto,
    }
}

/// Last line of defense: whatever the sidecar says, never emit `main_direct`,
/// never leave a task action without a task, and keep the iteration cap.
pub(crate) fn sanitize_decision(
    mut decision: RouterDecision,
    text: &str,
    context: &SidecarContext,
) -> RouterDecision {
    if decision.worktree_strategy == WorktreeStrategy::MainDirect {
        decision.worktree_strategy = WorktreeStrategy::IsolatedBranch;
        decision.loop_status = LoopStatus::NeedsHuman;
        decision.reasoning = append_reason(
            &decision.reasoning,
            "Trabajar sobre la rama base está prohibido; se propone un worktree aislado.",
        );
    }
    if decision.max_iterations == 0 {
        decision.max_iterations = match decision.intent {
            RouterIntent::SpawnAgent | RouterIntent::RetryTask => 3,
            _ => 1,
        };
    }
    decision.max_iterations = decision.max_iterations.clamp(1, 6);

    decision.project_ref =
        clean_ref(decision.project_ref).map(|value| value.chars().take(200).collect());
    decision.task_ref = clean_ref(decision.task_ref).map(|value| value.chars().take(200).collect());
    decision.model_override =
        clean_ref(decision.model_override).map(|value| value.chars().take(200).collect());
    decision.task_prompt = decision
        .task_prompt
        .map(|value| value.trim().chars().take(4000).collect::<String>())
        .filter(|value| !value.is_empty());
    decision.success_criterion = decision
        .success_criterion
        .map(|value| value.trim().chars().take(500).collect::<String>())
        .filter(|value| !value.is_empty());
    decision.reasoning = decision.reasoning.trim().chars().take(400).collect();

    let project = decision
        .project_ref
        .as_deref()
        .and_then(|name| context.projects.iter().find(|project| project.name == name))
        .cloned()
        .or_else(|| selected_project(context));

    if decision.intent == RouterIntent::SpawnAgent {
        if decision
            .task_prompt
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            decision.task_prompt = Some(text.to_string());
        }
        if decision.project_ref.is_none() {
            decision.project_ref = project.as_ref().map(|project| project.name.clone());
        }
        if decision.success_criterion.is_none() {
            decision.success_criterion = project
                .as_ref()
                .and_then(|project| non_empty(&project.verify_command));
        }
        if decision.agent_target == AgentTarget::Auto
            && let Some(project) = &project
        {
            decision.agent_target = agent_target(&project.agent);
        }
        if decision.branch_name.is_none() {
            decision.branch_name = decision
                .task_prompt
                .as_deref()
                .map(|prompt| format!("oni/{}", git::slugify(prompt, 60)));
        }
        if project.is_none() {
            decision.loop_status = LoopStatus::NeedsHuman;
            decision.reasoning = append_reason(
                &decision.reasoning,
                "Falta elegir el proyecto; no se inventa uno.",
            );
        }
    } else {
        decision.task_prompt = None;
        decision.success_criterion = None;
    }

    if let Some(branch) = decision.branch_name.take() {
        let slug = branch
            .trim()
            .strip_prefix("oni/")
            .unwrap_or(branch.trim())
            .to_string();
        decision.branch_name = Some(format!("oni/{}", git::slugify(&slug, 72)));
    }
    if decision.worktree_strategy == WorktreeStrategy::ExistingBranch
        && decision.branch_name.is_none()
    {
        decision.worktree_strategy = WorktreeStrategy::IsolatedBranch;
    }

    if matches!(
        decision.intent,
        RouterIntent::MergeWorktree
            | RouterIntent::RetryTask
            | RouterIntent::CancelTask
            | RouterIntent::DiscardTask
    ) && decision.task_ref.is_none()
    {
        decision.loop_status = LoopStatus::NeedsHuman;
        decision.reasoning = append_reason(&decision.reasoning, "Falta identificar la tarea.");
    }
    if decision.intent == RouterIntent::Unknown {
        decision.loop_status = LoopStatus::NeedsHuman;
    }
    if decision.reasoning.is_empty() {
        decision.reasoning = "Decisión del router.".into();
    }
    decision
}

fn clean_ref(value: Option<String>) -> Option<String> {
    let value = value?;
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    if value.is_empty()
        || matches!(
            value.to_lowercase().as_str(),
            "null" | "none" | "nil" | "n/a" | "auto" | "undefined" | "-"
        )
    {
        None
    } else {
        Some(value)
    }
}

fn append_reason(current: &str, extra: &str) -> String {
    let current = current.trim();
    if current.contains(extra) {
        return current.chars().take(400).collect();
    }
    let combined = if current.is_empty() {
        extra.to_string()
    } else {
        format!("{current} {extra}")
    };
    combined.chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> SidecarContext {
        SidecarContext {
            projects: vec![
                SidecarProject {
                    id: "aaaa1111-0000-0000-0000-000000000000".into(),
                    name: "landing".into(),
                    base_branch: "main".into(),
                    verify_command: "npm run build".into(),
                    agent: "opencode".into(),
                },
                SidecarProject {
                    id: "bbbb2222-0000-0000-0000-000000000000".into(),
                    name: "api".into(),
                    base_branch: "main".into(),
                    verify_command: "pytest -q".into(),
                    agent: "claude".into(),
                },
            ],
            tasks: vec![SidecarTask {
                id: "3f2a1b9c-0000-0000-0000-000000000000".into(),
                title: "Add pricing section".into(),
                state: "review".into(),
                project: "landing".into(),
                branch: "oni/add-pricing-3f2a1b9c".into(),
            }],
            selected_project_id: Some("aaaa1111-0000-0000-0000-000000000000".into()),
            selected_task_id: None,
        }
    }

    #[test]
    fn fallback_routes_spawn_to_selected_project() {
        let decision = fallback_decision("agregá una sección de precios", &context());
        assert_eq!(decision.intent, RouterIntent::SpawnAgent);
        assert_eq!(decision.project_ref.as_deref(), Some("landing"));
        assert_eq!(decision.worktree_strategy, WorktreeStrategy::IsolatedBranch);
        assert_eq!(decision.success_criterion.as_deref(), Some("npm run build"));
    }

    #[test]
    fn fallback_routes_merge_by_id() {
        let decision = fallback_decision("mergeá 3f2a1b9c", &context());
        assert_eq!(decision.intent, RouterIntent::MergeWorktree);
        assert_eq!(decision.task_ref.as_deref(), Some("3f2a1b9c"));
        assert_eq!(decision.project_ref.as_deref(), Some("landing"));
    }

    #[test]
    fn fallback_asks_for_task_when_missing() {
        let decision = fallback_decision("mergeá la tarea", &context());
        assert_eq!(decision.intent, RouterIntent::MergeWorktree);
        assert_eq!(decision.loop_status, LoopStatus::NeedsHuman);
        assert!(decision.task_ref.is_none());
    }

    #[test]
    fn fallback_refuses_main() {
        let decision = fallback_decision("agregá tests directo en main", &context());
        assert_eq!(decision.intent, RouterIntent::SpawnAgent);
        assert_eq!(decision.worktree_strategy, WorktreeStrategy::IsolatedBranch);
        assert_eq!(decision.loop_status, LoopStatus::NeedsHuman);
    }

    #[test]
    fn sanitize_downgrades_main_direct() {
        let decision = RouterDecision {
            intent: RouterIntent::SpawnAgent,
            worktree_strategy: WorktreeStrategy::MainDirect,
            task_prompt: Some("hacelo en main".into()),
            ..Default::default()
        };
        let sanitized = sanitize_decision(decision, "hacelo en main", &context());
        assert_eq!(
            sanitized.worktree_strategy,
            WorktreeStrategy::IsolatedBranch
        );
        assert_eq!(sanitized.loop_status, LoopStatus::NeedsHuman);
        assert!(sanitized.branch_name.unwrap().starts_with("oni/"));
    }

    #[test]
    fn sanitize_clamps_iterations_and_clears_nulls() {
        let decision = RouterDecision {
            intent: RouterIntent::RetryTask,
            task_ref: Some("null".into()),
            max_iterations: 99,
            ..Default::default()
        };
        let sanitized = sanitize_decision(decision, "retry", &context());
        assert_eq!(sanitized.max_iterations, 6);
        assert!(sanitized.task_ref.is_none());
        assert_eq!(sanitized.loop_status, LoopStatus::NeedsHuman);
    }

    #[test]
    fn project_name_matching_uses_boundaries() {
        assert!(contains_name("en api por favor", "api"));
        assert!(!contains_name("rapid thing", "api"));
    }

    #[test]
    fn append_reason_does_not_duplicate() {
        let reason = append_reason("Falta elegir el proyecto.", "Falta elegir el proyecto.");
        assert_eq!(reason, "Falta elegir el proyecto.");
    }

    #[test]
    fn sanitize_uses_selected_project_for_spawn() {
        let decision = RouterDecision {
            intent: RouterIntent::SpawnAgent,
            task_prompt: Some("agregá un test".into()),
            ..Default::default()
        };
        let sanitized = sanitize_decision(decision, "agregá un test", &context());
        assert_eq!(sanitized.project_ref.as_deref(), Some("landing"));
        assert_eq!(sanitized.agent_target, AgentTarget::Opencode);
    }
}
