from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Mapping

if TYPE_CHECKING:
    from .graph_model import TypedGraph


SCHEMA_ID = "agent.semantic-protocols.graph-turbo-resident-server"
SCHEMA_VERSION = "1"


class ResidentProtocolError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass
class ResidentGraphTurboSession:
    runtime_artifact_digest: str | None = None
    execution_command_digest: str | None = None
    loaded_generations: dict[tuple[str, str], "TypedGraph"] = field(default_factory=dict)
    closed: bool = False

    def handle(self, message: Mapping[str, Any]) -> dict[str, object]:
        self._validate_envelope(message)
        kind = message["messageKind"]
        request_id = message["requestId"]
        if kind == "hello":
            return self._hello(message, request_id)
        if kind == "shutdown":
            self.closed = True
            return self._receipt(request_id, "cancelled")
        if kind == "load-generation":
            return self._load_generation(message, request_id)
        if kind != "rank":
            raise ResidentProtocolError("unsupported-message-kind", f"unsupported messageKind: {kind!r}")
        if self.closed or self.runtime_artifact_digest is None:
            raise ResidentProtocolError("process-not-open", "hello is required before rank")
        return self._rank(message, request_id)

    def _hello(self, message: Mapping[str, Any], request_id: int) -> dict[str, object]:
        self.runtime_artifact_digest = _digest(message, "runtimeArtifactDigest")
        self.execution_command_digest = _digest(message, "executionCommandDigest")
        self.loaded_generations.clear()
        self.closed = False
        receipt = self._receipt(request_id, "ready")
        receipt["processIdentity"] = {
            "processId": os.getpid(),
            "runtimeArtifactDigest": self.runtime_artifact_digest,
            "executionCommandDigest": self.execution_command_digest,
        }
        return receipt

    def _load_generation(self, message: Mapping[str, Any], request_id: int) -> dict[str, object]:
        # Keep numerical/graph imports out of the process hello path.  The
        # Runtime Server owns a bounded startup handshake; graph dependencies
        # are needed only when it publishes the first generation.
        from .graph_model import TypedGraph

        if self.closed or self.runtime_artifact_digest is None:
            raise ResidentProtocolError("process-not-open", "hello is required before load-generation")
        workspace, generation, page_roots = self._generation_identity(message)
        payload = message.get("generationPayload")
        if not isinstance(payload, Mapping):
            raise ResidentProtocolError("invalid-generation-payload", "generationPayload must be an object")
        graph_packet = payload.get("graph")
        if not isinstance(graph_packet, Mapping):
            raise ResidentProtocolError("missing-generation-graph", "generationPayload.graph must be an object")
        load_key = (workspace, generation)
        newly_loaded = load_key not in self.loaded_generations
        self.loaded_generations[load_key] = TypedGraph.from_packet(graph_packet)
        receipt = self._receipt(request_id, "ready", workspace, generation)
        receipt.update(
            {
                "processSpawns": 0,
                "generationLoads": int(newly_loaded),
                "graphPageReads": len(page_roots),
                "rankedNodes": [],
            }
        )
        return receipt

    def _rank(self, message: Mapping[str, Any], request_id: int) -> dict[str, object]:
        from .ranking import rank_frontier
        from .result_packet import result_to_packet

        workspace, generation, page_roots = self._generation_identity(message)
        terms = message.get("terms")
        payload = message.get("rankPayload")
        if not isinstance(terms, list) or any(not isinstance(term, str) or not term for term in terms):
            raise ResidentProtocolError("invalid-terms", "rank terms must be a string array")
        if not isinstance(payload, Mapping):
            raise ResidentProtocolError("invalid-rank-payload", "rankPayload must be an object")
        load_key = (workspace, generation)
        graph = self.loaded_generations.get(load_key)
        if graph is None:
            raise ResidentProtocolError("generation-not-loaded", "rank requires a Runtime-owned load-generation receipt")
        result = rank_frontier(
            graph,
            profile=str(message.get("profile", "owner-query")),
            seeds=_string_sequence(payload.get("seedIds")),
            limit=_positive_int(message.get("budget"), 8),
            kind_budgets=_string_int_mapping(payload.get("kindBudgets")),
            window_merge_enabled=bool(_mapping(payload.get("windowMerge")).get("enabled", True)),
            window_merge_max_gap_lines=_nonnegative_int(_mapping(payload.get("windowMerge")).get("maxGapLines"), 8),
            path_budget=_positive_int(payload.get("pathBudget"), 4),
            path_max_hops=_positive_int(payload.get("pathMaxHops"), 4),
            cache_enabled=bool(_mapping(payload.get("cache")).get("enabled", True)),
            query_clauses=_string_sequence(payload.get("queryClauses")),
        )
        result_packet = result_to_packet(result)
        receipt = self._receipt(request_id, "completed", workspace, generation)
        receipt.update(
            {
                "rankedNodes": [
                    {"nodeId": node.id, "score": result.scores[node.id], "evidenceDigest": generation}
                    for node in result.ranked_nodes
                ],
                "processSpawns": 0,
                "generationLoads": 0,
                "graphPageReads": len(page_roots),
                "result": result_packet,
            }
        )
        return receipt

    @staticmethod
    def _generation_identity(message: Mapping[str, Any]) -> tuple[str, str, Mapping[str, Any]]:
        workspace = _required_string(message, "workspaceIdentity")
        generation = _digest(message, "generationDigest")
        page_roots = message.get("pageRoots")
        if not isinstance(page_roots, Mapping) or not page_roots or not all(
            isinstance(name, str) and name and isinstance(value, str) and value
            for name, value in page_roots.items()
        ):
            raise ResidentProtocolError("invalid-page-roots", "pageRoots must be a non-empty string map")
        return workspace, generation, page_roots

    @staticmethod
    def _validate_envelope(message: Mapping[str, Any]) -> None:
        if message.get("schemaId") != SCHEMA_ID or message.get("schemaVersion") != SCHEMA_VERSION:
            raise ResidentProtocolError("unsupported-schema", "Graph Turbo resident requires IPC schema v1")
        if not isinstance(message.get("requestId"), int) or isinstance(message.get("requestId"), bool) or message["requestId"] < 1:
            raise ResidentProtocolError("invalid-request-id", "requestId must be a positive integer")

    @staticmethod
    def _receipt(request_id: int, state: str, workspace: str | None = None, generation: str | None = None) -> dict[str, object]:
        receipt: dict[str, object] = {
            "schemaId": SCHEMA_ID,
            "schemaVersion": SCHEMA_VERSION,
            "messageKind": "receipt",
            "requestId": request_id,
            "state": state,
        }
        if workspace is not None and generation is not None:
            receipt["workspaceIdentity"] = workspace
            receipt["generationDigest"] = generation
        return receipt


def _required_string(message: Mapping[str, Any], key: str) -> str:
    value = message.get(key)
    if not isinstance(value, str) or not value:
        raise ResidentProtocolError("invalid-identity", f"{key} must be a non-empty string")
    return value


def _digest(message: Mapping[str, Any], key: str) -> str:
    value = _required_string(message, key)
    if not value.startswith("blake3-256:") or len(value) != 75:
        raise ResidentProtocolError("invalid-digest", f"{key} must be a blake3-256 digest")
    return value


def _mapping(value: object) -> Mapping[str, object]:
    return value if isinstance(value, Mapping) else {}


def _string_sequence(value: object) -> tuple[str, ...]:
    return tuple(item for item in value if isinstance(item, str) and item) if isinstance(value, list) else ()


def _string_int_mapping(value: object) -> dict[str, int] | None:
    if not isinstance(value, Mapping):
        return None
    return {key: item for key, item in value.items() if isinstance(key, str) and isinstance(item, int) and not isinstance(item, bool)}


def _positive_int(value: object, default: int) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) and value > 0 else default


def _nonnegative_int(value: object, default: int) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) and value >= 0 else default
