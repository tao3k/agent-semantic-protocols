import ASPProof.Audit.Core
import ASPProof.SearchRouteDerivedCapacityRank

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.bool_and_true_parts
    theoremFamily := "constructive-bool-and-elimination"
    rfcClauseIds := ["DCR-CONSTRUCTIVE"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.bool_or_true_cases
    theoremFamily := "constructive-bool-or-elimination"
    rfcClauseIds := ["DCR-CONSTRUCTIVE"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.natLe_true_implies_le
    theoremFamily := "executable-natle-soundness"
    rfcClauseIds := ["DCR-SCALAR"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.natLt_true_implies_lt
    theoremFamily := "executable-natlt-soundness"
    rfcClauseIds := ["DCR-SCALAR"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.lt_implies_natLt_true
    theoremFamily := "executable-natlt-completeness"
    rfcClauseIds := ["DCR-SCALAR"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.natLt_true_is_transitive
    theoremFamily := "executable-natlt-transitivity"
    rfcClauseIds := ["DCR-SCALAR"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.strict_dominance_decreases_total_cost
    theoremFamily := "strict-dominance-total-cost-descent"
    rfcClauseIds := ["DCR-DESCENT"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.strict_dominance_decreases_total_cost_bool
    theoremFamily := "strict-dominance-executable-total-cost-descent"
    rfcClauseIds := ["DCR-DESCENT"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.lower_cost_count_le_length
    theoremFamily := "normalized-rank-capacity-upper-bound"
    rfcClauseIds := ["DCR-RANK", "DCR-BOUND"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.lower_cost_count_monotone
    theoremFamily := "normalized-rank-monotonicity"
    rfcClauseIds := ["DCR-RANK"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.lower_cost_count_strict_of_appears
    theoremFamily := "member-witness-normalized-rank-descent"
    rfcClauseIds := ["DCR-MEMBER", "DCR-RANK"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.lower_cost_count_lt_length_of_appears
    theoremFamily := "member-normalized-rank-capacity-bound"
    rfcClauseIds := ["DCR-MEMBER", "DCR-BOUND"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.list_member_implies_appears
    theoremFamily := "list-member-to-constructive-appearance"
    rfcClauseIds := ["DCR-MEMBER"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.derivedCapacityRankCertificate
    theoremFamily := "finite-universe-derived-capacity-rank-certificate"
    rfcClauseIds := ["DCR-CERTIFICATE"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.finite_candidate_capacity_fuel_is_retained
    theoremFamily := "finite-universe-capacity-fuel-retained"
    rfcClauseIds := ["DCR-RETAINED"]
  },
  {
    name := ``ASPProof.SearchRouteDerivedCapacityRank.finite_candidate_capacity_fuel_reaches_kept_endpoint
    theoremFamily := "finite-universe-capacity-fuel-kept-endpoint"
    rfcClauseIds := ["DCR-ENDPOINT"]
  }
]

elab "#writeDerivedCapacityRankAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteDerivedCapacityRank"
      "ASPProof/SearchRouteDerivedCapacityRank.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-derived-capacity-rank-audit-v1.json"
    json.pretty

#writeDerivedCapacityRankAudit
