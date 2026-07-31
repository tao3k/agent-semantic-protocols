import ASPProof.SearchRouteAdaptiveCatalogClosure
import Lean

namespace ASPProof.Audit.SearchRouteAdaptiveCatalogClosure

open Lean

private def strings (values : Array String) : Json :=
  Json.arr (values.map Json.str)

private def declaration
    (name theoremFamily declarationType : String)
    (rfcClauseIds : Array String) : Json :=
  Json.mkObj
    [ ("name", Json.str name)
    , ("kind", Json.str "theorem")
    , ("type", Json.str declarationType)
    , ("theoremFamily", Json.str theoremFamily)
    , ("rfcClauseIds", strings rfcClauseIds)
    , ("axioms", Json.arr #[])
    , ("hasSorryAx", Json.bool false)
    ]

def declarations : Array Json :=
  #[ declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.reachable_route_is_enumerated"
       "adaptive-reachability-closure"
       "initial coverage and successor closure enumerate every adaptively reachable route"
       #["ASP-RFC-10.05-ARC-REACHABLE"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.adaptive_frontier_covers_reachable_route"
       "adaptive-frontier-coverage"
       "a frontier over an adaptively closed catalog covers every reachable route"
       #["ASP-RFC-10.05-ARC-FRONTIER"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.reachably_undominated_route_is_retained"
       "reachable-undominated-retention"
       "catalog soundness makes every reachably undominated route retained"
       #["ASP-RFC-10.05-ARC-UNDOMINATED"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.discovered_route_is_reachable"
       "evidence-dependent-discovery"
       "an evidence-enabled successor is inductively reachable"
       #["ASP-RFC-10.05-ARC-DISCOVERY"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.static_frontier_covers_incomplete_catalog"
       "static-catalog-coverage"
       "a static frontier can validly cover every member of an incomplete catalog"
       #["ASP-RFC-10.05-ARC-STATIC"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.static_frontier_misses_reachable_route"
       "adaptive-route-miss"
       "a static frontier over an incomplete catalog can miss a reachable route"
       #["ASP-RFC-10.05-ARC-MISS"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.incomplete_catalog_is_not_successor_closed"
       "successor-closure-rejection"
       "a catalog omitting an evidence-enabled successor is not successor closed"
       #["ASP-RFC-10.05-ARC-NOT-CLOSED"]
   , declaration
       "ASPProof.SearchRouteAdaptiveCatalogClosure.initial_coverage_does_not_imply_adaptive_coverage"
       "initial-coverage-counterexample"
       "initial-route coverage alone does not imply adaptive route coverage"
       #["ASP-RFC-10.05-ARC-INITIAL-INSUFFICIENT"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteAdaptiveCatalogClosure")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteAdaptiveCatalogClosure.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 8)
    , ("axiomFreeDeclarationCount", Json.num 8)
    , ("axiomDependentDeclarationCount", Json.num 0)
    , ("axiomInventory", Json.arr #[])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteAdaptiveCatalogClosure
