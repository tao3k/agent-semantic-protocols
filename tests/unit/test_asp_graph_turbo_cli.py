"""Internal dispatcher tests for ASP Python Graphs algorithms."""

from __future__ import annotations

import tomllib
from pathlib import Path

from asp_python_graphs.graph_turbo_cli import main


def test_asp_python_graphs_package_exposes_no_console_script() -> None:
    pyproject = (
        Path(__file__).resolve().parents[2]
        / "packages/python/asp_python_graphs/pyproject.toml"
    )
    project = tomllib.loads(pyproject.read_text(encoding="utf-8"))["project"]

    assert "scripts" not in project


def test_graph_turbo_dispatcher_help_lists_subcommands(capsys) -> None:
    assert main(["help"]) == 0

    captured = capsys.readouterr()

    assert "usage: asp-python-graphs <command> [args]" in captured.out
    assert "rank" in captured.out
    assert "artifacts" in captured.out
    assert "timeline" in captured.out
    assert "metrics" in captured.out


def test_graph_turbo_dispatcher_routes_metrics_command(capsys) -> None:
    assert (
        main(
            [
                "metrics",
                "--scenario",
                "rust-lexical-default-rank",
                "--measured-at",
                "2026-06-07T00:50:24Z",
                "--profile",
                "owner-query",
                "--command",
                "asp rust search lexical graph_turbo owner tests .",
                "--command-count",
                "1",
                "--packet-bytes",
                "0",
                "--result-bytes",
                "1181",
                "--latency-ms",
                "811",
                "--repeated-trigger-patterns",
                "0",
                "--missing-facts",
                "0",
                "--confusing-next-actions",
                "0",
            ]
        )
        == 0
    )

    captured = capsys.readouterr()

    assert captured.out.startswith("[graph-turbo-real-trigger]")
    assert "commandCount=1" in captured.out
    assert "packetBytes=0" in captured.out
