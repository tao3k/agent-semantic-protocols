"""Cache command tests for the packaged ASP graph turbo CLI."""

from __future__ import annotations

import json
import os

from unit.asp_python_graphs_cli_support import (
    cache_key,
    changed_sample_graph_turbo_request,
    run_graph_turbo_cache,
    run_graph_turbo_rank,
    sample_graph_turbo_request,
)


def test_graph_turbo_rank_cache_persists_across_processes(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")
    env = {
        **os.environ,
        "PRJ_CACHE_HOME": str(tmp_path / "cache"),
    }

    first = run_graph_turbo_rank(packet_path, env)
    second = run_graph_turbo_rank(packet_path, env)

    assert "\ncache=miss:" in first.stdout
    assert "\ncache=hit:" in second.stdout


def test_graph_turbo_rank_changed_graph_uses_new_cache_key(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    env = {
        **os.environ,
        "PRJ_CACHE_HOME": str(tmp_path / "cache"),
    }
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")

    first = run_graph_turbo_rank(packet_path, env)
    second = run_graph_turbo_rank(packet_path, env)
    packet_path.write_text(
        json.dumps(changed_sample_graph_turbo_request()),
        encoding="utf-8",
    )
    changed = run_graph_turbo_rank(packet_path, env)

    assert "\ncache=miss:" in first.stdout
    assert "\ncache=hit:" in second.stdout
    assert "\ncache=miss:" in changed.stdout
    assert cache_key(first.stdout) != cache_key(changed.stdout)


def test_graph_turbo_cache_cli_status_prune_and_invalidate(tmp_path) -> None:
    packet_path = tmp_path / "graph-turbo-request.json"
    env = {
        **os.environ,
        "PRJ_CACHE_HOME": str(tmp_path / "cache"),
    }
    packet_path.write_text(json.dumps(sample_graph_turbo_request()), encoding="utf-8")
    run_graph_turbo_rank(packet_path, env)
    packet_path.write_text(
        json.dumps(changed_sample_graph_turbo_request()),
        encoding="utf-8",
    )
    run_graph_turbo_rank(packet_path, env)

    status = run_graph_turbo_cache(["status", "--format", "json"], env)
    assert json.loads(status.stdout)["entries"] == 2

    pruned = run_graph_turbo_cache(
        ["prune", "--max-entries", "1", "--format", "json"],
        env,
    )
    assert json.loads(pruned.stdout)["entries"] == 1
    invalidated = run_graph_turbo_cache(["invalidate", "--format", "json"], env)
    assert json.loads(invalidated.stdout)["entries"] == 0
