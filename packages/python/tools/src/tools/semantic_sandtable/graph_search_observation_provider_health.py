"""Provider health inference for graph search observation packets."""

from __future__ import annotations

import os
from typing import Any

from tools.semantic_sandtable.graph_search_observation_contract import (
    _drop_none,
    _safe_scalar,
    _string_or_none,
)
from tools.semantic_sandtable.graph_search_observation_values import (
    _command_text,
    _string_values,
)


def _provider_health(
    scenario: dict[str, Any],
    steps: list[dict[str, Any]],
    usage_records: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    language = _safe_scalar(scenario.get("language") or "unknown")
    failure_kind = _infer_failure_kind(scenario, steps)
    binary_ref = _infer_binary_ref(steps)
    if failure_kind:
        health = {
            "language": language,
            "providerId": _infer_provider_id(language, usage_records),
            "binaryRef": binary_ref,
            "status": "failed",
            "failureKind": failure_kind,
            "message": _failure_message(failure_kind),
        }
        return [_drop_none(health)]
    if usage_records:
        health = {
            "language": language,
            "providerId": _infer_provider_id(language, usage_records),
            "binaryRef": binary_ref,
            "status": "ok",
        }
        return [_drop_none(health)]
    return [_drop_none({"language": language, "binaryRef": binary_ref, "status": "unknown"})]


def _infer_provider_id(language: str, usage_records: list[dict[str, Any]]) -> str | None:
    if usage_records:
        provider = _string_or_none(usage_records[0].get("provider"))
        if provider:
            return provider
    if language and language != "unknown":
        return f"{language}-provider"
    return None


def _infer_failure_kind(scenario: dict[str, Any], steps: list[dict[str, Any]]) -> str | None:
    text = "\n".join(_string_values({"scenario": scenario, "steps": steps})).lower()
    if "libjulia" in text or "library not loaded" in text:
        return "dynamic-library-rpath"
    if "no module named" in text and "python_lang_project_harness" in text:
        return "python-provider-module-missing"
    if "absolute path" in text:
        return "absolute-path-contract"
    if _any_failed(scenario, steps):
        return "command-failed"
    return None


def _failure_message(failure_kind: str) -> str:
    messages = {
        "dynamic-library-rpath": "provider runtime cannot resolve its dynamic library path",
        "python-provider-module-missing": "python provider module is not importable from the active binary environment",
        "absolute-path-contract": "report attempted to carry host-local absolute path data",
        "command-failed": "provider command failed before a route observation could complete",
    }
    return messages.get(failure_kind, failure_kind)


def _first_provider_failure_kind(provider_health: list[dict[str, Any]]) -> str | None:
    for item in provider_health:
        failure_kind = item.get("failureKind")
        if isinstance(failure_kind, str):
            return failure_kind
    return None


def _scenario_status(scenario: dict[str, Any], provider_health: list[dict[str, Any]]) -> str:
    status = _string_or_none(scenario.get("status"))
    if status:
        return status
    if any(item.get("status") == "failed" for item in provider_health):
        return "fail"
    if any(item.get("status") == "ok" for item in provider_health):
        return "pass"
    return "unknown"


def _infer_binary_ref(steps: list[dict[str, Any]]) -> dict[str, str] | None:
    for step in steps:
        command = step.get("command")
        tokens = command if isinstance(command, list) else _command_text(command).split()
        ref = _first_binary_ref(tokens)
        if ref:
            return ref
    return None


def _first_binary_ref(tokens: Any) -> dict[str, str] | None:
    for token in tokens:
        if not isinstance(token, str):
            continue
        name = os.path.basename(token)
        if name.startswith("asp-") or name in {"asp", "py-harness"}:
            if ".local/bin" in token:
                return {"kind": "home-local-bin", "value": name}
            if "/.bin/" in token or token.startswith(".bin/"):
                return {"kind": "workspace-bin", "value": name}
            return {"kind": "external", "value": name}
    return None


def _any_failed(scenario: dict[str, Any], steps: list[dict[str, Any]]) -> bool:
    status = _string_or_none(scenario.get("status"))
    if status in {"fail", "failed", "error"}:
        return True
    for step in steps:
        step_status = _string_or_none(step.get("status"))
        exit_code = step.get("exitCode")
        if step_status in {"fail", "failed", "error"}:
            return True
        if isinstance(exit_code, int) and exit_code != 0:
            return True
    return False
