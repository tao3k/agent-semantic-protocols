import ASPProof.Audit.Core
import ASPProof.ActivationRepairLiveness

namespace ASPProof.Audit.ActivationRepairLiveness

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.ActivationRepairLiveness.all_available_satisfies_guarantees
      theoremFamily := "complete-guarantee-witness"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-NEXT"] }
  , { name := `ASPProof.ActivationRepairLiveness.complete_repair_is_reachable
      theoremFamily := "conditional-bounded-liveness"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-NEXT",
        "ASP-RFC-10.05-ARCL-RETIREMENT"] }
  , { name := `ASPProof.ActivationRepairLiveness.every_step_decreases_remaining
      theoremFamily := "strict-progress-ranking"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-NEXT"] }
  , { name := `ASPProof.ActivationRepairLiveness.initial_progress_requires_authority
      theoremFamily := "authority-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-AUTHORITY"] }
  , { name := `ASPProof.ActivationRepairLiveness.capability_progress_requires_stability
      theoremFamily := "capability-stability-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-CAPABILITY"] }
  , { name := `ASPProof.ActivationRepairLiveness.capability_progress_requires_scheduling
      theoremFamily := "writer-scheduling-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-SCHEDULER"] }
  , { name := `ASPProof.ActivationRepairLiveness.committed_progress_requires_durable_outbox
      theoremFamily := "durable-outbox-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-OUTBOX"] }
  , { name := `ASPProof.ActivationRepairLiveness.enqueued_progress_requires_delivery_fairness
      theoremFamily := "delivery-fairness-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-DELIVERY"] }
  , { name := `ASPProof.ActivationRepairLiveness.published_progress_requires_host_lease
      theoremFamily := "host-lease-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-HOST-LEASE"] }
  , { name := `ASPProof.ActivationRepairLiveness.host_progress_requires_dispatch_reconciliation
      theoremFamily := "dispatch-reconciliation-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-DISPATCH"] }
  , { name := `ASPProof.ActivationRepairLiveness.reconciled_progress_requires_fresh_evidence
      theoremFamily := "fresh-readiness-evidence-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-FRESHNESS"] }
  , { name := `ASPProof.ActivationRepairLiveness.ready_progress_requires_rollback_clock
      theoremFamily := "rollback-clock-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-ROLLBACK"] }
  , { name := `ASPProof.ActivationRepairLiveness.closed_rollback_requires_retirement_store
      theoremFamily := "retirement-store-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-RETIREMENT"] }
  , { name := `ASPProof.ActivationRepairLiveness.missing_authority_blocks_initial_progress
      theoremFamily := "missing-authority-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-AUTHORITY"] }
  , { name := `ASPProof.ActivationRepairLiveness.unstable_capability_blocks_commit_progress
      theoremFamily := "unstable-capability-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-CAPABILITY"] }
  , { name := `ASPProof.ActivationRepairLiveness.unscheduled_writer_blocks_commit_progress
      theoremFamily := "unscheduled-writer-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-SCHEDULER"] }
  , { name := `ASPProof.ActivationRepairLiveness.unavailable_outbox_blocks_publication_enqueue
      theoremFamily := "unavailable-outbox-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-OUTBOX"] }
  , { name := `ASPProof.ActivationRepairLiveness.unfair_delivery_blocks_publication
      theoremFamily := "unfair-delivery-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-DELIVERY"] }
  , { name := `ASPProof.ActivationRepairLiveness.missing_host_lease_blocks_observation
      theoremFamily := "missing-host-lease-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-HOST-LEASE"] }
  , { name := `ASPProof.ActivationRepairLiveness.missing_dispatch_fairness_blocks_reconciliation
      theoremFamily := "unreconciled-dispatch-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-DISPATCH"] }
  , { name := `ASPProof.ActivationRepairLiveness.missing_fresh_evidence_blocks_readiness
      theoremFamily := "missing-fresh-evidence-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-FRESHNESS"] }
  , { name := `ASPProof.ActivationRepairLiveness.frozen_clock_blocks_retirement_window
      theoremFamily := "frozen-rollback-clock-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-ROLLBACK"] }
  , { name := `ASPProof.ActivationRepairLiveness.missing_retirement_store_blocks_retirement
      theoremFamily := "missing-retirement-store-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ARCL-RETIREMENT"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.ActivationRepairLiveness"
    "ASPProof/ActivationRepairLiveness.lean"
    targets

end ASPProof.Audit.ActivationRepairLiveness
