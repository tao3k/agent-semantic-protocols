import ASPProof.Audit.Core
import ASPProof.SearchRouteCausalExecutionFeedback

namespace ASPProof.Audit.SearchRouteCausalExecutionFeedback

def declarationFamilies : List (String × List String) :=
  [
    (
      "causal-binding",
      [
        "feedback_strictly_advances_generation",
        "feedback_preserves_prior_problem",
        "feedback_preserves_admission_snapshot",
        "feasible_receipt_is_bound",
        "mismatched_receipt_is_rejected"
      ]
    ),
    (
      "post-hoc-counterexample",
      [
        "equal_observed_cost_does_not_determine_admission_feasibility",
        "successful_incomplete_execution_is_not_exact",
        "example_feedback_advances_to_generation_eight"
      ]
    )
  ]

def expectedAxiomFree : List String :=
  declarationFamilies.flatMap Prod.snd

def familyJson (family : String × List String) : Lean.Json :=
  Lean.Json.mkObj
    [
      ("family", .str family.1),
      ("declarations", .arr (family.2.map Lean.Json.str).toArray)
    ]

def auditManifest : Lean.Json :=
  Lean.Json.mkObj
    [
      ("schema", .str "asp.lean-audit-manifest.v1"),
      ("rfc", .str "10.05.10.00.47"),
      (
        "module",
        .str "ASPProof.SearchRouteCausalExecutionFeedback"
      ),
      (
        "families",
        .arr (declarationFamilies.map familyJson).toArray
      ),
      (
        "expectedAxiomFree",
        .arr (expectedAxiomFree.map Lean.Json.str).toArray
      )
    ]

end ASPProof.Audit.SearchRouteCausalExecutionFeedback
