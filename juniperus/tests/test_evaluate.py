from __future__ import annotations

from juniperus.dataset import generate_offline
from juniperus.evaluate import evaluate, write_report
from juniperus.runtime import MockBackend, OracleBackend


def test_oracle_scores_perfect() -> None:
    records = generate_offline(40, seed=13)
    report = evaluate(records, OracleBackend())
    assert report["metrics"]["intent_accuracy"] == 1.0
    assert report["metrics"]["schema_ok"] == 1.0
    assert report["metrics"]["exact_match"] == 1.0


def test_mock_baseline_is_valid(tmp_path) -> None:
    records = generate_offline(40, seed=17)
    report = evaluate(records, MockBackend())
    assert report["metrics"]["schema_ok"] == 1.0
    assert 0.2 <= report["metrics"]["intent_accuracy"] <= 1.0
    json_path, md_path = write_report(report, tmp_path)
    assert json_path.is_file()
    assert "Juniperus bench" in md_path.read_text(encoding="utf-8")
