"""Public value types and canonical digests for polyglot search semantics."""

from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass
from typing import Any

from asp_proofs._polyglot_search_semantics_model import Scalar


@dataclass(frozen=True, slots=True)
class Relation:
    predicate: str
    columns: tuple[str, ...]
    rows: tuple[tuple[Scalar, ...], ...]


@dataclass(frozen=True, slots=True)
class SourceASTBinding:
    source_digest: str
    ast_digest: str
    binding_digest: str


@dataclass(frozen=True, slots=True)
class SemanticTrace:
    gql_binding: SourceASTBinding
    logic_binding: SourceASTBinding | None
    graph_digest_before: str
    graph_digest_after: str
    registry_digest: str
    execution_binding_digest: str
    gql_rows: tuple[tuple[tuple[str, Scalar], ...], ...]
    logic_rows: tuple[tuple[tuple[str, Scalar], ...], ...]
    multiplicity: str
    ordering: str
    trace_digest: str

    def payload_without_digest(self) -> dict[str, Any]:
        payload = asdict(self)
        payload.pop("trace_digest")
        return payload


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def digest(value: Any) -> str:
    encoded = canonical_json(value).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"
