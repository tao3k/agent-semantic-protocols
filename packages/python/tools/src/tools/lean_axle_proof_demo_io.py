"""IO and rendering helpers for the AXLE proof-obligation demo."""

from __future__ import annotations

import json
import subprocess
from importlib.resources import files
from pathlib import Path
from typing import Any, Iterable


PROOF_RESOURCE_PACKAGE = "tools.proofs.search_packet_selector_identity"


def detect_local_lean_environment() -> str | None:
    try:
        result = subprocess.run(
            ["lean", "--version"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None
    parts = result.stdout.split()
    if len(parts) >= 3 and parts[0] == "Lean" and parts[1] == "(version":
        return f"lean-{parts[2].rstrip(',')}"
    return None


def load_proof_artifacts() -> tuple[str, str]:
    proof_dir = files(PROOF_RESOURCE_PACKAGE)
    statement = proof_dir.joinpath("formal_statement.lean").read_text(encoding="utf-8")
    candidate = proof_dir.joinpath("candidate_proof.lean").read_text(encoding="utf-8")
    return statement, candidate


def materialize_proof_inputs(
    out_dir: Path,
    projection_formal_lean: str,
    projection_candidate_lean: str,
    extra_formal_lean: Iterable[str] = (),
    extra_candidate_lean: Iterable[str] = (),
) -> tuple[str, str, Path, Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    base_formal_statement, base_candidate_proof = load_proof_artifacts()
    formal_statement = "\n\n".join(
        [projection_formal_lean, *extra_formal_lean, base_formal_statement]
    )
    candidate_proof = "\n\n".join(
        [projection_candidate_lean, *extra_candidate_lean, base_candidate_proof]
    )
    statement_path = out_dir / "formal-statement.lean"
    proof_path = out_dir / "candidate-proof.lean"
    statement_path.write_text(formal_statement, encoding="utf-8")
    proof_path.write_text(candidate_proof, encoding="utf-8")
    return formal_statement, candidate_proof, statement_path, proof_path


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_planning_artifacts(out_dir: Path, obligation: dict[str, Any], recipe: dict[str, Any]) -> None:
    write_json(out_dir / "proof-obligation.json", obligation)
    write_json(out_dir / "proof-recipe.json", recipe)


def write_schema_projection(
    out_dir: Path,
    projection: dict[str, Any],
    formal_lean: str,
    candidate_lean: str,
) -> None:
    write_json(out_dir / "schema-projection.json", projection)
    (out_dir / "schema-projection-formal.lean").write_text(formal_lean, encoding="utf-8")
    (out_dir / "schema-projection-candidate.lean").write_text(candidate_lean, encoding="utf-8")


def write_packet_projection(
    out_dir: Path,
    projection: dict[str, Any],
    formal_lean: str,
    candidate_lean: str,
) -> None:
    write_json(out_dir / "packet-projection.json", projection)
    (out_dir / "packet-projection-formal.lean").write_text(formal_lean, encoding="utf-8")
    (out_dir / "packet-projection-candidate.lean").write_text(candidate_lean, encoding="utf-8")


def write_verification_artifacts(
    out_dir: Path,
    receipt: dict[str, Any],
    report: dict[str, Any],
    response_json: dict[str, Any],
) -> None:
    write_json(out_dir / "axle-response.json", response_json)
    write_json(out_dir / "proof-receipt.json", receipt)
    write_json(out_dir / "verification-report.json", report)
    (out_dir / "how-frame.org").write_text(render_how_frame(receipt), encoding="utf-8")


def render_how_frame(receipt: dict[str, Any]) -> str:
    claims = receipt["validatedClaims"]
    assessment = receipt["defensiveEngineeringAssessment"]
    claim_lines = "\n".join(f"- {claim['id']} :: {claim['meaning']}" for claim in claims)
    return f"""\
#+TITLE: AXLE Proof Obligation Demo HowFrame

* Decision

{receipt["summaryForAgent"]}

* Verified Claims

{claim_lines}

* Defensive Engineering Assessment

- result :: {assessment["result"]}
- blockedBranch :: {assessment["blockedBranch"]}
- whyBlocked :: {assessment["whyBlocked"]}
- correctBoundary :: {assessment["correctBoundary"]}

* Branch Legality

- illegal :: {", ".join(receipt["branchLegalityUpdate"]["illegal"]) or "none"}
- legal :: {", ".join(receipt["branchLegalityUpdate"]["legal"]) or "none"}

* Receipt

- id :: {receipt["receiptId"]}
- checker :: {receipt["checker"]}
- environment :: {receipt["environment"]}
- okay :: {str(receipt["okay"]).lower()}
- failedDeclarations :: {receipt["failedDeclarations"]}
"""
