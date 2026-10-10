# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Authority-boundary tests for the ASP Python Graphs package."""

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
    assert not (pyproject.parent / "src/asp_python_graphs/cli.py").exists()


def test_gerbil_provider_publishes_facts_without_owning_search() -> None:
    guide = (
        Path(__file__).resolve().parents[2]
        / "languages/asp-gerbil-scheme/src/commands/guide-sections.ss"
    ).read_text(encoding="utf-8")

    assert "search-playbook-owner=ASP-Rust-Runtime" in guide
    assert "provider never plans, ranks, caches, or renders Search" in guide
    assert "source-index-owner=asp-server" in guide
    assert "native-fact-owner=gerbil-scheme" in guide
    assert "contains no Search playbook" in guide

    source_index_schema = (
        Path(__file__).resolve().parents[2]
        / "schemas/semantic-source-index.v1.schema.json"
    ).read_text(encoding="utf-8")
    assert '"description": "ASP Server-owned source index' in source_index_schema
    assert '"const": "asp-server"' in source_index_schema

    runtime_source_schema = (
        Path(__file__).resolve().parents[2]
        / "schemas/semantic-runtime-source-acquisition.v1.schema.json"
    ).read_text(encoding="utf-8")
    assert runtime_source_schema.count('"const": "asp-server"') == 3

    gerbil_path_contract = (
        Path(__file__).resolve().parents[2]
        / "languages/asp-gerbil-scheme/src/build-api/build-path-contract.ss"
    ).read_text(encoding="utf-8")
    gxtest_build = (
        Path(__file__).resolve().parents[2]
        / "languages/asp-gerbil-scheme/src/testing/gxtest-build.ss"
    ).read_text(encoding="utf-8")
    path_contract = gerbil_path_contract + gxtest_build
    assert ".bin/asp-gerbil-scheme" in path_contract
    assert ".local/bin/asp-gerbil-scheme" in path_contract
    assert 'path-expand "asp-gerbil-scheme" (asp-install-launcher-directory)' in path_contract
    assert "runtime/bin" in path_contract
    assert "gslph" not in path_contract


def test_runtime_session_does_not_depend_on_offline_cli_adapters() -> None:
    session = (
        Path(__file__).resolve().parents[2]
        / "packages/python/asp_python_graphs/src/asp_python_graphs/service_session.py"
    ).read_text(encoding="utf-8")

    assert "graph_turbo_cli" not in session
    assert "timeline_cli" not in session
    assert "from .algorithm import rank_graph" in session


def test_graph_turbo_dispatcher_help_lists_subcommands(capsys) -> None:
    assert main(["help"]) == 0

    captured = capsys.readouterr()

    assert "usage: asp-python-graphs <command> [args]" in captured.out
    assert "\n  rank " not in captured.out
    assert "artifacts" in captured.out
    assert "timeline" in captured.out
    assert "metrics" in captured.out


def test_graph_turbo_dispatcher_does_not_expose_runtime_ranking_authority(capsys) -> None:
    assert main(["rank"]) == 2

    captured = capsys.readouterr()

    assert "unknown command: rank" in captured.err


def test_graph_turbo_dispatcher_does_not_expose_service_authority(capsys) -> None:
    assert main(["serve"]) == 2

    captured = capsys.readouterr()

    assert "unknown command: serve" in captured.err


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
                "asp search playbook --language rust --rg -n -e graph_turbo . --tantivy term graph_turbo",
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
