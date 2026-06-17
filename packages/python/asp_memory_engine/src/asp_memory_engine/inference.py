"""Typed memory object inference from Org-style properties."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class InferredMemoryObjectKind(str, Enum):
    FINALITY = "finality"
    CLAIM = "claim"
    EVIDENCE = "evidence"
    FAILURE = "failure"
    PREFERENCE = "preference"

    @property
    def facet_label(self) -> str:
        return f"memory-{self.value}"


@dataclass(frozen=True)
class InferredMemoryObject:
    kind: InferredMemoryObjectKind
    question: str
    value: str


def infer_memory_object_from_reflection(
    question: str,
    value: str,
) -> InferredMemoryObject | None:
    question = question.strip()
    value = value.strip()
    if not question or not value:
        return None
    kind = infer_memory_object_kind_from_question(question)
    return InferredMemoryObject(kind, question, value) if kind else None


def infer_memory_objects_from_properties(
    properties: list[tuple[str, str]],
) -> list[InferredMemoryObject]:
    return [
        item
        for key, value in properties
        if (item := infer_memory_object_from_property(key, value)) is not None
    ]


def infer_memory_object_from_property(
    key: str,
    value: str,
) -> InferredMemoryObject | None:
    key = key.strip()
    value = value.strip()
    if not key or not value:
        return None
    kind = infer_memory_object_kind_from_property_key(key)
    if kind is None or not _property_value_matches(kind, value):
        return None
    return InferredMemoryObject(kind, key, value)


def infer_memory_object_kind_from_question(
    question: str,
) -> InferredMemoryObjectKind | None:
    normalized = question.lower()
    if "finality" in normalized or "outcome" in normalized:
        return InferredMemoryObjectKind.FINALITY
    if "evidence" in normalized or "proof" in normalized:
        return InferredMemoryObjectKind.EVIDENCE
    if "failure" in normalized or "avoid" in normalized:
        return InferredMemoryObjectKind.FAILURE
    if any(term in normalized for term in ("preference", "naming", "correction")):
        return InferredMemoryObjectKind.PREFERENCE
    if "claim" in normalized:
        return InferredMemoryObjectKind.CLAIM
    return None


def infer_memory_object_kind_from_property_key(
    key: str,
) -> InferredMemoryObjectKind | None:
    normalized = _normalize_property_key(key)
    if normalized in {"OUTCOME", "RESULT", "SIGNAL", "TASK_OUTCOME"}:
        return InferredMemoryObjectKind.FINALITY
    if normalized in {"CLAIM", "REUSABLE_KNOWLEDGE"}:
        return InferredMemoryObjectKind.CLAIM
    if normalized in {"EVIDENCE", "EVIDENCE_REF", "PROOF", "REFERENCE", "REFERENCES"}:
        return InferredMemoryObjectKind.EVIDENCE
    if normalized in {"SYMPTOM", "CAUSE", "FIX", "FAILURE_NOTE", "FAILURE_MODE"}:
        return InferredMemoryObjectKind.FAILURE
    if normalized in {"PREFERENCE", "PREFERENCE_SIGNAL", "REUSE_RULE", "NAMING_RULE", "CORRECTION"}:
        return InferredMemoryObjectKind.PREFERENCE
    return None


def _property_value_matches(kind: InferredMemoryObjectKind, value: str) -> bool:
    if kind is InferredMemoryObjectKind.EVIDENCE:
        return _is_evidence_reference(value)
    return any(character.isalpha() for character in value)


def _is_evidence_reference(value: str) -> bool:
    return (
        value.startswith(("http://", "https://", "id:", "orgid:", "commit:", "artifact:"))
        or "/" in value
        or "#" in value
        or any(value.endswith(ext) for ext in _EVIDENCE_EXTENSIONS)
        or (7 <= len(value) <= 64 and all(ch in "0123456789abcdefABCDEF" for ch in value))
    )


def _normalize_property_key(key: str) -> str:
    return "".join(ch.upper() if ch.isalnum() else "_" for ch in key.strip())


_EVIDENCE_EXTENSIONS = (
    ".arrow",
    ".csv",
    ".duckdb",
    ".html",
    ".json",
    ".jsonl",
    ".log",
    ".md",
    ".org",
    ".parquet",
    ".pdf",
    ".png",
    ".svg",
    ".tsv",
    ".txt",
)
