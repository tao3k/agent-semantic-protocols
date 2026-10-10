# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Contract checks for the agent hook interception RFC."""

from pathlib import Path


_REPO_ROOT = Path(__file__).resolve().parents[2]
_RFC_PATH = (
    _REPO_ROOT
    / "docs"
    / "10-19-rfcs"
    / "10.15-agent-hook-interception-protocol.org"
)


def test_hook_rfc_keeps_source_enforcement_inside_compiled_policy() -> None:
    text = " ".join(_RFC_PATH.read_text(encoding="utf-8").split())

    required_terms = [
        "compiled policy + provider projection",
        "without contacting the Runtime Server",
        "Semantic capability MUST NOT be copied from the Host tool name",
        "An unobserved Host event produces no Action IR and no decision receipt",
        "source access remains fail-closed",
        "performs no Runtime RPC, provider execution, socket discovery",
    ]
    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_hook_rfc_defines_standalone_runtime_and_typed_decision() -> None:
    text = " ".join(_RFC_PATH.read_text(encoding="utf-8").split())

    required_terms = [
        "asp-hook pre-tool --client codex",
        "asp-hook pre-tool --client claude",
        "=asp-hook-exec= adapter resolves the active Runtime artifact slot",
        "=runtime/bin/asp-hook=",
        "MUST NOT insert an =asp hook= subcommand",
        "=--emit decision= exposes the typed packet",
        "Rendered prose is not an orchestration protocol",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []


def test_hook_rfc_closure_gates_cover_compiled_policy_and_retired_routes() -> None:
    text = _RFC_PATH.read_text(encoding="utf-8")

    required_terms = [
        "The prior =HookRoutes=, =HookRouteBindings=, =HookActivation=",
        "remain immutable archive contracts",
        "MUST NOT be used to reconstruct a compatibility path",
        "dependency gates prove Runtime Server and Client Core do not depend on",
        "source gates reject reintroduction of route-bearing Hook activation DTOs",
        "latency evidence proves every covered decision is strictly below 1 ms",
        "Zero tests, an unregistered test file, stale binary evidence",
    ]

    missing_terms = [term for term in required_terms if term not in text]

    assert missing_terms == []
