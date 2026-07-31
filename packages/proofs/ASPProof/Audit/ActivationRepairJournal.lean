import ASPProof.Audit.Core
import ASPProof.ActivationRepairJournal

namespace ASPProof.Audit.ActivationRepairJournal

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.ActivationRepairJournal.empty_journal_is_consistent
      theoremFamily := "empty-journal-invariant"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-BINDING"] }
  , { name := `ASPProof.ActivationRepairJournal.atomic_commit_is_consistent
      theoremFamily := "atomic-commit-invariant"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-ATOMIC",
        "ASP-RFC-10.05-ARAJ-BINDING"] }
  , { name := `ASPProof.ActivationRepairJournal.split_commit_with_enqueue_is_inconsistent
      theoremFamily := "split-commit-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-ATOMIC",
        "ASP-RFC-10.05-ARAJ-OUTBOX"] }
  , { name := `ASPProof.ActivationRepairJournal.crash_before_enqueue_remains_replayable
      theoremFamily := "pre-enqueue-crash-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-ATOMIC"] }
  , { name := `ASPProof.ActivationRepairJournal.enqueue_preserves_head_binding
      theoremFamily := "enqueue-head-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-BINDING",
        "ASP-RFC-10.05-ARAJ-OUTBOX"] }
  , { name := `ASPProof.ActivationRepairJournal.duplicate_sink_delivery_is_idempotent_for_acceptance
      theoremFamily := "sink-acceptance-idempotency"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-KEY",
        "ASP-RFC-10.05-ARAJ-SINK"] }
  , { name := `ASPProof.ActivationRepairJournal.delivery_does_not_advance_activation_head
      theoremFamily := "delivery-head-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-KEY"] }
  , { name := `ASPProof.ActivationRepairJournal.crash_after_sink_can_record_ack
      theoremFamily := "lost-ack-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-SINK",
        "ASP-RFC-10.05-ARAJ-ACK"] }
  , { name := `ASPProof.ActivationRepairJournal.replay_after_lost_ack_preserves_sink_acceptance
      theoremFamily := "lost-ack-replay-idempotency"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-KEY",
        "ASP-RFC-10.05-ARAJ-SINK"] }
  , { name := `ASPProof.ActivationRepairJournal.wrong_sink_key_cannot_ack
      theoremFamily := "conflicting-sink-key-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-ACK"] }
  , { name := `ASPProof.ActivationRepairJournal.commit_without_ack_cannot_derive_ready
      theoremFamily := "unacknowledged-readiness-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-READY"] }
  , { name := `ASPProof.ActivationRepairJournal.ready_without_rollback_cannot_retire
      theoremFamily := "open-rollback-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-RETIRE"] }
  , { name := `ASPProof.ActivationRepairJournal.proof_gated_retirement_is_consistent
      theoremFamily := "proof-gated-retirement-invariant"
      rfcClauseIds := ["ASP-RFC-10.05-ARAJ-READY",
        "ASP-RFC-10.05-ARAJ-RETIRE"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.ActivationRepairJournal"
    "ASPProof/ActivationRepairJournal.lean"
    targets

end ASPProof.Audit.ActivationRepairJournal
