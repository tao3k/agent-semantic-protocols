import ASPProof.SearchRouteSemanticPathCacheSeparation
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteSemanticPathCacheSeparation

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
       "cache_preserves_semantic_graph_hops"
       "semantic-path-invariance"
       "Cache-adjusted execution preserves semanticGraphHops"
       "ASP-RFC-10.05-SPCS-SEMANTIC-INVARIANT"
   , theoremDeclaration
       "execution_credit_conservation"
       "execution-cost-conservation"
       "Realized execution cost plus cache credits equals planned execution cost"
       "ASP-RFC-10.05-SPCS-EXECUTION-CONSERVATION"
       ["Classical.choice", "Quot.sound", "propext"]
   , theoremDeclaration
       "search_cache_is_model_cost_orthogonal"
       "cache-lane-orthogonality"
       "Search cache credit does not change uncached model tokens"
       "ASP-RFC-10.05-SPCS-CACHE-ORTHOGONAL"
   , theoremDeclaration
       "model_cache_is_search_execution_orthogonal"
       "cache-lane-orthogonality"
       "Model prefix credit does not change semantic or search execution cost"
       "ASP-RFC-10.05-SPCS-CACHE-ORTHOGONAL"
       ["propext"]
   , theoremDeclaration
       "legacy_cache_projection_changes_declared_graph_hops"
       "legacy-cost-countermodel"
       "The legacy cache projection reduces its graphHops field"
       "ASP-RFC-10.05-SPCS-LEGACY-COUNTERMODEL"
   , theoremDeclaration
       "equal_realized_traversal_does_not_identify_semantic_path"
       "semantic-observation-nonidentity"
       "Equal realized traversal work does not identify semantic path length"
       "ASP-RFC-10.05-SPCS-OBSERVATION-NONIDENTITY"
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteSemanticPathCacheSeparation")
    , ("sourcePath",
        toJson
          "packages/proofs/ASPProof/SearchRouteSemanticPathCacheSeparation.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 4)
    , ("axiomDependentDeclarationCount", toJson 2)
    , ("declarations", toJson declarations)
    , ("axiomInventory",
        toJson ["Classical.choice", "Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc",
        toJson "00.94-semantic-path-cache-execution-separation")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteSemanticPathCacheSeparation
