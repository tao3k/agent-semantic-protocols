import ASPProof.Audit.Core
import ASPProof.SearchRouterInteractiveGraphState

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouterInteractiveGraphState

open ASPProof.Audit.Core
open ASPProof.SearchRouterInteractiveGraphState

def targets : List Target := [
  Target.mk ``semantic_tool_observation_can_advance_the_graph
    "graph-delta-admission" ["SRIG-GRAPH-DELTA", "SRIG-OBSERVATION"],
  Target.mk ``model_invented_edge_is_rejected
    "invented-edge-counterexample" ["SRIG-GROUNDING", "SRIG-COUNTEREXAMPLE"],
  Target.mk ``accepted_transition_must_decrease_budget
    "transition-budget-decrease" ["SRIG-BUDGET", "SRIG-TRANSITION"],
  Target.mk ``unavailable_tool_vertical_pair_is_rejected
    "capability-admission" ["SRIG-CAPABILITY", "SRIG-VERTICAL"],
  Target.mk ``typed_no_hit_is_a_valid_graph_transition
    "typed-no-hit-transition" ["SRIG-NO-HIT", "SRIG-OBSERVATION"],
  Target.mk ``resume_to_live_frontier_is_one_bounded_hop
    "bounded-resume-hop" ["SRIG-CONTINUATION", "SRIG-HOPS"],
  Target.mk ``stale_continuation_is_rejected
    "stale-continuation-rejection" ["SRIG-CONTINUATION", "SRIG-SNAPSHOT"],
  Target.mk ``resume_cannot_exceed_existing_hop_budget
    "resume-hop-budget" ["SRIG-CONTINUATION", "SRIG-BUDGET"],
  Target.mk ``unresolved_obligation_prevents_closure
    "closure-obligation" ["SRIG-OBLIGATION", "SRIG-CLOSURE"],
  Target.mk ``obligation_discharge_without_observation_evidence_is_rejected
    "unsupported-discharge-counterexample" ["SRIG-OBLIGATION", "SRIG-GROUNDING"],
  Target.mk ``observation_evidence_can_discharge_an_obligation
    "grounded-discharge" ["SRIG-OBLIGATION", "SRIG-OBSERVATION"],
  Target.mk ``datalog_derivation_is_grounded_in_live_facts
    "datalog-live-fact-grounding" ["SRIG-DATALOG", "SRIG-GROUNDING"],
  Target.mk ``datalog_derivation_with_unknown_source_is_rejected
    "datalog-unknown-source-counterexample" ["SRIG-DATALOG", "SRIG-COUNTEREXAMPLE"]
]

def canonicalTargets : List Target :=
  targets.map fun target =>
    { target with
      rfcClauseIds := target.rfcClauseIds.map fun clauseId =>
        "ASP-RFC-" ++ clauseId }

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouterInteractiveGraphState"
    "ASPProof/SearchRouterInteractiveGraphState.lean"
    canonicalTargets

end ASPProof.Audit.SearchRouterInteractiveGraphState

open ASPProof.Audit.SearchRouterInteractiveGraphState

elab "writeSearchRouterInteractiveGraphStateAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/search-router-interactive-graph-state-audit-v1.json"
    auditJson
