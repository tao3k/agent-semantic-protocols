# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Offline Graph-Turbo evidence dispatcher.

Runtime ordering is not exposed here.  The ASP Server's private gRPC endpoint
is the only Runtime authority; this dispatcher remains solely for offline
benchmark, calibration, and receipt analysis.
"""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable, Sequence


_COMMANDS: dict[str, tuple[str, str, str]] = {
    "benchmark": (
        "asp_python_graphs.benchmark_cli",
        "main",
        "Benchmark Graph-Turbo ordering for offline sandtable evidence.",
    ),
    "ablate": (
        "asp_python_graphs.ablation_cli",
        "main",
        "Generate graph turbo ablation packet variants for sandtable calibration.",
    ),
    "ablation-report": (
        "asp_python_graphs.ablation_report_cli",
        "main",
        "Compare Graph-Turbo ablation variants for offline calibration.",
    ),
    "agent-benefit": (
        "asp_python_graphs.agent_benefit_cli",
        "main",
        "Report whether graph turbo improves agent reading and locator behavior.",
    ),
    "artifacts": (
        "asp_python_graphs.artifacts_cli",
        "main",
        "Evaluate graph turbo against cached ASP search artifacts.",
    ),
    "timeline": (
        "asp_python_graphs.timeline_cli",
        "main",
        "Audit cached ASP artifacts as timeline, episode, and frontier actions.",
    ),
    "metrics": (
        "asp_python_graphs.metrics_cli",
        "main",
        "Render real-trigger metrics for graph turbo RFC validation.",
    ),
    "receipt": (
        "asp_python_graphs.frontier_receipt_cli",
        "main",
        "Capture a semantic fact frontier receipt from a graph turbo request.",
    ),
    "sandtable-summary": (
        "asp_python_graphs.sandtable_summary_cli",
        "main",
        "Summarize benchmark and receipt packets for sandtable comparison.",
    ),
    "cache": (
        "asp_python_graphs.cache_cli",
        "main",
        "Inspect, prune, or invalidate the graph turbo backend cache.",
    ),
    "feedback": (
        "asp_python_graphs.feedback_cli",
        "main",
        "Build graph-turbo feedback packets from sandtable reports.",
    ),
    "calibrate": (
        "asp_python_graphs.calibration_cli",
        "main",
        "Build profile calibration packets from graph-turbo feedback facts.",
    ),
}


def main(argv: Sequence[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args or args[0] in {"help", "--help", "-h"}:
        _print_help()
        return 0 if args else 2
    command = args[0]
    spec = _COMMANDS.get(command)
    if spec is None:
        sys.stderr.write(f"asp-python-graphs: unknown command: {command}\n")
        _print_help(file=sys.stderr)
        return 2
    module_name, function_name, _ = spec
    return int(_load_function(module_name, function_name)(args[1:]) or 0)


def _load_function(module_name: str, function_name: str) -> Callable[..., object]:
    module = importlib.import_module(module_name)
    function = getattr(module, function_name)
    if not callable(function):
        raise TypeError(f"{module_name}:{function_name} is not callable")
    return function


def _print_help(*, file: object | None = None) -> None:
    output = sys.stdout if file is None else file
    output.write("usage: asp-python-graphs <command> [args]\n\n")
    output.write("commands:\n")
    for name, (_, _, summary) in sorted(_COMMANDS.items()):
        output.write(f"  {name:<12} {summary}\n")
