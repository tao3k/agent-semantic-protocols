"""Semantic admission for one MRR-defined Project Topology program."""

from __future__ import annotations

from collections.abc import Mapping

from jsonschema import Draft202012Validator
from jsonschema.exceptions import ValidationError


STANDARD_PROFILES = frozenset(
    {
        "mrr.topology.workspace.v1",
        "mrr.topology.engineering.v1",
        "mrr.topology.domain.v1",
        "mrr.topology.file.v1",
        "mrr.topology.structure.v1",
        "mrr.topology.reference.v1",
    }
)

REQUIRED_INPUT_EXCLUSIONS = frozenset(
    {".agents/asp/topology/**", ".cache/**", ".git/**"}
)


class TopologyProgramError(ValueError):
    """Typed semantic rejection for a topology program binding."""

    def __init__(self, reason_kind: str):
        super().__init__(reason_kind)
        self.reason_kind = reason_kind


def _require(condition: bool, reason_kind: str) -> None:
    if not condition:
        raise TopologyProgramError(reason_kind)


def _validate_schema(manifest: dict, schema: dict) -> None:
    try:
        Draft202012Validator.check_schema(schema)
        Draft202012Validator(schema).validate(manifest)
    except ValidationError as exc:
        raise TopologyProgramError("topology-program-schema-invalid") from exc


def _validate_profiles(manifest: dict) -> None:
    profiles = set(manifest["profiles"])
    _require(
        STANDARD_PROFILES <= profiles,
        "topology-standard-profile-missing",
    )


def _validate_program_modules(manifest: dict) -> None:
    profiles = set(manifest["profiles"])
    module_ids: set[str] = set()
    signatures: dict[tuple[str, str], int] = {}
    for module in manifest["program"]["modules"]:
        module_id = module["id"]
        _require(module_id not in module_ids, "topology-program-module-duplicate")
        module_ids.add(module_id)
        _require(module_id in profiles, "topology-program-module-profile-unresolved")
        owner = module["ownerNamespace"]
        if module["operation"] in {"refine", "replace"}:
            _require(
                not owner.startswith("mrr.topology"),
                "topology-program-replacement-owner-mismatch",
            )
        for imported in module["imports"]:
            _require(imported in profiles, "topology-program-import-unresolved")
        for signature in module["relationSignatures"]:
            _require(
                signature["namespace"] == owner,
                "topology-program-relation-owner-mismatch",
            )
            key = (signature["namespace"], signature["name"])
            _require(key not in signatures, "topology-program-relation-conflict")
            signatures[key] = signature["arity"]


def _validate_compilation_receipt(
    manifest: dict, admitted_receipts: Mapping[str, dict]
) -> None:
    binding = manifest["binding"]
    receipt = manifest["program"]["compilationReceipt"]
    _require(
        admitted_receipts.get(receipt["id"]) == receipt,
        "topology-program-compilation-receipt-mismatch",
    )
    _require(
        receipt["schemeProgramDigest"] == binding["schemeProgramDigest"]
        and receipt["compiledProgramAbiDigest"]
        == binding["compiledProgramAbiDigest"]
        and receipt["mrrBundleIdentity"] == binding["mrrBundleIdentity"],
        "topology-program-compilation-binding-mismatch",
    )


def _validate_materialization(manifest: dict) -> None:
    materialization = manifest["materialization"]
    _require(
        REQUIRED_INPUT_EXCLUSIONS <= set(materialization["excludedInputs"]),
        "topology-self-indexing-not-excluded",
    )
    root = materialization["root"] + "/"
    for field in ("programPaths", "factSegmentPaths", "annotationPaths"):
        _require(
            all(path.startswith(root) for path in materialization[field]),
            "topology-materialization-path-outside-root",
        )


def validate_project_topology_program(
    manifest: dict,
    schema: dict,
    admitted_receipts: Mapping[str, dict],
) -> None:
    """Validate one compiled MRR topology program and its materialization binding."""

    _validate_schema(manifest, schema)
    _validate_profiles(manifest)
    _validate_program_modules(manifest)
    _validate_compilation_receipt(manifest, admitted_receipts)
    _validate_materialization(manifest)
