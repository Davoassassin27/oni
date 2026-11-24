from __future__ import annotations

from juniperus.export import export, find_llama_cpp


def test_export_dry_run_does_not_need_llama_cpp(tmp_path) -> None:
    adapter = tmp_path / "adapter"
    adapter.mkdir()
    result = export(
        adapter=adapter,
        merged_dir=tmp_path / "merged",
        out=tmp_path / "juniperus.gguf",
        llama_cpp_dir=tmp_path / "missing-llama",
        dry_run=True,
    )
    assert result["dry_run"] is True
    assert result["gguf"].endswith("juniperus.gguf")


def test_find_llama_cpp_returns_none_for_missing_explicit_path(tmp_path) -> None:
    assert find_llama_cpp(tmp_path / "nope", clone=False) is None
