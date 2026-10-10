-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteProvenanceEvidenceExecutionConformance

namespace ASPProof.Audit.SearchRouteProvenanceEvidenceExecutionConformance

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String := []) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", Lean.toJson name),
    ("kind", Lean.toJson "theorem"),
    ("theoremFamily", Lean.toJson theoremFamily),
    ("type", Lean.toJson type),
    ("rfcClauseIds", Lean.toJson [rfcClauseId]),
    ("axioms", Lean.toJson axioms),
    ("hasSorryAx", Lean.toJson false)
  ]

def declarations : Array Lean.Json := #[
  theoremDeclaration
    "valid_executable_evidence_claim_binds_all_stages"
    "claim-stage-closure"
    "Executable evidence binds all five claim stages"
    "ASP-RFC-10.05-PEEC-CLAIM-STAGE-CLOSURE",
  theoremDeclaration
    "valid_executable_evidence_claim_binds_kind_policy_subject"
    "claim-target-binding"
    "Executable evidence binds claim kind, policy, and subject"
    "ASP-RFC-10.05-PEEC-CLAIM-KIND",
  theoremDeclaration
    "valid_executable_evidence_claim_binds_resolution"
    "claim-resolution"
    "Executable evidence binds claim-aware verifier resolution"
    "ASP-RFC-10.05-PEEC-RESOLUTION",
  theoremDeclaration
    "valid_executable_evidence_claim_binds_invocation_and_decode"
    "invocation-decode"
    "Executable evidence binds invocation and decoded response"
    "ASP-RFC-10.05-PEEC-INVOCATION-DECODE",
  theoremDeclaration
    "valid_executable_evidence_claim_has_distinct_replay_executor"
    "replay-independence"
    "Executable evidence requires a distinct replay executor"
    "ASP-RFC-10.05-PEEC-INDEPENDENT-REPLAY",
  theoremDeclaration
    "valid_executable_provenance_evidence_projects_structural_provenance"
    "structural-projection"
    "Execution-backed evidence projects to structural provenance"
    "ASP-RFC-10.05-PEEC-STRUCTURAL-PROJECTION",
  theoremDeclaration
    "valid_executable_provenance_evidence_binds_all_claim_targets"
    "claim-target-binding"
    "Execution-backed provenance binds all eight claim targets"
    "ASP-RFC-10.05-PEEC-SUBJECT",
  theoremDeclaration
    "valid_executable_provenance_evidence_binds_current_checkpoint"
    "checkpoint-binding"
    "Execution-backed evidence binds the current structural checkpoint"
    "ASP-RFC-10.05-PEEC-CHECKPOINT-BINDING",
  theoremDeclaration
    "valid_executable_provenance_evidence_has_monotone_fresh_checkpoint"
    "checkpoint-continuity"
    "Execution-backed provenance has a monotone fresh checkpoint"
    "ASP-RFC-10.05-PEEC-CHECKPOINT-CONTINUITY",
  theoremDeclaration
    "invalid_execution_backing_cannot_win_by_lower_cost"
    "feasibility-first"
    "Invalid execution backing cannot win through lower route cost"
    "ASP-RFC-10.05-PEEC-FEASIBILITY-FIRST",
  theoremDeclaration
    "example_executable_provenance_evidence_is_valid"
    "claim-stage-closure"
    "The complete example closes all executable provenance claims"
    "ASP-RFC-10.05-PEEC-CLAIM-STAGE-CLOSURE"
    ["propext"],
  theoremDeclaration
    "boolean_provenance_only_does_not_prove_executable_evidence"
    "boolean-prefix-insufficiency"
    "Structural Boolean provenance does not prove executable evidence"
    "ASP-RFC-10.05-PEEC-PREFIX-INSUFFICIENT"
    ["propext"],
  theoremDeclaration
    "subject_only_binding_allows_cross_kind_replay"
    "cross-kind-rejection"
    "Subject-only binding permits an invalid cross-kind replay"
    "ASP-RFC-10.05-PEEC-CLAIM-KIND"
    ["propext"],
  theoremDeclaration
    "cross_policy_claim_replay_is_rejected"
    "cross-policy-rejection"
    "A claim verified under another policy is rejected"
    "ASP-RFC-10.05-PEEC-POLICY"
    ["propext"],
  theoremDeclaration
    "same_executor_claim_replay_is_rejected"
    "replay-independence"
    "Same-executor claim replay is rejected"
    "ASP-RFC-10.05-PEEC-INDEPENDENT-REPLAY"
    ["propext"],
  theoremDeclaration
    "stale_checkpoint_execution_is_rejected"
    "checkpoint-continuity"
    "A checkpoint outside its freshness window is rejected"
    "ASP-RFC-10.05-PEEC-CHECKPOINT-CONTINUITY"
    ["propext"],
  theoremDeclaration
    "checkpoint_tree_rollback_is_rejected"
    "checkpoint-rollback-rejection"
    "A checkpoint tree rollback is rejected"
    "ASP-RFC-10.05-PEEC-CHECKPOINT-BINDING"
    ["propext"],
  theoremDeclaration
    "accepted_boolean_does_not_prove_executable_claim"
    "boolean-prefix-insufficiency"
    "An accepted Boolean with unresolved verifier is incomplete"
    "ASP-RFC-10.05-PEEC-PREFIX-INSUFFICIENT"
    ["propext"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson
      "ASPProof.SearchRouteProvenanceEvidenceExecutionConformance"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteProvenanceEvidenceExecutionConformance.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 10),
    ("axiomDependentDeclarationCount", Lean.toJson 8),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["propext"]),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson
      "01.19-searchroute-provenance-evidence-execution-conformance"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteProvenanceEvidenceExecutionConformance
