import ASPProof.Audit.Core
import ASPProof.SearchRouteReservationLifecycleConservation

namespace ASPProof.Audit.SearchRouteReservationLifecycleConservation

def declarationFamilies : List (String × List String) :=
  [
    (
      "ledger-conservation",
      [
        "reserve_preserves_total",
        "consume_held_preserves_total",
        "quarantine_held_preserves_total",
        "release_held_preserves_total",
        "consume_quarantined_preserves_total",
        "release_quarantined_preserves_total",
        "every_ledger_transition_preserves_total"
      ]
    ),
    (
      "reservation-lifecycle",
      [
        "consumed_is_terminal",
        "released_is_terminal",
        "record_transition_advances_revision",
        "quarantine_release_path_preserves_one_hundred",
        "quarantine_consume_path_preserves_one_hundred"
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
      ("rfc", .str "10.05.10.00.52"),
      (
        "module",
        .str "ASPProof.SearchRouteReservationLifecycleConservation"
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

end ASPProof.Audit.SearchRouteReservationLifecycleConservation

