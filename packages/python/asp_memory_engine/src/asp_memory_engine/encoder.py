"""Small deterministic intent encoder used by the Python memory engine."""

from __future__ import annotations

import hashlib
import math


class IntentEncoder:
    def __init__(self, embedding_dim: int = 384) -> None:
        self.embedding_dim = max(1, int(embedding_dim))

    def encode(self, text: str) -> tuple[float, ...]:
        vector = [0.0] * self.embedding_dim
        for token in _tokens(text):
            digest = hashlib.blake2b(token.encode("utf-8"), digest_size=8).digest()
            index = int.from_bytes(digest[:4], "big") % self.embedding_dim
            sign = 1.0 if digest[4] % 2 == 0 else -1.0
            vector[index] += sign
        return _normalize(vector)

    @staticmethod
    def cosine_similarity(left: tuple[float, ...], right: tuple[float, ...]) -> float:
        if not left or not right:
            return 0.0
        size = min(len(left), len(right))
        dot = sum(left[index] * right[index] for index in range(size))
        left_norm = math.sqrt(sum(value * value for value in left[:size]))
        right_norm = math.sqrt(sum(value * value for value in right[:size]))
        if left_norm == 0.0 or right_norm == 0.0:
            return 0.0
        return dot / (left_norm * right_norm)


def _tokens(text: str) -> tuple[str, ...]:
    return tuple(token for token in text.lower().replace("_", " ").split() if token)


def _normalize(values: list[float]) -> tuple[float, ...]:
    norm = math.sqrt(sum(value * value for value in values))
    if norm == 0.0:
        return tuple(values)
    return tuple(value / norm for value in values)
