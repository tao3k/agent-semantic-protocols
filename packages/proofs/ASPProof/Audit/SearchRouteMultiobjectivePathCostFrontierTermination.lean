import ASPProof.Audit.Core
import ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteMultiobjectivePathCostFrontierTermination

open ASPProof.Audit.Core
open ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination

def targets : List Target := [
  Target.mk ``stay_path_has_zero_local_cost
    "stay-path-zero-local-cost" ["MOPC-PATH", "MOPC-ZERO"],
  Target.mk ``step_path_accumulates_local_cost
    "step-path-local-accumulation" ["MOPC-PATH", "MOPC-ADDITIVE"],
  Target.mk ``assembled_path_route_derives_graph_hops
    "assembled-route-derived-hops" ["MOPC-PATH", "MOPC-HOPS"],
  Target.mk ``local_cost_is_no_worse_before_cycle_addition
    "local-cycle-removal-no-worse" ["MOPC-CYCLE", "MOPC-LOCAL"],
  Target.mk ``full_route_cycle_removal_requires_schedule_monotonicity
    "full-cycle-removal-schedule-contract" ["MOPC-CYCLE", "MOPC-SCHEDULE"],
  Target.mk ``positive_hop_cycle_removal_strictly_dominates
    "positive-cycle-removal-dominates" ["MOPC-CYCLE", "MOPC-PARETO"],
  Target.mk ``path_local_improvement_alone_does_not_imply_full_route_order
    "local-improvement-insufficient" ["MOPC-SCHEDULE", "MOPC-COUNTEREXAMPLE"],
  Target.mk ``projected_token_count_has_strict_nonadditive_witness
    "token-count-not-edge-additive" ["MOPC-TOKEN", "MOPC-COUNTEREXAMPLE"],
  Target.mk ``singleton_antichain_does_not_imply_pareto_coverage
    "antichain-not-frontier-complete" ["MOPC-FRONTIER", "MOPC-COMPLETE"],
  Target.mk ``positive_expansion_consumes_fuel
    "expansion-fuel-decreases" ["MOPC-TERMINATION", "MOPC-FUEL"],
  Target.mk ``zero_expansion_fuel_blocks_expansion
    "zero-fuel-fail-closed" ["MOPC-TERMINATION", "MOPC-FAIL-CLOSED"],
  Target.mk ``generated_path_bound_is_monotone_in_fuel
    "generated-path-capacity-bound" ["MOPC-TERMINATION", "MOPC-BRANCHING"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteMultiobjectivePathCostFrontierTermination"
    "ASPProof/SearchRouteMultiobjectivePathCostFrontierTermination.lean"
    targets

end ASPProof.Audit.SearchRouteMultiobjectivePathCostFrontierTermination

open ASPProof.Audit.SearchRouteMultiobjectivePathCostFrontierTermination

elab "writeSearchRouteMultiobjectivePathCostFrontierTerminationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-multiobjective-path-cost-frontier-termination-audit-v1.json"
    auditJson
