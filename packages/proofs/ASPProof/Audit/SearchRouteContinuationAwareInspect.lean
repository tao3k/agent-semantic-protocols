import ASPProof.Audit.Core
import ASPProof.SearchRouteContinuationAwareInspect

namespace ASPProof.Audit.SearchRouteContinuationAwareInspect

def declarationFamilies : List (String × List String) :=
  [
    (
      "split-progress",
      [
        "greedy_split_is_informative",
        "strategic_split_is_informative",
        "greedy_is_immediately_no_worse"
      ]
    ),
    (
      "continuation-counterexample",
      [
        "greedy_minimum_total_is_three",
        "strategic_minimum_total_is_two",
        "greedy_is_not_continuation_feasible",
        "strategic_is_continuation_feasible",
        "immediate_preference_does_not_imply_continuation_feasibility"
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
      ("rfc", .str "10.05.10.00.43"),
      (
        "module",
        .str "ASPProof.SearchRouteContinuationAwareInspect"
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

end ASPProof.Audit.SearchRouteContinuationAwareInspect

