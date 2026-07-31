import ASPProof.SearchRouteProviderBatchRealization
import Lean

namespace ASPProof.Audit.SearchRouteProviderBatchRealization

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
       "ASPProof.SearchRouteProviderBatchRealization.certified_discovered_route_has_call_witness"
       "identity-coverage"
       "every certified discovered route has a scheduled provider-call witness"
       #["ASP-RFC-10.05-PBR-COVERAGE"]
       #[]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.certified_realized_route_is_discovered"
       "realization-soundness"
       "every route realized by a scheduled call belongs to the discovered set"
       #["ASP-RFC-10.05-PBR-SOUNDNESS"]
       #[]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.certificate_earns_unique_progress_credit"
       "unique-progress-credit"
       "a realization certificate earns credit only from duplicate-free route identities"
       #["ASP-RFC-10.05-PBR-UNIQUE-CREDIT"]
       #[]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.bundled_certificate_covers_alpha"
       "bundled-alpha-coverage"
       "the bundled provider call realizes route alpha"
       #["ASP-RFC-10.05-PBR-ALPHA"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.bundled_certificate_covers_beta"
       "bundled-beta-coverage"
       "the bundled provider call realizes route beta"
       #["ASP-RFC-10.05-PBR-BETA"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.one_call_covers_two_distinct_routes"
       "one-call-multi-route"
       "one scheduled call covers two distinct route identities and earns two credits"
       #["ASP-RFC-10.05-PBR-ONE-CALL"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.bundled_batch_cost_is_valid"
       "certificate-cost-binding"
       "aggregate unique-route and call counts bind to the realization certificate"
       #["ASP-RFC-10.05-PBR-COST"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.one_call_can_realize_multiple_routes"
       "multi-route-cost-conservation"
       "one call realizes multiple routes while preserving call and synthesis token cost"
       #["ASP-RFC-10.05-PBR-MULTI"]
       #[]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.count_equality_does_not_imply_identity_coverage"
       "count-only-counterexample"
       "equal scalar route counts do not imply identity coverage"
       #["ASP-RFC-10.05-PBR-COUNT-COUNTEREXAMPLE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.duplicate_claims_cannot_earn_two_credits"
       "duplicate-credit-rejection"
       "duplicate route identities cannot earn multiple unique progress credits"
       #["ASP-RFC-10.05-PBR-DUPLICATE-CREDIT"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.duplicate_receipt_has_no_beta_witness"
       "missing-beta-witness"
       "duplicate alpha claims provide no realization witness for beta"
       #["ASP-RFC-10.05-PBR-NO-BETA"]
       #[]
   , declaration
       "ASPProof.SearchRouteProviderBatchRealization.duplicate_relation_cannot_cover_distinct_routes"
       "duplicate-relation-rejection"
       "a duplicate-only realization relation cannot cover the distinct expected route set"
       #["ASP-RFC-10.05-PBR-REJECT"]
       #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteProviderBatchRealization")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteProviderBatchRealization.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 12)
    , ("axiomFreeDeclarationCount", Json.num 5)
    , ("axiomDependentDeclarationCount", Json.num 7)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteProviderBatchRealization
