"""Public facade for graph search observation sandtable records."""

from __future__ import annotations

from tools.semantic_sandtable.graph_search_observation_builder import (
    observations_from_report,
    write_jsonl,
)
from tools.semantic_sandtable.graph_search_observation_cli import main
from tools.semantic_sandtable.graph_search_observation_contract import (
    AbsolutePathError,
    assert_no_absolute_paths,
    is_absolute_path,
    path_ref,
)

__all__ = [
    "AbsolutePathError",
    "assert_no_absolute_paths",
    "is_absolute_path",
    "observations_from_report",
    "path_ref",
    "write_jsonl",
]


if __name__ == "__main__":
    raise SystemExit(main())
