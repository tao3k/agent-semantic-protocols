-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteCost
import ASPProof.SearchRouteDAG
import ASPProof.SearchRouteDAGEnumeration
import ASPProof.SearchRouteBranchBound
import ASPProof.SearchRouteInspectScheduler
import ASPProof.SearchRouteInspectLoop
import ASPProof.SearchRouteInspectDriver
import ASPProof.SearchRouteInspectTraceCost
import ASPProof.SearchLoopCacheRelocation
import ASPProof.SearchLoopCacheDiagnostics
import ASPProof.SearchLoopCacheIdentity
import ASPProof.Audit.ActivationAdmission
import ASPProof.Audit.RuntimeSelectorOverlay
import ASPProof.Audit.SearchEvidenceDerivation
import ASPProof.Audit.ProjectTopologyProgram
import ASPProof.Audit.ProjectTopologyIdentityRefinement
import ASPProof.Audit.RuntimeProjectTopologyAttachment
import ASPProof.Audit.EnhancedSyntaxQueryPlan
import Lean.Elab.Command

namespace ASPProof.Audit

open Lean Elab Command Meta

structure AuditTarget where
  name : Name
  theoremFamily : String
  rfcClauseIds : List String

private def traceSafetyClauses : List String :=
  [ "ASP-RFC-10.05-CFR-GOD-DECISION"
  , "ASP-RFC-10.05-RPGE-CLOSED-WORLD"
  ]

private def progressClauses : List String :=
  [ "ASP-RFC-10.05-CFR-GOD-GLOBAL-BUDGET"
  , "ASP-RFC-10.05-BECA-STATES"
  ]

def searchLoopTraceTargets : List AuditTarget :=
  [ { name := ``SearchLoopTrace.step_preserves_domain
      theoremFamily := "trace-safety"
      rfcClauseIds := traceSafetyClauses }
  , { name := ``SearchLoopTrace.trace_preserves_domain
      theoremFamily := "trace-safety"
      rfcClauseIds := traceSafetyClauses }
  , { name := ``SearchLoopTrace.step_preserves_closure_safety
      theoremFamily := "trace-safety"
      rfcClauseIds := traceSafetyClauses }
  , { name := ``SearchLoopTrace.trace_preserves_closure_safety
      theoremFamily := "trace-safety"
      rfcClauseIds := traceSafetyClauses }
  , { name := ``SearchLoopTrace.productive_step_exactly_reduces_work
      theoremFamily := "bounded-progress"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.productive_step_decreases_work
      theoremFamily := "bounded-progress"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.productive_trace_conserves_work
      theoremFamily := "bounded-progress"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.productive_trace_is_bounded
      theoremFamily := "bounded-progress"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.step_decreases_work_or_typed_terminal
      theoremFamily := "typed-progress"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.zero_mandatory_does_not_imply_safe_closure
      theoremFamily := "counterexample"
      rfcClauseIds := traceSafetyClauses }
  , { name := ``SearchLoopTrace.mandatory_only_budget_undercounts_unresolved_work
      theoremFamily := "counterexample"
      rfcClauseIds := progressClauses }
  , { name := ``SearchLoopTrace.typed_terminalization_need_not_decrease_work
      theoremFamily := "counterexample"
      rfcClauseIds := progressClauses }
  ]

private def mergeAlgebraClauses : List String :=
  [ "ASP-RFC-10.05-SMA-OUTCOME-JOIN"
  , "ASP-RFC-10.05-SMA-CONFLUENCE"
  ]

private def mergeAdmissionClauses : List String :=
  [ "ASP-RFC-10.05-SMA-ADMISSION"
  , "ASP-RFC-10.05-SMA-TYPED-CONFLICT"
  ]

def searchLoopMergeTargets : List AuditTarget :=
  [ { name := ``SearchLoopMerge.joinOutcome_commutative
      theoremFamily := "merge-algebra"
      rfcClauseIds := mergeAlgebraClauses }
  , { name := ``SearchLoopMerge.joinOutcome_associative
      theoremFamily := "merge-algebra"
      rfcClauseIds := mergeAlgebraClauses }
  , { name := ``SearchLoopMerge.joinOutcome_idempotent
      theoremFamily := "merge-algebra"
      rfcClauseIds := mergeAlgebraClauses }
  , { name := ``SearchLoopMerge.admitted_merges_commute
      theoremFamily := "merge-confluence"
      rfcClauseIds := mergeAlgebraClauses ++ mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.admitted_merge_is_idempotent
      theoremFamily := "merge-confluence"
      rfcClauseIds := mergeAlgebraClauses ++ mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.mergeMany_permutation_invariant
      theoremFamily := "merge-confluence"
      rfcClauseIds := mergeAlgebraClauses ++ mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.independent_writes_commute
      theoremFamily := "ledger-independence"
      rfcClauseIds := mergeAlgebraClauses }
  , { name := ``SearchLoopMerge.mergePair_commutative
      theoremFamily := "merge-algebra"
      rfcClauseIds := mergeAlgebraClauses ++ mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.mergePair_idempotent
      theoremFamily := "merge-algebra"
      rfcClauseIds := mergeAlgebraClauses ++ mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.domain_mismatch_is_typed_conflict
      theoremFamily := "typed-conflict"
      rfcClauseIds := mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.clause_mismatch_is_typed_conflict
      theoremFamily := "typed-conflict"
      rfcClauseIds := mergeAdmissionClauses }
  , { name := ``SearchLoopMerge.last_write_wins_is_order_dependent
      theoremFamily := "counterexample"
      rfcClauseIds := mergeAlgebraClauses }
  , { name := ``SearchLoopMerge.conflicting_outcomes_join_to_contradiction
      theoremFamily := "counterexample"
      rfcClauseIds := mergeAlgebraClauses }
  ]

private def ledgerAxiomFamilies : List String :=
  [ "merge-confluence", "ledger-independence" ]

def searchLoopMergeAlgebraTargets : List AuditTarget :=
  searchLoopMergeTargets.filter fun target =>
    !ledgerAxiomFamilies.contains target.theoremFamily

def searchLoopMergeLedgerTargets : List AuditTarget :=
  searchLoopMergeTargets.filter fun target =>
    ledgerAxiomFamilies.contains target.theoremFamily

private def cacheIdentityClauses : List String :=
  [ "ASP-RFC-10.05-SCI-KEY-IDENTITY"
  , "ASP-RFC-10.05-SCI-TYPED-INVALIDATION"
  ]

private def cacheRelocationClauses : List String :=
  [ "ASP-RFC-10.05-SCI-SELECTOR-RELOCATION" ]

private def cacheTierClauses : List String :=
  [ "ASP-RFC-10.05-SCI-TIER-SCOPE" ]

def searchLoopCacheIdentityTargets : List AuditTarget :=
  [ { name := ``SearchLoopCacheIdentity.stable_semantic_key_reuses
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.semantic_reuse_iff_exact_key
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.semantic_key_drift_invalidates
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_provider_allows_false_reuse
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_provider_artifact_allows_false_reuse
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_policy_allows_false_reuse
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_rfc_allows_false_reuse
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.whole_generation_causes_false_invalidation
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.model_drift_preserves_semantic_reuse
      theoremFamily := "tier-scope"
      rfcClauseIds := cacheIdentityClauses ++ cacheTierClauses }
  , { name := ``SearchLoopCacheIdentity.model_drift_invalidates_l4
      theoremFamily := "tier-scope"
      rfcClauseIds := cacheIdentityClauses ++ cacheTierClauses }
  , { name := ``SearchLoopCacheIdentity.reconcile_drift_is_stale
      theoremFamily := "cache-transition"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheIdentity.reconciled_valid_implies_exact_key
      theoremFamily := "cache-transition"
      rfcClauseIds := cacheIdentityClauses }
  ]

def searchLoopCacheRelocationTargets : List AuditTarget :=
  [ { name := ``SearchLoopCacheRelocation.selector_drift_invalidates_without_relocation
      theoremFamily := "selector-relocation"
      rfcClauseIds := cacheIdentityClauses ++ cacheRelocationClauses }
  , { name := ``SearchLoopCacheRelocation.explicit_relocation_rekeys_to_reuse
      theoremFamily := "selector-relocation"
      rfcClauseIds := cacheIdentityClauses ++ cacheRelocationClauses }
  ]

def searchLoopCacheDiagnosticTargets : List AuditTarget :=
  [ { name := ``SearchLoopCacheDiagnostics.no_diagnostic_iff_exact_key
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheDiagnostics.diagnostic_implies_invalidate
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheDiagnostics.provider_artifact_drift_has_typed_reason
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCacheDiagnostics.policy_drift_has_typed_reason
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  ]

private def cacheDiagnosticComputationalNames : List Name :=
  [ ``SearchLoopCacheDiagnostics.provider_artifact_drift_has_typed_reason
  , ``SearchLoopCacheDiagnostics.policy_drift_has_typed_reason
  ]

def searchLoopCacheDiagnosticComputationalTargets : List AuditTarget :=
  searchLoopCacheDiagnosticTargets.filter fun target =>
    cacheDiagnosticComputationalNames.contains target.name

def searchLoopCacheDiagnosticEqualityTargets : List AuditTarget :=
  searchLoopCacheDiagnosticTargets.filter fun target =>
    !cacheDiagnosticComputationalNames.contains target.name

private def cacheComputationalNames : List Name :=
  [ ``SearchLoopCacheIdentity.omitting_provider_allows_false_reuse
  , ``SearchLoopCacheIdentity.omitting_provider_artifact_allows_false_reuse
  , ``SearchLoopCacheIdentity.omitting_policy_allows_false_reuse
  , ``SearchLoopCacheIdentity.omitting_rfc_allows_false_reuse
  , ``SearchLoopCacheIdentity.model_drift_preserves_semantic_reuse
  , ``SearchLoopCacheIdentity.model_drift_invalidates_l4
  ]

def searchLoopCacheComputationalTargets : List AuditTarget :=
  searchLoopCacheIdentityTargets.filter fun target =>
    cacheComputationalNames.contains target.name

def searchLoopCacheEqualityTargets : List AuditTarget :=
  searchLoopCacheIdentityTargets.filter fun target =>
    !cacheComputationalNames.contains target.name

private def cacheIdentityRefinementClauses : List String :=
  [ "ASP-RFC-10.05-SCI-EXACT-IDENTITY"
  , "ASP-RFC-10.05-SCI-DEPENDENCY-INVALIDATION"
  ]

private def cacheIdentityScopeRefinementClauses : List String :=
  [ "ASP-RFC-10.05-SCI-TIER-SCOPE"
  , "ASP-RFC-10.05-SCI-MINIMAL-WITNESS"
  ]

def searchLoopCacheIdentityRefinementTargets : List AuditTarget :=
  [ { name := ``SearchLoopCacheIdentity.semantic_reuse_iff_exact_key
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.stable_semantic_key_reuses
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.semantic_key_drift_invalidates
      theoremFamily := "invalidation-safety"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_provider_allows_false_reuse
      theoremFamily := "counterexample"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_policy_allows_false_reuse
      theoremFamily := "counterexample"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_provider_artifact_allows_false_reuse
      theoremFamily := "counterexample"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.omitting_rfc_allows_false_reuse
      theoremFamily := "counterexample"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.whole_generation_causes_false_invalidation
      theoremFamily := "counterexample"
      rfcClauseIds := cacheIdentityScopeRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.model_drift_invalidates_l4
      theoremFamily := "cache-tier-scope"
      rfcClauseIds := cacheIdentityScopeRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.model_drift_preserves_semantic_reuse
      theoremFamily := "cache-tier-scope"
      rfcClauseIds := cacheIdentityScopeRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.reconcile_drift_is_stale
      theoremFamily := "invalidation-safety"
      rfcClauseIds := cacheIdentityRefinementClauses }
  , { name := ``SearchLoopCacheIdentity.reconciled_valid_implies_exact_key
      theoremFamily := "invalidation-safety"
      rfcClauseIds := cacheIdentityRefinementClauses }
  ]

private def jsonStrings (values : List String) : Json :=
  Json.arr <| values.toArray.map Json.str

private def normalizedJsonStrings (values : List String) : Json :=
  jsonStrings (values.mergeSort.eraseDups)

private def auditTargetJson (target : AuditTarget) : TermElabM Json := do
  let info ← getConstInfo target.name
  let typeFormat ← ppExpr info.type
  let axioms ← collectAxioms target.name
  let axiomNames := axioms.toList.map Name.toString |>.mergeSort
  let hasSorryAx := axiomNames.any fun name => name.contains "sorryAx"
  pure <| Json.mkObj
    [ ("name", Json.str target.name.toString)
    , ("kind", Json.str "theorem")
    , ("theoremFamily", Json.str target.theoremFamily)
    , ("rfcClauseIds", normalizedJsonStrings target.rfcClauseIds)
    , ("type", Json.str typeFormat.pretty)
    , ("axioms", jsonStrings axiomNames)
    , ("hasSorryAx", Json.bool hasSorryAx)
    ]

-- RFC-owned theorem inventories live under `ASPProof.Audit.*`, not in this core.
def proofAuditJson
    (moduleName sourcePath : String)
    (targets : List AuditTarget) :
    TermElabM Json := do
  let declarations ← targets.mapM auditTargetJson
  let axiomFreeFlags ← targets.mapM fun target => do
    let axioms ← collectAxioms target.name
    pure axioms.isEmpty
  let axiomFreeCount := axiomFreeFlags.count true
  let axiomDependentCount := declarations.length - axiomFreeCount
  let allAxioms ← targets.foldlM (init := #[]) fun names target => do
    let axioms ← collectAxioms target.name
    pure <| names ++ axioms
  let axiomNames :=
    allAxioms.toList.map Name.toString |>.mergeSort |>.eraseDups
  pure <| Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str Lean.versionString)
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str moduleName)
    , ("sourcePath", Json.str sourcePath)
    , ("declarationCount", toJson declarations.length)
    , ("axiomFreeDeclarationCount", toJson axiomFreeCount)
    , ("axiomDependentDeclarationCount", toJson axiomDependentCount)
    , ("declarations", Json.arr declarations.toArray)
    , ("axiomInventory", jsonStrings axiomNames)
    , ("hasSorryAx", Json.bool <| axiomNames.any fun name => name.contains "sorryAx")
    ]

def searchLoopTraceAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopTrace"
    "packages/proofs/ASPProof/SearchLoopTrace.lean"
    searchLoopTraceTargets

def searchLoopMergeAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopMerge"
    "packages/proofs/ASPProof/SearchLoopMerge.lean"
    searchLoopMergeTargets

def searchLoopMergeAlgebraAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopMerge"
    "packages/proofs/ASPProof/SearchLoopMerge.lean"
    searchLoopMergeAlgebraTargets

def searchLoopMergeLedgerAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopMerge"
    "packages/proofs/ASPProof/SearchLoopMerge.lean"
    searchLoopMergeLedgerTargets

def searchLoopCacheAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheIdentity"
    "packages/proofs/ASPProof/SearchLoopCacheIdentity.lean"
    searchLoopCacheIdentityTargets

def searchLoopCacheComputationalAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheIdentity"
    "packages/proofs/ASPProof/SearchLoopCacheIdentity.lean"
    searchLoopCacheComputationalTargets

def searchLoopCacheEqualityAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheIdentity"
    "packages/proofs/ASPProof/SearchLoopCacheIdentity.lean"
    searchLoopCacheEqualityTargets

def searchLoopCacheRelocationAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheRelocation"
    "packages/proofs/ASPProof/SearchLoopCacheRelocation.lean"
    searchLoopCacheRelocationTargets

def searchLoopCacheDiagnosticAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheDiagnostics"
    "packages/proofs/ASPProof/SearchLoopCacheDiagnostics.lean"
    searchLoopCacheDiagnosticTargets

def searchLoopCacheDiagnosticComputationalAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheDiagnostics"
    "packages/proofs/ASPProof/SearchLoopCacheDiagnostics.lean"
    searchLoopCacheDiagnosticComputationalTargets

def searchLoopCacheDiagnosticEqualityAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheDiagnostics"
    "packages/proofs/ASPProof/SearchLoopCacheDiagnostics.lean"
    searchLoopCacheDiagnosticEqualityTargets

def searchLoopCacheIdentityRefinementAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchLoopCacheIdentity"
    "packages/proofs/ASPProof/SearchLoopCacheIdentity.lean"
    searchLoopCacheIdentityRefinementTargets

elab "#emit_searchloop_trace_audit" : command => do
  let audit ← liftTermElabM searchLoopTraceAuditJson
  IO.println audit.pretty

elab "#write_searchloop_trace_audit" : command => do
  let audit ← liftTermElabM searchLoopTraceAuditJson
  let path := "receipts/searchloop-trace-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#emit_searchloop_merge_audit" : command => do
  let audit ← liftTermElabM searchLoopMergeAuditJson
  IO.println audit.pretty

elab "#write_searchloop_merge_audit" : command => do
  let audit ← liftTermElabM searchLoopMergeAuditJson
  let path := "receipts/searchloop-merge-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_merge_algebra_audit" : command => do
  let audit ← liftTermElabM searchLoopMergeAlgebraAuditJson
  let path := "receipts/searchloop-merge-algebra-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_merge_ledger_audit" : command => do
  let audit ← liftTermElabM searchLoopMergeLedgerAuditJson
  let path := "receipts/searchloop-merge-ledger-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheAuditJson
  let path := "receipts/searchloop-cache-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_identity_refinement_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheIdentityRefinementAuditJson
  let path := "receipts/searchloop-cache-identity-refinement-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_computational_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheComputationalAuditJson
  let path := "receipts/searchloop-cache-computational-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_equality_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheEqualityAuditJson
  let path := "receipts/searchloop-cache-equality-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_relocation_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheRelocationAuditJson
  let path := "receipts/searchloop-cache-relocation-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_diagnostic_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheDiagnosticAuditJson
  let path := "receipts/searchloop-cache-diagnostic-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_diagnostic_computational_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheDiagnosticComputationalAuditJson
  let path := "receipts/searchloop-cache-diagnostic-computational-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

elab "#write_searchloop_cache_diagnostic_equality_audit" : command => do
  let audit ← liftTermElabM searchLoopCacheDiagnosticEqualityAuditJson
  let path := "receipts/searchloop-cache-diagnostic-equality-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteAdmissibilityClauses : List String :=
  [ "ASP-RFC-10.05-SRC-ADMISSIBLE-ROUTE"
  , "ASP-RFC-10.05-SRC-FAIL-CLOSED-CACHE"
  ]

private def searchRouteOptimizationClauses : List String :=
  [ "ASP-RFC-10.05-SRC-LEX-OPTIMAL"
  , "ASP-RFC-10.05-SRC-ROUTER-JUMP"
  , "ASP-RFC-10.05-SRC-ROUND-COMPRESSION"
  ]

private def searchRouteCacheClauses : List String :=
  [ "ASP-RFC-10.05-SRC-SEARCH-CACHE"
  , "ASP-RFC-10.05-SRC-MODEL-PREFIX-CACHE"
  , "ASP-RFC-10.05-SRC-TOKEN-COST"
  ]

private def searchRouteGraphClauses : List String :=
  [ "ASP-RFC-10.05-SRC-GRAPH-ESCALATION"
  , "ASP-RFC-10.05-SRC-TOKEN-BOUND"
  ]

def searchRouteCostTargets : List AuditTarget :=
  [ { name := `SearchRouteCost.lexNoWorse_trans
      theoremFamily := "route-order"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.preferRoute_lexNoWorse_left
      theoremFamily := "route-order"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.preferRoute_lexNoWorse_right
      theoremFamily := "route-order"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.chooseShortestFeasible_none_implies_infeasible
      theoremFamily := "route-optimizer"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.chooseShortestFeasible_mem_and_feasible
      theoremFamily := "route-optimizer"
      rfcClauseIds := searchRouteAdmissibilityClauses }
  , { name := `SearchRouteCost.chooseShortestFeasible_is_lex_optimal
      theoremFamily := "route-optimizer"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.router_jump_preserves_admissibility
      theoremFamily := "route-admissibility"
      rfcClauseIds := searchRouteAdmissibilityClauses }
  , { name := `SearchRouteCost.router_jump_strictly_dominates_baseline
      theoremFamily := "cost-dominance"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.router_jump_cost_receipt
      theoremFamily := "cost-receipt"
      rfcClauseIds := searchRouteOptimizationClauses }
  , { name := `SearchRouteCost.model_prefix_cache_reduces_uncached_tokens
      theoremFamily := "model-prefix-cache"
      rfcClauseIds := searchRouteCacheClauses }
  , { name := `SearchRouteCost.semantic_search_cache_reduces_route_tokens
      theoremFamily := "semantic-search-cache"
      rfcClauseIds := searchRouteCacheClauses }
  , { name := `SearchRouteCost.joint_cache_route_strictly_dominates_baseline
      theoremFamily := "joint-cache-dominance"
      rfcClauseIds := searchRouteCacheClauses }
  , { name := `SearchRouteCost.unconstrained_shortest_path_is_unsound
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteGraphClauses }
  , { name := `SearchRouteCost.full_graph_dump_violates_token_budget
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteGraphClauses }
  , { name := `SearchRouteCost.unvalidated_search_cache_is_rejected
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteAdmissibilityClauses }
  , { name := `SearchRouteCost.impossible_model_cache_claim_is_rejected
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteAdmissibilityClauses }
  , { name := `SearchRouteCost.bounded_router_selects_graph_route
      theoremFamily := "graph-routing"
      rfcClauseIds := searchRouteGraphClauses }
  , { name := `SearchRouteCost.joint_cache_route_is_selected
      theoremFamily := "route-optimizer"
      rfcClauseIds := searchRouteCacheClauses }
  , { name := `SearchRouteCost.router_jump_saves_two_rounds
      theoremFamily := "cost-receipt"
      rfcClauseIds := searchRouteOptimizationClauses }
  ]

def searchRouteCostAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteCost"
    "packages/proofs/ASPProof/SearchRouteCost.lean"
    searchRouteCostTargets

elab "#write_searchroute_cost_audit" : command => do
  let audit ← liftTermElabM searchRouteCostAuditJson
  let path : String := "receipts/searchroute-cost-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteDagStructureClauses : List String :=
  [ "ASP-RFC-10.05-SRD-SEPARATE-HOPS"
  , "ASP-RFC-10.05-SRD-RANKED-DAG"
  ]

private def searchRouteDagCompletenessClauses : List String :=
  [ "ASP-RFC-10.05-SRD-CANDIDATE-COMPLETE"
  , "ASP-RFC-10.05-SRD-GLOBAL-OPTIMALITY"
  ]

private def searchRouteDagValidationClauses : List String :=
  [ "ASP-RFC-10.05-SRD-PATH-VALIDATION"
  , "ASP-RFC-10.05-SRD-SEPARATE-HOPS"
  ]

def searchRouteDagTargets : List AuditTarget :=
  [ { name := `SearchRouteDAG.walk_rank_progress
      theoremFamily := "ranked-dag"
      rfcClauseIds := searchRouteDagStructureClauses }
  , { name := `SearchRouteDAG.ranked_dag_has_no_positive_cycle
      theoremFamily := "ranked-dag"
      rfcClauseIds := searchRouteDagStructureClauses }
  , { name := `SearchRouteDAG.walk_hops_are_bounded
      theoremFamily := "ranked-dag"
      rfcClauseIds := searchRouteDagStructureClauses }
  , { name := `SearchRouteDAG.complete_generation_lifts_local_to_global
      theoremFamily := "candidate-completeness"
      rfcClauseIds := searchRouteDagCompletenessClauses }
  , { name := `SearchRouteDAG.transition_minimization_can_choose_longer_graph_path
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteDagValidationClauses }
  , { name := `SearchRouteDAG.corrected_router_selects_shorter_validated_graph_path
      theoremFamily := "graph-router"
      rfcClauseIds := searchRouteDagValidationClauses }
  , { name := `SearchRouteDAG.unvalidated_graph_path_is_not_a_shortcut
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteDagValidationClauses }
  ]

def searchRouteDagAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteDAG"
    "packages/proofs/ASPProof/SearchRouteDAG.lean"
    searchRouteDagTargets

elab "#write_searchroute_dag_audit" : command => do
  let audit ← liftTermElabM searchRouteDagAuditJson
  let path : String := "receipts/searchroute-dag-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteEnumerationWalkClauses : List String :=
  [ "ASP-RFC-10.05-SRE-WALK-ENUMERATOR"
  , "ASP-RFC-10.05-SRE-WALK-SOUNDNESS"
  , "ASP-RFC-10.05-SRE-WALK-COMPLETENESS"
  ]

private def searchRouteEnumerationCatalogClauses : List String :=
  [ "ASP-RFC-10.05-SRE-ROUTE-CATALOG"
  , "ASP-RFC-10.05-SRE-TWO-STAGE-COMPLETENESS"
  ]

private def searchRouteEnumerationBoundClauses : List String :=
  [ "ASP-RFC-10.05-SRE-TRANSITION-BOUND"
  , "ASP-RFC-10.05-SRE-ROUTE-CATALOG"
  ]

private def searchRouteStepCatalogClauses : List String :=
  [ "ASP-RFC-10.05-SRE-STEP-INSTANCE-CATALOG"
  , "ASP-RFC-10.05-SRE-ROUTE-CATALOG"
  , "ASP-RFC-10.05-SRE-TWO-STAGE-COMPLETENESS"
  ]

def searchRouteDagEnumerationTargets : List AuditTarget :=
  [ { name := `SearchRouteDAGEnumeration.enumerateWalkEndpoints_complete
      theoremFamily := "walk-enumeration-completeness"
      rfcClauseIds := searchRouteEnumerationWalkClauses }
  , { name := `SearchRouteDAGEnumeration.enumerateWalkEndpoints_sound
      theoremFamily := "walk-enumeration-soundness"
      rfcClauseIds := searchRouteEnumerationWalkClauses }
  , { name := `SearchRouteDAGEnumeration.rankedDagFuel_is_complete
      theoremFamily := "ranked-dag-fuel"
      rfcClauseIds := searchRouteEnumerationWalkClauses }
  , { name :=
        `SearchRouteDAGEnumeration.complete_walks_and_route_catalog_generate_complete_candidates
      theoremFamily := "two-stage-completeness"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name := `SearchRouteDAGEnumeration.enumerateStepLists_complete
      theoremFamily := "route-enumeration-completeness"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.feasible_route_mem_requirement_variants
      theoremFamily := "metadata-enumeration-completeness"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.bounded_feasible_route_catalog_is_complete
      theoremFamily := "route-catalog-construction"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.finite_step_catalog_generates_complete_candidates
      theoremFamily := "constructive-candidate-completeness"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name := `SearchRouteDAGEnumeration.member_measure_le_foldl
      theoremFamily := "per-step-budget-bound"
      rfcClauseIds := searchRouteStepCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.feasible_step_mem_enumerateRouteSteps
      theoremFamily := "step-instance-coverage"
      rfcClauseIds := searchRouteStepCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.budget_step_catalog_is_complete
      theoremFamily := "step-catalog-construction"
      rfcClauseIds := searchRouteStepCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.budget_catalog_generates_complete_candidates
      theoremFamily := "unconditional-bounded-completeness"
      rfcClauseIds := searchRouteStepCatalogClauses }
  , { name := `SearchRouteDAGEnumeration.zero_padding_cost_receipt
      theoremFamily := "cost-receipt"
      rfcClauseIds := searchRouteEnumerationBoundClauses }
  , { name := `SearchRouteDAGEnumeration.zero_padding_transition_receipt
      theoremFamily := "cost-receipt"
      rfcClauseIds := searchRouteEnumerationBoundClauses }
  , { name :=
        `SearchRouteDAGEnumeration.route_budget_allows_unbounded_zero_cost_transitions
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteEnumerationBoundClauses }
  , { name :=
        `SearchRouteDAGEnumeration.route_budget_has_no_transition_ceiling
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteEnumerationBoundClauses }
  , { name :=
        `SearchRouteDAGEnumeration.graph_route_budget_rejects_excess_zero_padding
      theoremFamily := "transition-bound"
      rfcClauseIds := searchRouteEnumerationBoundClauses }
  , { name :=
        `SearchRouteDAGEnumeration.graph_walk_completeness_does_not_imply_route_completeness
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  , { name :=
        `SearchRouteDAGEnumeration.graph_only_candidates_are_not_candidate_complete
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteEnumerationCatalogClauses }
  ]

def searchRouteDagEnumerationAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteDAGEnumeration"
    "packages/proofs/ASPProof/SearchRouteDAGEnumeration.lean"
    searchRouteDagEnumerationTargets

elab "#write_searchroute_dag_enumeration_audit" : command => do
  let audit ← liftTermElabM searchRouteDagEnumerationAuditJson
  let path : String :=
    "receipts/searchroute-dag-enumeration-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteBranchBoundClauses : List String :=
  [ "ASP-RFC-10.05-SBB-LOWER-BOUND"
  , "ASP-RFC-10.05-SBB-SAFE-PRUNING"
  ]

private def searchRouteCoverageClauses : List String :=
  [ "ASP-RFC-10.05-SBB-COVERAGE"
  , "ASP-RFC-10.05-SBB-SNAPSHOT"
  ]

def searchRouteBranchBoundTargets : List AuditTarget :=
  [ { name := `SearchRouteBranchBound.keyLexNoWorse_trans
      theoremFamily := "lower-bound-order"
      rfcClauseIds := searchRouteBranchBoundClauses }
  , { name :=
        `SearchRouteBranchBound.valid_lower_bound_pruning_is_safe
      theoremFamily := "safe-pruning"
      rfcClauseIds := searchRouteBranchBoundClauses }
  , { name :=
        `SearchRouteBranchBound.certified_lazy_frontier_is_globally_optimal
      theoremFamily := "lazy-global-optimality"
      rfcClauseIds :=
        searchRouteBranchBoundClauses ++ searchRouteCoverageClauses }
  , { name := `SearchRouteBranchBound.snapshotMatchesB_true_iff
      theoremFamily := "snapshot-identity"
      rfcClauseIds := searchRouteCoverageClauses }
  , { name :=
        `SearchRouteBranchBound.snapshot_accepted_lazy_frontier_is_globally_optimal
      theoremFamily := "snapshot-bound-optimality"
      rfcClauseIds :=
        searchRouteBranchBoundClauses ++ searchRouteCoverageClauses }
  , { name :=
        `SearchRouteBranchBound.invalid_lower_bound_can_prune_a_better_candidate
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteBranchBoundClauses }
  , { name :=
        `SearchRouteBranchBound.visible_optimality_without_coverage_is_unsound
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteCoverageClauses }
  , { name :=
        `SearchRouteBranchBound.stale_snapshot_coverage_receipt_is_rejected
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteCoverageClauses }
  ]

def searchRouteBranchBoundAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteBranchBound"
    "packages/proofs/ASPProof/SearchRouteBranchBound.lean"
    searchRouteBranchBoundTargets

elab "#write_searchroute_branch_bound_audit" : command => do
  let audit ← liftTermElabM searchRouteBranchBoundAuditJson
  let path : String :=
    "receipts/searchroute-branch-bound-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRoutePartialBoundClauses : List String :=
  [ "ASP-RFC-10.05-SPR-PARTIAL-BOUND"
  , "ASP-RFC-10.05-SPR-MONOTONE-EXTENSION"
  ]

private def searchRouteInspectSchedulerClauses : List String :=
  [ "ASP-RFC-10.05-SPR-SCHEDULER"
  , "ASP-RFC-10.05-SPR-COMPLETE"
  ]

def searchRouteInspectSchedulerTargets : List AuditTarget :=
  [ { name :=
        `SearchRouteInspectScheduler.partial_uncached_tokens_extend
      theoremFamily := "partial-cost-extension"
      rfcClauseIds := searchRoutePartialBoundClauses }
  , { name := `SearchRouteInspectScheduler.partial_rounds_extend
      theoremFamily := "partial-cost-extension"
      rfcClauseIds := searchRoutePartialBoundClauses }
  , { name :=
        `SearchRouteInspectScheduler.partial_lower_bound_is_monotone
      theoremFamily := "partial-lower-bound"
      rfcClauseIds := searchRoutePartialBoundClauses }
  , { name :=
        `SearchRouteInspectScheduler.keyLexNoWorseB_true_iff
      theoremFamily := "executable-order"
      rfcClauseIds := searchRouteInspectSchedulerClauses }
  , { name := `SearchRouteInspectScheduler.scheduleInspect_covers
      theoremFamily := "scheduler-partition"
      rfcClauseIds := searchRouteInspectSchedulerClauses }
  , { name := `SearchRouteInspectScheduler.scheduled_pruning_is_safe
      theoremFamily := "scheduler-pruning"
      rfcClauseIds := searchRouteInspectSchedulerClauses }
  , { name :=
        `SearchRouteInspectScheduler.scheduled_inspection_is_not_prunable
      theoremFamily := "scheduler-inspection"
      rfcClauseIds := searchRouteInspectSchedulerClauses }
  , { name :=
        `SearchRouteInspectScheduler.completed_schedule_prunes_every_frontier
      theoremFamily := "scheduler-completion"
      rfcClauseIds := searchRouteInspectSchedulerClauses }
  , { name :=
        `SearchRouteInspectScheduler.completed_inspect_schedule_is_globally_optimal
      theoremFamily := "scheduler-global-optimality"
      rfcClauseIds :=
        searchRoutePartialBoundClauses ++
          searchRouteInspectSchedulerClauses }
  , { name :=
        `SearchRouteInspectScheduler.decreasing_graph_bound_breaks_extension_monotonicity
      theoremFamily := "counterexample"
      rfcClauseIds := searchRoutePartialBoundClauses }
  ]

def searchRouteInspectSchedulerAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectScheduler"
    "packages/proofs/ASPProof/SearchRouteInspectScheduler.lean"
    searchRouteInspectSchedulerTargets

elab "#write_searchroute_inspect_scheduler_audit" : command => do
  let audit ← liftTermElabM searchRouteInspectSchedulerAuditJson
  let path : String :=
    "receipts/searchroute-inspect-scheduler-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteInspectLoopDecisionClauses : List String :=
  [ "ASP-RFC-10.05-SIL-DECISION"
  , "ASP-RFC-10.05-SIL-WORK-DECREASE"
  ]

private def searchRouteInspectLoopCoverageClauses : List String :=
  [ "ASP-RFC-10.05-SIL-COVERAGE"
  , "ASP-RFC-10.05-SIL-TERMINAL"
  ]

private def searchRouteInspectLoopTraceClauses : List String :=
  [ "ASP-RFC-10.05-SIL-TRACE-BOUND"
  , "ASP-RFC-10.05-SIL-TERMINAL"
  ]

def searchRouteInspectLoopTargets : List AuditTarget :=
  [ { name := `SearchRouteInspectLoop.valid_decision_decreases_work
      theoremFamily := "loop-work-decrease"
      rfcClauseIds := searchRouteInspectLoopDecisionClauses }
  , { name :=
        `SearchRouteInspectLoop.valid_decision_preserves_well_formed
      theoremFamily := "loop-invariant"
      rfcClauseIds := searchRouteInspectLoopDecisionClauses }
  , { name :=
        `SearchRouteInspectLoop.valid_decision_selected_no_worse
      theoremFamily := "selected-non-regression"
      rfcClauseIds := searchRouteInspectLoopDecisionClauses }
  , { name := `SearchRouteInspectLoop.valid_prune_is_safe
      theoremFamily := "safe-pruning"
      rfcClauseIds := searchRouteInspectLoopCoverageClauses }
  , { name :=
        `SearchRouteInspectLoop.valid_decision_preserves_coverage
      theoremFamily := "coverage-conservation"
      rfcClauseIds := searchRouteInspectLoopCoverageClauses }
  , { name := `SearchRouteInspectLoop.trace_work_bound
      theoremFamily := "trace-bound"
      rfcClauseIds := searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.trace_length_bound
      theoremFamily := "trace-bound"
      rfcClauseIds := searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.trace_preserves_well_formed
      theoremFamily := "trace-invariant"
      rfcClauseIds :=
        searchRouteInspectLoopDecisionClauses ++
          searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.trace_preserves_coverage
      theoremFamily := "trace-invariant"
      rfcClauseIds :=
        searchRouteInspectLoopCoverageClauses ++
          searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.zero_work_well_formed_is_closed
      theoremFamily := "terminal-closure"
      rfcClauseIds := searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.closed_coverage_is_globally_optimal
      theoremFamily := "terminal-optimality"
      rfcClauseIds := searchRouteInspectLoopCoverageClauses }
  , { name :=
        `SearchRouteInspectLoop.zero_work_trace_closes_and_is_globally_optimal
      theoremFamily := "terminal-global-optimality"
      rfcClauseIds :=
        searchRouteInspectLoopCoverageClauses ++
          searchRouteInspectLoopTraceClauses }
  , { name :=
        `SearchRouteInspectLoop.full_budget_trace_closes_and_is_globally_optimal
      theoremFamily := "bounded-global-optimality"
      rfcClauseIds :=
        searchRouteInspectLoopDecisionClauses ++
          searchRouteInspectLoopCoverageClauses ++
          searchRouteInspectLoopTraceClauses }
  , { name := `SearchRouteInspectLoop.non_strict_credit_allows_stutter
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteInspectLoopDecisionClauses }
  ]

def searchRouteInspectLoopAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectLoop"
    "packages/proofs/ASPProof/SearchRouteInspectLoop.lean"
    searchRouteInspectLoopTargets

elab "#write_searchroute_inspect_loop_audit" : command => do
  let audit ← liftTermElabM searchRouteInspectLoopAuditJson
  let path : String :=
    "receipts/searchroute-inspect-loop-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteInspectDriverTotalClauses : List String :=
  [ "ASP-RFC-10.05-SID-DECISION-TOTAL"
  , "ASP-RFC-10.05-SID-CLOSED-TRACE"
  ]

private def searchRouteInspectDriverOptimalityClauses : List String :=
  [ "ASP-RFC-10.05-SID-GLOBAL"
  , "ASP-RFC-10.05-SID-CLOSED-TRACE"
  ]

private def searchRouteInspectDriverBoundaryClauses : List String :=
  [ "ASP-RFC-10.05-SID-SAFETY-LIVENESS"
  , "ASP-RFC-10.05-SID-DECISION-TOTAL"
  ]

def searchRouteInspectDriverTargets : List AuditTarget :=
  [ { name :=
        `SearchRouteInspectDriver.decision_total_produces_closed_trace
      theoremFamily := "liveness-existence"
      rfcClauseIds := searchRouteInspectDriverTotalClauses }
  , { name :=
        `SearchRouteInspectDriver.closed_trace_is_within_initial_work
      theoremFamily := "bounded-liveness"
      rfcClauseIds := searchRouteInspectDriverTotalClauses }
  , { name := `SearchRouteInspectDriver.trace_selected_no_worse
      theoremFamily := "selected-non-regression"
      rfcClauseIds := searchRouteInspectDriverOptimalityClauses }
  , { name :=
        `SearchRouteInspectDriver.decision_total_produces_bounded_global_optimum
      theoremFamily := "liveness-global-optimality"
      rfcClauseIds :=
        searchRouteInspectDriverTotalClauses ++
          searchRouteInspectDriverOptimalityClauses }
  , { name :=
        `SearchRouteInspectDriver.safety_trace_alone_allows_early_stop
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteInspectDriverBoundaryClauses }
  ]

def searchRouteInspectDriverAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectDriver"
    "packages/proofs/ASPProof/SearchRouteInspectDriver.lean"
    searchRouteInspectDriverTargets

elab "#write_searchroute_inspect_driver_audit" : command => do
  let audit ← liftTermElabM searchRouteInspectDriverAuditJson
  let path : String :=
    "receipts/searchroute-inspect-driver-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

private def searchRouteInspectTraceProjectionClauses : List String :=
  [ "ASP-RFC-10.05-SIT-COST-STEP"
  , "ASP-RFC-10.05-SIT-PROJECTION"
  ]

private def searchRouteInspectTraceCostClauses : List String :=
  [ "ASP-RFC-10.05-SIT-COST-FIDELITY"
  , "ASP-RFC-10.05-SIT-PROJECTION"
  ]

private def searchRouteInspectTraceCacheClauses : List String :=
  [ "ASP-RFC-10.05-SIT-CACHE-SEPARATION"
  , "ASP-RFC-10.05-SIT-COST-STEP"
  ]

private def searchRouteInspectTraceCollisionClauses : List String :=
  [ "ASP-RFC-10.05-SIT-CACHE-COLLISION"
  , "ASP-RFC-10.05-SIT-OPTIMALITY-BOUNDARY"
  ]

def searchRouteInspectTraceCostTargets : List AuditTarget :=
  [ { name :=
        `SearchRouteInspectTraceCost.costed_trace_length_matches_decisions
      theoremFamily := "trace-alignment"
      rfcClauseIds := searchRouteInspectTraceProjectionClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_transition_count_is_trace_length
      theoremFamily := "route-projection"
      rfcClauseIds := searchRouteInspectTraceProjectionClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_evidence_tokens_are_trace_actual
      theoremFamily := "cost-fidelity"
      rfcClauseIds := searchRouteInspectTraceCostClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_prompt_tokens_are_trace_prompt
      theoremFamily := "cost-fidelity"
      rfcClauseIds := searchRouteInspectTraceCostClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_uncached_tokens_are_trace_cost
      theoremFamily := "cost-fidelity"
      rfcClauseIds := searchRouteInspectTraceCostClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_rounds_are_trace_cost
      theoremFamily := "cost-fidelity"
      rfcClauseIds := searchRouteInspectTraceCostClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_semantic_search_cache_accounting
      theoremFamily := "cache-accounting"
      rfcClauseIds := searchRouteInspectTraceCacheClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_model_prefix_cache_accounting
      theoremFamily := "cache-accounting"
      rfcClauseIds := searchRouteInspectTraceCacheClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_closed_receipt_is_closed_and_path_validated
      theoremFamily := "projection-validity"
      rfcClauseIds := searchRouteInspectTraceProjectionClauses }
  , { name :=
        `SearchRouteInspectTraceCost.costed_trace_all_steps_are_sound
      theoremFamily := "cache-soundness"
      rfcClauseIds := searchRouteInspectTraceCacheClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_prefix_cache_is_sound
      theoremFamily := "cache-soundness"
      rfcClauseIds := searchRouteInspectTraceCacheClauses }
  , { name :=
        `SearchRouteInspectTraceCost.projected_semantic_cache_is_sound
      theoremFamily := "cache-soundness"
      rfcClauseIds := searchRouteInspectTraceCacheClauses }
  , { name :=
        `SearchRouteInspectTraceCost.cache_layer_examples_are_sound
      theoremFamily := "counterexample"
      rfcClauseIds := searchRouteInspectTraceCollisionClauses }
  , { name :=
        `SearchRouteInspectTraceCost.equal_total_cost_does_not_identify_cache_layer
      theoremFamily := "counterexample"
      rfcClauseIds :=
        searchRouteInspectTraceCacheClauses ++
          searchRouteInspectTraceCollisionClauses }
  ]

def searchRouteInspectTraceCostAuditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.SearchRouteInspectTraceCost"
    "packages/proofs/ASPProof/SearchRouteInspectTraceCost.lean"
    searchRouteInspectTraceCostTargets

elab "#write_searchroute_inspect_trace_cost_audit" : command => do
  let audit ← liftTermElabM searchRouteInspectTraceCostAuditJson
  let path : String :=
    "receipts/searchroute-inspect-trace-cost-audit-v1.json"
  IO.FS.writeFile path (audit.pretty ++ "\n")
  logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit

end ASPProof.Audit
