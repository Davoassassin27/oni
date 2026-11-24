"""Catálogo de escenarios y entidades para el dataset sintético.

La generación offline no necesita ningún modelo: combina estos escenarios con
plantillas y un contexto verosímil de proyectos/tareas. El profesor (si hay
API key) sólo aporta paráfrasis; las etiquetas se calculan acá, nunca las
inventa el profesor.
"""

from __future__ import annotations

import hashlib
import random
import re
import unicodedata
from dataclasses import dataclass, field

from schema.intent_schema import (
    AgentTarget,
    Intent,
    LoopStatus,
    ProjectContext,
    RouterContext,
    RouterIntent,
    TaskContext,
    WorktreeStrategy,
)


@dataclass(frozen=True)
class ProjectSeed:
    name: str
    base_branch: str
    verify_command: str
    agent: str


@dataclass(frozen=True)
class TaskSeed:
    id: str
    title: str
    state: str
    project: str
    branch: str


@dataclass(frozen=True)
class SpawnSpec:
    es: str | None
    en: str | None
    project_hint: str | None = None
    unsafe_main: bool = False
    ambiguous: bool = False


@dataclass(frozen=True)
class Scenario:
    """Un escenario con una o más plantillas de comando."""

    id: str
    family: str
    intent: Intent
    templates_es: tuple[str, ...] = ()
    templates_en: tuple[str, ...] = ()
    spawn: SpawnSpec | None = None
    needs_task: bool = False
    static_prompt: str | None = None
    loop_status: LoopStatus | None = None
    extra: dict[str, str] = field(default_factory=dict)


PROJECTS: tuple[ProjectSeed, ...] = (
    ProjectSeed("landing", "main", "npm run build", "opencode"),
    ProjectSeed("oni", "main", "cargo test", "opencode"),
    ProjectSeed("api", "main", "pytest -q", "claude"),
    ProjectSeed("pwa", "main", "npm run build", "opencode"),
    ProjectSeed("juniperus", "main", "uv run pytest -q", "opencode"),
    ProjectSeed("infra", "main", "terraform validate", "custom"),
)

TASKS: tuple[TaskSeed, ...] = (
    TaskSeed("3f2a1b9c", "Add pricing section", "review", "landing", "oni/add-pricing-3f2a1b9c"),
    TaskSeed("8d4e7f21", "Fix login token expiry", "failed", "api", "oni/fix-login-token-8d4e7f21"),
    TaskSeed("a1b2c3d4", "Dark mode toggle", "review", "pwa", "oni/dark-mode-toggle-a1b2c3d4"),
    TaskSeed(
        "5e6f7a8b", "Contact form validation", "acting", "landing", "oni/contact-form-5e6f7a8b"
    ),
    TaskSeed("c9d0e1f2", "Rate limit /api/tasks", "verifying", "api", "oni/rate-limit-c9d0e1f2"),
    TaskSeed("7a8b9c0d", "Refactor git merge", "review", "oni", "oni/refactor-git-merge-7a8b9c0d"),
    TaskSeed("d1e2f3a4", "SQLite persistence", "queued", "oni", "oni/sqlite-persistence-d1e2f3a4"),
    TaskSeed("2b3c4d5e", "Task list pagination", "merged", "pwa", "oni/task-pagination-2b3c4d5e"),
    TaskSeed("e5f6a7b8", "Healthcheck endpoint", "review", "api", "oni/healthcheck-e5f6a7b8"),
    TaskSeed(
        "4c5d6e7f",
        "Dataset sintético del router",
        "acting",
        "juniperus",
        "oni/dataset-router-4c5d6e7f",
    ),
    TaskSeed("9a0b1c2d", "Export GGUF v1", "review", "juniperus", "oni/export-gguf-9a0b1c2d"),
    TaskSeed(
        "6d7e8f90", "Router contract v1", "review", "juniperus", "oni/router-contract-6d7e8f90"
    ),
    TaskSeed("f1a2b3c4", "Provision staging VPS", "failed", "infra", "oni/provision-vps-f1a2b3c4"),
)

SPAWN_SPECS: tuple[SpawnSpec, ...] = (
    SpawnSpec(
        "agregá una sección de precios con tres planes",
        "add a pricing section with three plans",
        "landing",
    ),
    SpawnSpec("implementá un toggle de modo oscuro", "add a dark mode toggle", "pwa"),
    SpawnSpec(
        "arreglá el bug de login cuando expira el token",
        "fix the login bug when the token expires",
        "api",
    ),
    SpawnSpec(
        "creá un formulario de contacto con validación",
        "build a contact form with validation",
        "landing",
    ),
    SpawnSpec("implementá rate limiting en /api/tasks", "add rate limiting to /api/tasks", "api"),
    SpawnSpec(
        "refactorizá el módulo de git para separar merge y discard",
        "refactor the git module to split merge and discard",
        "oni",
    ),
    SpawnSpec(
        "escribí tests para la máquina de estados", "write tests for the task state machine", "oni"
    ),
    SpawnSpec(
        "actualizá el README con el nuevo endpoint",
        "update the README with the new endpoint",
        "oni",
    ),
    SpawnSpec(
        "hacé que la PWA muestre un toast cuando falla la verificación",
        "make the PWA show a toast when verification fails",
        "pwa",
    ),
    SpawnSpec(
        "migrá la persistencia de JSON a SQLite", "migrate persistence from JSON to SQLite", "oni"
    ),
    SpawnSpec(
        "cambiá el color del botón de merge a vermilion",
        "change the merge button color to vermilion",
        "pwa",
    ),
    SpawnSpec("agregá paginación a la lista de tareas", "add pagination to the task list", "pwa"),
    SpawnSpec("arreglá el test flaky de runner", "fix the flaky runner test", "oni"),
    SpawnSpec("dockerizá el daemon", "dockerize the daemon", "oni"),
    SpawnSpec("eliminá los imports sin usar de api.rs", "remove unused imports from api.rs", "oni"),
    SpawnSpec("renombrá retry a requeue en el CLI", "rename retry to requeue in the CLI", "oni"),
    SpawnSpec("agregá un endpoint de healthcheck", "add a healthcheck endpoint", "api"),
    SpawnSpec("optimizá el arranque del daemon", "optimize daemon startup time", "oni"),
    SpawnSpec("documentá el contrato del router", "document the router contract", "juniperus"),
    SpawnSpec("corregí el typo del README", "fix the README typo", "oni"),
    SpawnSpec(
        "generá el dataset sintético del router",
        "generate the router synthetic dataset",
        "juniperus",
    ),
    SpawnSpec("exportá el modelo a GGUF", "export the model to GGUF", "juniperus"),
    SpawnSpec("evaluá el router con el bench", "run the router benchmark", "juniperus"),
    SpawnSpec("agregá notificaciones push", "add push notifications", "pwa"),
    SpawnSpec(
        "protegé el endpoint de merge con un chequeo de rama",
        "protect the merge endpoint with a branch check",
        "oni",
    ),
    SpawnSpec("creá un script de smoke test", "create a smoke test script", "oni"),
    SpawnSpec(
        "agregá un filtro por estado en la lista de tareas",
        "add a state filter to the task list",
        "pwa",
    ),
    SpawnSpec("implementá un command palette", "implement a command palette", "pwa"),
    SpawnSpec("escribí una guía de migración", "write a migration guide", "oni"),
    SpawnSpec("limpiá los logs viejos al arrancar", "clean old logs on startup", "oni"),
    SpawnSpec(
        "agregá un badge de estado en las tarjetas", "add a status badge to task cards", "pwa"
    ),
    SpawnSpec(
        "validá el token antes de crear el worktree",
        "validate the token before creating the worktree",
        "api",
    ),
    SpawnSpec("hacé un spike de WebSockets", "spike a WebSocket transport", "oni"),
    SpawnSpec("actualizá axum a la última versión", "bump axum to the latest version", "oni"),
    SpawnSpec(
        "separá el runner en módulos más chicos", "split the runner into smaller modules", "oni"
    ),
    SpawnSpec("agregá métricas de duración por tarea", "add per-task duration metrics", "api"),
    SpawnSpec("documentá cómo correr el smoke test", "document how to run the smoke test", "oni"),
    SpawnSpec("agregá un lockfile check al CI", "add a lockfile check to CI", "oni"),
    SpawnSpec(
        "implementá el botón de discard en mobile", "implement the discard button on mobile", "pwa"
    ),
    SpawnSpec("revisá el diff antes de mergear", "review the diff before merging", "oni"),
    # Seguridad: el usuario pide la rama base. La etiqueta correcta sigue siendo
    # un worktree aislado y el loop queda a la espera de un humano.
    SpawnSpec("hacelo directo en main", "just do it directly on main", "oni", unsafe_main=True),
    SpawnSpec(
        "commiteá en la rama principal sin worktree",
        "commit to the main branch without a worktree",
        "oni",
        unsafe_main=True,
    ),
    SpawnSpec("arreglá el bug de login", "fix the login bug", None, ambiguous=True),
    SpawnSpec("cambiá los colores", "change the colors", None, ambiguous=True),
    SpawnSpec("agregá tests", "add tests", None, ambiguous=True),
    SpawnSpec("mejorá el rendimiento", "improve performance", None, ambiguous=True),
)

QUERY_TEMPLATES_ES = (
    "cómo va {task}",
    "cómo viene {task}",
    "estado de {task}",
    "mostrame el estado de {task}",
    "qué pasó con {task}",
    "mostrame el log de {task}",
    "en qué quedó {task}",
    "che, cómo va {task}",
    "decime el estado de {task}",
    "está lista {task}?",
)

QUERY_TEMPLATES_EN = (
    "how is {task} going",
    "status of {task}",
    "show me the status of {task}",
    "what happened with {task}",
    "show the log for {task}",
    "is {task} ready",
)

GLOBAL_QUERY_ES = (
    "cómo va todo",
    "estado general",
    "qué tareas tengo",
    "mostrame las tareas",
    "qué está en review",
    "hay algo trabado?",
    "listá las tareas activas",
    "qué tareas fallaron",
)

GLOBAL_QUERY_EN = (
    "list tasks",
    "show me all tasks",
    "what is in review",
    "any task stuck",
    "what failed",
)

MERGE_TEMPLATES_ES = (
    "mergeá {task}",
    "dale merge a {task}",
    "integrá {task}",
    "fusioná {task} con main",
    "mergeá la tarea {task}",
    "mergeá {task} directo a main",
    "che, mergeá {task}",
    "si está verde, mergeá {task}",
)

MERGE_TEMPLATES_EN = (
    "merge {task}",
    "merge the {task} task",
    "ship {task}",
    "integrate {task} into main",
)

RETRY_TEMPLATES_ES = (
    "reintentá {task}",
    "volvé a correr {task}",
    "la verificación falló, reintentá {task}",
    "dale otra oportunidad a {task}",
    "retry {task}",
)

RETRY_TEMPLATES_EN = (
    "retry {task}",
    "run {task} again",
    "the verification failed, retry {task}",
)

CANCEL_TEMPLATES_ES = (
    "pará {task}",
    "cancelá {task}",
    "frená {task}",
    "detené {task}",
    "matá {task}",
    "stop {task}",
)

CANCEL_TEMPLATES_EN = (
    "stop {task}",
    "cancel {task}",
    "kill {task}",
)

DISCARD_TEMPLATES_ES = (
    "descartá {task}",
    "tirá {task}",
    "borrá {task}",
    "descartá la tarea {task}",
    "discard {task}",
)

DISCARD_TEMPLATES_EN = (
    "discard {task}",
    "throw away {task}",
    "delete {task}",
)

LIST_PROJECTS_ES = (
    "qué proyectos tengo",
    "mostrame los proyectos",
    "listá los repos conectados",
    "qué repos están registrados",
    "cuáles son mis proyectos",
    "en qué repos podés trabajar",
    "decime los proyectos disponibles",
    "qué proyectos hay en Oni",
    "mostrame el catálogo de repos",
)

LIST_PROJECTS_EN = (
    "list projects",
    "show me the connected repos",
    "which projects are registered",
    "what repos can you work on",
    "show the project catalog",
)

HELP_ES = (
    "qué podés hacer",
    "ayuda",
    "cómo funciona esto",
    "para qué servís",
    "mostrame los comandos",
    "qué acciones soportás",
    "en qué me podés ayudar",
    "cómo te uso",
    "qué puedo pedirte",
)

HELP_EN = (
    "what can you do",
    "help",
    "how does this work",
    "which actions do you support",
    "how do I use you",
)

UNKNOWN_ES = (
    "hola",
    "gracias",
    "chau",
    "contame un chiste",
    "asdf",
    "qué opinás de rust",
    "hacé algo",
    "arreglalo",
    "dale",
    "no sé",
)

UNKNOWN_EN = (
    "hello",
    "thanks",
    "tell me a joke",
    "asdf",
    "what do you think about rust",
    "do something",
    "fix it",
)

SPAWN_STYLES_ES = (
    "{text}",
    "che, {text}",
    "dale, {text}",
    "por favor, {text}",
    "urgente: {text}",
    "{text} dale",
    "{text}, gracias",
    "necesito {text}",
)

SPAWN_STYLES_EN = (
    "{text}",
    "please {text}",
    "can you {text}?",
    "hey, {text}",
    "urgent: {text}",
    "{text}, thanks",
)

MIXED_STYLES = (
    "{text}",
    "che, {text}",
    "dale {text}",
    "ok, {text}",
)


def _strip_accents(value: str) -> str:
    normalized = unicodedata.normalize("NFKD", value)
    return normalized.encode("ascii", "ignore").decode("ascii")


def _messy(text: str, rng: random.Random) -> str:
    if rng.random() < 0.22:
        text = text.lower()
    if rng.random() < 0.18:
        text = text.replace(" porque ", " xq ").replace(" que ", " q ")
    if rng.random() < 0.15:
        text = re.sub(r"\s+", " ", text).strip() + " " + rng.choice(("", "xfa", "pls", "🙏"))
    if rng.random() < 0.12:
        text = (
            text.replace("á", "a")
            .replace("é", "e")
            .replace("í", "i")
            .replace("ó", "o")
            .replace("ú", "u")
        )
    return text.strip()


def _project_ctx(project: ProjectSeed) -> ProjectContext:
    digest = hashlib.sha1(project.name.encode("utf-8")).hexdigest()[:8]
    return ProjectContext(
        id=digest,
        name=project.name,
        base_branch=project.base_branch,
        verify_command=project.verify_command,
        agent=project.agent,
    )


PROJECT_CONTEXTS: dict[str, ProjectContext] = {
    project.name: _project_ctx(project) for project in PROJECTS
}
TASK_CONTEXTS: dict[str, TaskContext] = {
    task.id: TaskContext(
        id=task.id,
        title=task.title,
        state=task.state,
        project=task.project,
        branch=task.branch,
    )
    for task in TASKS
}


def project_for(name: str) -> ProjectSeed:
    return next(project for project in PROJECTS if project.name == name)


def agent_target(project: ProjectSeed | None) -> AgentTarget:
    if project is None:
        return AgentTarget.AUTO
    try:
        return AgentTarget(project.agent)
    except ValueError:
        return AgentTarget.CUSTOM


def branch_for(prompt: str, max_len: int = 60) -> str:
    slug = _strip_accents(prompt).lower()
    slug = re.sub(r"[^a-z0-9]+", "-", slug).strip("-")
    slug = re.sub(r"-{2,}", "-", slug)
    return f"oni/{slug[:max_len].strip('-') or 'task'}"


def _task_display(task: TaskSeed, use_title: bool) -> str:
    return f'"{task.title}"' if use_title else task.id


def _choose_context(
    rng: random.Random,
    project: ProjectSeed | None,
    task: TaskSeed | None,
    *,
    selected_project: ProjectSeed | None = None,
    include_tasks: bool = True,
) -> RouterContext:
    tasks: list[TaskSeed] = []
    if include_tasks:
        pool = list(TASKS)
        rng.shuffle(pool)
        tasks = pool[: rng.randint(0, 5)]
        if task is not None and task not in tasks:
            tasks.insert(0, task)
        tasks = list(dict.fromkeys(tasks))

    # El contexto debe incluir el proyecto de cada tarea visible: el modelo no
    # debe inventar un proyecto que no está en la lista.
    ordered: list[ProjectSeed] = []
    for candidate in (selected_project, project):
        if candidate is not None and candidate not in ordered:
            ordered.append(candidate)
    for item in tasks:
        candidate = project_for(item.project)
        if candidate not in ordered:
            ordered.append(candidate)

    pool = list(PROJECTS)
    rng.shuffle(pool)
    target = max(1, rng.randint(1, 4))
    for candidate in pool:
        if len(ordered) >= target:
            break
        if candidate not in ordered:
            ordered.append(candidate)
    projects = ordered[:4]

    return RouterContext(
        projects=[PROJECT_CONTEXTS[item.name] for item in projects],
        tasks=[TASK_CONTEXTS[item.id] for item in tasks],
        selected_project_id=(
            PROJECT_CONTEXTS[selected_project.name].id if selected_project is not None else None
        ),
        selected_task_id=TASK_CONTEXTS[task.id].id if task is not None else None,
    )


def _spawn_example(rng: random.Random, spec: SpawnSpec) -> tuple[str, RouterContext, RouterIntent]:
    lang = "es" if spec.es and (not spec.en or rng.random() < 0.6) else "en"
    prompt = (spec.es if lang == "es" else spec.en) or spec.es or spec.en
    assert prompt

    project = project_for(spec.project_hint) if spec.project_hint else None
    selected = project
    if spec.ambiguous:
        # Hay proyectos, pero ninguno seleccionado y el pedido no nombra uno.
        project = None
        selected = None

    context = _choose_context(rng, project, None, selected_project=selected, include_tasks=True)

    if project is not None and rng.random() < 0.55:
        styles = SPAWN_STYLES_ES if lang == "es" else SPAWN_STYLES_EN
        template = rng.choice(styles)
        text = template.format(text=f"{prompt} en {project.name}")
        if lang == "en":
            text = template.format(text=f"{prompt} in {project.name}")
    elif project is not None:
        text = rng.choice(SPAWN_STYLES_ES if lang == "es" else SPAWN_STYLES_EN).format(text=prompt)
    else:
        text = rng.choice(SPAWN_STYLES_ES if lang == "es" else SPAWN_STYLES_EN).format(text=prompt)

    if rng.random() < 0.15:
        text = rng.choice(MIXED_STYLES).format(text=text)
    text = _messy(text, rng)

    loop = LoopStatus.SINGLE_SHOT
    if spec.unsafe_main:
        loop = LoopStatus.NEEDS_HUMAN
    if spec.ambiguous:
        loop = LoopStatus.NEEDS_HUMAN

    decision = RouterIntent(
        intent=Intent.SPAWN_AGENT,
        agent_target=agent_target(project),
        worktree_strategy=WorktreeStrategy.ISOLATED_BRANCH,
        branch_name=branch_for(prompt),
        project_ref=project.name if project else None,
        task_prompt=text,
        loop_status=loop,
        max_iterations=3,
        success_criterion=project.verify_command if project else None,
        reasoning=(
            "La rama base está prohibida; se usará un worktree aislado."
            if spec.unsafe_main
            else "Falta elegir el proyecto; no se inventa uno."
            if spec.ambiguous
            else "Pedido de implementación delegado a un agente."
        ),
    )
    return text, context, decision


def _task_example(
    rng: random.Random,
    templates_es: tuple[str, ...],
    templates_en: tuple[str, ...],
    intent: Intent,
    *,
    loop: LoopStatus,
    max_iterations: int = 1,
    retry_agent: bool = False,
) -> tuple[str, RouterContext, RouterIntent]:
    task = rng.choice(TASKS)
    project = project_for(task.project)
    use_title = rng.random() < 0.35
    templates = templates_es if rng.random() < 0.65 else templates_en
    text = rng.choice(templates).format(task=_task_display(task, use_title))
    text = _messy(text, rng)
    context = _choose_context(rng, project, task, selected_project=None)
    decision = RouterIntent(
        intent=intent,
        agent_target=agent_target(project) if retry_agent else AgentTarget.AUTO,
        worktree_strategy=WorktreeStrategy.ISOLATED_BRANCH,
        project_ref=project.name,
        task_ref=task.title if use_title else task.id,
        loop_status=loop,
        max_iterations=max_iterations,
        reasoning={
            Intent.MERGE_WORKTREE: "Se pidió integrar una tarea.",
            Intent.RETRY_TASK: "Se pidió reintentar una tarea.",
            Intent.CANCEL_TASK: "Se pidió detener una tarea.",
            Intent.DISCARD_TASK: "Se pidió descartar una tarea.",
        }[intent],
    )
    return text, context, decision


def _global_example(
    rng: random.Random,
    templates_es: tuple[str, ...],
    templates_en: tuple[str, ...],
    intent: Intent,
) -> tuple[str, RouterContext, RouterIntent]:
    text = rng.choice(templates_es if rng.random() < 0.65 else templates_en)
    text = _messy(text, rng)
    context = _choose_context(rng, None, None, include_tasks=True)
    decision = RouterIntent(
        intent=intent,
        loop_status=LoopStatus.COMPLETED
        if intent is not Intent.UNKNOWN
        else LoopStatus.NEEDS_HUMAN,
        max_iterations=1,
        reasoning={
            Intent.QUERY_STATUS: "Consulta de estado.",
            Intent.LIST_PROJECTS: "Consulta de proyectos.",
            Intent.HELP: "El usuario pide ayuda.",
            Intent.UNKNOWN: "No se pudo interpretar el pedido.",
        }[intent],
    )
    return text, context, decision


def sample_example(
    rng: random.Random,
) -> tuple[str, str, str, RouterContext, RouterIntent, str]:
    """Devuelve (scenario_id, family, lang, context, label, text)."""

    roll = rng.random()
    if roll < 0.34:
        spec = rng.choice(SPAWN_SPECS)
        text, context, decision = _spawn_example(rng, spec)
        lang = "es" if re.search(r"[áéíóúñ¿¡]", text) or "que" in text.lower() else "en"
        return "spawn", "spawn", lang, context, decision, text
    if roll < 0.64:
        task_intent = rng.choice(
            (
                (
                    Intent.MERGE_WORKTREE,
                    MERGE_TEMPLATES_ES,
                    MERGE_TEMPLATES_EN,
                    LoopStatus.COMPLETED,
                    1,
                    False,
                ),
                (
                    Intent.RETRY_TASK,
                    RETRY_TEMPLATES_ES,
                    RETRY_TEMPLATES_EN,
                    LoopStatus.CONTINUE_LOOP,
                    3,
                    True,
                ),
                (
                    Intent.CANCEL_TASK,
                    CANCEL_TEMPLATES_ES,
                    CANCEL_TEMPLATES_EN,
                    LoopStatus.COMPLETED,
                    1,
                    False,
                ),
                (
                    Intent.DISCARD_TASK,
                    DISCARD_TEMPLATES_ES,
                    DISCARD_TEMPLATES_EN,
                    LoopStatus.COMPLETED,
                    1,
                    False,
                ),
            )
        )
        intent, es, en, loop, iterations, retry_agent = task_intent
        text, context, decision = _task_example(
            rng,
            es,
            en,
            intent,
            loop=loop,
            max_iterations=iterations,
            retry_agent=retry_agent,
        )
        return intent.value, intent.value, "mixed", context, decision, text
    if roll < 0.82:
        if rng.random() < 0.7:
            task = rng.choice(TASKS)
            project = project_for(task.project)
            use_title = rng.random() < 0.35
            templates = QUERY_TEMPLATES_ES if rng.random() < 0.65 else QUERY_TEMPLATES_EN
            text = rng.choice(templates).format(task=_task_display(task, use_title))
            context = _choose_context(rng, project, task)
            decision = RouterIntent(
                intent=Intent.QUERY_STATUS,
                project_ref=project.name,
                task_ref=task.title if use_title else task.id,
                loop_status=LoopStatus.COMPLETED,
                max_iterations=1,
                reasoning="Consulta de estado.",
            )
        else:
            text, context, decision = _global_example(
                rng, GLOBAL_QUERY_ES, GLOBAL_QUERY_EN, Intent.QUERY_STATUS
            )
        text = _messy(text, rng)
        return "query_status", "query_status", "mixed", context, decision, text
    if roll < 0.88:
        text, context, decision = _global_example(
            rng, LIST_PROJECTS_ES, LIST_PROJECTS_EN, Intent.LIST_PROJECTS
        )
        return "list_projects", "list_projects", "mixed", context, decision, text
    if roll < 0.94:
        text, context, decision = _global_example(rng, HELP_ES, HELP_EN, Intent.HELP)
        return "help", "help", "mixed", context, decision, text
    text, context, decision = _global_example(rng, UNKNOWN_ES, UNKNOWN_EN, Intent.UNKNOWN)
    return "unknown", "unknown", "mixed", context, decision, text
