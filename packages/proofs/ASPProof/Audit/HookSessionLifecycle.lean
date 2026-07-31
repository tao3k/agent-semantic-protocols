import ASPProof.Audit.Core
import ASPProof.HookSessionLifecycle

namespace ASPProof.Audit.HookSessionLifecycle

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.HookSessionLifecycle.registered_absence_requires_materialization
      theoremFamily := "registered-absence-materialization"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-CONFIGURED",
        "ASP-RFC-10.05-HSLA-MATERIALIZE"] }
  , { name := `ASPProof.HookSessionLifecycle.legacy_resume_is_not_executable_for_registered_absence
      theoremFamily := "legacy-resume-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-RESUME"] }
  , { name := `ASPProof.HookSessionLifecycle.partial_spawn_requires_registry_reconciliation
      theoremFamily := "partial-spawn-reconciliation"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-RECONCILE"] }
  , { name := `ASPProof.HookSessionLifecycle.bound_registry_without_target_requires_target_binding
      theoremFamily := "message-target-reconciliation"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-TARGET",
        "ASP-RFC-10.05-HSLA-RECONCILE"] }
  , { name := `ASPProof.HookSessionLifecycle.stale_generation_requires_replacement
      theoremFamily := "stale-generation-replacement"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-GENERATION",
        "ASP-RFC-10.05-HSLA-REPLACEMENT"] }
  , { name := `ASPProof.HookSessionLifecycle.materialization_produces_active_resident
      theoremFamily := "materialization-active-invariant"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-MATERIALIZE"] }
  , { name := `ASPProof.HookSessionLifecycle.active_resident_dispatches
      theoremFamily := "active-dispatch-selection"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ACTION"] }
  , { name := `ASPProof.HookSessionLifecycle.dispatch_action_implies_active
      theoremFamily := "dispatch-active-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ACTION"] }
  , { name := `ASPProof.HookSessionLifecycle.bound_receipt_authorizes_dispatch
      theoremFamily := "receipt-bound-dispatch"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-TARGET",
        "ASP-RFC-10.05-HSLA-RECEIPT"] }
  , { name := `ASPProof.HookSessionLifecycle.replacement_increments_generation_and_revision
      theoremFamily := "replacement-monotonicity"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-REPLACEMENT"] }
  , { name := `ASPProof.HookSessionLifecycle.old_receipt_cannot_dispatch_replacement
      theoremFamily := "stale-receipt-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-GENERATION",
        "ASP-RFC-10.05-HSLA-RECEIPT"] }
  , { name := `ASPProof.HookSessionLifecycle.materialization_eliminates_registered_absence_deadlock
      theoremFamily := "materialization-deadlock-elimination"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-MATERIALIZE",
        "ASP-RFC-10.05-HSLA-ACTION"] }
  , { name := `ASPProof.HookSessionLifecycle.manifest_activation_does_not_imply_provider_execution
      theoremFamily := "activation-status-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ECOSYSTEM"] }
  , { name := `ASPProof.HookSessionLifecycle.active_resident_does_not_imply_end_to_end_execution
      theoremFamily := "local-readiness-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ECOSYSTEM"] }
  , { name := `ASPProof.HookSessionLifecycle.observed_ecosystem_first_repairs_hook_routing
      theoremFamily := "observed-hook-routing-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-CALIBRATION"] }
  , { name := `ASPProof.HookSessionLifecycle.routed_hook_then_repairs_activation_schema
      theoremFamily := "activation-schema-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-CALIBRATION"] }
  , { name := `ASPProof.HookSessionLifecycle.repaired_schema_then_restores_workspace_owner
      theoremFamily := "workspace-owner-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-CALIBRATION"] }
  , { name := `ASPProof.HookSessionLifecycle.restored_owner_then_materializes_provider_generation
      theoremFamily := "provider-generation-recovery"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-CALIBRATION"] }
  , { name := `ASPProof.HookSessionLifecycle.ecosystem_dispatch_implies_end_to_end_execution
      theoremFamily := "end-to-end-dispatch-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ECOSYSTEM"] }
  , { name := `ASPProof.HookSessionLifecycle.fully_executable_ecosystem_dispatches
      theoremFamily := "end-to-end-dispatch-sufficiency"
      rfcClauseIds := ["ASP-RFC-10.05-HSLA-ECOSYSTEM"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookSessionLifecycle"
    "ASPProof/HookSessionLifecycle.lean"
    targets

end ASPProof.Audit.HookSessionLifecycle
