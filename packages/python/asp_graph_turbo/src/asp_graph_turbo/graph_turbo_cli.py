"""Explicit asp-graph-turbo command dispatcher."""

from __future__ import annotations

import importlib
import sys
from collections.abc import Callable, Sequence


_COMMANDS: dict[str, tuple[str, str, str]] = {
    "rank": (
        "asp_graph_turbo.cli",
        "main",
        "Rank a graph turbo request packet into compact frontier output.",
    ),
    "benchmark": (
        "asp_graph_turbo.benchmark_cli",
        "main",
        "Benchmark graph turbo ranking for sandtable evidence.",
    ),
    "ablate": (
        "asp_graph_turbo.ablation_cli",
        "main",
        "Generate graph turbo ablation packet variants for sandtable calibration.",
    ),
    "ablation-report": (
        "asp_graph_turbo.ablation_report_cli",
        "main",
        "Compare graph turbo ablation variants for ranking calibration.",
    ),
    "agent-benefit": (
        "asp_graph_turbo.agent_benefit_cli",
        "main",
        "Report whether graph turbo improves agent reading and locator behavior.",
    ),
    "artifacts": (
        "asp_graph_turbo.artifacts_cli",
        "main",
        "Evaluate graph turbo against cached ASP search artifacts.",
    ),
    "timeline": (
        "asp_graph_turbo.timeline_cli",
        "main",
        "Audit cached ASP artifacts as timeline, episode, and frontier actions.",
    ),
    "metrics": (
        "asp_graph_turbo.metrics_cli",
        "main",
        "Render real-trigger metrics for graph turbo RFC validation.",
    ),
    "receipt": (
        "asp_graph_turbo.frontier_receipt_cli",
        "main",
        "Capture a semantic fact frontier receipt from a graph turbo request.",
    ),
    "sandtable-summary": (
        "asp_graph_turbo.sandtable_summary_cli",
        "main",
        "Summarize benchmark and receipt packets for sandtable comparison.",
    ),
    "cache": (
        "asp_graph_turbo.cache_cli",
        "main",
        "Inspect, prune, or invalidate the graph turbo backend cache.",
    ),
    "feedback": (
        "asp_graph_turbo.feedback_cli",
        "main",
        "Build graph-turbo feedback packets from sandtable reports.",
    ),
    "calibrate": (
        "asp_graph_turbo.calibration_cli",
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
        sys.stderr.write(f"asp-graph-turbo: unknown command: {command}\n")
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
    output.write("usage: asp-graph-turbo <command> [args]\n\n")
    output.write("commands:\n")
    for name, (_, _, summary) in sorted(_COMMANDS.items()):
        output.write(f"  {name:<12} {summary}\n")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
