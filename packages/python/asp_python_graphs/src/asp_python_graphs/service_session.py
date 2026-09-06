"""Own graph generation state and execute typed algorithm operations."""

from __future__ import annotations

import os
import threading
from pathlib import Path
from dataclasses import dataclass, field
from typing import Any, Mapping

from .model import TypedGraph
from .service_protocol import (
    ServiceProtocolError,
    required_digest,
    required_string,
    service_receipt,
    validate_service_envelope,
)


MAX_RETAINED_GENERATIONS = 8


@dataclass(frozen=True, slots=True)
class RetainedGenerationGraph:
    project_id: str
    workspace_identity: str
    generation_digest: str
    root_digest: str
    artifact_digest: str
    graph: TypedGraph


@dataclass
class AspPythonGraphsSession:
    runtime_artifact_digest: str | None = None
    execution_artifact_digest: str | None = None
    service_epoch: str | None = None
    closed: bool = False
    _last_sequence: int = field(default=0, init=False, repr=False)
    _generation_lock: threading.RLock = field(
        default_factory=threading.RLock, init=False, repr=False
    )
    _active_requests: set[str] = field(default_factory=set, init=False, repr=False)
    _admitted_requests: set[str] = field(default_factory=set, init=False, repr=False)
    _cancelled_requests: set[str] = field(default_factory=set, init=False, repr=False)
    _generation_graphs: dict[tuple[str, str], RetainedGenerationGraph] = field(
        default_factory=dict, init=False, repr=False
    )

    def handle(self, message: Mapping[str, Any]) -> dict[str, object]:
        return self._handle(message, sequence_admitted=False)

    def admit_message(
        self, message: Mapping[str, Any], *, validate_service_epoch: bool = True
    ) -> None:
        """Validate and reserve transport order before a worker is spawned.

        The gRPC reader is the actor that observes the incoming stream order.
        Evaluation work may run in ``to_thread`` workers, so sequence
        admission cannot be performed for the first time inside those workers
        without making an otherwise ordered stream race.
        """
        validate_service_envelope(message)
        sequence = message["sequence"]
        assert isinstance(sequence, int)
        kind = str(message["messageKind"])
        service_epoch = str(message["serviceEpoch"])
        with self._generation_lock:
            if sequence <= self._last_sequence:
                raise ServiceProtocolError(
                    "non-monotonic-sequence",
                    "session sequence must increase monotonically",
                )
            if (
                validate_service_epoch
                and kind != "hello"
                and self.service_epoch != service_epoch
            ):
                raise ServiceProtocolError(
                    "service-epoch-drift",
                    "message serviceEpoch does not match the bound service session",
                )
            self._last_sequence = sequence

    def handle_admitted(self, message: Mapping[str, Any]) -> dict[str, object]:
        """Handle a message whose transport order was admitted by the reader."""
        return self._handle(message, sequence_admitted=True)

    def _handle(
        self, message: Mapping[str, Any], *, sequence_admitted: bool
    ) -> dict[str, object]:
        if not sequence_admitted:
            self.admit_message(message)
        request_id = str(message["requestId"])
        kind = str(message["messageKind"])
        service_epoch = str(message["serviceEpoch"])
        with self._generation_lock:
            if kind != "hello" and self.service_epoch != service_epoch:
                raise ServiceProtocolError(
                    "service-epoch-drift",
                    "message serviceEpoch does not match the bound service session",
                )
        if kind == "cancel":
            cancellation_id = str(message["cancellationId"])
            with self._generation_lock:
                if (
                    cancellation_id not in self._active_requests
                    and cancellation_id not in self._admitted_requests
                ):
                    raise ServiceProtocolError(
                        "unknown-cancellation", "cancellationId is not in flight"
                    )
                if len(self._cancelled_requests) >= 64:
                    raise ServiceProtocolError(
                        "cancellation-capacity-exhausted",
                        "cancellation registry is saturated",
                    )
                self._cancelled_requests.add(cancellation_id)
            return self._receipt(
                request_id, "cancelled", sequence=int(message["sequence"])
            )
        if kind in {"timeline", "generation-graph", "evaluate-resident"}:
            with self._generation_lock:
                if request_id in self._active_requests:
                    raise ServiceProtocolError(
                        "duplicate-request-id", "requestId is already in flight"
                    )
                self._admitted_requests.discard(request_id)
                if request_id in self._cancelled_requests:
                    self._cancelled_requests.discard(request_id)
                    return self._receipt(
                        request_id, "cancelled", sequence=int(message["sequence"])
                    )
                if self.closed or self.runtime_artifact_digest is None:
                    raise ServiceProtocolError(
                        "process-not-open", "hello is required before evaluate"
                    )
                self._active_requests.add(request_id)
            try:
                if kind == "timeline":
                    result = self._timeline(message, request_id)
                elif kind == "generation-graph":
                    result = self._generation_graph(message, request_id)
                else:
                    result = self._evaluate_resident(message, request_id)
                with self._generation_lock:
                    if request_id in self._cancelled_requests:
                        return self._receipt(
                            request_id, "cancelled", sequence=int(message["sequence"])
                        )
                return result
            finally:
                with self._generation_lock:
                    self._active_requests.discard(request_id)
        with self._generation_lock:
            if kind == "hello":
                return self._hello(message, request_id)
            if kind == "health":
                return self._health(message, request_id)
            if kind == "release-generation":
                return self._release_generation(message, request_id)
            if kind == "shutdown":
                return self._shutdown(message, request_id)
            raise ServiceProtocolError(
                "unsupported-message-kind", f"unsupported messageKind: {kind!r}"
            )

    def _timeline(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        from .artifact_event_packet import artifact_events_from_packet
        from .artifact_timeline import evaluate_artifact_events_timeline
        from .artifact_timeline_parameters import (
            TimelineParameters,
            parse_since,
            parse_timeline_args,
        )

        payload = message.get("payload")
        if not isinstance(payload, Mapping):
            raise ServiceProtocolError("invalid-payload", "payload must be an object")
        unknown = sorted(set(payload) - {"eventPacket", "arguments"})
        if unknown:
            raise ServiceProtocolError(
                "unknown-timeline-field",
                f"timeline payload contains unsupported fields: {unknown}",
            )
        packet = payload.get("eventPacket")
        if not isinstance(packet, Mapping):
            raise ServiceProtocolError(
                "invalid-event-packet", "timeline payload.eventPacket must be an object"
            )
        arguments = payload.get("arguments", [])
        if not isinstance(arguments, list) or any(
            not isinstance(argument, str) for argument in arguments
        ):
            raise ServiceProtocolError(
                "invalid-timeline-arguments",
                "timeline payload.arguments must be strings",
            )
        try:
            parsed = parse_timeline_args(["--format", "json", *arguments])
            parameters = TimelineParameters(
                subagent_start_gap_seconds=parsed.subagent_start_gap_seconds,
                subagent_soft_max_seconds=parsed.subagent_soft_max_seconds,
                subagent_hard_max_seconds=parsed.subagent_hard_max_seconds,
                session_gap_seconds=parsed.session_gap_seconds,
                examples=parsed.examples,
                since_timestamp=parse_since(parsed.since),
                recent_sessions=parsed.recent_sessions,
            )
            events = artifact_events_from_packet(packet)
            artifact_dir = Path(str(packet.get("artifactDir") or "."))
            source = packet.get("source")
            event_source = (
                source.get("kind")
                if isinstance(source, Mapping) and isinstance(source.get("kind"), str)
                else "events-json"
            )
            report = evaluate_artifact_events_timeline(
                events,
                artifact_dir=artifact_dir,
                parameters=parameters,
                event_source=event_source,
            )
        except SystemExit as error:
            raise ServiceProtocolError(
                "invalid-timeline-arguments", "timeline arguments are invalid"
            ) from error
        except (TypeError, ValueError, OSError) as error:
            raise ServiceProtocolError("invalid-event-packet", str(error)) from error
        receipt = self._receipt(
            request_id, "completed", sequence=int(message["sequence"])
        )
        receipt["payload"] = {"result": report}
        return receipt

    def _generation_graph(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        from .generation_graph import compile_generation_graph

        payload = message.get("payload")
        if not isinstance(payload, Mapping):
            raise ServiceProtocolError("invalid-payload", "payload must be an object")
        try:
            workspace, generation, _ = self._generation_candidate_identity(message)
            compiled = compile_generation_graph(payload)
            identity = compiled.receipt["identity"]
            if not isinstance(identity, Mapping):
                raise ValueError("search generation graph identity must be an object")
            project_id = required_string(identity, "projectId")
            if required_string(identity, "workspaceId") != workspace:
                raise ValueError("search generation graph workspace identity drift")
            if required_digest(identity, "generationCandidateDigest") != generation:
                raise ValueError("search generation graph generation identity drift")
            root_digest = required_digest(identity, "sourceRootDigest")
            artifact_digest = required_digest(compiled.receipt, "artifactDigest")
            retained = RetainedGenerationGraph(
                project_id=project_id,
                workspace_identity=workspace,
                generation_digest=generation,
                root_digest=root_digest,
                artifact_digest=artifact_digest,
                graph=TypedGraph.from_packet({"graph": compiled.graph}),
            )
            key = (workspace, generation)
            with self._generation_lock:
                current = self._generation_graphs.get(key)
                if current is not None and (
                    current.project_id != retained.project_id
                    or current.root_digest != retained.root_digest
                    or current.artifact_digest != retained.artifact_digest
                ):
                    raise ValueError(
                        "search generation graph identity already retained with drift"
                    )
                if (
                    current is None
                    and len(self._generation_graphs) >= MAX_RETAINED_GENERATIONS
                ):
                    raise ServiceProtocolError(
                        "generation-capacity-exhausted",
                        "resident generation graph capacity is saturated",
                    )
                self._generation_graphs[key] = retained
        except ServiceProtocolError:
            raise
        except (TypeError, ValueError) as error:
            raise ServiceProtocolError(
                "invalid-generation-graph", str(error)
            ) from error
        receipt = self._receipt(
            request_id,
            "completed",
            workspace,
            generation,
            sequence=int(message["sequence"]),
        )
        receipt["payload"] = {"result": dict(compiled.receipt), "retained": True}
        return receipt

    def _evaluate_resident(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        from .algorithm import rank_graph
        from .result_packet import result_to_packet

        workspace, generation, payload = self._generation_candidate_identity(message)
        _validate_resident_evaluation_payload(payload)
        with self._generation_lock:
            retained = self._generation_graphs.get((workspace, generation))
        if retained is None:
            raise ServiceProtocolError(
                "resident-generation-unavailable",
                "the exact workspace generation graph is not retained",
            )
        profile = {
            "balanced": "owner-query",
            "structural": "owner-query",
            "dependency": "query-deps",
        }[str(payload["profile"])]
        budget = payload["budget"]
        assert isinstance(budget, Mapping)
        controls = {
            "entryNodeIds": list(payload["entryNodeIds"]),
            "budget": int(budget["maxResults"]),
            "queryClauses": list(payload["queryClauses"]),
        }
        try:
            result = result_to_packet(
                rank_graph(retained.graph, controls, profile=profile)
            )
        except (TypeError, ValueError, KeyError) as error:
            raise ServiceProtocolError(
                "resident-evaluation-failed", str(error)
            ) from error
        candidate_node_ids = frozenset(payload["candidateNodeIds"])
        if candidate_node_ids:
            _project_candidate_nodes(result, candidate_node_ids)
        receipt = self._receipt(
            request_id,
            "completed",
            workspace,
            generation,
            sequence=int(message["sequence"]),
        )
        receipt["payload"] = {
            "result": result,
            "projectId": retained.project_id,
            "rootDigest": retained.root_digest,
            "graphArtifactDigest": retained.artifact_digest,
        }
        return receipt

    def _release_generation(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        workspace, generation, payload = self._generation_candidate_identity(message)
        if payload:
            raise ServiceProtocolError(
                "invalid-release-payload", "release-generation payload must be empty"
            )
        with self._generation_lock:
            if self._generation_graphs.pop((workspace, generation), None) is None:
                raise ServiceProtocolError(
                    "resident-generation-unavailable",
                    "the exact workspace generation graph is not retained",
                )
        return self._receipt(
            request_id,
            "released",
            workspace,
            generation,
            sequence=int(message["sequence"]),
        )

    def _hello(self, message: Mapping[str, Any], request_id: str) -> dict[str, object]:
        if self.closed:
            raise ServiceProtocolError(
                "process-closed", "shutdown is terminal for this service session"
            )
        runtime_artifact_digest = required_digest(message, "runtimeArtifactDigest")
        execution_artifact_digest = required_digest(message, "executionArtifactDigest")
        service_epoch = required_string(message, "serviceEpoch")
        if self.runtime_artifact_digest is not None:
            if (
                runtime_artifact_digest != self.runtime_artifact_digest
                or execution_artifact_digest != self.execution_artifact_digest
                or service_epoch != self.service_epoch
            ):
                raise ServiceProtocolError(
                    "hello-drift", "hello digest or service epoch changed"
                )
        else:
            self.runtime_artifact_digest = runtime_artifact_digest
            self.execution_artifact_digest = execution_artifact_digest
            self.service_epoch = service_epoch
        receipt = self._receipt(request_id, "ready", sequence=int(message["sequence"]))
        receipt["payload"] = {
            "processId": os.getpid(),
            "runtimeArtifactDigest": runtime_artifact_digest,
            "executionArtifactDigest": execution_artifact_digest,
            "retainedGenerationCount": len(self._generation_graphs),
        }
        return receipt

    def _shutdown(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        self.closed = True
        self._active_requests.clear()
        self._admitted_requests.clear()
        self._cancelled_requests.clear()
        self._generation_graphs.clear()
        return self._receipt(request_id, "cancelled", sequence=int(message["sequence"]))

    def _health(self, message: Mapping[str, Any], request_id: str) -> dict[str, object]:
        if self.closed or self.runtime_artifact_digest is None:
            raise ServiceProtocolError("process-not-open", "service is not ready")
        receipt = self._receipt(request_id, "ready", sequence=int(message["sequence"]))
        receipt["payload"] = {
            "processId": os.getpid(),
            "retainedGenerationCount": len(self._generation_graphs),
        }
        return receipt

    def is_cancelled(self, request_id: str) -> bool:
        with self._generation_lock:
            return request_id in self._cancelled_requests

    def register_request(self, request_id: str) -> None:
        with self._generation_lock:
            if not request_id:
                raise ServiceProtocolError(
                    "invalid-identity", "requestId must be non-empty"
                )
            if (
                request_id in self._active_requests
                or request_id in self._admitted_requests
            ):
                raise ServiceProtocolError(
                    "duplicate-request-id", "requestId is already in flight"
                )
            self._admitted_requests.add(request_id)

    def complete_request(self, request_id: str) -> None:
        with self._generation_lock:
            self._active_requests.discard(request_id)
            self._admitted_requests.discard(request_id)
            self._cancelled_requests.discard(request_id)

    def _generation_candidate_identity(
        self, message: Mapping[str, Any]
    ) -> tuple[str, str, Mapping[str, Any]]:
        workspace = required_string(message, "workspaceIdentity")
        generation = required_digest(message, "generationDigest")
        payload = message.get("payload")
        if not isinstance(payload, Mapping):
            raise ServiceProtocolError("invalid-payload", "payload must be an object")
        return workspace, generation, payload

    def _receipt(
        self,
        request_id: str,
        state: str,
        workspace: str | None = None,
        generation: str | None = None,
        sequence: int = 1,
    ) -> dict[str, object]:
        return service_receipt(
            request_id=request_id,
            service_epoch=self.service_epoch,
            state=state,
            workspace=workspace,
            generation=generation,
            sequence=sequence,
        )


def _validate_resident_evaluation_payload(payload: Mapping[str, Any]) -> None:
    required = {
        "schemaId",
        "schemaVersion",
        "protocolId",
        "protocolVersion",
        "packetKind",
        "languageId",
        "surface",
        "queryTerms",
        "queryClauses",
        "profile",
        "entryNodeIds",
        "candidateNodeIds",
        "budget",
    }
    if set(payload) != required:
        raise ServiceProtocolError(
            "invalid-resident-evaluation",
            "resident evaluation payload must use the exact V1 field set",
        )
    exact = {
        "schemaId": "agent.semantic-protocols.semantic-graph-resident-evaluation-request",
        "schemaVersion": "1",
        "protocolId": "agent.semantic-protocols.search",
        "protocolVersion": "1",
        "packetKind": "resident-graph-evaluation-request",
    }
    if any(payload.get(key) != value for key, value in exact.items()):
        raise ServiceProtocolError(
            "invalid-resident-evaluation", "resident evaluation V1 identity mismatch"
        )
    if payload.get("surface") not in {"search-playbook", "query"}:
        raise ServiceProtocolError(
            "invalid-resident-evaluation",
            "resident evaluation surface must be search-playbook or query",
        )
    if payload.get("profile") not in {"balanced", "structural", "dependency"}:
        raise ServiceProtocolError(
            "invalid-resident-evaluation", "resident evaluation profile is unsupported"
        )
    for key in ("queryTerms", "queryClauses", "entryNodeIds", "candidateNodeIds"):
        value = payload.get(key)
        if not isinstance(value, list) or any(
            not isinstance(item, str) or not item for item in value
        ):
            raise ServiceProtocolError(
                "invalid-resident-evaluation", f"{key} must be a string array"
            )
    if not payload["queryTerms"] and not payload["entryNodeIds"]:
        raise ServiceProtocolError(
            "invalid-resident-evaluation",
            "resident evaluation requires queryTerms or entryNodeIds",
        )
    if not set(payload["candidateNodeIds"]).issubset(payload["entryNodeIds"]):
        raise ServiceProtocolError(
            "invalid-resident-evaluation",
            "candidateNodeIds must be a subset of entryNodeIds",
        )
    budget = payload.get("budget")
    expected_budget = {"maxDepth", "maxNodes", "maxEdges", "maxResults"}
    if not isinstance(budget, Mapping) or set(budget) != expected_budget:
        raise ServiceProtocolError(
            "invalid-resident-evaluation", "resident evaluation budget is invalid"
        )
    if any(
        not isinstance(budget[field], int)
        or isinstance(budget[field], bool)
        or budget[field] < (0 if field in {"maxDepth", "maxEdges"} else 1)
        for field in expected_budget
    ):
        raise ServiceProtocolError(
            "invalid-resident-evaluation", "resident evaluation budget is invalid"
        )


def _project_candidate_nodes(
    result: dict[str, object], candidate_node_ids: frozenset[str]
) -> None:
    ranked_nodes = result.get("rankedNodes")
    if not isinstance(ranked_nodes, list):
        raise ServiceProtocolError(
            "resident-evaluation-failed", "rankedNodes projection is absent"
        )
    projected = [
        node
        for node in ranked_nodes
        if isinstance(node, Mapping) and node.get("id") in candidate_node_ids
    ]
    projected_ids = [str(node["id"]) for node in projected]
    projected_id_set = frozenset(projected_ids)
    result["rankedNodes"] = projected
    result["rank"] = projected_ids
    scores = result.get("scores")
    if isinstance(scores, Mapping):
        result["scores"] = {
            node_id: score
            for node_id, score in scores.items()
            if node_id in projected_id_set
        }
    explanations = result.get("rankExplanations")
    if isinstance(explanations, list):
        result["rankExplanations"] = [
            explanation
            for explanation in explanations
            if isinstance(explanation, Mapping)
            and explanation.get("nodeId") in projected_id_set
        ]
    frontier = result.get("frontier")
    if isinstance(frontier, list):
        result["frontier"] = [
            entry
            for entry in frontier
            if isinstance(entry, Mapping) and entry.get("nodeId") in projected_id_set
        ]
