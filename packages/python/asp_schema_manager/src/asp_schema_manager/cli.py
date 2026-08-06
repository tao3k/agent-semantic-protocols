"""Render deterministic ASP Schema Manager audit and gate commands."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Sequence, TextIO

from .audit import audit_workspace


COMMANDS = ("audit", "references", "families", "rust-usage", "lifecycle", "check")


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="asp-schema-manager", description="Audit and manage the ASP JSON Schema registry.")
    parser.add_argument("command", choices=COMMANDS, nargs="?", default="audit")
    parser.add_argument("--workspace-root", type=Path, default=Path.cwd())
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--family-registry", type=Path)
    parser.add_argument("--minimum-reference-bytes", type=int, default=120)
    parser.add_argument("--reference-limit", type=int, default=200)
    parser.add_argument("--json", action="store_true")
    return parser


def _projection(report: dict[str, Any], command: str) -> Any:
    if command == "references":
        return {"summary": report["summary"], "referenceOpportunities": report["referenceOpportunities"]}
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
                if item["lifecycleStatus"] != "active" or item["lifecycleSource"] == "manifest"
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
    manifest = args.manifest
    if manifest is not None and not manifest.is_absolute():
        manifest = args.workspace_root / manifest
    family_registry = args.family_registry
    if family_registry is not None and not family_registry.is_absolute():
        family_registry = args.workspace_root / family_registry
    report = audit_workspace(
        args.workspace_root,
        manifest_path=manifest,
        family_registry_path=family_registry,
        minimum_reference_bytes=args.minimum_reference_bytes,
        reference_limit=args.reference_limit,
    )
    projection = _projection(report, args.command)
    output = stdout or sys.stdout
    rendered = (
        json.dumps(projection, indent=2, sort_keys=True, ensure_ascii=False)
        if args.json
        else _render_text(report, args.command)
    )
    output.write(rendered + "\n")
    if args.command == "check" and any(item["severity"] == "error" for item in report["diagnostics"]):
        return 1
    return 0
