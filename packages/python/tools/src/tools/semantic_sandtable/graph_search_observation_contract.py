"""GitHub-safe schema and path contract for graph search observations."""

from __future__ import annotations

import os
import re
from typing import Any


SCHEMA_ID = "agent.semantic-protocols.graph-search-observation.v1"
SCHEMA_VERSION = 1

ABSOLUTE_PATH_PREFIX = re.compile(r"^(?:/|~(?:/|$)|[A-Za-z]:[\\/])")
ABSOLUTE_PATH_FRAGMENT = re.compile(
    r"(?<![A-Za-z0-9+:])(?:/(?:[^\s'\"`<>|:]+/)+[^\s'\"`<>|:]*|[A-Za-z]:[\\/])"
)


class AbsolutePathError(ValueError):
    pass


def is_absolute_path(value: str) -> bool:
    return bool(ABSOLUTE_PATH_PREFIX.search(value))


def assert_no_absolute_paths(value: Any, path: str = "$") -> None:
    if isinstance(value, str):
        if is_absolute_path(value) or ABSOLUTE_PATH_FRAGMENT.search(value):
            raise AbsolutePathError(f"absolute path is not allowed at {path}: {value!r}")
        return
    if isinstance(value, list):
        for index, item in enumerate(value):
            assert_no_absolute_paths(item, f"{path}[{index}]")
        return
    if isinstance(value, dict):
        for key, item in value.items():
            assert_no_absolute_paths(item, f"{path}.{key}")


def path_ref(kind: str, value: str | os.PathLike[str] | None) -> dict[str, str] | None:
    if value is None:
        return None
    text = str(value)
    if is_absolute_path(text):
        raise AbsolutePathError(f"absolute path is not allowed in pathRef: {text!r}")
    return {"kind": kind, "value": text}


def _safe_optional(value: Any) -> str | None:
    if value is None:
        return None
    return _safe_scalar(value)


def _safe_scalar(value: Any) -> str:
    text = str(value)
    text = ABSOLUTE_PATH_FRAGMENT.sub("<absolute-path>/", text)
    if is_absolute_path(text):
        return os.path.basename(text)
    return text


def _string_or_none(value: Any) -> str | None:
    if isinstance(value, str):
        return _safe_scalar(value)
    return None


def _bool_or_none(value: Any) -> bool | None:
    return value if isinstance(value, bool) else None


def _number_or_none(value: Any) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def _int_or_none(value: Any) -> int | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, int):
        return value
    return None


def _int_or_zero(value: Any) -> int:
    if isinstance(value, bool):
        return 0
    if isinstance(value, int):
        return max(value, 0)
    return 0


def _drop_none(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _drop_none(item) for key, item in value.items() if item is not None}
    if isinstance(value, list):
        return [_drop_none(item) for item in value if item is not None]
    return value
