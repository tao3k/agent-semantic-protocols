import ASPProof.Audit.Core
import ASPProof.HookGenerationAdmissionNonBlocking

namespace ASPProof.Audit.HookGenerationAdmissionNonBlocking

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.failed_admission_preserves_generic_commands
      theoremFamily := "hook-generic-command-availability"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.failed_admission_preserves_control_plane_recovery
      theoremFamily := "hook-control-plane-recovery-availability"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.failed_admission_does_not_release_direct_source_read
      theoremFamily := "direct-source-read-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.failed_generation_query_remains_data_plane_fail_closed
      theoremFamily := "generation-query-data-plane-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.failed_admission_cannot_globally_deadlock_the_hook
      theoremFamily := "hook-global-deadlock-exclusion"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.hook_allow_does_not_imply_generation_query_execution
      theoremFamily := "hook-data-plane-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookGenerationAdmissionNonBlocking"
    "ASPProof/HookGenerationAdmissionNonBlocking.lean"
    targets

end ASPProof.Audit.HookGenerationAdmissionNonBlocking
