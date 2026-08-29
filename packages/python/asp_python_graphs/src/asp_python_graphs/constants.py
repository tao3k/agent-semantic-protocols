"""Shared graph turbo protocol constants."""

ALGORITHM_ID = "typed-ppr-diverse"
COMPACT_OMISSIONS = ("code", "full-score-vector", "full-graph")
COMPACT_AVOID_ACTIONS = ("raw-read", "repeat-owner", "broad-lexical")
FAILURE_FRONTIER_OMISSIONS = ("full-source", "unrelated-functions", "wide-windows")
FAILURE_FRONTIER_AVOID_ACTIONS = (
    "manual-window-scan",
    "duplicate-read",
    "raw-read",
    "broad-lexical",
)
DEFAULT_PAGERANK_ALPHA = 0.85
DEFAULT_WINDOW_MERGE_MAX_GAP_LINES = 8


def compact_omissions_for_profile(profile_name: str) -> tuple[str, ...]:
    if profile_name == "failure-frontier":
        return FAILURE_FRONTIER_OMISSIONS
    return COMPACT_OMISSIONS


def compact_avoid_actions_for_profile(profile_name: str) -> tuple[str, ...]:
    if profile_name == "failure-frontier":
        return FAILURE_FRONTIER_AVOID_ACTIONS
    return COMPACT_AVOID_ACTIONS
