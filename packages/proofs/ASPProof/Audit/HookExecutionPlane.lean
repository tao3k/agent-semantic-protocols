-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.HookExecutionPlane
import ASPProof.HookPolicyAntiHardcoding

namespace ASPProof.Audit.HookExecutionPlane

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.HookExecutionPlane.wildcard_dispatch_does_not_imply_runtime_dispatch
      theoremFamily := "hook-wildcard-host-dispatch"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.codex_hook_has_one_plugin_bundle_authority
      theoremFamily := "hook-codex-single-plugin-bundle-authority"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.plugin_install_cannot_mutate_agent_or_runtime_lifecycle
      theoremFamily := "hook-plugin-install-control-plane-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.decision_authority_survives_diagnostic_projection_timeout
      theoremFamily := "hook-decision-authority-survives-event-projection-failure"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.bounded_scalar_document_predicate_is_not_a_bulk_dump
      theoremFamily := "hook-bounded-structured-document-scalar-predicate"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.decision_projection_never_waits_for_event_writer_lock
      theoremFamily := "hook-decision-projection-zero-lock-wait"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.unavailable_projection_cannot_reenter_runtime_or_change_authority
      theoremFamily := "hook-event-projection-failure-cannot-reenter-runtime"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.codex_hook_uses_canonical_host_event_name
      theoremFamily := "hook-codex-canonical-event-name"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.unrelated_action_is_local_zero_io
      theoremFamily := "hook-unrelated-local-passthrough"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.source_policy_never_enters_runtime
      theoremFamily := "hook-source-local-policy"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.command_policy_never_enters_runtime
      theoremFamily := "hook-command-local-policy"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.nested_policy_never_enters_runtime
      theoremFamily := "hook-nested-local-policy"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.missing_identity_fails_closed_without_runtime_dependency
      theoremFamily := "hook-missing-identity-local-deny"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.unrelated_typed_action_path_cannot_create_read_policy
      theoremFamily := "hook-typed-action-before-path"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.missing_identity_path_remains_local_fail_closed
      theoremFamily := "hook-missing-identity-path-local-deny"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.structured_document_axis_cannot_capture_provider_source
      theoremFamily := "hook-structured-document-provider-source-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.provider_source_axis_cannot_capture_structured_document
      theoremFamily := "hook-provider-source-structured-document-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_runtime_server_is_unreachable
      theoremFamily := "hook-runtime-server-unreachable"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.org_contract_interactive_owns_choice_description
      theoremFamily := "hook-choice-org-contract-description"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_execution_router_cannot_invent_choice
      theoremFamily := "hook-choice-execution-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.org_projection_does_not_create_transition_authority
      theoremFamily := "hook-choice-asp-admission-authority"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_recovery_can_reference_but_not_reimplement_choice_plane
      theoremFamily := "hook-choice-org-window-reference-only"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.full_layer_witness_implies_no_escape
      theoremFamily := "hook-conformance-four-layer-no-escape"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.missing_normalization_witness_can_escape
      theoremFamily := "hook-conformance-normalization-counterexample"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.enabled_rule_and_registered_extension_require_executable_witness
      theoremFamily := "hook-conformance-generated-rule-extension-witness"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.registered_extension_read_is_denied_independent_of_wrapper
      theoremFamily := "hook-read-provider-extension-wrapper-independent"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.plugin_disabled_task_cannot_complete_host_acceptance
      theoremFamily := "hook-host-acceptance-requires-plugin-delivery"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.asp_direct_config_mutation_is_not_valid_authority
      theoremFamily := "hook-marketplace-authority-owner-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.direct_asp_mutation_cannot_satisfy_authority_witness
      theoremFamily := "hook-direct-config-mutation-counterexample"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.desired_postcondition_subsumes_reconcile_step_failure
      theoremFamily := "hook-reconcile-desired-state-linearization"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.org_choice_plane_excludes_every_legacy_rust_surface
      theoremFamily := "hook-choice-no-legacy-rust-surface"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.response_owner_flushes_before_immediate_termination
      theoremFamily := "hook-response-owner-flush"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_deadline_cannot_depend_on_process_global_cleanup
      theoremFamily := "hook-no-process-global-cleanup"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.end_to_end_budget_implies_entry_execution_budget
      theoremFamily := "hook-latency-clock-separation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_data_path_cannot_reconcile_supervisor
      theoremFamily := "hook-no-supervisor-reconcile"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_data_path_cannot_contact_runtime_server
      theoremFamily := "hook-no-runtime-server-contact"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_data_path_cannot_inspect_agent_session_or_rollout
      theoremFamily := "hook-no-agent-session-or-rollout-io"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_data_path_cannot_schedule_codex_agent
      theoremFamily := "hook-no-codex-agent-scheduling"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.plugin_disabled_task_cannot_complete_host_acceptance
      theoremFamily := "hook-host-acceptance-plugin-provenance"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.source_byte_leak_cannot_complete_host_acceptance
      theoremFamily := "hook-host-acceptance-no-source-leak"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.missing_host_delivery_evidence_cannot_complete_host_acceptance
      theoremFamily := "hook-host-acceptance-delivery-evidence"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.hook_data_path_cannot_install_binary
      theoremFamily := "hook-no-binary-install"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.one_shot_hook_cache_is_process_independent
      theoremFamily := "hook-process-independent-mmap-cache"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.warm_hook_requires_digest_validated_read_only_mmap
      theoremFamily := "hook-warm-mmap-digest-validation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.matcher_snapshot_recovery_never_contacts_runtime
      theoremFamily := "hook-matcher-snapshot-zero-runtime"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.concurrent_hook_writers_require_workspace_os_lock
      theoremFamily := "hook-event-cross-process-writer-lock"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.event_writer_lock_timeout_precedes_host_deadline
      theoremFamily := "hook-event-lock-deadline"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.event_replay_and_state_size_are_bounded
      theoremFamily := "hook-event-state-bounded-tail"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.local_policy_failure_cannot_recommend_server_reconcile
      theoremFamily := "hook-local-policy-no-server-reconcile"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.local_policy_failure_has_configuration_independent_diagnostic
      theoremFamily := "hook-local-policy-diagnostic-escape"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.nested_deadlines_leave_cleanup_slack
      theoremFamily := "hook-stateful-nested-deadline"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.timed_out_stateful_call_cannot_leave_runtime_healthy
      theoremFamily := "hook-stateful-timeout-terminal"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.concurrency_cap_is_terminal_not_a_queue
      theoremFamily := "hook-stateful-cap-terminal"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.optional_telemetry_cannot_gate_control_endpoint_publication
      theoremFamily := "runtime-control-before-optional-telemetry"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.provider_catalog_repair_precedes_supervisor_generation
      theoremFamily := "runtime-provider-catalog-before-supervisor"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.undrained_resources_are_detected
      theoremFamily := "hook-stateful-resource-leak-detection"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.isolated_test_home_cannot_target_user_supervisor
      theoremFamily := "hook-test-supervisor-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.registered_source_read_requires_composed_policy_axes
      theoremFamily := "hook-composable-action-policy-conjunction"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.materialized_deny_preserves_message_route_and_registration
      theoremFamily := "hook-materialized-deny-authority-composition"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  , { name := `ASPProof.HookExecutionPlane.plugin_bootstrap_cannot_leak_exit_127_or_enter_runtime_server
      theoremFamily := "hook-plugin-bootstrap-typed-exec-failure"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-EXECUTION-PLANE"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookExecutionPlane"
    "ASPProof/HookExecutionPlane.lean"
    targets

end ASPProof.Audit.HookExecutionPlane
