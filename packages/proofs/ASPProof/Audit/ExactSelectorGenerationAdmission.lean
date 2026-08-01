import ASPProof.ExactSelectorGenerationAdmission

namespace ASPProof.Audit.ExactSelectorGenerationAdmission

open ASPProof.ExactSelectorGenerationAdmission

theorem removed_owner_is_never_resolved
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    resolve active request ≠ .resolved active.generationId active.rootDigest := by
  rw [absent_owner_is_selector_stale active request ownerAbsent]
  intro impossible
  contradiction

theorem live_owner_item_miss_is_not_stale
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (ownerPresent : request.ownerPath ∈ active.owners)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request ≠
      .selectorStale active.generationId active.rootDigest := by
  rw [active_owner_absent_selector_is_item_missing
    active request ownerPresent selectorAbsent]
  intro impossible
  contradiction

theorem stale_recovery_is_symbol_owned
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    recoveryRoute (resolve active request) = .symbolSearch := by
  rw [absent_owner_is_selector_stale active request ownerAbsent]
  exact stale_selector_never_reuses_absent_owner_authority
    active.generationId active.rootDigest

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
