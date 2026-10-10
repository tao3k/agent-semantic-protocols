-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteSharedCostAllocation
import Lean

namespace ASPProof.Audit.SearchRouteSharedCostAllocation

open Lean

private def strings (values : Array String) : Json :=
  Json.arr (values.map Json.str)

private def declaration
    (name theoremFamily declarationType : String)
    (rfcClauseIds axioms : Array String) : Json :=
  Json.mkObj
    [ ("name", Json.str name)
    , ("kind", Json.str "theorem")
    , ("type", Json.str declarationType)
    , ("theoremFamily", Json.str theoremFamily)
    , ("rfcClauseIds", strings rfcClauseIds)
    , ("axioms", strings axioms)
    , ("hasSorryAx", Json.bool false)
    ]

def declarations : Array Json :=
  #[ declaration
       "ASPProof.SearchRouteSharedCostAllocation.certificate_total_cost_is_conserved"
       "shared-cost-conservation"
       "route allocations plus shared tokens equal provider-call plus synthesis tokens"
       #["ASP-RFC-10.05-SCA-CONSERVATION"]
       #[]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.certificate_routes_are_distinct"
       "allocation-route-distinctness"
       "the allocation certificate binds duplicate-free route identities"
       #["ASP-RFC-10.05-SCA-DISTINCT"]
       #[]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.aligned_cost_context_preserves_all_identities"
       "cost-context-alignment"
       "cost comparability preserves graph, policy, search-cache, and model-prefix identities"
       #["ASP-RFC-10.05-SCA-CONTEXT"]
       #[]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.balanced_certificate_conserves_complete_cost"
       "balanced-allocation-witness"
       "the balanced two-route allocation conserves the complete 25-token batch"
       #["ASP-RFC-10.05-SCA-BALANCED"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.indivisible_remainder_is_explicit"
       "shared-remainder-preservation"
       "the indivisible token remainder remains explicit as shared batch cost"
       #["ASP-RFC-10.05-SCA-REMAINDER"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.full_cost_per_route_double_charges"
       "double-charge-counterexample"
       "assigning full batch cost to every route exceeds the conserved total"
       #["ASP-RFC-10.05-SCA-DOUBLE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.omitted_remainder_underreports"
       "underreport-counterexample"
       "dropping the indivisible shared remainder underreports total cost"
       #["ASP-RFC-10.05-SCA-UNDERREPORT"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.equal_total_cost_does_not_determine_route_allocation"
       "total-cost-insufficiency"
       "equal batch totals do not determine the allocated cost of a route"
       #["ASP-RFC-10.05-SCA-TOTAL-INSUFFICIENT"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.different_allocation_policies_are_not_comparable"
       "allocation-policy-rejection"
       "different allocation policy identities reject route-level comparison"
       #["ASP-RFC-10.05-SCA-POLICY"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.cache_context_mismatch_rejects_comparison"
       "cache-context-rejection"
       "a search-cache context mismatch rejects cost comparison"
       #["ASP-RFC-10.05-SCA-CACHE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.allocation_policy_changes_route_pareto_result"
       "policy-dependent-pareto"
       "two conserved policies reverse the alpha route comparison against cost 13"
       #["ASP-RFC-10.05-SCA-PARETO"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteSharedCostAllocation.certificate_is_comparable_with_itself"
       "cost-context-reflexivity"
       "a valid certificate is cost-context comparable with itself"
       #["ASP-RFC-10.05-SCA-REFLEXIVE"]
       #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteSharedCostAllocation")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteSharedCostAllocation.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 12)
    , ("axiomFreeDeclarationCount", Json.num 3)
    , ("axiomDependentDeclarationCount", Json.num 9)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteSharedCostAllocation
