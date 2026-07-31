import ASPProof.Audit.Core
import ASPProof.SearchRouteAuthorizedQuarantineResolution

namespace ASPProof.Audit.SearchRouteAuthorizedQuarantineResolution

def declarationFamilies : List (String × List String) :=
  [
    (
      "ledger-conservation",
      [
        "resolution_preserves_total",
        "resolution_clears_quarantine",
        "completed_effect_preserves_available",
        "no_effect_preserves_consumed",
        "resolution_evidence_is_exclusive",
        "authorized_resolution_preserves_total",
        "authority_mismatch_rejects_resolution"
      ]
    ),
    (
      "concrete-resolution",
      [
        "example_initial_total_is_one_hundred",
        "example_completed_total_is_one_hundred",
        "example_no_effect_total_is_one_hundred",
        "example_completed_resolution",
        "example_no_effect_resolution"
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
      ("rfc", .str "10.05.10.00.51"),
      (
        "module",
        .str "ASPProof.SearchRouteAuthorizedQuarantineResolution"
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

end ASPProof.Audit.SearchRouteAuthorizedQuarantineResolution
