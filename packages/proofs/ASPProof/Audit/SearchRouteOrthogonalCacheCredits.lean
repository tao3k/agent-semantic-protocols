import ASPProof.SearchRouteOrthogonalCacheCredits
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteOrthogonalCacheCredits

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String := []) : Json :=
  Json.mkObj
    [ ("name", toJson name)
    , ("kind", toJson "theorem")
    , ("theoremFamily", toJson theoremFamily)
    , ("type", toJson type)
    , ("rfcClauseIds", toJson [rfcClauseId])
    , ("axioms", toJson axioms)
    , ("hasSorryAx", toJson false)
    ]

def declarations : Array Json :=
  #[ theoremDeclaration
       "cache_credit_conservation"
       "cost-conservation"
       "CreditsWithinPlan -> realized plus credits equals planned in every dimension"
       "ASP-RFC-10.05-OCC-CONSERVATION"
       ["Classical.choice", "Quot.sound", "propext"]
   , theoremDeclaration
       "cache_realized_cost_is_no_worse"
       "cost-monotonicity"
       "CostNoWorse realized planned"
       "ASP-RFC-10.05-OCC-NO-WORSE"
       ["propext"]
   , theoremDeclaration
       "search_credit_is_model_orthogonal"
       "cache-orthogonality"
       "search credit does not change uncached model tokens"
       "ASP-RFC-10.05-OCC-SEARCH-ORTHOGONAL"
   , theoremDeclaration
       "model_credit_is_search_orthogonal"
       "cache-orthogonality"
       "model credit does not change graph, round, or search-token cost"
       "ASP-RFC-10.05-OCC-MODEL-ORTHOGONAL"
       ["propext"]
   , theoremDeclaration
       "cache_application_preserves_semantics"
       "semantic-preservation"
       "cache application preserves initial potential, final potential, and completion key"
       "ASP-RFC-10.05-OCC-SEMANTICS"
       ["propext"]
   , theoremDeclaration
       "model_prefix_binding_ignores_graph_generation"
       "prefix-generation-independence"
       "model prefix binding survives a graph-generation-only context update"
       "ASP-RFC-10.05-OCC-PREFIX-TRANSPORT"
   , theoremDeclaration
       "committed_credit_rejects_replay"
       "credit-replay"
       "not (CanCommitCredit committed proposal)"
       "ASP-RFC-10.05-OCC-REPLAY"
       ["propext"]
   , theoremDeclaration
       "example_credits_are_balanced"
       "credit-bounds"
       "CreditsWithinPlan example planned search and model credits"
       "ASP-RFC-10.05-OCC-CONSERVATION"
       ["propext"]
   , theoremDeclaration
       "stale_search_receipt_is_rejected"
       "search-stale"
       "not (SearchReceiptBound next generation context old receipt)"
       "ASP-RFC-10.05-OCC-SEARCH-STALE"
       ["propext"]
   , theoremDeclaration
       "model_prefix_receipt_survives_unrelated_graph_change"
       "prefix-transport"
       "ModelPrefixReceiptBound next generation context stable prefix receipt"
       "ASP-RFC-10.05-OCC-PREFIX-TRANSPORT"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteOrthogonalCacheCredits")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteOrthogonalCacheCredits.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 2)
    , ("axiomDependentDeclarationCount", toJson 8)
    , ("declarations", toJson declarations)
    , ("axiomInventory",
        toJson ["Classical.choice", "Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.63-orthogonal-cache-credit-ledger")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteOrthogonalCacheCredits

