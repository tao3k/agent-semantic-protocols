import ASPProof.Audit.Core
import ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteFiniteMultiobjectiveParetoMaskWitness

open ASPProof.Audit.Core
open ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness

def targets : List Target := [
  Target.mk
    ``findDominator_some_is_sound
    "found-dominator-is-sound"
    ["FPM-SEARCH", "FPM-SOUNDNESS"],
  Target.mk
    ``findDominator_none_excludes_dominator
    "no-found-dominator-excludes-every-appearing-route"
    ["FPM-SEARCH", "FPM-SOUNDNESS", "FPM-COMPLETENESS"],
  Target.mk
    ``kept_candidate_findDominator_none
    "kept-route-has-no-dominator-result"
    ["FPM-MASK", "FPM-SEARCH", "FPM-SOUNDNESS"],
  Target.mk
    ``findDominator_none_candidate_kept
    "no-dominator-result-keeps-route"
    ["FPM-MASK", "FPM-SEARCH", "FPM-COMPLETENESS"],
  Target.mk
    ``kept_candidate_is_undominated
    "kept-route-undominated-soundness"
    ["FPM-MASK", "FPM-SOUNDNESS"],
  Target.mk
    ``undominated_candidate_is_kept
    "undominated-route-mask-completeness"
    ["FPM-MASK", "FPM-COMPLETENESS"],
  Target.mk
    ``removed_candidate_has_dominating_witness
    "removed-route-dominating-witness"
    ["FPM-REMOVAL", "FPM-WITNESS", "FPM-COMPLETENESS"],
  Target.mk
    ``natLt_is_irreflexive
    "executable-natural-less-than-irreflexive"
    ["FPM-COST-VECTOR", "FPM-DOMINANCE"],
  Target.mk
    ``strictlyImproves_is_irreflexive
    "strict-improvement-irreflexive"
    ["FPM-COST-VECTOR", "FPM-DOMINANCE"],
  Target.mk
    ``equal_cost_does_not_strictly_dominate
    "equal-cost-is-not-strict-dominance"
    ["FPM-EQUAL-COST", "FPM-DOMINANCE"],
  Target.mk
    ``strict_dominance_is_irreflexive
    "strict-dominance-irreflexive"
    ["FPM-DOMINANCE"],
  Target.mk
    ``equal_cost_distinct_routes_are_both_kept
    "equal-cost-distinct-routes-preserved"
    ["FPM-EQUAL-COST", "FPM-CANONICAL-ID", "FPM-MASK"],
  Target.mk
    ``adjacent_swap_preserves_keep_mask
    "adjacent-swap-preserves-keep-mask"
    ["FPM-PERMUTATION", "FPM-DETERMINISM"],
  Target.mk
    ``candidate_permutation_preserves_keep_mask
    "constructive-permutation-preserves-keep-mask"
    ["FPM-PERMUTATION", "FPM-DETERMINISM", "FPM-EXTENSIONAL"],
  Target.mk
    ``faster_route_strictly_dominates_slower_route
    "shorter-lower-token-route-dominates"
    ["FPM-COST-VECTOR", "FPM-DOMINANCE"],
  Target.mk
    ``slower_route_is_removed_with_faster_witness
    "dominated-route-is-removed"
    ["FPM-REMOVAL", "FPM-WITNESS"],
  Target.mk
    ``direct_dominating_witness_may_itself_be_removed
    "direct-witness-may-be-nonfrontier-counterexample"
    ["FPM-WITNESS", "FPM-WITNESS-CHAIN", "FPM-COUNTEREXAMPLE"],
  Target.mk
    ``dominance_witness_receipt_is_capacity_bounded
    "dominance-witness-receipt-capacity-bound"
    ["FPM-RECEIPT", "FPM-CAPACITY"],
  Target.mk
    ``summarized_dominance_receipt_is_removed_count_independent
    "summary-dominance-receipt-constant-size"
    ["FPM-RECEIPT", "FPM-SUMMARY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteFiniteMultiobjectiveParetoMaskWitness"
    "ASPProof/SearchRouteFiniteMultiobjectiveParetoMaskWitness.lean"
    targets

elab "writeSearchRouteFiniteMultiobjectiveParetoMaskWitnessAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-finite-multiobjective-pareto-mask-witness-audit-v1.json"
    auditJson

end ASPProof.Audit.SearchRouteFiniteMultiobjectiveParetoMaskWitness

open ASPProof.Audit.SearchRouteFiniteMultiobjectiveParetoMaskWitness

writeSearchRouteFiniteMultiobjectiveParetoMaskWitnessAudit
