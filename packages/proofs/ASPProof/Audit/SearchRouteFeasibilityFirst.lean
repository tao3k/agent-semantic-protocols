import ASPProof.Audit.Core
import ASPProof.SearchRouteFeasibilityFirst

namespace ASPProof.Audit.SearchRouteFeasibilityFirst

def declarationFamilies : List (String × List String) :=
  [
    (
      "admission",
      [
        "admitted_no_worse_implies_feasible",
        "longer_route_is_admitted_over_short_route"
      ]
    ),
    (
      "budget-counterexample",
      [
        "hop_first_prefers_short_expensive",
        "short_expensive_is_infeasible",
        "longer_route_is_feasible",
        "hop_first_preference_does_not_imply_feasibility"
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
      ("rfc", .str "10.05.10.00.35"),
      (
        "module",
        .str "ASPProof.SearchRouteFeasibilityFirst"
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

end ASPProof.Audit.SearchRouteFeasibilityFirst

