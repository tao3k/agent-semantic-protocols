# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Command catalog for the Python tooling entrypoint."""

from __future__ import annotations

from .command_spec import CommandSpec


COMMANDS: tuple[CommandSpec, ...] = (
    CommandSpec(
        ("schema", "profiles"),
        "tools.schema_profiles",
        "main",
        "argv",
        "Validate package-local copies of shared ASP schemas.",
    ),
    CommandSpec(
        ("sandtable",),
        "tools.semantic_sandtable.cli",
        "semantic_sandtable_main",
        "argv",
        "Run semantic sandtable scenarios and receipt checks.",
    ),
    CommandSpec(
        ("parser", "compact-snapshots"),
        "tools.parser_compact_snapshots",
        "main",
        "argv",
        "Print the retired root compact snapshot migration notice.",
    ),
    CommandSpec(
        ("codeql", "bounded-evidence"),
        "tools.codeql_bounded_evidence",
        "emit_codeql_bounded_evidence",
        "argv",
        "Emit bounded CodeQL evidence metadata for ASP flow fixtures.",
    ),
    CommandSpec(
        ("codeql", "evidence"),
        "tools.codeql_evidence",
        "emit_codeql_evidence",
        "argv",
        "Emit CodeQL CLI metadata as ASP evidence.",
    ),
    CommandSpec(
        ("cache", "validate", "julia-performance"),
        "tools.julia_cache_performance",
        "main",
        "argv",
        "Validate Julia cache miss/hit performance evidence.",
    ),
    CommandSpec(
        ("graph", "turbo", "benchmark"),
        "asp_python_graphs.benchmark_cli",
        "main",
        "argv",
        "Benchmark Graph-Turbo algorithm output for offline evidence only.",
    ),
    CommandSpec(
        ("graph", "turbo", "sandtable-summary"),
        "asp_python_graphs.sandtable_summary_cli",
        "main",
        "argv",
        "Summarize offline Graph-Turbo benchmark and receipt evidence.",
    ),
    CommandSpec(
        ("graph", "turbo", "ablation-report"),
        "asp_python_graphs.ablation_report_cli",
        "main",
        "argv",
        "Compare offline Graph-Turbo ablation variants for calibration.",
    ),
    CommandSpec(
        ("graph", "turbo", "agent-benefit"),
        "asp_python_graphs.agent_benefit_cli",
        "main",
        "argv",
        "Report offline Graph-Turbo reading, locator, and explanation evidence.",
    ),
    CommandSpec(
        ("graph", "turbo", "artifacts"),
        "asp_python_graphs.artifacts_cli",
        "main",
        "argv",
        "Evaluate offline Graph-Turbo output against cached ASP artifacts.",
    ),
    CommandSpec(
        ("graph", "turbo", "timeline"),
        "asp_python_graphs.timeline_cli",
        "main",
        "argv",
        "Infer timeline evidence from cached ASP artifacts; no Runtime authority.",
    ),
    CommandSpec(
        ("syntax", "real-evidence"),
        "tools.syntax_real_evidence",
        "main",
        "argv",
        "Render RFC 011 syntax real-project evidence records.",
    ),
    CommandSpec(
        ("validate", "provider-registry-contracts"),
        "tools.provider_registry_contracts",
        "main",
        "argv",
        "Validate real provider registry output against shared query contracts.",
    ),
)
