-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.CodexMultiAgentV2WorkspaceAdmissionLifecycle

structure WorkspaceIdentity where
  workspaceId : String
  canonicalRoot : String
  deriving DecidableEq, Repr

inductive AdmissionPhase where
  | missing
  | building (ownerEpoch : Nat)
  | ready (ownerEpoch : Nat) (generationDigest : String)
  | failed (ownerEpoch : Nat)
  | cancelled (ownerEpoch : Nat)
  deriving DecidableEq, Repr

structure AdmissionCatalog where
  identity : Option WorkspaceIdentity
  phase : AdmissionPhase
  deriving DecidableEq, Repr

inductive EnsureDecision where
  | started
  | joined
  | reused
  | rejectedCanonicalRootMismatch
  deriving DecidableEq, Repr

inductive DesiredRuntimeState where
  | running
  | stopped
  deriving DecidableEq, Repr

def supervisorMaySpawn : DesiredRuntimeState → Bool
  | .running => true
  | .stopped => false

structure EnsureResult where
  catalog : AdmissionCatalog
  decision : EnsureDecision
  deriving DecidableEq, Repr

def startBuilding (identity : WorkspaceIdentity) (ownerEpoch : Nat) : EnsureResult :=
  ⟨⟨some identity, .building ownerEpoch⟩, .started⟩

def ensureAdmission
    (requested : WorkspaceIdentity)
    (ownerEpoch : Nat)
    (catalog : AdmissionCatalog) : EnsureResult :=
  match catalog.identity with
  | none => startBuilding requested ownerEpoch
  | some existing =>
      if existing = requested then
        match catalog.phase with
        | .missing => startBuilding requested ownerEpoch
        | .building _ => ⟨catalog, .joined⟩
        | .ready _ _ => ⟨catalog, .reused⟩
        | .failed _ => startBuilding requested ownerEpoch
        | .cancelled _ => startBuilding requested ownerEpoch
      else
        ⟨catalog, .rejectedCanonicalRootMismatch⟩

def publishReady
    (ownerEpoch : Nat)
    (generationDigest : String)
    (catalog : AdmissionCatalog) : AdmissionCatalog :=
  match catalog.phase with
  | .building currentEpoch =>
      if currentEpoch = ownerEpoch then
        { catalog with phase := .ready ownerEpoch generationDigest }
      else
        catalog
  | _ => catalog

def publishFailed
    (ownerEpoch : Nat)
    (catalog : AdmissionCatalog) : AdmissionCatalog :=
  match catalog.phase with
  | .building currentEpoch =>
      if currentEpoch = ownerEpoch then
        { catalog with phase := .failed ownerEpoch }
      else
        catalog
  | _ => catalog

def publishCancelled
    (ownerEpoch : Nat)
    (catalog : AdmissionCatalog) : AdmissionCatalog :=
  match catalog.phase with
  | .building currentEpoch =>
      if currentEpoch = ownerEpoch then
        { catalog with phase := .cancelled ownerEpoch }
      else
        catalog
  | _ => catalog

theorem concurrent_ensure_joins_existing_build
    (identity : WorkspaceIdentity)
    (currentEpoch requestedEpoch : Nat) :
    ensureAdmission identity requestedEpoch
      ⟨some identity, .building currentEpoch⟩ =
      ⟨⟨some identity, .building currentEpoch⟩, .joined⟩ := by
  simp [ensureAdmission]

theorem canonical_root_mismatch_cannot_replace_catalog
    (existing requested : WorkspaceIdentity)
    (differentIdentity : existing ≠ requested)
    (phase : AdmissionPhase)
    (ownerEpoch : Nat) :
    (ensureAdmission requested ownerEpoch ⟨some existing, phase⟩).catalog =
      ⟨some existing, phase⟩ := by
  simp [ensureAdmission, differentIdentity]

theorem stale_builder_cannot_publish_ready
    (identity : WorkspaceIdentity)
    (currentEpoch staleEpoch : Nat)
    (generationDigest : String)
    (stale : currentEpoch ≠ staleEpoch) :
    publishReady staleEpoch generationDigest
      ⟨some identity, .building currentEpoch⟩ =
      ⟨some identity, .building currentEpoch⟩ := by
  simp [publishReady, stale]

theorem current_builder_publishes_ready
    (identity : WorkspaceIdentity)
    (ownerEpoch : Nat)
    (generationDigest : String) :
    publishReady ownerEpoch generationDigest
      ⟨some identity, .building ownerEpoch⟩ =
      ⟨some identity, .ready ownerEpoch generationDigest⟩ := by
  simp [publishReady]

theorem current_builder_failure_is_terminal
    (identity : WorkspaceIdentity)
    (ownerEpoch : Nat) :
    publishFailed ownerEpoch ⟨some identity, .building ownerEpoch⟩ =
      ⟨some identity, .failed ownerEpoch⟩ := by
  simp [publishFailed]

theorem current_builder_cancellation_is_terminal
    (identity : WorkspaceIdentity)
    (ownerEpoch : Nat) :
    publishCancelled ownerEpoch ⟨some identity, .building ownerEpoch⟩ =
      ⟨some identity, .cancelled ownerEpoch⟩ := by
  simp [publishCancelled]

theorem failed_generation_can_start_a_new_epoch
    (identity : WorkspaceIdentity)
    (failedEpoch nextEpoch : Nat) :
    ensureAdmission identity nextEpoch ⟨some identity, .failed failedEpoch⟩ =
      startBuilding identity nextEpoch := by
  simp [ensureAdmission]

theorem operator_stop_prevents_supervisor_restart :
    supervisorMaySpawn .stopped = false := by
  rfl

end ASPProof.CodexMultiAgentV2WorkspaceAdmissionLifecycle
