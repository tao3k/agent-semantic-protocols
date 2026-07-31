import ASPProof.SearchRouteBoundedAdaptiveClosure
import Lean

namespace ASPProof.Audit.SearchRouteBoundedAdaptiveClosure

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
       "ASPProof.SearchRouteBoundedAdaptiveClosure.certified_transition_decreases"
       "weighted-transition-decrease"
       "every certified inspect or discovery transition decreases weighted closure measure"
       #["ASP-RFC-10.05-BAC-DECREASE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.certified_run_round_bound"
       "interaction-round-bound"
       "a certified run uses no more rounds than its initial closure measure"
       #["ASP-RFC-10.05-BAC-ROUND-BOUND"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.zero_measure_iff_closed"
       "zero-measure-completion"
       "weighted closure measure is zero exactly for a closed adaptive state"
       #["ASP-RFC-10.05-BAC-ZERO"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.no_certified_self_loop"
       "certified-loop-rejection"
       "an unchanged closure state cannot carry a certified transition"
       #["ASP-RFC-10.05-BAC-NO-LOOP"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.bounded_discovery_is_certified"
       "bounded-discovery-witness"
       "discovering one route with bounded introduced inspect potential is certified"
       #["ASP-RFC-10.05-BAC-DISCOVERY"]
       #[]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.weighted_measure_decreases_on_discovery"
       "productive-discovery-decrease"
       "weighted closure measure decreases across the concrete productive discovery"
       #["ASP-RFC-10.05-BAC-WEIGHTED"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.inspect_only_measure_increases_on_discovery"
       "inspect-only-counterexample"
       "inspect-only potential can increase during productive route discovery"
       #["ASP-RFC-10.05-BAC-INSPECT-ONLY"]
       #[]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.zero_inspect_potential_does_not_imply_closed"
       "zero-inspect-counterexample"
       "zero active inspect potential does not imply adaptive closure"
       #["ASP-RFC-10.05-BAC-NOT-CLOSED"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBoundedAdaptiveClosure.uncertified_self_loop_exists"
       "raw-self-loop-witness"
       "an unchanged raw execution step exists but has no certified progress"
       #["ASP-RFC-10.05-BAC-UNCERTIFIED-LOOP"]
       #[]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteBoundedAdaptiveClosure")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteBoundedAdaptiveClosure.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 9)
    , ("axiomFreeDeclarationCount", Json.num 3)
    , ("axiomDependentDeclarationCount", Json.num 6)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteBoundedAdaptiveClosure
