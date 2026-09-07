# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Render and execute the Schema Manager proof-plan command."""

from __future__ import annotations

import argparse
import json
from typing import Any, TextIO

from .logical_projection import generate_proof_plan


def render_proof_plan_text(plan: dict[str, Any]) -> str:
    lines = [
        "[asp-schema-proof-plan] "
        f"schemas={len(plan['schemas'])} state={plan['validity']['state']} "
        f"planDigest={plan['planDigest']}"
    ]
    for item in plan["schemas"]:
        keywords = item["keywordObligations"]
        lines.append(
            f"|schema family={item['family']['familyId'] or '-'} "
            f"lifecycle={item['lifecycle']['status']} "
            f"supported={len(keywords['supported'])} "
            f"unsupported={len(keywords['unsupported'])} "
            f"closure={len(item['resolvedReferenceClosure'])} "
            f"projectionDigest={item['projectionDigest']} "
            f"path={item['sourceSchema']}"
        )
    return "\n".join(lines)


def run_proof_plan(args: argparse.Namespace, output: TextIO) -> int:
    try:
        plan = generate_proof_plan(
            args.workspace_root,
            schema_paths=args.schema_paths,
            expected_plan_digest=args.expected_plan_digest,
        )
    except ValueError as error:
        return _render_invalid_request(args, output, error)
    rendered = (
        json.dumps(plan, indent=2, sort_keys=True, ensure_ascii=False)
        if args.json
        else render_proof_plan_text(plan)
    )
    output.write(rendered + "\n")
    return 1 if plan["validity"]["state"] == "stale" else 0


def _render_invalid_request(
    args: argparse.Namespace, output: TextIO, error: ValueError
) -> int:
    failure = {
        "projectionKind": "schema-logical-proof-plan-failure",
        "projectionVersion": "1",
        "state": "invalid-request",
        "reasonKind": "registered-schema-selection-invalid",
        "message": str(error),
    }
    rendered = (
        json.dumps(failure, indent=2, sort_keys=True, ensure_ascii=False)
        if args.json
        else (
            "[asp-schema-proof-plan] state=invalid-request "
            f"reasonKind={failure['reasonKind']} message={error}"
        )
    )
    output.write(rendered + "\n")
    return 2
