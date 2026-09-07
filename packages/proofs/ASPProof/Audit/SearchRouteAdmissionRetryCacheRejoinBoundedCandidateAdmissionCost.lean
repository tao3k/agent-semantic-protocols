-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

def targets : List Target := [
  Target.mk
    ``authorized_admission_stays_within_capacity
    "global-admission-capacity-bound"
    ["BCAC-ADMISSION", "BCAC-CAPACITY"],
  Target.mk
    ``full_round_blocks_admission
    "full-round-admission-rejection"
    ["BCAC-CAPACITY", "BCAC-TERMINAL"],
  Target.mk
    ``authorized_principal_admission_stays_within_quota
    "principal-quota-bound"
    ["BCAC-PRINCIPAL", "BCAC-QUOTA"],
  Target.mk
    ``authorized_registration_processing_stays_within_capacity
    "registration-processing-capacity-bound"
    ["BCAC-REGISTRATION", "BCAC-CAPACITY"],
  Target.mk
    ``full_registration_processing_budget_blocks_work
    "registration-processing-terminal-bound"
    ["BCAC-REGISTRATION", "BCAC-TERMINAL"],
  Target.mk
    ``per_principal_quota_alone_does_not_bound_global_count
    "quota-without-capacity-counterexample"
    ["BCAC-COUNTEREXAMPLE", "BCAC-CAPACITY"],
  Target.mk
    ``comparison_count_is_bounded_by_candidate_count
    "comparison-candidate-bound"
    ["BCAC-COMPARISON", "BCAC-PROOF-SIZE"],
  Target.mk
    ``comparison_count_is_bounded_by_round_capacity
    "comparison-capacity-bound"
    ["BCAC-COMPARISON", "BCAC-CAPACITY"],
  Target.mk
    ``round_proof_cost_is_capacity_bounded
    "round-proof-cost-bound"
    ["BCAC-COST", "BCAC-ROUND"],
  Target.mk
    ``recovery_proof_cost_is_attempt_and_capacity_bounded
    "end-to-end-recovery-proof-cost-bound"
    ["BCAC-COST", "BCAC-ATTEMPT"],
  Target.mk
    ``round_search_cost_is_dual_capacity_bounded
    "round-search-dual-capacity-bound"
    ["BCAC-COST", "BCAC-REGISTRATION"],
  Target.mk
    ``recovery_search_cost_is_globally_bounded
    "end-to-end-recovery-search-cost-bound"
    ["BCAC-COST", "BCAC-ATTEMPT", "BCAC-CAPACITY"],
  Target.mk
    ``admission_round_compatibility_binds_capacity
    "round-capacity-identity-binding"
    ["BCAC-IDENTITY", "BCAC-CAPACITY"],
  Target.mk
    ``changed_capacity_invalidates_admission_round
    "capacity-cache-invalidation"
    ["BCAC-CACHE", "BCAC-CAPACITY"],
  Target.mk
    ``same_admitted_count_does_not_imply_round_compatibility
    "same-count-round-separation"
    ["BCAC-COUNTEREXAMPLE", "BCAC-IDENTITY"],
  Target.mk
    ``changed_candidate_set_invalidates_admission_round
    "candidate-set-cache-invalidation"
    ["BCAC-CACHE", "BCAC-CANDIDATES"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost
