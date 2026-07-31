import ASPProof.SearchRouteCertifiedFrontierCoverage
import Lean

namespace ASPProof.Audit.SearchRouteCertifiedFrontierCoverage

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
       "ASPProof.SearchRouteCertifiedFrontierCoverage.global_frontier_coverage"
       "global-coverage"
       "aligned enumeration and frontier certificates imply coverage of every globally admissible route"
       #["ASP-RFC-10.05-CRF-GLOBAL"]
       #[]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.globally_undominated_route_is_retained"
       "undominated-retention"
       "every globally undominated admissible route belongs to the certified frontier"
       #["ASP-RFC-10.05-CRF-RETAIN"]
       #[]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.frontier_route_is_globally_undominated"
       "frontier-soundness"
       "every route retained by a certified frontier is globally undominated"
       #["ASP-RFC-10.05-CRF-SOUND"]
       #[]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.certified_frontier_is_exactly_global_undominated"
       "frontier-exactness"
       "frontier membership is equivalent to global admissibility and global undominatedness"
       #["ASP-RFC-10.05-CRF-EXACT"]
       #[]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.aligned_certificates_use_current_generation"
       "catalog-identity"
       "aligned certificates bind both certificate identities to the current graph generation"
       #["ASP-RFC-10.05-CRF-IDENTITY"]
       #[]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.stale_catalog_identity_is_rejected"
       "stale-rejection"
       "a certificate bound to a different catalog identity cannot align with the current identity"
       #["ASP-RFC-10.05-CRF-STALE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.local_frontier_can_miss_global_undominated_route"
       "local-frontier-counterexample"
       "a locally valid frontier over an incomplete catalog can omit a globally undominated route"
       #["ASP-RFC-10.05-CRF-COUNTEREXAMPLE"]
       #["Quot.sound", "propext"]
   , declaration
       "ASPProof.SearchRouteCertifiedFrontierCoverage.incomplete_catalog_has_no_global_enumeration_certificate"
       "enumeration-impossibility"
       "an incomplete route catalog cannot carry the required global enumeration certificate"
       #["ASP-RFC-10.05-CRF-NO-CERTIFICATE"]
       #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteCertifiedFrontierCoverage")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteCertifiedFrontierCoverage.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 8)
    , ("axiomFreeDeclarationCount", Json.num 5)
    , ("axiomDependentDeclarationCount", Json.num 3)
    , ("axiomInventory", strings #["Quot.sound", "propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteCertifiedFrontierCoverage
