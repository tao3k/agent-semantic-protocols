"""Render deterministic ASP Schema Manager audit and gate commands."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Sequence, TextIO

from ._proof_plan_cli import run_proof_plan
from .audit import audit_workspace


COMMANDS = (
    "audit",
    "references",
    "families",
    "rust-usage",
    "lifecycle",
    "proof-plan",
    "check",
)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="asp-schema-manager",
        description="Audit and manage the ASP JSON Schema registry.",
    )
    parser.add_argument("command", choices=COMMANDS, nargs="?", default="audit")
    parser.add_argument("--workspace-root", type=Path, default=Path.cwd())
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--family-registry", type=Path)
    parser.add_argument("--reference-decisions", type=Path)
    parser.add_argument("--minimum-reference-bytes", type=int, default=120)
    parser.add_argument("--reference-limit", type=int, default=200)
    parser.add_argument(
        "--schema",
        action="append",
        dest="schema_paths",
        help="repository-relative registered schema path; repeat to select several",
    )
    parser.add_argument("--expected-plan-digest")
    parser.add_argument("--fail-on-family-local-refs", action="store_true")
    parser.add_argument("--fail-on-unclassified-schemas", action="store_true")
    parser.add_argument("--fail-on-mixed-family-refs", action="store_true")
    parser.add_argument("--fail-on-reference-decision-drift", action="store_true")
    parser.add_argument("--json", action="store_true")
    return parser


def _projection(report: dict[str, Any], command: str) -> Any:
    if command == "proof-plan":
        return report
    if command == "references":
        return {
            "summary": report["summary"],
            "referenceOpportunities": report["referenceOpportunities"],
        }
    if command == "rust-usage":
        return {"summary": report["summary"], "schemas": report["schemas"]}
    if command == "families":
        return {
            "summary": report["summary"],
            "schemas": report["schemas"],
        }
    if command == "lifecycle":
        return {
            "summary": report["summary"],
            "schemas": [
                item
                for item in report["schemas"]
                if item["lifecycleStatus"] != "active"
                or item["lifecycleSource"] == "manifest"
            ],
        }
    return report


def _render_text(report: dict[str, Any], command: str) -> str:
    summary = report["summary"]
    lines = [
        "[asp-schema-manager] "
        f"schemas={summary['schemaCount']} refs={summary['schemaWithRefCount']} "
        f"bytes={summary['totalSchemaBytes']} "
        f"opportunities={summary['referenceOpportunityCount']} "
        f"reducibleBytes={summary['estimatedReducibleBytes']} "
        f"reductionBps={summary['estimatedReductionBasisPoints']} "
        f"rustDirect={summary['directRustCount']} rustTransitive={summary['transitiveRustCount']} "
        f"rustUnobserved={summary['unobservedRustCount']} removalCandidates={summary['removalCandidateCount']} "
        f"families={summary['familyCount']} classified={summary['classifiedSchemaCount']} "
        f"unclassified={summary['unclassifiedSchemaCount']} "
        f"familyLocalRefs={summary['referenceOpportunityScopeCounts']['family-local']} "
        f"crossFamilyRefs={summary['referenceOpportunityScopeCounts']['cross-family']} "
        f"mixedFamilyRefs={summary['referenceOpportunityScopeCounts']['mixed']} "
        f"unclassifiedRefs={summary['referenceOpportunityScopeCounts']['unclassified']}"
        f" unusedDefinitions={summary['unusedDefinitionCount']}"
    ]
    if command in {"audit", "check"}:
        for diagnostic in report["diagnostics"]:
            lines.append(
                f"|{diagnostic['severity']} code={diagnostic['code']} "
                f"schema={diagnostic.get('schemaPath', '-')} message={diagnostic['message']}"
            )
    elif command == "references":
        for item in report["referenceOpportunities"][:20]:
            lines.append(
                f"|ref-candidate occurrences={len(item['occurrences'])} "
                f"saving={item['estimatedReducibleBytes']} scope={item['familyScope']} "
                f"recommendation={item['recommendation']}"
            )
    elif command == "lifecycle":
        for item in report["schemas"]:
            if item["lifecycleStatus"] != "active":
                lines.append(
                    f"|lifecycle status={item['lifecycleStatus']} rust={item['rustUsage']} "
                    f"inbound={item['inboundReferenceCount']} schema={item['schemaPath']}"
                )
    elif command == "families":
        for item in report["schemas"]:
            lines.append(
                f"|family familyId={item['familyId'] or '-'} source={item['familySource']} "
                f"schema={item['schemaPath']}"
            )
    return "\n".join(lines)


def main(argv: Sequence[str] | None = None, *, stdout: TextIO | None = None) -> int:
    args = _parser().parse_args(argv)
    output = stdout or sys.stdout
    if args.fail_on_family_local_refs and args.command != "check":
        _parser().error("--fail-on-family-local-refs requires check command")
    if args.fail_on_unclassified_schemas and args.command != "check":
        _parser().error("--fail-on-unclassified-schemas requires check command")
    if args.fail_on_mixed_family_refs and args.command != "check":
        _parser().error("--fail-on-mixed-family-refs requires check command")
    if args.fail_on_reference_decision_drift and args.command != "check":
        _parser().error("--fail-on-reference-decision-drift requires check command")
    if args.schema_paths and args.command != "proof-plan":
        _parser().error("--schema requires proof-plan command")
    if args.expected_plan_digest and args.command != "proof-plan":
        _parser().error("--expected-plan-digest requires proof-plan command")
    if args.command == "proof-plan":
        return run_proof_plan(args, output)
    manifest = args.manifest
    if manifest is not None and not manifest.is_absolute():
        manifest = args.workspace_root / manifest
    family_registry = args.family_registry
    if family_registry is not None and not family_registry.is_absolute():
        family_registry = args.workspace_root / family_registry
    reference_decisions = args.reference_decisions
    if reference_decisions is not None and not reference_decisions.is_absolute():
        reference_decisions = args.workspace_root / reference_decisions
    report = audit_workspace(
        args.workspace_root,
        manifest_path=manifest,
        family_registry_path=family_registry,
        reference_decisions_path=reference_decisions,
        minimum_reference_bytes=args.minimum_reference_bytes,
        reference_limit=args.reference_limit,
    )
    family_local_count = report["summary"]["referenceOpportunityScopeCounts"][
        "family-local"
    ]
    if args.fail_on_family_local_refs and family_local_count:
        report["diagnostics"].append(
            {
                "severity": "error",
                "code": "family-local-reference-opportunity",
                "message": "family-local reference opportunities remain",
            "count": family_local_count,
            }
        )
    unclassified_count = report["summary"]["unclassifiedSchemaCount"]
    if args.fail_on_unclassified_schemas and unclassified_count:
        report["diagnostics"].append(
            {
                "severity": "error",
                "code": "unclassified-schema",
                "message": "unclassified schemas remain",
            "count": unclassified_count,
            }
        )
    mixed_family_count = report["summary"]["referenceOpportunityScopeCounts"]["mixed"]
    if args.fail_on_mixed_family_refs and mixed_family_count:
        report["diagnostics"].append(
            {
                "severity": "error",
                "code": "mixed-family-reference-opportunity",
                "message": "mixed-family reference opportunities remain",
            "count": mixed_family_count,
            }
        )
    reference_decision_drift_count = sum(
        item.get("code")
        in {"cross-family-reference-decision-missing", "stale-reference-decision"}
        for item in report["diagnostics"]
    )
    if args.fail_on_reference_decision_drift and reference_decision_drift_count:
        report["diagnostics"].append(
            {
                "severity": "error",
                "code": "reference-decision-drift",
                "message": "cross-family reference decision registry has drifted",
            "count": reference_decision_drift_count,
            }
        )
    projection = _projection(report, args.command)
    rendered = (
        json.dumps(projection, indent=2, sort_keys=True, ensure_ascii=False)
        if args.json
        else _render_text(report, args.command)
    )
    output.write(rendered + "\n")
    if args.command == "check" and any(
        item["severity"] == "error" for item in report["diagnostics"]
    ):
        return 1
    return 0
