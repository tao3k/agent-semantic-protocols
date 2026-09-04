import ASPProof.ExactSelectorGenerationAdmission

namespace ASPProof.Audit.ExactSelectorGenerationAdmission

open ASPProof.ExactSelectorGenerationAdmission

#print axioms exact_query_terminal_uses_active_source_root

theorem a_daemon_registry_sync_bridge_cannot_be_admitted
    (runtimeOwnerCount : Nat) :
    registryBootstrapAdmitted runtimeOwnerCount .synchronousNestedRuntime = false := by
  exact daemon_nested_runtime_registry_bootstrap_is_rejected runtimeOwnerCount

theorem removed_owner_is_never_resolved
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    resolve active request ≠ .resolved active.generationId active.rootDigest := by
  rw [absent_owner_in_active_generation_is_owner_missing
    active request generationCurrent ownerAbsent]
  intro impossible
  contradiction

theorem live_owner_item_miss_is_not_stale
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerPresent : request.ownerPath ∈ active.owners)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request ≠
      .selectorStale active.generationId active.rootDigest := by
  rw [active_owner_absent_selector_is_item_missing
    active request generationCurrent ownerPresent selectorAbsent]
  intro impossible
  contradiction

theorem fabricated_name_cannot_cross_exact_admission
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request ≠ .resolved active.generationId active.rootDigest := by
  exact fabricated_selector_is_never_admitted_as_resolved active request selectorAbsent

theorem fabricated_name_cannot_trigger_repeat_discovery
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerPresent : request.ownerPath ∈ active.owners)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    recoveryRoute (resolve active request) = .none := by
  rw [active_owner_absent_selector_is_item_missing
    active request generationCurrent ownerPresent selectorAbsent]
  exact active_item_missing_is_terminal active.generationId active.rootDigest

theorem hook_bypass_cannot_leave_a_witnessed_edit_readable
    (runtime : ReconciliationRuntime)
    (witness : SourceMutationWitness)
    (sameWorkspace : witness.workspaceId = runtime.workspaceId) :
    exactReadAdmitted (observeSourceMutation runtime witness) = false := by
  exact observed_same_workspace_mutation_blocks_exact_read_until_publication
    runtime witness sameWorkspace

theorem cross_workspace_events_preserve_isolation
    (runtime : ReconciliationRuntime)
    (witness : SourceMutationWitness)
    (otherWorkspace : witness.workspaceId ≠ runtime.workspaceId) :
    observeSourceMutation runtime witness = runtime := by
  exact another_workspace_mutation_does_not_block_this_workspace
    runtime witness otherWorkspace

theorem stale_recovery_is_symbol_owned
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (stale : resolve active request =
      .selectorStale active.generationId active.rootDigest) :
    recoveryRoute (resolve active request) = .symbolSearch := by
  rw [stale]
  exact stale_selector_never_reuses_absent_owner_authority
    active.generationId active.rootDigest

theorem current_missing_owner_is_not_stale
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    resolve active request ≠
      .selectorStale active.generationId active.rootDigest := by
  rw [absent_owner_in_active_generation_is_owner_missing
    active request generationCurrent ownerAbsent]
  intro impossible
  contradiction

theorem changed_workspace_batch_cannot_erase_an_admitted_identity
    (batch : WorkspaceMutationBatch)
    (envelope : WorkspaceGenerationEnvelope)
    (present : envelope ∈ batch.envelopes) :
    envelope.workspaceId ∈ affectedWorkspaceIds batch := by
  exact every_generation_envelope_retains_its_workspace_identity batch envelope present

theorem repeated_path_set_requires_a_new_mutation_identity
    (leftMutationId rightMutationId : MutationId)
    (different : leftMutationId ≠ rightMutationId)
    (envelopes : List WorkspaceGenerationEnvelope) :
    ({ mutationId := leftMutationId, envelopes } : WorkspaceMutationBatch) ≠
      ({ mutationId := rightMutationId, envelopes } : WorkspaceMutationBatch) := by
  exact equal_envelopes_do_not_collapse_distinct_mutation_events
    leftMutationId rightMutationId different envelopes

theorem queued_post_tool_receipt_cannot_release_exact_query
    (mutationId : MutationId) :
    submissionEnablesExactQuery {
      mutationId
      state := .queued
    } = false := by
  exact mutation_submission_never_authorizes_exact_query _

theorem distinct_mutation_during_build_cannot_be_erased_by_single_flight
    (active successor : WorkspaceMutationBatch)
    (different : active.mutationId ≠ successor.mutationId) :
    completeInFlightMutation
        (enqueueMutation
          { inFlight := some active, pending := [] }
          successor) =
      { inFlight := some successor, pending := [] } := by
  exact queued_distinct_mutation_becomes_the_next_flight
    active successor different

theorem only_equal_mutation_identity_may_avoid_a_successor_queue_entry
    (active duplicate : WorkspaceMutationBatch)
    (same : active.mutationId = duplicate.mutationId) :
    enqueueMutation
        { inFlight := some active, pending := [] }
        duplicate =
      { inFlight := some active, pending := [] } := by
  exact duplicate_mutation_identity_is_coalesced_without_a_second_queue_entry
    active duplicate same

theorem old_install_cannot_replace_a_newer_active_artifact_receipt
    (state : ActiveArtifactReceiptState)
    (publication : ActiveArtifactPublication)
    (stale : publication.expectedRootDigest ≠ state.rootDigest) :
    publishActiveArtifactReceipt state publication = none := by
  exact stale_artifact_publisher_cannot_overwrite_the_active_root
    state publication stale

theorem accepted_but_building_generation_is_fail_closed :
    finishPreToolAdmission .building = .deny := by
  rfl

theorem completed_generation_cannot_permanently_suppress_change_detection
    (active : ActiveGeneration) :
    admissionAccepts (.ready active) = true := by
  exact ready_generation_does_not_block_incremental_readmission active

theorem ensure_and_restore_cannot_duplicate_a_completed_build
    (active : ActiveGeneration) :
    schedulesBuild .ensureCurrent (.ready active) = false ∧
      schedulesBuild .restoreCurrent (.ready active) = false := by
  exact ⟨ensure_ready_generation_is_observation_only active,
    restore_ready_generation_is_idempotent active⟩

theorem exact_query_requires_ready_generation
    (state : AdmissionState)
    (enabled : exactQueryEnabled (finishPreToolAdmission state) = true) :
    ∃ active, state = .ready active := by
  cases state with
  | building => contradiction
  | failed => contradiction
  | ready active => exact ⟨active, rfl⟩

theorem distinct_sessions_do_not_imply_distinct_db_owners
    (runtime : WorkspaceRuntime)
    (left right : SessionId)
    (_distinct : left ≠ right) :
    let registered := registerSession (registerSession runtime left) right
    sessionOwner registered left = sessionOwner registered right := by
  dsimp
  have shared := two_sessions_share_the_same_workspace_owner runtime left right
  exact shared.1.trans shared.2.symm

theorem provider_scope_change_cannot_fork_workspace_identity
    (workspace : WorktreeWorkspace)
    (left right : ProviderScopeReceipt)
    (leftAdmitted : left.workspaceId = workspace.workspaceId)
    (rightAdmitted : right.workspaceId = workspace.workspaceId) :
    (publishProviderScope workspace left).map (·.workspace.workspaceId) =
      (publishProviderScope workspace right).map (·.workspace.workspaceId) := by
  exact provider_scope_digest_cannot_create_a_workspace_identity
    workspace left right leftAdmitted rightAdmitted

end ASPProof.Audit.ExactSelectorGenerationAdmission
