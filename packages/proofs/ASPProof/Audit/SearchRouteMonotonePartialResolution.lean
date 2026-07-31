import ASPProof.Audit.Core
import ASPProof.SearchRouteMonotonePartialResolution

namespace ASPProof.Audit.SearchRouteMonotonePartialResolution

def declarationFamilies : List (String × List String) :=
  [
    (
      "dimension-finality",
      [
        "consumed_refines_only_to_consumed",
        "no_effect_refines_only_to_no_effect",
        "strict_refinement_is_not_reflexive",
        "resolved_plus_unknown_equals_three",
        "strict_refinement_decreases_unknown"
      ]
    ),
    (
      "bounded-resolution",
      [
        "resolution_step_advances_revision",
        "resolution_step_decreases_unknown",
        "resolution_run_length_le_initial_unknown",
        "example_token_finalization_is_valid",
        "example_terminal_money_rewrite_is_invalid",
        "example_unknown_count_decreases"
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
      ("rfc", .str "10.05.10.00.56"),
      (
        "module",
        .str "ASPProof.SearchRouteMonotonePartialResolution"
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

end ASPProof.Audit.SearchRouteMonotonePartialResolution

