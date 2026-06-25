"""Text scoring helpers for Rust-owned Org plan ranking."""

from __future__ import annotations

import re
from time import time

_TOKEN_PATTERN = re.compile(r"[A-Za-z0-9]+")


def candidate_text(plan: dict[str, object], properties: dict[object, object]) -> str:
    return " ".join(
        [
            _display_title(str(plan.get("title") or "")),
            str(properties.get("OBJECTIVE") or ""),
            str(properties.get("NEXT_ACTION") or ""),
            str(properties.get("RECOVERY_REF") or ""),
        ]
    )


def token_overlap_tokens(left_tokens: set[str], right: str) -> float:
    if not left_tokens:
        return 0.0
    return len(left_tokens & tokens(right)) / len(left_tokens)


def tokens(value: object) -> set[str]:
    return {
        token for token in _TOKEN_PATTERN.findall(str(value).lower()) if len(token) > 1
    }


def recency_score(mtime: float) -> float:
    age_days = max(0.0, time() - mtime) / 86_400.0
    return 1.0 / (1.0 + age_days)


def _display_title(title: str) -> str:
    return " ".join(token for token in title.split() if not _is_progress_cookie(token))


def _is_progress_cookie(token: str) -> bool:
    if not token.startswith("[") or not token.endswith("]"):
        return False
    inner = token[1:-1]
    if inner.endswith("%"):
        return inner[:-1].isdigit()
    left, separator, right = inner.partition("/")
    return bool(separator) and left.isdigit() and right.isdigit()
