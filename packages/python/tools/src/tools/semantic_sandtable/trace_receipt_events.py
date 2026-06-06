"""Parse recorded trace lines into receipt command entries."""

from __future__ import annotations

import json
import re
import shlex
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .utils import dict_value, optional_int, string_list


_COMMAND_KINDS = {
    "search",
    "hook-deny",
    "subagent",
    "external-ingest",
    "check",
    "other",
}
_OUTPUT_MODES = {"compact", "json", "unknown"}


@dataclass(frozen=True)
class TraceCommandFilter:
    session_id: str | None = None
    language_id: str | None = None
    provider_id: str | None = None


def trace_commands_from_path(
    trace_path: Path,
    *,
    filters: TraceCommandFilter | None = None,
) -> list[dict[str, Any]]:
    parser = TraceCommandParser(filters=filters or TraceCommandFilter())
    return parser.commands_from_path(trace_path)


class TraceCommandParser:
    def __init__(self, *, filters: TraceCommandFilter) -> None:
        self.filters = filters

    def commands_from_path(self, trace_path: Path) -> list[dict[str, Any]]:
        commands: list[dict[str, Any]] = []
        for path in self.trace_files(trace_path):
            commands.extend(self.commands_from_file(path))
        return commands

    def commands_from_file(self, trace_path: Path) -> list[dict[str, Any]]:
        commands: list[dict[str, Any]] = []
        with trace_path.open("r", encoding="utf-8") as handle:
            for line_number, raw_line in enumerate(handle, start=1):
                command = self.command_from_line(raw_line, line_number)
                if command is not None:
                    commands.append(command)
        return commands

    def trace_files(self, trace_path: Path) -> list[Path]:
        if trace_path.is_file():
            return [trace_path]
        if not trace_path.is_dir():
            return [trace_path]
        command_logs = [
            path for path in trace_path.rglob("*.jsonl") if "commands" in path.parts
        ]
        if command_logs:
            return sorted(command_logs)
        return sorted(trace_path.rglob("*.jsonl"))

    def command_from_line(
        self,
        raw_line: str,
        line_number: int,
    ) -> dict[str, Any] | None:
        line = raw_line.strip()
        if not line or line.startswith("#"):
            return None
        payload = self.json_payload(line)
        if payload is not None:
            if not self.payload_matches_filters(payload):
                return None
            return self.json_command(payload, line_number)
        if self.has_filters():
            return None
        return self.text_command(line, line_number)

    def json_command(
        self,
        payload: dict[str, Any],
        line_number: int,
    ) -> dict[str, Any]:
        command_payload = dict_value(payload.get("command"))
        result_payload = dict_value(payload.get("result"))
        metrics_payload = dict_value(payload.get("metrics"))
        argv = self.argv_from_payload(payload, command_payload)
        command = self.base_json_command(
            payload,
            line_number,
            argv,
            result_payload,
            metrics_payload,
        )
        self.attach_next_items(command, payload, command_payload)
        self.attach_output_artifacts(command, payload, result_payload, metrics_payload)
        self.copy_optional_metadata(payload, command)
        return command

    def base_json_command(
        self,
        payload: dict[str, Any],
        line_number: int,
        argv: list[str],
        result_payload: dict[str, Any],
        metrics_payload: dict[str, Any],
    ) -> dict[str, Any]:
        return {
            "id": self.command_id(payload, line_number),
            "kind": self.command_kind(payload, argv),
            "argv": argv,
            "outputMode": self.output_mode(payload, argv),
            "metrics": self.metrics(payload, result_payload, metrics_payload),
        }

    def attach_next_items(
        self,
        command: dict[str, Any],
        payload: dict[str, Any],
        command_payload: dict[str, Any],
    ) -> None:
        next_items = string_list(payload.get("next"))
        if not next_items:
            next_items = string_list(command_payload.get("next"))
        if next_items:
            command["next"] = next_items

    def attach_output_artifacts(
        self,
        command: dict[str, Any],
        payload: dict[str, Any],
        result_payload: dict[str, Any],
        metrics_payload: dict[str, Any],
    ) -> None:
        output_artifacts = self.output_artifacts(
            result_payload, metrics_payload, payload
        )
        if output_artifacts:
            command["outputArtifacts"] = output_artifacts

    def copy_optional_metadata(
        self,
        payload: dict[str, Any],
        command: dict[str, Any],
    ) -> None:
        self.copy_optional_string(payload, command, "decisionReasonKind")
        self.copy_optional_string(payload, command, "routeKind")
        self.copy_optional_string(payload, command, "stdinShape")

    def output_artifacts(
        self,
        result_payload: dict[str, Any],
        metrics_payload: dict[str, Any],
        payload: dict[str, Any],
    ) -> dict[str, str]:
        artifacts: dict[str, str] = {}
        for field in ("stdoutPath", "stderrPath"):
            value = self.first_str(
                result_payload, metrics_payload, payload, field=field
            )
            if value:
                artifacts[field] = value
        return artifacts

    def payload_matches_filters(self, payload: dict[str, Any]) -> bool:
        return (
            self.matches_field(payload, "sessionId", self.filters.session_id)
            and self.matches_field(payload, "languageId", self.filters.language_id)
            and self.matches_field(payload, "providerId", self.filters.provider_id)
        )

    def matches_field(
        self,
        payload: dict[str, Any],
        field: str,
        expected: str | None,
    ) -> bool:
        if expected is None:
            return True
        return payload.get(field) == expected

    def has_filters(self) -> bool:
        return any(
            value is not None
            for value in (
                self.filters.session_id,
                self.filters.language_id,
                self.filters.provider_id,
            )
        )

    def text_command(self, line: str, line_number: int) -> dict[str, Any]:
        argv = self.split_command_line(self.strip_prompt(line))
        return {
            "id": f"command-{line_number}",
            "kind": self.infer_kind(argv),
            "argv": argv,
            "outputMode": self.infer_output_mode(argv),
            "metrics": {"elapsedMs": 0, "stdoutBytes": 0, "stderrBytes": 0},
        }

    def argv_from_payload(
        self,
        payload: dict[str, Any],
        command_payload: dict[str, Any],
    ) -> list[str]:
        for value in (payload.get("argv"), command_payload.get("argv")):
            argv = string_list(value)
            if argv:
                return argv
        for key in ("commandLine", "cmd"):
            value = payload.get(key)
            if isinstance(value, str):
                return self.split_command_line(value)
        query = command_payload.get("query")
        if isinstance(query, str) and query.strip():
            return self.split_command_line(query)
        return self.method_argv(payload, command_payload)

    def method_argv(
        self,
        payload: dict[str, Any],
        command_payload: dict[str, Any],
    ) -> list[str]:
        method = command_payload.get("method")
        language_id = payload.get("languageId")
        if isinstance(language_id, str) and isinstance(method, str):
            return ["asp", language_id, method]
        return ["unknown"]

    def command_id(self, payload: dict[str, Any], line_number: int) -> str:
        for key in ("id", "commandId", "eventId"):
            value = payload.get(key)
            if isinstance(value, str) and value:
                return self.safe_command_id(value, line_number)
        return f"command-{line_number}"

    def command_kind(self, payload: dict[str, Any], argv: list[str]) -> str:
        kind = payload.get("kind")
        if isinstance(kind, str) and kind in _COMMAND_KINDS:
            return kind
        if isinstance(payload.get("decisionReasonKind"), str):
            return "hook-deny"
        return self.infer_kind(argv)

    def output_mode(self, payload: dict[str, Any], argv: list[str]) -> str:
        output_mode = payload.get("outputMode")
        if isinstance(output_mode, str) and output_mode in _OUTPUT_MODES:
            return output_mode
        return self.infer_output_mode(argv)

    def metrics(
        self,
        payload: dict[str, Any],
        result_payload: dict[str, Any],
        metrics_payload: dict[str, Any],
    ) -> dict[str, int]:
        return {
            "elapsedMs": self.first_int(
                metrics_payload,
                result_payload,
                payload,
                field="elapsedMs",
            ),
            "stdoutBytes": self.first_int(
                metrics_payload,
                result_payload,
                payload,
                field="stdoutBytes",
            ),
            "stderrBytes": self.first_int(
                metrics_payload,
                result_payload,
                payload,
                field="stderrBytes",
            ),
        }

    def first_int(self, *mappings: dict[str, Any], field: str) -> int:
        for mapping in mappings:
            value = optional_int(mapping.get(field))
            if value is not None:
                return max(0, value)
        return 0

    def first_str(self, *mappings: dict[str, Any], field: str) -> str:
        for mapping in mappings:
            value = mapping.get(field)
            if isinstance(value, str) and value:
                return value
        return ""

    def infer_kind(self, argv: list[str]) -> str:
        if "search" in argv:
            return "search"
        if "check" in argv:
            return "check"
        return "other"

    def infer_output_mode(self, argv: list[str]) -> str:
        if not argv or argv == ["unknown"]:
            return "unknown"
        return "json" if "--json" in argv else "compact"

    def copy_optional_string(
        self,
        source: dict[str, Any],
        target: dict[str, Any],
        field: str,
    ) -> None:
        value = source.get(field)
        if isinstance(value, str) and value:
            target[field] = value

    def json_payload(self, line: str) -> dict[str, Any] | None:
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            return None
        return payload if isinstance(payload, dict) else None

    def strip_prompt(self, line: str) -> str:
        if line.startswith("$ "):
            return line[2:].strip()
        return line

    def split_command_line(self, command_line: str) -> list[str]:
        try:
            argv = shlex.split(self.strip_prompt(command_line))
        except ValueError:
            return [self.strip_prompt(command_line)]
        return argv or ["unknown"]

    def safe_command_id(self, value: str, line_number: int) -> str:
        normalized = re.sub(r"[^A-Za-z0-9_.:-]+", "-", value).strip("-")
        return normalized or f"command-{line_number}"
