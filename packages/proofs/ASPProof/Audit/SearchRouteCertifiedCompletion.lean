import ASPProof.Audit.Core
import ASPProof.SearchRouteCertifiedCompletion

namespace ASPProof.Audit.SearchRouteCertifiedCompletion

def declarationFamilies : List (String × List String) :=
  [
    (
      "bound-soundness",
      [
        "bound_certificate_is_sound",
        "factor_one_certificate_is_exact"
      ]
    ),
    (
      "bounded-counterexample",
      [
        "example_six_is_global_lower_bound",
        "heuristic_is_factor_two_optimal",
        "heuristic_is_not_exact",
        "bounded_does_not_imply_exact"
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
      ("rfc", .str "10.05.10.00.39"),
      (
        "module",
        .str "ASPProof.SearchRouteCertifiedCompletion"
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

end ASPProof.Audit.SearchRouteCertifiedCompletion

