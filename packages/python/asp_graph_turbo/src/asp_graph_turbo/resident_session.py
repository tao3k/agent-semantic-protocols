from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Any, Mapping

from .graph_model import TypedGraph
from .ranking import rank_frontier
from .result_packet import result_to_packet


class ResidentProtocolError(ValueError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


@dataclass
class ResidentGraphTurboSession:
    runtime_artifact_digest: str | None = None
    execution_command_digest: str | None = None
    invocation_counts: dict[tuple[str, str, str, str, str], int] = field(
        default_factory=dict
    )
    closed: bool = False

    def handle(self, message: Mapping[str, Any]) -> dict[str, object]:
        if message.get("schemaVersion") != "1":
            raise ResidentProtocolError(
                "unsupported-schema-version",
                "resident Graph Turbo supports schemaVersion 1 only",
            )
        kind = message.get("messageKind")
        request_id = _required_string(message, "requestId")
        if kind == "process-handshake":
            return self._handshake(message, request_id)
        self._require_live_process()
        if kind == "shutdown":
            self.closed = True
            return self._receipt(request_id, "shutdown-accepted")
        if kind != "rank":
            raise ResidentProtocolError(
                "unsupported-message-kind", f"unsupported messageKind: {kind!r}"
            )
        return self._rank(message, request_id)

    def _handshake(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        if self.runtime_artifact_digest is not None and not self.closed:
            raise ResidentProtocolError(
                "process-already-open",
                "resident process already has an active process identity",
            )
        semantic_fields = (
            "sessionId",
            "nodeId",
            "snapshotDigest",
            "workspaceGenerationRootDigest",
            "routeId",
            "request",
        )
        if any(field_name in message for field_name in semantic_fields):
            raise ResidentProtocolError(
                "process-semantic-identity-forbidden",
                "process-handshake cannot carry graph-session identity",
            )
        self.runtime_artifact_digest = _required_string(
            message, "runtimeArtifactDigest"
        )
        self.execution_command_digest = _required_string(
            message, "executionCommandDigest"
        )
        self.invocation_counts.clear()
        self.closed = False
        receipt = self._receipt(request_id, "process-handshake-accepted")
        receipt["processIdentity"] = {
            "processId": os.getpid(),
            "runtimeArtifactDigest": self.runtime_artifact_digest,
            "executionCommandDigest": self.execution_command_digest,
        }
        return receipt

    def _rank(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        identity = self._graph_session_identity(message)
        request = message.get("request")
        if not isinstance(request, Mapping):
            raise ResidentProtocolError(
                "missing-rank-request", "rank requires an object request"
            )
        self._validate_request_identity(request, identity)
        result = self._rank_request(request)
        identity_key = tuple(
            identity[key]
            for key in (
                "sessionId",
                "nodeId",
                "snapshotDigest",
                "workspaceGenerationRootDigest",
                "routeId",
            )
        )
        invocation_count = self.invocation_counts.get(identity_key, 0) + 1
        self.invocation_counts[identity_key] = invocation_count
        receipt = self._receipt(request_id, "rank-completed")
        receipt.update(
            {
                "graphSessionIdentity": identity,
                "authority": "candidate",
                "accounting": {
                    "graphTurboInvocations": invocation_count,
                    "semanticGraphHops": 0,
                    "executedGraphHops": 0,
                    "toolActions": 0,
                },
                "result": result,
            }
        )
        return receipt

    @staticmethod
    def _rank_request(request: Mapping[str, Any]) -> dict[str, object]:
        graph_packet = request.get("graph")
        if not isinstance(graph_packet, Mapping):
            raise ResidentProtocolError(
                "missing-graph", "rank request requires a graph object"
            )
        graph = TypedGraph.from_packet(graph_packet)
        window_merge = request.get("windowMerge")
        cache = request.get("cache")
        read_memory = request.get("readMemory")
        window_merge = window_merge if isinstance(window_merge, Mapping) else {}
        cache = cache if isinstance(cache, Mapping) else {}
        read_memory = read_memory if isinstance(read_memory, Mapping) else {}
        result = rank_frontier(
            graph,
            profile=str(request.get("profile", "owner-query")),
            seeds=_string_sequence(request.get("seedIds")),
            limit=_positive_int(request.get("budget"), 8),
            kind_budgets=_string_int_mapping(request.get("kindBudgets")),
            window_merge_enabled=bool(window_merge.get("enabled", True)),
            window_merge_max_gap_lines=_nonnegative_int(
                window_merge.get("maxGapLines"), 8
            ),
            path_budget=_positive_int(request.get("pathBudget"), 4),
            path_max_hops=_positive_int(request.get("pathMaxHops"), 4),
            cache_enabled=bool(cache.get("enabled", True)),
            seen_selectors=_string_sequence(read_memory.get("seenSelectors")),
            query_clauses=_string_sequence(request.get("queryTerms")),
        )
        return result_to_packet(result)

    def _require_live_process(self) -> None:
        if self.runtime_artifact_digest is None or self.closed:
            raise ResidentProtocolError(
                "process-not-open", "process-handshake is required before this message"
            )

    @staticmethod
    def _graph_session_identity(message: Mapping[str, Any]) -> dict[str, str]:
        return {
            "sessionId": _required_string(message, "sessionId"),
            "nodeId": _required_string(message, "nodeId"),
            "snapshotDigest": _required_string(message, "snapshotDigest"),
            "workspaceGenerationRootDigest": _required_string(
                message, "workspaceGenerationRootDigest"
            ),
            "routeId": _required_string(message, "routeId"),
        }

    @staticmethod
    def _validate_request_identity(
        request: Mapping[str, Any], identity: Mapping[str, str]
    ) -> None:
        source_snapshot = request.get("sourceSnapshot")
        workspace_generation = request.get("workspaceGeneration")
        if not isinstance(source_snapshot, Mapping) or (
            source_snapshot.get("rootDigest") != identity["snapshotDigest"]
        ):
            raise ResidentProtocolError(
                "snapshot-identity-mismatch",
                "rank request source snapshot must match the graph-session identity",
            )
        if not isinstance(workspace_generation, Mapping) or (
            workspace_generation.get("rootDigest")
            != identity["workspaceGenerationRootDigest"]
        ):
            raise ResidentProtocolError(
                "workspace-generation-identity-mismatch",
                "rank request workspace generation must match the graph-session identity",
            )

    @staticmethod
    def _receipt(request_id: str, status: str) -> dict[str, object]:
        return {
            "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-receipt",
            "schemaVersion": "1",
            "protocolId": "agent.semantic-protocols.semantic-language",
            "protocolVersion": "1",
            "packetKind": "graph-turbo-resident-receipt",
            "requestId": request_id,
            "status": status,
        }


def _required_string(message: Mapping[str, Any], key: str) -> str:
    value = message.get(key)
    if not isinstance(value, str) or not value:
        raise ResidentProtocolError(
            "invalid-identity", f"{key} must be a non-empty string"
        )
    return value


def _string_sequence(value: object) -> tuple[str, ...]:
    if not isinstance(value, list):
        return ()
    return tuple(item for item in value if isinstance(item, str) and item)


def _string_int_mapping(value: object) -> dict[str, int] | None:
    if not isinstance(value, Mapping):
        return None
    return {
        key: item
        for key, item in value.items()
        if isinstance(key, str)
        and isinstance(item, int)
        and not isinstance(item, bool)
    }


def _positive_int(value: object, default: int) -> int:
    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value > 0
        else default
    )


def _nonnegative_int(value: object, default: int) -> int:
    return (
        value
        if isinstance(value, int) and not isinstance(value, bool) and value >= 0
        else default
    )
