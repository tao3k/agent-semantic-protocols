-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.GlobalRuntimeServer

namespace ASPProof.Audit.GlobalRuntimeServer

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.GlobalRuntimeServer.identity_unique_per_state_home
      theoremFamily := "global-server-state-home-identity"
      rfcClauseIds := ["ASP-RFC-10.32-GLOBAL-SERVER-IDENTITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.project_id_cannot_create_server_identity
      theoremFamily := "no-project-server-identity"
      rfcClauseIds := ["ASP-RFC-10.32-GLOBAL-SERVER-IDENTITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.ensure_is_idempotent
      theoremFamily := "ensure-idempotence"
      rfcClauseIds := ["ASP-RFC-10.32-ENSURE-IDEMPOTENCE"] }
  , { name := `ASPProof.GlobalRuntimeServer.concurrent_ensure_returns_one_identity
      theoremFamily := "concurrent-ensure-singleton"
      rfcClauseIds :=
        [ "ASP-RFC-10.32-GLOBAL-SERVER-IDENTITY"
        , "ASP-RFC-10.32-ENSURE-IDEMPOTENCE" ] }
  , { name := `ASPProof.GlobalRuntimeServer.begin_stop_rejects_new_requests
      theoremFamily := "graceful-stop-admission-fence"
      rfcClauseIds := ["ASP-RFC-10.32-GRACEFUL-STOP"] }
  , { name := `ASPProof.GlobalRuntimeServer.completed_stop_requires_drain_and_checkpoint
      theoremFamily := "graceful-stop-drain-checkpoint"
      rfcClauseIds := ["ASP-RFC-10.32-GRACEFUL-STOP"] }
  , { name := `ASPProof.GlobalRuntimeServer.restart_converges_under_explicit_assumptions
      theoremFamily := "restart-assumption-convergence"
      rfcClauseIds := ["ASP-RFC-10.32-RESTART-CONVERGENCE"] }
  , { name := `ASPProof.GlobalRuntimeServer.restart_is_not_unconditionally_claimed
      theoremFamily := "restart-assumption-counterexample"
      rfcClauseIds := ["ASP-RFC-10.32-RESTART-CONVERGENCE"] }
  , { name := `ASPProof.GlobalRuntimeServer.startup_failure_is_typed
      theoremFamily := "typed-startup-failure"
      rfcClauseIds := ["ASP-RFC-10.32-TYPED-FAILURE"] }
  , { name := `ASPProof.GlobalRuntimeServer.dispatch_updates_only_the_target_project
      theoremFamily := "internal-project-dispatch-isolation"
      rfcClauseIds := ["ASP-RFC-10.32-INTERNAL-PROJECT-DISPATCH"] }
  , { name := `ASPProof.GlobalRuntimeServer.dispatch_appends_to_the_target_project
      theoremFamily := "internal-project-dispatch-targeting"
      rfcClauseIds := ["ASP-RFC-10.32-INTERNAL-PROJECT-DISPATCH"] }
  , { name := `ASPProof.GlobalRuntimeServer.provider_failure_preserves_global_server_lifecycle
      theoremFamily := "provider-failure-global-lifecycle-isolation"
      rfcClauseIds := ["ASP-RFC-10.32-TYPED-FAILURE"] }
  , { name := `ASPProof.GlobalRuntimeServer.provider_failure_isolated_from_other_projects
      theoremFamily := "provider-failure-project-isolation"
      rfcClauseIds :=
        [ "ASP-RFC-10.32-INTERNAL-PROJECT-DISPATCH"
        , "ASP-RFC-10.32-TYPED-FAILURE" ] }
  , { name := `ASPProof.GlobalRuntimeServer.runtime_ensure_preserves_fixed_hook_contract
      theoremFamily := "fixed-hook-contract-preservation"
      rfcClauseIds := ["ASP-RFC-10.32-FIXED-HOOK-CONTRACT"] }
  , { name := `ASPProof.GlobalRuntimeServer.endpoint_hint_cannot_establish_ready
      theoremFamily := "endpoint-hint-not-liveness"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.socket_file_existence_cannot_establish_ready
      theoremFamily := "socket-file-not-liveness"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.authenticated_matching_handshake_admits_liveness
      theoremFamily := "authenticated-socket-liveness"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.liveness_requires_authenticated_matching_handshake
      theoremFamily := "socket-liveness-handshake-necessity"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.missing_socket_allows_election
      theoremFamily := "missing-socket-election"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.refused_socket_allows_election
      theoremFamily := "refused-socket-election"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.protocol_mismatch_fails_closed_without_spawn_or_cleanup
      theoremFamily := "protocol-mismatch-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY"] }
  , { name := `ASPProof.GlobalRuntimeServer.identity_mismatch_fails_closed_without_spawn_or_cleanup
      theoremFamily := "identity-mismatch-fail-closed"
      rfcClauseIds :=
        [ "ASP-RFC-10.32-GLOBAL-SERVER-IDENTITY"
        , "ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY" ] }
  , { name := `ASPProof.GlobalRuntimeServer.operator_stop_has_precedence
      theoremFamily := "operator-stop-precedence"
      rfcClauseIds :=
        [ "ASP-RFC-10.32-GRACEFUL-STOP"
        , "ASP-RFC-10.32-SOCKET-LIVENESS-AUTHORITY" ] }
  , { name := `ASPProof.GlobalRuntimeServer.secure_admission_requires_kernel_peer_uid_match
      theoremFamily := "kernel-peer-uid-admission"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.accepted_nonce_belongs_to_owner_epoch_and_is_fresh
      theoremFamily := "nonce-owner-epoch-single-use"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.repeated_nonce_fails_closed
      theoremFamily := "nonce-replay-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.repeated_nonce_causes_no_mutation
      theoremFamily := "nonce-replay-no-mutation"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.accepted_nonce_is_recorded_as_used
      theoremFamily := "nonce-recorded-single-use"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.recorded_nonce_replay_fails_closed
      theoremFamily := "recorded-nonce-replay-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.nonce_capacity_exhaustion_fails_closed_without_eviction
      theoremFamily := "nonce-capacity-exhaustion-no-eviction"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.nonce_capacity_exhaustion_causes_no_mutation
      theoremFamily := "nonce-capacity-exhaustion-no-mutation"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.accepted_nonce_preserves_capacity_bound
      theoremFamily := "nonce-capacity-bound"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.secure_runtime_paths_require_current_uid_and_non_symlink
      theoremFamily := "runtime-directory-owner-symlink-security"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.secure_runtime_paths_require_private_modes
      theoremFamily := "runtime-path-private-modes"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.bounded_admission_requires_all_resource_bounds
      theoremFamily := "socket-resource-bounds"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.oversized_frame_fails_closed
      theoremFamily := "frame-bound-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.connection_exhaustion_fails_closed
      theoremFamily := "connection-bound-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.hook_worker_exhaustion_fails_closed
      theoremFamily := "hook-worker-bound-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.drain_bound_exhaustion_fails_closed
      theoremFamily := "drain-bound-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  , { name := `ASPProof.GlobalRuntimeServer.secret_log_field_is_redacted
      theoremFamily := "secret-redaction"
      rfcClauseIds := ["ASP-RFC-10.32-SOCKET-SECURITY-HARDENING"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.GlobalRuntimeServer"
    "ASPProof/GlobalRuntimeServer.lean"
    targets

end ASPProof.Audit.GlobalRuntimeServer
