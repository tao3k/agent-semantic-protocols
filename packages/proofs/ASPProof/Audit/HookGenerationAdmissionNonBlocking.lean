-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.unmatched_policy_cannot_erase_normalized_apply_patch_mutation
      theoremFamily := "post-tool-mutation-policy-independence"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.mutation_projection_is_policy_independent
      theoremFamily := "canonical-tool-action-mutation-projection"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.runtime_server_submission_does_not_wait_for_candidate
      theoremFamily := "daemon-owned-candidate-discovery"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.runtime_server_submission_survives_hook_exit
      theoremFamily := "daemon-owned-generation-background-work"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  , { name :=
        `ASPProof.HookGenerationAdmissionNonBlocking.client_candidate_discovery_violates_nonblocking_submission
      theoremFamily := "hook-client-candidate-discovery-counterexample"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-GENERATION-NONBLOCKING"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookGenerationAdmissionNonBlocking"
    "ASPProof/HookGenerationAdmissionNonBlocking.lean"
    targets

end ASPProof.Audit.HookGenerationAdmissionNonBlocking
