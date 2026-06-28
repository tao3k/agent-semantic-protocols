"""Durable task checkpoint records for session-scoped agent memory."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass, field
from typing import Any

from .episode import (
    GLOBAL_PROJECT_SCOPE,
    normalize_optional_plan_token,
    normalize_plan_token,
    now_ms,
)


def _first_mapping_value(value: dict[str, Any], *keys: str) -> Any:
    for key in keys:
        if key in value:
            return value[key]
    return None


def _optional_text(value: Any) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _text(value: Any, default: str = "") -> str:
    text = _optional_text(value)
    return text if text is not None else default


def _metadata(value: Any) -> dict[str, str]:
    if not isinstance(value, dict):
        return {}
    return {str(key): str(item) for key, item in value.items()}


def stable_checkpoint_id(
    *,
    session_id: str,
    project_id: str,
    plan_id: str | None,
    branch_id: str | None,
    source_locator: str | None,
    title: str,
) -> str:
    material = "\x1f".join(
        [
            session_id,
            project_id,
            plan_id or "",
            branch_id or "",
            source_locator or "",
            title,
        ]
    )
    digest = hashlib.sha256(material.encode("utf-8")).hexdigest()[:20]
    return f"checkpoint-{digest}"


@dataclass
class Checkpoint:
    id: str
    session_id: str
    title: str
    status: str = "open"
    kind: str = "task"
    project_id: str = GLOBAL_PROJECT_SCOPE
    plan_id: str | None = None
    branch_id: str | None = None
    source_locator: str | None = None
    resume_command: str | None = None
    metadata: dict[str, str] = field(default_factory=dict)
    created_at: int = field(default_factory=now_ms)
    updated_at: int = field(default_factory=now_ms)

    @classmethod
    def from_mapping(cls, value: dict[str, Any]) -> "Checkpoint":
        session_id = _text(_first_mapping_value(value, "session_id", "sessionId", "session"))
        if not session_id:
            raise ValueError("checkpoint session_id must not be empty")
        title = _text(_first_mapping_value(value, "title", "name"))
        if not title:
            raise ValueError("checkpoint title must not be empty")
        project_id = normalize_plan_token(
            _text(_first_mapping_value(value, "project_id", "projectId", "project"), GLOBAL_PROJECT_SCOPE),
            GLOBAL_PROJECT_SCOPE,
        )
        plan_id = normalize_optional_plan_token(_first_mapping_value(value, "plan_id", "planId", "plan"))
        branch_id = normalize_optional_plan_token(
            _first_mapping_value(value, "branch_id", "branchId", "branch")
        )
        source_locator = _optional_text(
            _first_mapping_value(value, "source_locator", "sourceLocator")
        )
        checkpoint_id = _optional_text(value.get("id")) or stable_checkpoint_id(
            session_id=session_id,
            project_id=project_id,
            plan_id=plan_id,
            branch_id=branch_id,
            source_locator=source_locator,
            title=title,
        )
        created_at = int(_first_mapping_value(value, "created_at", "createdAt") or now_ms())
        return cls(
            id=checkpoint_id,
            session_id=session_id,
            title=title,
            status=_text(_first_mapping_value(value, "status"), "open"),
            kind=_text(_first_mapping_value(value, "kind"), "task"),
            project_id=project_id,
            plan_id=plan_id,
            branch_id=branch_id,
            source_locator=source_locator,
            resume_command=_optional_text(
                _first_mapping_value(value, "resume_command", "resumeCommand")
            ),
            metadata=_metadata(value.get("metadata")),
            created_at=created_at,
            updated_at=int(_first_mapping_value(value, "updated_at", "updatedAt") or created_at),
        )

    def to_mapping(self) -> dict[str, object]:
        return {
            "id": self.id,
            "session_id": self.session_id,
            "title": self.title,
            "status": self.status,
            "kind": self.kind,
            "project_id": self.project_id,
            "plan_id": self.plan_id,
            "branch_id": self.branch_id,
            "source_locator": self.source_locator,
            "resume_command": self.resume_command,
            "metadata": dict(self.metadata),
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        }

    def normalize_tracking_fields(self) -> None:
        if not self.id.strip():
            self.id = stable_checkpoint_id(
                session_id=self.session_id,
                project_id=self.project_id,
                plan_id=self.plan_id,
                branch_id=self.branch_id,
                source_locator=self.source_locator,
                title=self.title,
            )
        self.session_id = normalize_plan_token(self.session_id, "")
        self.project_id = normalize_plan_token(self.project_id, GLOBAL_PROJECT_SCOPE)
        self.plan_id = normalize_optional_plan_token(self.plan_id)
        self.branch_id = normalize_optional_plan_token(self.branch_id)
        self.status = self.status.strip() or "open"
        self.kind = self.kind.strip() or "task"
        self.title = self.title.strip()
        if self.updated_at == 0:
            self.updated_at = self.created_at

    def matches(
        self,
        *,
        project_id: str | None = None,
        session_id: str | None = None,
        plan_id: str | None = None,
        branch_id: str | None = None,
        status: str | None = None,
    ) -> bool:
        if project_id is not None and self.project_id != normalize_plan_token(project_id, GLOBAL_PROJECT_SCOPE):
            return False
        if session_id is not None and self.session_id != normalize_plan_token(session_id, ""):
            return False
        if plan_id is not None and self.plan_id != normalize_optional_plan_token(plan_id):
            return False
        if branch_id is not None and self.branch_id != normalize_optional_plan_token(branch_id):
            return False
        if status is not None and self.status != status:
            return False
        return True
