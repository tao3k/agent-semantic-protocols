"""Own graph generation state and execute typed algorithm operations."""

from __future__ import annotations

import json
import os
import threading
from pathlib import Path
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Mapping

from .service_protocol import (
    ServiceProtocolError,
    nonnegative_int,
    optional_mapping,
    positive_int,
    required_digest,
    required_string,
    service_receipt,
    string_int_mapping,
    string_sequence,
    validate_service_envelope,
)

if TYPE_CHECKING:
    from .graph_model import TypedGraph


@dataclass
class AspPythonGraphsSession:
    runtime_artifact_digest: str | None = None
    execution_artifact_digest: str | None = None
    service_epoch: str | None = None
    loaded_generations: dict[tuple[str, str, int], "TypedGraph"] = field(
        default_factory=dict
    )
    generation_packets: dict[tuple[str, str, int], str] = field(
        default_factory=dict
    )
    closed: bool = False
    _last_sequence: int = field(default=0, init=False, repr=False)
    _generation_lock: threading.RLock = field(
        default_factory=threading.RLock, init=False, repr=False
    )
    _active_requests: set[str] = field(default_factory=set, init=False, repr=False)
    _admitted_requests: set[str] = field(default_factory=set, init=False, repr=False)
    _cancelled_requests: set[str] = field(default_factory=set, init=False, repr=False)

    def handle(self, message: Mapping[str, Any]) -> dict[str, object]:
        validate_service_envelope(message)
        sequence = message["sequence"]
        assert isinstance(sequence, int)
        kind = str(message["messageKind"])
        service_epoch = str(message["serviceEpoch"])
        with self._generation_lock:
            if sequence <= self._last_sequence:
                raise ServiceProtocolError(
                    "non-monotonic-sequence", "session sequence must increase monotonically"
                )
            if kind != "hello" and self.service_epoch != service_epoch:
                raise ServiceProtocolError(
                    "service-epoch-drift",
                    "message serviceEpoch does not match the bound service session",
                )
            self._last_sequence = sequence
        request_id = str(message["requestId"])
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
            return self._receipt(request_id, "cancelled", sequence=int(message["sequence"]))
        if kind in {"evaluate", "timeline"}:
            with self._generation_lock:
                if request_id in self._active_requests:
                    raise ServiceProtocolError(
                        "duplicate-request-id", "requestId is already in flight"
                    )
                self._admitted_requests.discard(request_id)
                if request_id in self._cancelled_requests:
                    self._cancelled_requests.discard(request_id)
                    return self._receipt(request_id, "cancelled", sequence=int(message["sequence"]))
                if self.closed or self.runtime_artifact_digest is None:
                    raise ServiceProtocolError(
                        "process-not-open", "hello is required before evaluate"
                    )
                graph = None
                if kind == "evaluate":
                    workspace, generation, token, _ = self._generation_identity(message)
                    graph = self.loaded_generations.get((workspace, generation, token))
                    if graph is None:
                        raise ServiceProtocolError(
                            "generation-not-loaded",
                            "evaluate requires an ASP Server-owned open-generation receipt",
                        )
                self._active_requests.add(request_id)
            try:
                result = (
                    self._evaluate(message, request_id, graph)
                    if kind == "evaluate"
                    else self._timeline(message, request_id)
                )
                with self._generation_lock:
                    if request_id in self._cancelled_requests:
                        return self._receipt(request_id, "cancelled", sequence=int(message["sequence"]))
                return result
            finally:
                with self._generation_lock:
                    self._active_requests.discard(request_id)
        with self._generation_lock:
            if kind == "hello":
                return self._hello(message, request_id)
            if kind == "health":
                return self._health(message, request_id)
            if kind == "shutdown":
                return self._shutdown(message, request_id)
            if kind == "open-generation":
                return self._open_generation(message, request_id)
            if kind == "release-generation":
                return self._release_generation(message, request_id)
            raise ServiceProtocolError(
                "unsupported-message-kind", f"unsupported messageKind: {kind!r}"
            )

    def _timeline(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        from .artifact_event_packet import artifact_events_from_packet
        from .artifact_timeline import evaluate_artifact_events_timeline
        from .artifact_timeline_parameters import TimelineParameters
        from .timeline_cli import _parse_args, _parse_since

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
                "invalid-timeline-arguments", "timeline payload.arguments must be strings"
            )
        try:
            parsed = _parse_args(["--format", "json", *arguments])
            parameters = TimelineParameters(
                subagent_start_gap_seconds=parsed.subagent_start_gap_seconds,
                subagent_soft_max_seconds=parsed.subagent_soft_max_seconds,
                subagent_hard_max_seconds=parsed.subagent_hard_max_seconds,
                session_gap_seconds=parsed.session_gap_seconds,
                examples=parsed.examples,
                since_timestamp=_parse_since(parsed.since),
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

    def _hello(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        if self.closed:
            raise ServiceProtocolError(
                "process-closed", "shutdown is terminal for this service session"
            )
        runtime_artifact_digest = required_digest(message, "runtimeArtifactDigest")
        execution_artifact_digest = required_digest(
            message, "executionArtifactDigest"
        )
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
            "loadedGenerationCount": len(self.loaded_generations),
        }
        return receipt

    def _shutdown(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        self.closed = True
        self.loaded_generations.clear()
        self.generation_packets.clear()
        self._active_requests.clear()
        self._admitted_requests.clear()
        self._cancelled_requests.clear()
        return self._receipt(
            request_id, "cancelled", sequence=int(message["sequence"])
        )

    def _health(self, message: Mapping[str, Any], request_id: str) -> dict[str, object]:
        if self.closed or self.runtime_artifact_digest is None:
            raise ServiceProtocolError("process-not-open", "service is not ready")
        receipt = self._receipt(request_id, "ready", sequence=int(message["sequence"]))
        receipt["payload"] = {
            "processId": os.getpid(),
            "loadedGenerationCount": len(self.loaded_generations),
        }
        return receipt

    def _open_generation(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        from .graph_model import TypedGraph

        if self.closed or self.runtime_artifact_digest is None:
            raise ServiceProtocolError(
                "process-not-open", "hello is required before open-generation"
            )
        workspace, generation, token, payload = self._generation_identity(message)
        graph_packet = payload.get("graph")
        if not isinstance(graph_packet, Mapping):
            raise ServiceProtocolError(
                "missing-generation-graph", "payload.graph must be an object"
            )
        load_key = (workspace, generation, token)
        packet_digest = json.dumps(graph_packet, sort_keys=True, separators=(",", ":"))
        existing_packet = self.generation_packets.get(load_key)
        if existing_packet is not None and existing_packet != packet_digest:
            raise ServiceProtocolError(
                "generation-graph-mismatch",
                "same generation identity was admitted with a different graph snapshot",
            )
        newly_loaded = load_key not in self.loaded_generations
        if newly_loaded:
            self.loaded_generations[load_key] = TypedGraph.from_packet(graph_packet)
            self.generation_packets[load_key] = packet_digest
        receipt = self._receipt(request_id, "ready", workspace, generation, token,
                                sequence=int(message["sequence"]))
        receipt["payload"] = {
            "generationLoads": int(newly_loaded),
            "loadedGenerationCount": len(self.loaded_generations),
        }
        return receipt

    def _release_generation(
        self, message: Mapping[str, Any], request_id: str
    ) -> dict[str, object]:
        workspace, generation, token, _ = self._generation_identity(message)
        load_key = (workspace, generation, token)
        released = self.loaded_generations.pop(load_key, None) is not None
        self.generation_packets.pop(load_key, None)
        receipt = self._receipt(request_id, "completed", workspace, generation, token,
                                sequence=int(message["sequence"]))
        receipt["payload"] = {"released": released}
        return receipt

    def _evaluate(
        self, message: Mapping[str, Any], request_id: str, graph: "TypedGraph"
    ) -> dict[str, object]:
        from .ranking import rank_frontier
        from .result_packet import result_to_packet

        workspace, generation, token, payload = self._generation_identity(message)
        terms = payload.get("terms")
        rank_payload = payload.get("rankPayload")
        if not isinstance(terms, list) or any(
            not isinstance(term, str) or not term for term in terms
        ):
            raise ServiceProtocolError(
                "invalid-terms", "payload.terms must be a string array"
            )
        if not isinstance(rank_payload, Mapping):
            raise ServiceProtocolError(
                "invalid-rank-payload", "payload.rankPayload must be an object"
            )
        result = rank_frontier(
            graph,
            profile=str(payload.get("profile", "owner-query")),
            seeds=string_sequence(rank_payload.get("seedIds")),
            limit=positive_int(payload.get("budget"), 8),
            kind_budgets=string_int_mapping(rank_payload.get("kindBudgets")),
            window_merge_enabled=bool(
                optional_mapping(rank_payload.get("windowMerge")).get("enabled", True)
            ),
            window_merge_max_gap_lines=nonnegative_int(
                optional_mapping(rank_payload.get("windowMerge")).get("maxGapLines"), 8
            ),
            path_budget=positive_int(rank_payload.get("pathBudget"), 4),
            path_max_hops=positive_int(rank_payload.get("pathMaxHops"), 4),
            cache_enabled=bool(
                optional_mapping(rank_payload.get("cache")).get("enabled", True)
            ),
            query_clauses=string_sequence(rank_payload.get("queryClauses")),
        )
        receipt = self._receipt(request_id, "completed", workspace, generation, token,
                                sequence=int(message["sequence"]))
        receipt["payload"] = {
            "generationLoads": 0,
            "result": result_to_packet(result),
        }
        return receipt

    def is_cancelled(self, request_id: str) -> bool:
        with self._generation_lock:
            return request_id in self._cancelled_requests

    def register_request(self, request_id: str) -> None:
        with self._generation_lock:
            if not request_id:
                raise ServiceProtocolError("invalid-identity", "requestId must be non-empty")
            if request_id in self._active_requests or request_id in self._admitted_requests:
                raise ServiceProtocolError("duplicate-request-id", "requestId is already in flight")
            self._admitted_requests.add(request_id)

    def complete_request(self, request_id: str) -> None:
        with self._generation_lock:
            self._active_requests.discard(request_id)
            self._admitted_requests.discard(request_id)
            self._cancelled_requests.discard(request_id)

    def _generation_identity(
        self, message: Mapping[str, Any]
    ) -> tuple[str, str, int, Mapping[str, Any]]:
        workspace = required_string(message, "workspaceIdentity")
        generation = required_digest(message, "generationDigest")
        token = message.get("generationToken")
        if not isinstance(token, int) or isinstance(token, bool) or token < 1:
            raise ServiceProtocolError(
                "invalid-generation-token", "generationToken must be a positive integer"
            )
        payload = message.get("payload")
        if not isinstance(payload, Mapping):
            raise ServiceProtocolError("invalid-payload", "payload must be an object")
        return workspace, generation, token, payload

    def _receipt(
        self,
        request_id: str,
        state: str,
        workspace: str | None = None,
        generation: str | None = None,
        generation_token: int | None = None,
        sequence: int = 1,
    ) -> dict[str, object]:
        return service_receipt(
            request_id=request_id,
            service_epoch=self.service_epoch,
            state=state,
            workspace=workspace,
            generation=generation,
            generation_token=generation_token,
            sequence=sequence,
        )
