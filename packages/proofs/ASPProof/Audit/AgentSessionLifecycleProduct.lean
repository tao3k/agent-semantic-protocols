-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.Audit.AgentSessionLifecycleProduct

open ASPProof.AgentSessionLifecycleProduct

def readyState : LifecycleProduct :=
  { server := ⟨4, .ready⟩
    session := ⟨7, .active⟩
    binding := ⟨7, 11, 13, .fresh, .present⟩
    dispatch := ⟨7, 17, .idle⟩ }

def absentState : LifecycleProduct :=
  { readyState with binding := ⟨7, 0, 0, .unbound, .absent⟩ }

def legacyProfileGate (expected observed : Nat) : Bool :=
  durableDispatchAuthorized readyState && expected == observed

theorem ready_state_has_durable_dispatch_authority :
    durableDispatchAuthorized readyState = true := by
  decide

theorem legacy_profile_gate_can_reject_authoritative_host_binding :
    durableDispatchAuthorized readyState = true ∧
      legacyProfileGate 1 2 = false := by
  decide

theorem server_failure_does_not_change_ready_session :
    (loseServerTransport readyState).session.phase = .active := by
  rfl

theorem binding_loss_does_not_change_ready_session :
    (observeBindingStale readyState).session.phase = .active ∧
      (observeBindingStale readyState).binding.phase = .stale := by
  exact ⟨rfl, rfl⟩

theorem dispatch_timeout_does_not_retarget_agent :
    (quarantineDispatch readyState).session = readyState.session ∧
      (quarantineDispatch readyState).binding = readyState.binding ∧
      (quarantineDispatch readyState).dispatch.phase = .quarantined := by
  exact ⟨rfl, rfl, rfl⟩

theorem present_path_uses_followup_and_rejects_spawn :
    requiredDispatchAction readyState = .followupTask ∧
      spawnAgentAdmitted readyState = false := by
  decide

theorem absent_path_uses_spawn_and_rejects_followup :
    requiredDispatchAction absentState = .spawnAgent ∧
      followupTaskAdmitted absentState = false := by
  decide

theorem interrupt_keeps_followup_path :
    requiredDispatchAction (interruptTurn readyState) = .followupTask := by
  decide

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
