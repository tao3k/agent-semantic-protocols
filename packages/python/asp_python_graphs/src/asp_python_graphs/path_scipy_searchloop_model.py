# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Value objects for bounded SearchLoop path optimization."""

from __future__ import annotations

from dataclasses import dataclass

from .graph_model import Edge

MAX_EXACT_FLOAT_INTEGER = (1 << 53) - 1


@dataclass(frozen=True, slots=True)
class SearchLoopCost:
    """Additive SearchLoop cost with distinct cache domains."""

    hops: int = 0
    interaction_rounds: int = 0
    tokens: int = 0
    search_cache_misses: int = 0
    model_cache_misses: int = 0

    def objective(self) -> tuple[int, int, int, int, int]:
        return (
            self.hops,
            self.interaction_rounds,
            self.tokens,
            self.search_cache_misses,
            self.model_cache_misses,
        )


@dataclass(frozen=True, slots=True)
class SearchLoopRoute:
    """A deterministic route and its independently auditable cost."""

    node_ids: tuple[str, ...]
    relations: tuple[str, ...]
    cost: SearchLoopCost
    encoded_cost: int


@dataclass(frozen=True, slots=True)
class MixedRadix:
    rounds_base: int
    tokens_base: int
    search_cache_base: int
    model_cache_base: int

    def encode(self, cost: SearchLoopCost) -> int:
        value = cost.hops * self.rounds_base + cost.interaction_rounds
        value = value * self.tokens_base + cost.tokens
        value = value * self.search_cache_base + cost.search_cache_misses
        return value * self.model_cache_base + cost.model_cache_misses


@dataclass(frozen=True, slots=True)
class EncodedEdge:
    edge: Edge
    cost: SearchLoopCost
    encoded_cost: int
