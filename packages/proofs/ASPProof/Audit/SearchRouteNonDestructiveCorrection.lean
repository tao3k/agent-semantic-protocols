import ASPProof.Audit.Core
import ASPProof.SearchRouteNonDestructiveCorrection

namespace ASPProof.Audit.SearchRouteNonDestructiveCorrection

def declarationFamilies : List (String × List String) :=
  [
    (
      "correction-separation",
      [
        "ordinary_consumed_to_no_effect_remains_invalid",
        "apply_correction_preserves_original",
        "corrected_view_carries_authorization",
        "corrected_view_advances_revision"
      ]
    ),
    (
      "authorized-correction",
      [
        "example_correction_is_bound",
        "example_historical_outcome_remains_consumed",
        "example_effective_outcome_becomes_no_effect",
        "mismatched_authority_rejects_correction",
        "mismatched_request_rejects_correction",
        "mismatched_dimension_rejects_correction"
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
      ("rfc", .str "10.05.10.00.57"),
      (
        "module",
        .str "ASPProof.SearchRouteNonDestructiveCorrection"
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

end ASPProof.Audit.SearchRouteNonDestructiveCorrection
