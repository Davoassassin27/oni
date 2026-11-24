from __future__ import annotations

from juniperus.runtime import (
    MockBackend,
    OracleBackend,
    build_backend,
    route_text,
)
from schema.intent_schema import Intent, RouterContext, RouterIntent


def context() -> RouterContext:
    return RouterContext.model_validate(
        {
            "projects": [
                {
                    "id": "aaaa1111",
                    "name": "landing",
                    "base_branch": "main",
                    "verify_command": "npm run build",
                    "agent": "opencode",
                },
                {
                    "id": "bbbb2222",
                    "name": "api",
                    "base_branch": "main",
                    "verify_command": "pytest -q",
                    "agent": "claude",
                },
            ],
            "tasks": [
                {
                    "id": "3f2a1b9c",
                    "title": "Add pricing section",
                    "state": "review",
                    "project": "landing",
                    "branch": "oni/add-pricing-3f2a1b9c",
                }
            ],
            "selected_project_id": "aaaa1111",
        }
    )


def test_mock_routes_merge() -> None:
    result = route_text(MockBackend(), "che, mergeá 3f2a1b9c", context())
    assert result.decision.intent is Intent.MERGE_WORKTREE
    assert result.decision.task_ref == "3f2a1b9c"
    assert result.meta.backend == "mock"


def test_mock_routes_spawn() -> None:
    result = route_text(MockBackend(), "agregá una sección de precios", context())
    assert result.decision.intent is Intent.SPAWN_AGENT
    assert result.decision.project_ref == "landing"
    assert result.decision.branch_name
    assert result.decision.worktree_strategy.value == "isolated_branch"


def test_mock_refuses_main() -> None:
    result = route_text(MockBackend(), "agregá tests directo en main", context())
    assert result.decision.intent is Intent.SPAWN_AGENT
    assert result.decision.worktree_strategy.value == "isolated_branch"
    assert result.decision.loop_status.value == "needs_human"


def test_mock_routes_status_without_task() -> None:
    result = route_text(MockBackend(), "qué tareas tengo", context())
    assert result.decision.intent is Intent.QUERY_STATUS
    assert result.decision.task_ref is None


def test_mock_unknown() -> None:
    result = route_text(MockBackend(), "contame un chiste", context())
    assert result.decision.intent is Intent.UNKNOWN
    assert result.decision.loop_status.value == "needs_human"


def test_oracle_returns_gold() -> None:
    gold = RouterIntent(intent=Intent.HELP, reasoning="ayuda")
    result = OracleBackend().route("ayuda", context(), gold)
    assert result.decision == gold


def test_auto_backend_falls_back_to_mock(tmp_path) -> None:
    backend = build_backend(
        "auto",
        model_path=tmp_path / "missing.gguf",
        adapter=tmp_path / "missing-adapter",
    )
    assert isinstance(backend, MockBackend)
