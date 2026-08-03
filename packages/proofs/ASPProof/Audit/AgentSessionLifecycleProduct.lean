import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.Audit.AgentSessionLifecycleProduct

open ASPProof.AgentSessionLifecycleProduct

def readyState : LifecycleProduct :=
  { server := ⟨4, .ready⟩
    session := ⟨7, .active⟩
    binding := ⟨7, 11, 13, .fresh, false, false⟩
    dispatch := ⟨7, 17, .idle⟩ }

def legacyProfileGate (expected observed : Nat) : Bool :=
  durableDispatchAuthorized readyState && expected == observed

theorem ready_state_has_durable_dispatch_authority :
    durableDispatchAuthorized readyState = true := by
  decide

theorem legacy_profile_gate_can_reject_authoritative_host_binding :
    durableDispatchAuthorized readyState = true ∧
      legacyProfileGate 1 2 = false := by
  decide

theorem server_failure_does_not_archive_ready_session :
    (loseServerTransport readyState).session.phase = .active := by
  rfl

theorem binding_loss_does_not_archive_ready_session :
    (observeBindingStale readyState).session.phase = .active ∧
      (observeBindingStale readyState).binding.phase = .stale := by
  exact ⟨rfl, rfl⟩

theorem dispatch_timeout_does_not_retarget_or_archive :
    (quarantineDispatch readyState).session = readyState.session ∧
      (quarantineDispatch readyState).binding = readyState.binding ∧
      (quarantineDispatch readyState).dispatch.phase = .quarantined := by
  exact ⟨rfl, rfl, rfl⟩

theorem archived_without_release_cannot_replace :
    ¬ replacementAdmitted (indexArchived readyState) 8 := by
  intro admitted
  cases admitted.2.1

def forgedReleasedPhase : LifecycleProduct :=
  { indexArchived readyState with
    binding := { readyState.binding with phase := .pathReleased } }

theorem released_phase_without_receipts_cannot_replace :
    ¬ replacementAdmitted forgedReleasedPhase 8 := by
  intro admitted
  cases admitted.2.2.1

def releasedArchivedState : LifecycleProduct :=
  indexPathReleased (indexHostTerminated (indexArchived readyState))

theorem released_archived_state_admits_next_generation :
    replacementAdmitted releasedArchivedState 8 := by
  exact ⟨Or.inl rfl, rfl, by decide⟩

def staleReceipt : HostBindingReceipt :=
  { generation := 6
    childId := 11
    canonicalTarget := 13
    typedRoleMatches := true
    bindingFresh := true }

theorem late_receipt_cannot_bind_current_generation :
    ¬ receiptMatches readyState.session staleReceipt := by
  intro hMatches
  cases hMatches.1

end ASPProof.Audit.AgentSessionLifecycleProduct
