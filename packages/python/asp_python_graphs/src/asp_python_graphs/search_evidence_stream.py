"""Incremental search evidence frames for one resident graph generation."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Mapping

from .service_protocol import ServiceProtocolError, nonnegative_int, string_sequence


LANES = {"indexed-lexical", "python-graph", "ripgrep"}
INTENTS = {"conceptual", "relationship", "exact-literal", "absence-proof"}


@dataclass(slots=True)
class SearchEvidenceAccumulator:
    intent: str | None = None
    frame_ids: set[str] = field(default_factory=set)
    owners_by_lane: dict[str, set[str]] = field(
        default_factory=lambda: {lane: set() for lane in LANES}
    )
    complete_lanes: set[str] = field(default_factory=set)
    elapsed_micros_by_lane: dict[str, int] = field(default_factory=dict)

    def observe(self, payload: Mapping[str, Any]) -> dict[str, object]:
        unknown = sorted(
            set(payload)
            - {"frameId", "intent", "lane", "ownerIds", "complete", "elapsedMicros"}
        )
        if unknown:
            raise ServiceProtocolError(
                "unknown-search-evidence-field",
                f"search evidence payload contains unsupported fields: {unknown}",
            )
        frame_id = payload.get("frameId")
        intent = payload.get("intent")
        lane = payload.get("lane")
        complete = payload.get("complete")
        if not isinstance(frame_id, str) or not frame_id:
            raise ServiceProtocolError(
                "invalid-search-frame-id", "payload.frameId must be non-empty"
            )
        if intent not in INTENTS:
            raise ServiceProtocolError(
                "invalid-search-intent", "payload.intent is not supported"
            )
        if lane not in LANES:
            raise ServiceProtocolError(
                "invalid-search-lane", "payload.lane is not supported"
            )
        if not isinstance(complete, bool):
            raise ServiceProtocolError(
                "invalid-search-completeness", "payload.complete must be boolean"
            )
        if self.intent is not None and self.intent != intent:
            raise ServiceProtocolError(
                "search-intent-drift", "one evidence stream cannot change intent"
            )
        raw_owner_ids = payload.get("ownerIds")
        if not isinstance(raw_owner_ids, list):
            raise ServiceProtocolError(
                "invalid-search-owner-id", "payload.ownerIds must be an array"
            )
        owner_ids = string_sequence(raw_owner_ids)
        if len(owner_ids) != len(raw_owner_ids) or len(set(owner_ids)) != len(owner_ids):
            raise ServiceProtocolError(
                "invalid-search-owner-id",
                "payload.ownerIds must contain unique non-empty strings",
            )
        duplicate = frame_id in self.frame_ids
        if not duplicate:
            self.intent = intent
            self.frame_ids.add(frame_id)
            self.owners_by_lane[lane].update(owner_ids)
            self.elapsed_micros_by_lane[lane] = self.elapsed_micros_by_lane.get(
                lane, 0
            ) + nonnegative_int(payload.get("elapsedMicros"), 0)
            if complete:
                self.complete_lanes.add(lane)
        return self.route_delta(duplicate=duplicate)

    def route_delta(self, *, duplicate: bool) -> dict[str, object]:
        lexical = self.owners_by_lane["indexed-lexical"]
        graph = self.owners_by_lane["python-graph"]
        verified = self.owners_by_lane["ripgrep"]
        candidate_union = lexical | graph
        if self.intent == "absence-proof":
            recommended_next = (
                "stop-evidence-sufficient"
                if "ripgrep" in self.complete_lanes
                else "prove-coverage"
            )
        elif "python-graph" not in self.complete_lanes:
            recommended_next = "submit-python-graph"
        elif "ripgrep" not in self.complete_lanes:
            recommended_next = "verify-candidates"
        else:
            recommended_next = "stop-evidence-sufficient"
        return {
            "state": "observed",
            "duplicate": duplicate,
            "frameCount": len(self.frame_ids),
            "candidateCounts": {
                "indexedLexical": len(lexical),
                "pythonGraph": len(graph),
                "ripgrepVerified": len(verified),
            },
            "correlation": {
                "lexicalGraphOverlapCount": len(lexical & graph),
                "graphMarginalCandidateCount": len(graph - lexical),
                "verifiedUnionCandidateCount": len(candidate_union & verified),
            },
            "completeLanes": sorted(self.complete_lanes),
            "elapsedMicrosByLane": dict(sorted(self.elapsed_micros_by_lane.items())),
            "recommendedNext": recommended_next,
        }
