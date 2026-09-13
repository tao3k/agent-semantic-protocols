-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ResidentWorkspaceRetirement

def idleTimeoutSeconds : Nat := 3600

structure Snapshot where
  workspacePathExists : Bool
  idleSeconds : Nat
  liveLeases : Nat
  inFlightRequests : Nat
  deriving DecidableEq, Repr

def eligible (snapshot : Snapshot) : Prop :=
  snapshot.liveLeases = 0 ∧
    snapshot.inFlightRequests = 0 ∧
    (snapshot.workspacePathExists = false ∨ idleTimeoutSeconds ≤ snapshot.idleSeconds)

instance (snapshot : Snapshot) : Decidable (eligible snapshot) := by
  unfold eligible
  infer_instance

inductive RetirementState where
  | resident
  | admissionClosed
  | checkpointed
  | endpointRetired
  deriving DecidableEq, Repr

def retire (snapshot : Snapshot) : RetirementState :=
  if eligible snapshot then RetirementState.endpointRetired else RetirementState.resident

theorem missing_workspace_without_activity_is_eligible
    (snapshot : Snapshot)
    (missing : snapshot.workspacePathExists = false)
    (leases : snapshot.liveLeases = 0)
    (requests : snapshot.inFlightRequests = 0) :
    eligible snapshot := by
  exact ⟨leases, requests, Or.inl missing⟩

theorem idle_workspace_without_activity_is_eligible
    (snapshot : Snapshot)
    (idle : idleTimeoutSeconds ≤ snapshot.idleSeconds)
    (leases : snapshot.liveLeases = 0)
    (requests : snapshot.inFlightRequests = 0) :
    eligible snapshot := by
  exact ⟨leases, requests, Or.inr idle⟩

theorem live_lease_prevents_retirement
    (snapshot : Snapshot)
    (lease : snapshot.liveLeases ≠ 0) :
    retire snapshot = RetirementState.resident := by
  unfold retire
  split
  · rename_i admitted
    exact False.elim (lease admitted.1)
  · rfl

theorem in_flight_request_prevents_retirement
    (snapshot : Snapshot)
    (request : snapshot.inFlightRequests ≠ 0) :
    retire snapshot = RetirementState.resident := by
  unfold retire
  split
  · rename_i admitted
    exact False.elim (request admitted.2.1)
  · rfl

theorem retirement_implies_quiescence
    (snapshot : Snapshot)
    (retired : retire snapshot = RetirementState.endpointRetired) :
    snapshot.liveLeases = 0 ∧ snapshot.inFlightRequests = 0 := by
  unfold retire at retired
  split at retired
  · rename_i admitted
    exact ⟨admitted.1, admitted.2.1⟩
  · contradiction

end ASPProof.ResidentWorkspaceRetirement
