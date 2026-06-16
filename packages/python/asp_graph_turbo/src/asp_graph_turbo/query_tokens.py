"""Query token normalization for graph-turbo ranking."""

from __future__ import annotations

import re

TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9_]*")
CAMEL_TOKEN_RE = re.compile(r"[A-Z]?[a-z]+|[A-Z]+(?=[A-Z]|$)|[0-9]+")
MIN_TOKEN_LENGTH = 2
GENERIC_PATH_TOKENS = {
    "bin",
    "cli",
    "lib",
    "package",
    "packages",
    "query",
    "src",
    "test",
    "tests",
    "type",
    "types",
    "unit",
}


def query_tokens_from_text(value: str) -> tuple[str, ...]:
    tokens: list[str] = []
    seen: set[str] = set()
    for raw_token in TOKEN_RE.findall(value):
        for token in _expanded_tokens(raw_token):
            if len(token) < MIN_TOKEN_LENGTH or token in seen:
                continue
            seen.add(token)
            tokens.append(token)
    return tuple(tokens)


def _expanded_tokens(raw_token: str) -> tuple[str, ...]:
    tokens = [raw_token.lower()]
    for segment in raw_token.split("_"):
        tokens.extend(match.group(0).lower() for match in CAMEL_TOKEN_RE.finditer(segment))
    return tuple(tokens)
