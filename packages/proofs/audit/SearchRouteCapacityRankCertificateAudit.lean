import ASPProof.Audit.Core
import ASPProof.SearchRouteCapacityRankCertificate

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.rank_bounded_resolution_is_retained
    theoremFamily := "rank-bounded-resolution-is-retained"
    rfcClauseIds := ["CRC-RANK", "CRC-DESCENT", "CRC-RETAINED"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.capacity_fuel_is_not_exhausted
    theoremFamily := "capacity-fuel-is-not-exhausted"
    rfcClauseIds := ["CRC-BOUND", "CRC-RETAINED"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.capacity_fuel_reaches_kept_endpoint
    theoremFamily := "capacity-fuel-reaches-kept-endpoint"
    rfcClauseIds := ["CRC-ENDPOINT"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.capacity_fuel_step_bound
    theoremFamily := "capacity-fuel-step-bound"
    rfcClauseIds := ["CRC-STEPS"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.total_cost_descent_does_not_supply_capacity_bound
    theoremFamily := "total-cost-descent-does-not-supply-capacity-bound"
    rfcClauseIds := ["CRC-RANK", "CRC-BOUND"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.zero_capacity_has_no_member
    theoremFamily := "zero-capacity-has-no-member"
    rfcClauseIds := ["CRC-BOUND"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.explicit_capacity_rank_receipt_is_linear
    theoremFamily := "explicit-capacity-rank-receipt-is-linear"
    rfcClauseIds := ["CRC-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.summarized_capacity_rank_receipt_is_constant
    theoremFamily := "summarized-capacity-rank-receipt-is-constant"
    rfcClauseIds := ["CRC-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteCapacityRankCertificate.summarized_capacity_rank_receipt_beats_explicit_nonempty
    theoremFamily := "summarized-capacity-rank-receipt-beats-explicit-nonempty"
    rfcClauseIds := ["CRC-RECEIPT"]
  }
]

elab "#writeCapacityRankAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteCapacityRankCertificate"
      "ASPProof/SearchRouteCapacityRankCertificate.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-capacity-rank-certificate-audit-v1.json"
    json.pretty

#writeCapacityRankAudit
