import ASPProof.SearchLoopCache
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

def searchLoopCacheTargets : List AuditTarget :=
  [ { name := ``SearchLoopCache.validateSemantic_self
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.validateSemantic_hit_implies_equal
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.validateSemantic_hit_iff_equal
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.validateReuse_self
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.validateReuse_hit_implies_equal
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.validateReuse_hit_iff_equal
      theoremFamily := "cache-identity"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.legacy_key_collides_under_provider_artifact_drift
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.provider_artifact_drift_is_typed_stale
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.legacy_key_collides_under_policy_drift
      theoremFamily := "cache-counterexample"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.policy_drift_is_typed_stale
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses }
  , { name := ``SearchLoopCache.selector_drift_is_stale_without_relocation
      theoremFamily := "typed-invalidation"
      rfcClauseIds := cacheIdentityClauses ++ cacheRelocationClauses }
  , { name := ``SearchLoopCache.explicit_relocation_rekeys_to_hit
      theoremFamily := "selector-relocation"
      rfcClauseIds := cacheIdentityClauses ++ cacheRelocationClauses }
  , { name := ``SearchLoopCache.model_drift_does_not_invalidate_semantic_receipt
      theoremFamily := "tier-scope"
      rfcClauseIds := cacheIdentityClauses ++ cacheTierClauses }
  , { name := ``SearchLoopCache.model_drift_invalidates_l4
      theoremFamily := "tier-scope"
      rfcClauseIds := cacheIdentityClauses ++ cacheTierClauses }
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

private def proofAuditJson
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
    "ASPProof.SearchLoopCache"
    "packages/proofs/ASPProof/SearchLoopCache.lean"
    searchLoopCacheTargets

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

end ASPProof.Audit
