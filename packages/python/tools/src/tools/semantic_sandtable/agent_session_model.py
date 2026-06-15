"""Shared model for agent-session observability."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class AgentSessionConfig:
    session_id: str
    scenario_id: str
    language: str
    project_name: str
    intent: str
    agent: str = "claude-sdk"
    model: str | None = None
    edit_boundary: str = "before-edit"
    project_source: str = "checkout"
    project_workdir: str | None = None
