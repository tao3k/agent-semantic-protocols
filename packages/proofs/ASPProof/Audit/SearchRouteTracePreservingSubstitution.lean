import ASPProof.SearchRouteTracePreservingSubstitution
import Lean

namespace ASPProof.Audit.SearchRouteTracePreservingSubstitution

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
       "ASPProof.SearchRouteTracePreservingSubstitution.safe_substitution_preserves_completion"
       "completion-preservation"
       "safe route substitution preserves the graph, completion, and potential boundary"
       #["ASP-RFC-10.05-TRS-COMPLETION"]
       #[]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.safe_substitution_preserves_cache_transitions"
       "cache-transition-preservation"
       "safe route substitution preserves search-cache and model-prefix-cache transitions"
       #["ASP-RFC-10.05-TRS-CACHE"]
       #[]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.safe_substitution_is_pareto_no_worse"
       "pareto-preservation"
       "safe route substitution is no worse in token cost and interaction rounds"
       #["ASP-RFC-10.05-TRS-PARETO"]
       #[]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.trace_refinement_preserves_requirements"
       "trace-refinement"
       "trace refinement preserves every evidence requirement realized by the original route"
       #["ASP-RFC-10.05-TRS-REFINEMENT"]
       #[]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.safe_substitution_preserves_requirements"
       "requirement-realization"
       "safe route substitution preserves realization of the required evidence atoms"
       #["ASP-RFC-10.05-TRS-REALIZATION"]
       #[]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.endpoint_pareto_does_not_preserve_trace"
       "trace-counterexample"
       "completion equality and Pareto improvement do not imply evidence trace preservation"
       #["ASP-RFC-10.05-TRS-TRACE-COUNTEREXAMPLE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.trace_and_endpoint_do_not_preserve_cache_transition"
       "cache-counterexample"
       "completion equality, trace refinement, and lower cost do not imply cache alignment"
       #["ASP-RFC-10.05-TRS-CACHE-COUNTEREXAMPLE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.missing_provenance_breaks_requirement"
       "provenance-requirement"
       "removing provenance breaks a provenance obligation while retaining the answer"
       #["ASP-RFC-10.05-TRS-PROVENANCE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteTracePreservingSubstitution.safe_substitution_rejects_short_route"
       "unsafe-substitution-rejection"
       "safe substitution rejects a cheaper endpoint-equivalent route with an incomplete trace"
       #["ASP-RFC-10.05-TRS-REJECT"]
       #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteTracePreservingSubstitution")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteTracePreservingSubstitution.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 9)
    , ("axiomFreeDeclarationCount", Json.num 5)
    , ("axiomDependentDeclarationCount", Json.num 4)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteTracePreservingSubstitution
