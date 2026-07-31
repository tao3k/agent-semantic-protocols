import ASPProof.SearchRouteCompletionEquivalentPareto
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteCompletionEquivalentPareto

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
       "cost_no_worse_reflexive"
       "cost-order"
       "CostNoWorse cost cost"
       "ASP-RFC-10.05-CEP-COST"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "cost_no_worse_transitive"
       "cost-order"
       "CostNoWorse first second -> CostNoWorse second third -> CostNoWorse first third"
       "ASP-RFC-10.05-CEP-COST"
   , theoremDeclaration
       "completion_equivalent_transitive"
       "completion-equivalence"
       "CompletionEquivalent first second -> CompletionEquivalent second third -> CompletionEquivalent first third"
       "ASP-RFC-10.05-CEP-EQUIVALENCE"
   , theoremDeclaration
       "dominance_is_irreflexive"
       "dominance-algebra"
       "not (Dominates route route)"
       "ASP-RFC-10.05-CEP-DOMINANCE"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "dominance_is_transitive"
       "dominance-algebra"
       "Dominates first second -> Dominates second third -> Dominates first third"
       "ASP-RFC-10.05-CEP-DOMINANCE"
   , theoremDeclaration
       "dominance_preserves_completion"
       "completion-preservation"
       "Dominates left right -> CompletionEquivalent left right"
       "ASP-RFC-10.05-CEP-COMPLETION"
   , theoremDeclaration
       "dominated_route_is_not_pareto_minimal"
       "pareto-pruning"
       "admissible better -> Dominates better dominated -> not (ParetoMinimal admissible dominated)"
       "ASP-RFC-10.05-CEP-PRUNE"
   , theoremDeclaration
       "fewer_hops_do_not_imply_full_cost_dominance"
       "hop-counterexample"
       "fewer graph hops and not CostNoWorse"
       "ASP-RFC-10.05-CEP-HOPS"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "local_efficiency_does_not_imply_global_dominance"
       "greedy-counterexample"
       "better local efficiency and same completion and not full CostNoWorse"
       "ASP-RFC-10.05-CEP-GREEDY"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "cheaper_incomplete_route_cannot_dominate_exact"
       "incomplete-exclusion"
       "cheap incomplete cost and not dominance over exact route"
       "ASP-RFC-10.05-CEP-INCOMPLETE"
       ["propext"]
   , theoremDeclaration
       "longer_complete_route_meets_budget_while_shorter_fails"
       "budget-feasibility"
       "longer complete route within budget and shorter route outside budget"
       "ASP-RFC-10.05-CEP-BUDGET"
       ["Quot.sound", "propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteCompletionEquivalentPareto")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteCompletionEquivalentPareto.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 5)
    , ("axiomDependentDeclarationCount", toJson 6)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.62-completion-equivalent-pareto-router")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteCompletionEquivalentPareto

