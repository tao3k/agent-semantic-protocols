"""Apply budget-aware second-pass suppression for repeated code frontier items."""

from __future__ import annotations

from collections import Counter

from dataclasses import dataclass

from .model import GraphProfile, Node

from .profiles import frontier_action

from .selector import (
    GraphTurboSelectorRange,
    graph_turbo_node_range,
    graph_turbo_owner_path_for_node,
    graph_turbo_ranges_adjacent,
    graph_turbo_selector_for_node,
)


@dataclass
class GraphTurboReadLoopSecondPass:
    candidate_count: int = 0
    duplicate_selector_suppressed_count: int = 0
    adjacent_range_merged_count: int = 0
    same_owner_suppressed_count: int = 0

    @property
    def suppressed_count(self) -> int:
        return (
            self.duplicate_selector_suppressed_count
            + self.adjacent_range_merged_count
            + self.same_owner_suppressed_count
        )


@dataclass
class _SecondPassState:
    kept: list[Node]
    adjacent_deferred: list[Node]
    owner_deferred: list[Node]
    seen_selectors: set[str]
    kept_code_ranges: list[GraphTurboSelectorRange]
    owner_code_counts: Counter[str]
    duplicate_selector_suppressed_count: int = 0


def graph_turbo_apply_read_loop_second_pass(
    profile: GraphProfile,
    ranked: tuple[Node, ...],
    *,
    limit: int,
    max_gap_lines: int = 8,
) -> tuple[tuple[Node, ...], GraphTurboReadLoopSecondPass]:
    state = _select_second_pass_candidates(
        profile, ranked, limit=limit, max_gap_lines=max_gap_lines
    )
    adjacent_restored_count = _restore_deferred_candidates(
        state.kept, state.adjacent_deferred, limit=limit
    )
    owner_restored_count = _restore_deferred_candidates(
        state.kept, state.owner_deferred, limit=limit
    )
    return (
        tuple(state.kept[:limit]),
        GraphTurboReadLoopSecondPass(
            candidate_count=len(ranked),
            duplicate_selector_suppressed_count=(
                state.duplicate_selector_suppressed_count
            ),
            adjacent_range_merged_count=(
                len(state.adjacent_deferred) - adjacent_restored_count
            ),
            same_owner_suppressed_count=(
                len(state.owner_deferred) - owner_restored_count
            ),
        ),
    )


def _select_second_pass_candidates(
    profile: GraphProfile,
    ranked: tuple[Node, ...],
    *,
    limit: int,
    max_gap_lines: int,
) -> _SecondPassState:
    state = _SecondPassState(
        kept=[],
        adjacent_deferred=[],
        owner_deferred=[],
        seen_selectors=set(),
        kept_code_ranges=[],
        owner_code_counts=Counter(),
    )
    for node in ranked:
        if len(state.kept) >= limit:
            break
        _consider_second_pass_candidate(
            profile, state, node, max_gap_lines=max_gap_lines
        )
    return state


def _consider_second_pass_candidate(
    profile: GraphProfile,
    state: _SecondPassState,
    node: Node,
    *,
    max_gap_lines: int,
) -> None:
    if frontier_action(profile, node) != "code":
        state.kept.append(node)
        return
    selector = graph_turbo_selector_for_node(node)
    if selector is not None and selector in state.seen_selectors:
        state.duplicate_selector_suppressed_count += 1
        return
    candidate_range = graph_turbo_node_range(node)
    if candidate_range is not None and _adjacent_to_kept_code_range(
        candidate_range, state.kept_code_ranges, max_gap_lines=max_gap_lines
    ):
        state.adjacent_deferred.append(node)
        return
    owner_path = graph_turbo_owner_path_for_node(node)
    if owner_path is not None and state.owner_code_counts[owner_path] >= 2:
        state.owner_deferred.append(node)
        return
    _append_code_candidate(
        state.kept,
        node,
        seen_selectors=state.seen_selectors,
        owner_code_counts=state.owner_code_counts,
        selector=selector,
        owner_path=owner_path,
        candidate_range=candidate_range,
        kept_code_ranges=state.kept_code_ranges,
    )


def _restore_deferred_candidates(
    kept: list[Node], deferred: list[Node], *, limit: int
) -> int:
    restored_count = 0
    for node in deferred:
        if len(kept) >= limit:
            break
        restored_count += 1
        kept.append(node)
    return restored_count


def _append_code_candidate(
    kept: list[Node],
    node: Node,
    *,
    seen_selectors: set[str],
    owner_code_counts: Counter[str],
    selector: str | None,
    owner_path: str | None,
    candidate_range: GraphTurboSelectorRange | None,
    kept_code_ranges: list[GraphTurboSelectorRange],
) -> None:
    kept.append(node)
    if selector is not None:
        seen_selectors.add(selector)
    if owner_path is not None:
        owner_code_counts[owner_path] += 1
    if candidate_range is not None:
        kept_code_ranges.append(candidate_range)


def _adjacent_to_kept_code_range(
    candidate_range: GraphTurboSelectorRange,
    kept_code_ranges: list[GraphTurboSelectorRange],
    *,
    max_gap_lines: int,
) -> bool:
    return any(
        graph_turbo_ranges_adjacent(
            candidate_range, kept_range, max_gap_lines=max_gap_lines
        )
        for kept_range in kept_code_ranges
    )
