"""Prompt building shared by training, evaluation and the runtime."""

from __future__ import annotations

from typing import Any

from schema.intent_schema import SYSTEM_PROMPT, RouterContext, RouterIntent

MAX_PROJECTS = 8
MAX_TASKS = 12


def short_id(value: str) -> str:
    return value.replace("-", "")[:8]


def _context_model(context: RouterContext | dict[str, Any] | None) -> RouterContext:
    if context is None:
        return RouterContext()
    if isinstance(context, RouterContext):
        return context
    return RouterContext.model_validate(context)


def format_context(context: RouterContext | dict[str, Any] | None) -> str:
    ctx = _context_model(context)
    lines: list[str] = ["CONTEXT"]

    if ctx.projects:
        lines.append("Proyectos:")
        for project in ctx.projects[:MAX_PROJECTS]:
            verify = f' verify="{project.verify_command}"' if project.verify_command else ""
            lines.append(
                f"- {project.name} [base={project.base_branch}]"
                f"{verify} agent={project.agent} id={short_id(project.id)}"
            )
    else:
        lines.append("Proyectos: (ninguno registrado)")

    actionable = [task for task in ctx.tasks if task.state not in {"merged", "discarded"}]
    recent = sorted(ctx.tasks, key=lambda task: task.id, reverse=True)[:MAX_TASKS]
    ordered = actionable[:MAX_TASKS] if actionable else recent
    if ordered:
        lines.append("Tareas recientes:")
        for task in ordered:
            title = task.title.replace('"', "'")
            lines.append(
                f'- {short_id(task.id)} [{task.state}] "{title}"'
                f" project={task.project} branch={task.branch}"
            )
    else:
        lines.append("Tareas recientes: (ninguna)")

    if ctx.selected_project_id or ctx.selected_task_id:
        lines.append(
            "Selección actual: "
            f"project={ctx.selected_project_id or '-'} task={ctx.selected_task_id or '-'}"
        )

    return "\n".join(lines)


def build_user_prompt(text: str, context: RouterContext | dict[str, Any] | None = None) -> str:
    return f"{format_context(context)}\n\nPEDIDO:\n{text.strip()}"


def build_messages(
    text: str, context: RouterContext | dict[str, Any] | None = None
) -> list[dict[str, str]]:
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": build_user_prompt(text, context)},
    ]


def target_json(decision: RouterIntent) -> str:
    """Compact target string, exactly what the model must learn to emit."""

    return decision.to_json()


def user_text(messages: list[dict[str, str]]) -> str:
    for message in reversed(messages):
        if message.get("role") == "user":
            return message.get("content", "")
    return ""
