import ASPProof.SearchRouteWellFoundedInspectPotential
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteWellFoundedInspectPotential

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String) : Json :=
  Json.mkObj
    [ ("name", toJson name)
    , ("kind", toJson "theorem")
    , ("theoremFamily", toJson theoremFamily)
    , ("type", toJson type)
    , ("rfcClauseIds", toJson [rfcClauseId])
    , ("axioms", toJson ["Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    ]

def declarations : Array Json :=
  #[ theoremDeclaration
       "componentwise_progress_strictly_decreases"
       "potential-descent"
       "ComponentwiseProgress current next -> TotalPotential next < TotalPotential current"
       "ASP-RFC-10.05-WIP-STRICT"
   , theoremDeclaration
       "no_productive_self_transition"
       "self-loop-exclusion"
       "not (ComponentwiseProgress state state)"
       "ASP-RFC-10.05-WIP-NO-SELF"
   , theoremDeclaration
       "productive_transition_is_not_stalled"
       "stall-exclusion"
       "ComponentwiseProgress current next -> not (Stalled current next)"
       "ASP-RFC-10.05-WIP-NO-STALL"
   , theoremDeclaration
       "resolved_state_has_no_productive_successor"
       "resolved-finality"
       "Resolved current -> not (ComponentwiseProgress current next)"
       "ASP-RFC-10.05-WIP-RESOLVED"
   , theoremDeclaration
       "productive_trace_budget"
       "trace-budget"
       "productive trace -> TotalPotential final + steps <= TotalPotential initial"
       "ASP-RFC-10.05-WIP-TRACE"
   , theoremDeclaration
       "productive_round_count_is_bounded"
       "round-bound"
       "productive trace -> steps <= TotalPotential initial"
       "ASP-RFC-10.05-WIP-BOUND"
   , theoremDeclaration
       "positive_productive_trace_is_not_a_cycle"
       "cycle-exclusion"
       "positive productive trace -> final state != initial state"
       "ASP-RFC-10.05-WIP-NO-CYCLE"
   , theoremDeclaration
       "ambiguity_need_not_strictly_decrease"
       "ambiguity-counterexample"
       "ComponentwiseProgress evidence step and not ambiguity decrease"
       "ASP-RFC-10.05-WIP-AMBIGUITY-GAP"
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteWellFoundedInspectPotential")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteWellFoundedInspectPotential.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 0)
    , ("axiomDependentDeclarationCount", toJson declarations.size)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.60-well-founded-inspect-potential")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteWellFoundedInspectPotential

