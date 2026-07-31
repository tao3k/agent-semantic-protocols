import ASPProof.Audit.Core
import ASPProof.SearchRouteBoundedDominanceWitnessChain

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteBoundedDominanceWitnessChain

open ASPProof.Audit.Core
open ASPProof.SearchRouteBoundedDominanceWitnessChain

def targets : List Target := [
  Target.mk
    ``resolution_steps_bounded_by_fuel
    "witness-resolution-steps-bounded-by-fuel"
    ["BWC-FUEL", "BWC-BOUND"],
  Target.mk
    ``retained_resolution_endpoint_is_kept
    "retained-resolution-endpoint-is-kept"
    ["BWC-ENDPOINT", "BWC-RETAINED", "BWC-SOUNDNESS"],
  Target.mk
    ``capacity_resolution_steps_are_bounded
    "capacity-resolution-step-bound"
    ["BWC-CAPACITY", "BWC-BOUND"],
  Target.mk
    ``one_step_fuel_is_insufficient_for_example
    "insufficient-fuel-exhaustion-counterexample"
    ["BWC-FUEL", "BWC-EXHAUSTED", "BWC-COUNTEREXAMPLE"],
  Target.mk
    ``two_step_fuel_reaches_retained_example_endpoint
    "sufficient-fuel-reaches-retained-endpoint"
    ["BWC-FUEL", "BWC-ENDPOINT", "BWC-EXAMPLE"],
  Target.mk
    ``descending_chain_rank_relation
    "nonempty-witness-chain-strict-rank-descent"
    ["BWC-DESCENT", "BWC-CHAIN"],
  Target.mk
    ``nonempty_descending_chain_cannot_cycle
    "strictly-descending-witness-chain-acyclic"
    ["BWC-DESCENT", "BWC-ACYCLIC"],
  Target.mk
    ``example_chain_has_retained_endpoint
    "example-chain-retained-endpoint"
    ["BWC-CHAIN", "BWC-RETAINED", "BWC-EXAMPLE"],
  Target.mk
    ``explicit_witness_chain_receipt_is_capacity_bounded
    "explicit-witness-chain-receipt-capacity-bound"
    ["BWC-RECEIPT", "BWC-CAPACITY"],
  Target.mk
    ``summarized_witness_chain_receipt_is_length_independent
    "summary-witness-chain-receipt-constant-size"
    ["BWC-RECEIPT", "BWC-SUMMARY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteBoundedDominanceWitnessChain"
    "ASPProof/SearchRouteBoundedDominanceWitnessChain.lean"
    targets

elab "writeSearchRouteBoundedDominanceWitnessChainAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-bounded-dominance-witness-chain-audit-v1.json"
    auditJson

end ASPProof.Audit.SearchRouteBoundedDominanceWitnessChain

open ASPProof.Audit.SearchRouteBoundedDominanceWitnessChain

writeSearchRouteBoundedDominanceWitnessChainAudit
