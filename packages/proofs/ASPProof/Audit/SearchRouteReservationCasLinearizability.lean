import ASPProof.Audit.Core
import ASPProof.SearchRouteReservationCasLinearizability

namespace ASPProof.Audit.SearchRouteReservationCasLinearizability

def declarationFamilies : List (String × List String) :=
  [
    (
      "cas-linearizability",
      [
        "commit_preserves_reservation_identity",
        "commit_preserves_amount",
        "commit_advances_revision",
        "competing_old_revision_is_rejected",
        "consume_proposal_is_initially_allowed",
        "quarantine_proposal_is_initially_allowed",
        "quarantine_proposal_is_stale_after_consume"
      ]
    ),
    (
      "atomic-lifecycle",
      [
        "atomic_commit_preserves_ledger_total",
        "atomic_commit_advances_record_revision",
        "consumed_record_rejects_every_proposal",
        "released_record_rejects_every_proposal"
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
      ("rfc", .str "10.05.10.00.53"),
      (
        "module",
        .str "ASPProof.SearchRouteReservationCasLinearizability"
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

end ASPProof.Audit.SearchRouteReservationCasLinearizability

