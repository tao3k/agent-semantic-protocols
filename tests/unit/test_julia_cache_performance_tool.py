"""Julia cache performance evidence validator contracts."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from tools.julia_cache_performance import load_receipt, main


def test_julia_cache_performance_summary_validates_hit_replay(
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _write_receipt(
        tmp_path,
        "miss",
        {
            "route": "local-native",
            "providerProcessesSpawned": 1,
            "packetBytes": 128,
            "sqliteWriteCount": 4,
            "elapsedMs": 80,
        },
    )
    _write_receipt(
        tmp_path,
        "hit",
        {
            "route": "local-cache",
            "cacheStatus": "hit",
            "providerProcessesSpawned": 0,
            "providerCommandCount": 0,
            "elapsedMs": 5,
        },
    )
    (tmp_path / "miss.out").write_text("same packet\n", encoding="utf-8")
    (tmp_path / "hit.out").write_text("same packet\n", encoding="utf-8")

    assert main([tmp_path.as_posix()]) == 0

    output = capsys.readouterr().out
    assert output.startswith("[perf-calibrate-julia-cache] ")
    assert "missElapsedMs=80" in output
    assert "hitElapsedMs=5" in output
    assert "packetBytes=128" in output
    assert f"evidence={tmp_path}" in output


def test_julia_cache_performance_rejects_provider_backed_hit(
    tmp_path: Path,
) -> None:
    _write_receipt(
        tmp_path,
        "miss",
        {
            "route": "local-native",
            "providerProcessesSpawned": 1,
            "packetBytes": 128,
            "sqliteWriteCount": 4,
        },
    )
    _write_receipt(
        tmp_path,
        "hit",
        {
            "route": "local-cache",
            "cacheStatus": "hit",
            "providerProcessesSpawned": 1,
            "providerCommandCount": 1,
        },
    )
    (tmp_path / "miss.out").write_text("same packet\n", encoding="utf-8")
    (tmp_path / "hit.out").write_text("same packet\n", encoding="utf-8")

    with pytest.raises(AssertionError):
        main([tmp_path.as_posix()])


def test_load_receipt_requires_json_line(tmp_path: Path) -> None:
    (tmp_path / "miss.receipt.json").write_text("no json here\n", encoding="utf-8")

    with pytest.raises(SystemExit, match="missing miss receipt json"):
        load_receipt(tmp_path, "miss")


def _write_receipt(root: Path, name: str, payload: dict[str, object]) -> None:
    (root / f"{name}.receipt.json").write_text(
        "diagnostic line\n" + json.dumps(payload, sort_keys=True) + "\n",
        encoding="utf-8",
    )
