import ASPProof.SearchRouteCertifiedPotentialCompletion
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteCertifiedPotentialCompletion

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
       "bounded_certificate_is_sound"
       "bounded-optimality"
       "BoundedAdmissible state certificate -> GapOptimal problem chosen gap"
       "ASP-RFC-10.05-CPC-BOUNDED"
   , theoremDeclaration
       "exact_certificate_is_sound"
       "exact-optimality"
       "ExactAdmissible state certificate -> GlobalOptimal problem chosen"
       "ASP-RFC-10.05-CPC-EXACT"
       ["propext"]
   , theoremDeclaration
       "exact_admission_requires_zero_potential"
       "termination-gate"
       "ExactAdmissible state certificate -> state.potential = 0"
       "ASP-RFC-10.05-CPC-ZERO"
   , theoremDeclaration
       "exact_admission_requires_current_generation"
       "generation-binding"
       "ExactAdmissible state certificate -> certificate.generation = state.generation"
       "ASP-RFC-10.05-CPC-GENERATION"
   , theoremDeclaration
       "stale_exact_certificate_is_rejected"
       "stale-rejection"
       "not (ExactAdmissible currentState staleExactCertificate)"
       "ASP-RFC-10.05-CPC-STALE"
       ["propext"]
   , theoremDeclaration
       "expensive_candidate_is_not_global_optimum"
       "local-zero-counterexample"
       "not (GlobalOptimal exampleProblem expensive)"
       "ASP-RFC-10.05-CPC-LOCAL-ZERO"
       ["propext"]
   , theoremDeclaration
       "zero_potential_with_incomplete_coverage_is_not_exact"
       "local-zero-counterexample"
       "zero potential and incomplete admission and non-optimal chosen candidate"
       "ASP-RFC-10.05-CPC-LOCAL-ZERO"
       ["propext"]
   , theoremDeclaration
       "current_exact_certificate_is_admitted"
       "current-completion"
       "ExactAdmissible currentState currentExactCertificate"
       "ASP-RFC-10.05-CPC-CURRENT"
   , theoremDeclaration
       "current_exact_completion_is_optimal"
       "current-completion"
       "GlobalOptimal exampleProblem cheap"
       "ASP-RFC-10.05-CPC-CURRENT"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module",
        toJson "ASPProof.SearchRouteCertifiedPotentialCompletion")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteCertifiedPotentialCompletion.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 4)
    , ("axiomDependentDeclarationCount", toJson 5)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.61-certified-potential-completion-gate")
    , ("status", toJson "kernel-compiled")
    ]

end ASPProof.Audit.SearchRouteCertifiedPotentialCompletion

