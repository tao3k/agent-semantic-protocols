# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Performance guards for graph turbo warm ranking paths."""

from __future__ import annotations

import time

from asp_python_graphs.model import TypedGraph
from asp_python_graphs.ranking import rank_frontier

_WARM_RANK_BUDGET_MS = 50.0


def test_graph_turbo_rank_warm_cache_is_millisecond_scale(tmp_path, monkeypatch) -> None:
    monkeypatch.setenv("PRJ_CACHE_HOME", str(tmp_path / "cache"))
    graph = TypedGraph.from_packet(_perf_graph_turbo_request(tmp_path.name))

    first = rank_frontier(
        graph,
        profile="owner-query",
        seeds=(f"query:cache:{tmp_path.name}",),
        limit=4,
        kind_budgets={"item": 2, "owner": 1, "hot": 1, "test": 1},
        path_budget=3,
    )
    started = time.perf_counter()
    second = rank_frontier(
        graph,
        profile="owner-query",
        seeds=(f"query:cache:{tmp_path.name}",),
        limit=4,
        kind_budgets={"item": 2, "owner": 1, "hot": 1, "test": 1},
        path_budget=3,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0

    assert first.graph_cache.status == "miss"
    assert second.graph_cache.status == "hit"
    assert elapsed_ms < _WARM_RANK_BUDGET_MS


def _perf_graph_turbo_request(suffix: str) -> dict[str, object]:
    query_id = f"query:cache:{suffix}"
    owner_id = f"owner:src/lib.rs:{suffix}"
    item_id = f"item:cache_root:{suffix}"
    hot_id = f"hot:src/lib.rs:cache_root:1:{suffix}"
    test_id = f"test:cache_root:{suffix}"
    return {
        "graph": {
            "nodes": [
                {"id": query_id, "kind": "query", "role": "term", "value": "cache"},
                {
                    "id": owner_id,
                    "kind": "owner",
                    "role": "path",
                    "value": "src/lib.rs",
                },
                {
                    "id": item_id,
                    "kind": "item",
                    "role": "symbol",
                    "value": "cache_root",
                },
                {
                    "id": hot_id,
                    "kind": "hot",
                    "role": "range",
                    "value": "cache_root",
                    "path": "src/lib.rs",
                    "startLine": 1,
                    "endLine": 1,
                },
                {
                    "id": test_id,
                    "kind": "test",
                    "role": "path",
                    "value": "tests/cache.rs",
                },
            ],
            "edges": [
                {"source": query_id, "target": item_id, "relation": "matches"},
                {"source": query_id, "target": owner_id, "relation": "matches"},
                {"source": owner_id, "target": item_id, "relation": "contains"},
                {"source": item_id, "target": hot_id, "relation": "contains"},
                {"source": owner_id, "target": test_id, "relation": "covers"},
            ],
        }
    }
