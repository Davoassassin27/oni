from __future__ import annotations

import json

import pytest
from pydantic import ValidationError

from schema.intent_schema import (
    Intent,
    IntentParseError,
    LoopStatus,
    parse_decision,
    sanitize_payload,
    validate_dataset_record,
)


def test_valid_spawn_roundtrip() -> None:
    label = {
        "intent": "spawn_agent",
        "agent_target": "opencode",
        "worktree_strategy": "isolated_branch",
        "branch_name": "oni/add-pricing",
        "project_ref": "landing",
        "task_prompt": "agregá una sección de precios",
        "loop_status": "single_shot",
        "max_iterations": 3,
        "success_criterion": "npm run build",
        "reasoning": "Pedido de implementación.",
    }
    decision = validate_dataset_record(label)
    assert decision.intent is Intent.SPAWN_AGENT
    assert json.loads(decision.to_json())["intent"] == "spawn_agent"


def test_main_direct_is_downgraded() -> None:
    payload = sanitize_payload(
        {
            "intent": "spawn_agent",
            "worktree_strategy": "main_direct",
            "task_prompt": "hacelo en main",
            "reasoning": "rápido",
        }
    )
    assert payload["worktree_strategy"] == "isolated_branch"
    assert payload["loop_status"] == "needs_human"
    assert "prohibido" in payload["reasoning"]
    assert payload["branch_name"].startswith("oni/")


def test_spawn_requires_prompt() -> None:
    with pytest.raises(ValidationError):
        validate_dataset_record({"intent": "spawn_agent", "max_iterations": 3})


def test_task_action_without_task_ref_needs_human() -> None:
    decision = validate_dataset_record(
        {"intent": "merge_worktree", "max_iterations": 1, "reasoning": "merge"}
    )
    assert decision.loop_status is LoopStatus.NEEDS_HUMAN


def test_iterations_and_nulls_are_normalized() -> None:
    payload = sanitize_payload(
        {
            "intent": "retry_task",
            "task_ref": "3f2a1b9c",
            "max_iterations": 99,
            "model_override": "auto",
            "success_criterion": "cargo test",
            "branch_name": "main",
        }
    )
    assert payload["max_iterations"] == 6
    assert payload["model_override"] is None
    assert payload["success_criterion"] is None
    assert payload["branch_name"] is None


def test_parse_fenced_json_with_noise() -> None:
    raw = """Aquí está:
```json
{"intent": "query_status", "task_ref": "3f2a1b9c", "reasoning": "estado"}
```
"""
    decision = parse_decision(raw)
    assert decision.intent is Intent.QUERY_STATUS
    assert decision.task_ref == "3f2a1b9c"


def test_parse_rejects_non_json() -> None:
    with pytest.raises(IntentParseError):
        parse_decision("no tengo idea")


def test_branch_is_prefixed_and_slugged() -> None:
    payload = sanitize_payload(
        {
            "intent": "spawn_agent",
            "task_prompt": "Agregá pricing!!! y dark mode",
            "branch_name": "main/foo",
        }
    )
    assert payload["branch_name"].startswith("oni/")
    assert " " not in payload["branch_name"]


def test_existing_branch_requires_branch_name() -> None:
    payload = sanitize_payload(
        {
            "intent": "spawn_agent",
            "task_prompt": "continuá en una rama existente",
            "worktree_strategy": "existing_branch",
        }
    )
    assert payload["worktree_strategy"] == "isolated_branch"
