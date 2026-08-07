"""Own shared typed models for polyglot search conformance receipts."""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from typing import Any, Iterable, Mapping


RECEIPT_SCHEMA_ID = "asp.polyglot-search-conformance-receipt.v1"
RELATION_ID_PATTERN = re.compile(r"^[a-z][a-z0-9-]*(\.[a-z][a-z0-9_-]*)+$")


@dataclass(frozen=True, slots=True)
class Violation:
    code: str
    path: str
    message: str

    def to_dict(self) -> dict[str, str]:
        return {"code": self.code, "path": self.path, "message": self.message}


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def digest_bytes(value: bytes) -> str:
    return f"sha256:{hashlib.sha256(value).hexdigest()}"


def digest_json(value: Any) -> str:
    return digest_bytes(canonical_json(value).encode("utf-8"))


@dataclass(frozen=True, slots=True)
class ConformanceReceipt:
    subject_kind: str
    input_digest: str
    state_before_digest: str
    state_after_digest: str
    parsed_summary: Mapping[str, Any] | None
    violations: tuple[Violation, ...]

    @property
    def admitted(self) -> bool:
        return not self.violations

    def to_dict(self) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "schemaId": RECEIPT_SCHEMA_ID,
            "schemaVersion": "1",
            "subjectKind": self.subject_kind,
            "state": "admitted" if self.admitted else "rejected",
            "inputDigest": self.input_digest,
            "stateBeforeDigest": self.state_before_digest,
            "stateAfterDigest": self.state_after_digest,
            "parsedSummary": self.parsed_summary,
            "violations": [violation.to_dict() for violation in self.violations],
        }
        payload["receiptDigest"] = digest_json(payload)
        return payload


def receipt(
    *,
    subject_kind: str,
    raw_input: bytes,
    state_digest: str,
    parsed_summary: Mapping[str, Any] | None,
    violations: Iterable[Violation],
) -> ConformanceReceipt:
    ordered_violations = tuple(
        sorted(violations, key=lambda violation: (violation.path, violation.code))
    )
    return ConformanceReceipt(
        subject_kind=subject_kind,
        input_digest=digest_bytes(raw_input),
        state_before_digest=state_digest,
        state_after_digest=state_digest,
        parsed_summary=parsed_summary if not ordered_violations else None,
        violations=ordered_violations,
    )


def mapping(value: Any) -> Mapping[str, Any]:
    return value if isinstance(value, Mapping) else {}
