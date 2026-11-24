from __future__ import annotations

import json

from juniperus.dataset import (
    DatasetArgs,
    dedupe,
    generate_offline,
    load_records,
    split_records,
    write_dataset,
)
from schema.intent_schema import RouterIntent, validate_dataset_record


def test_offline_dataset_is_valid_and_balanced() -> None:
    records = generate_offline(120, seed=7)
    assert len(records) >= 108
    intents = set()
    for record in records:
        label = validate_dataset_record(record["label"])
        assert label.worktree_strategy.value != "main_direct"
        assert record["messages"][0]["role"] == "system"
        assert record["messages"][-1]["role"] == "assistant"
        assert json.loads(record["messages"][-1]["content"])["intent"] == label.intent.value
        intents.add(label.intent.value)
    assert len(intents) >= 6


def test_dedupe_is_stable() -> None:
    records = generate_offline(40, seed=3)
    assert len(dedupe(records + records)) == len(records)


def test_split_has_no_leakage() -> None:
    records = generate_offline(120, seed=9)
    train, val, test = split_records(records, seed=9)
    ids_train = {record["id"] for record in train}
    ids_val = {record["id"] for record in val}
    ids_test = {record["id"] for record in test}
    assert not ids_train & ids_val
    assert not ids_train & ids_test
    assert not ids_val & ids_test
    assert len(train) + len(val) + len(test) == len(records)


def test_write_and_load_dataset(tmp_path) -> None:
    records = generate_offline(80, seed=5)
    args = DatasetArgs(mode="offline", count=80, seed=5, out_dir=tmp_path)
    manifest = write_dataset(records, tmp_path, args)
    assert (tmp_path / "train.jsonl").is_file()
    assert (tmp_path / "val.jsonl").is_file()
    assert (tmp_path / "test.jsonl").is_file()
    assert (tmp_path / "intent.schema.json").is_file()
    assert manifest["counts"]["total"] == len(dedupe(records))
    loaded = load_records(tmp_path / "test.jsonl")
    assert loaded
    assert validate_dataset_record(loaded[0]["label"])


def test_teacher_batch_rebuilds_spawn_label(monkeypatch) -> None:
    from juniperus import dataset as dataset_module

    records = generate_offline(40, seed=11)
    prototype = next(record for record in records if record["label"]["intent"] == "spawn_agent")
    original = prototype["text"]
    monkeypatch.setattr(
        dataset_module,
        "_call_teacher",
        lambda *args, **kwargs: [f"che, {original}", original],
    )
    batch = dataset_module._teacher_batch(None, prototype, 2, "fake-teacher", 0.0, retries=1)
    assert batch
    for record in batch:
        label = RouterIntent.model_validate(record["label"])
        assert label.intent.value == "spawn_agent"
        assert label.task_prompt == record["text"]
        assert record["meta"]["source"] == "teacher"


def test_reference_preservation() -> None:
    from juniperus.dataset import _references_intact

    label = {"task_ref": "3f2a1b9c", "project_ref": "landing"}
    assert _references_intact(
        "mergeá 3f2a1b9c en landing", "dale, mergeá 3f2a1b9c en landing", label
    )
    assert not _references_intact("mergeá 3f2a1b9c en landing", "mergeá la tarea", label)
