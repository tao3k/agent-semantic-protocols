-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ActivationLifecycleAudit

namespace ASPProof.ActivationRepairTransaction

open ASPProof.ActivationLifecycleAudit

/-- A repair capability is issued by authority independently of the runtime
artifact that may need repair. -/
structure RepairCapability where
  canonicalIdentity : Nat
  allowedSourceGeneration : Nat
  maxTargetGeneration : Nat
  issuedRevision : Nat
  expiresRevision : Nat
  authorityDigest : Nat
  revoked : Bool
  deriving Repr, DecidableEq

/-- A proposal binds repair intent to one observed head and one candidate
generation. -/
structure RepairProposal where
  canonicalIdentity : Nat
  expectedGeneration : Nat
  expectedAuthorityRevision : Nat
  targetGeneration : Nat
  targetSchemaVersion : Nat
  targetBinaryDigest : Nat
  compatibilityWitness : Bool
  rollbackCheckpoint : Bool
  ambiguousDispatchQuarantined : Bool
  deriving Repr, DecidableEq

/-- The linearizable repair head. `authorityRevision` is the compare-and-swap
coordinate shared by competing repair writers. -/
structure RepairHead where
  canonicalIdentity : Nat
  authorityRevision : Nat
  state : ActivationState

def CapabilityAuthorizes
    (capability : RepairCapability)
    (proposal : RepairProposal)
    (head : RepairHead) : Prop :=
  capability.revoked = false ∧
  capability.canonicalIdentity = head.canonicalIdentity ∧
  capability.canonicalIdentity = proposal.canonicalIdentity ∧
  capability.allowedSourceGeneration = head.state.declaredGeneration ∧
  capability.issuedRevision ≤ head.authorityRevision ∧
  head.authorityRevision ≤ capability.expiresRevision ∧
  proposal.targetGeneration ≤ capability.maxTargetGeneration

def RepairablePhase (phase : ActivationPhase) : Prop :=
  phase = .degraded ∨ phase = .repairing

instance dispatchRecoverableDecidable
    (evidence : DispatchEvidence) :
    Decidable (DispatchRecoverable evidence) := by
  cases evidence with
  | none => exact isTrue trivial
  | claimed => exact isTrue trivial
  | terminal => exact isTrue trivial
  | missingAfterDispatch => exact isFalse (fun impossible => impossible)

/-- Safety gate for publishing a candidate repair generation.

This predicate deliberately does not claim liveness. A scheduler, authority
service, and host must still make progress for an authorized proposal to be
committed. -/
def CanCommitRepair
    (capability : RepairCapability)
    (proposal : RepairProposal)
    (head : RepairHead) : Prop :=
  RepairablePhase head.state.phase ∧
  head.state.activationReadable = true ∧
  (head.state.repairToolAvailable = true ∨
    head.state.emergencyRepairCapability = true) ∧
  CapabilityAuthorizes capability proposal head ∧
  proposal.expectedGeneration = head.state.declaredGeneration ∧
  proposal.expectedAuthorityRevision = head.authorityRevision ∧
  head.state.declaredGeneration < proposal.targetGeneration ∧
  proposal.compatibilityWitness = true ∧
  proposal.rollbackCheckpoint = true ∧
  (DispatchRecoverable head.state.dispatchEvidence ∨
    proposal.ambiguousDispatchQuarantined = true) ∧
  proposal.targetBinaryDigest ≠ 0

instance canCommitRepairDecidable
    (capability : RepairCapability)
    (proposal : RepairProposal)
    (head : RepairHead) :
    Decidable (CanCommitRepair capability proposal head) := by
  unfold CanCommitRepair RepairablePhase CapabilityAuthorizes
  infer_instance

/-- A proof-gated commit publishes only candidate intent. It returns to
`activating`; readiness must be re-derived from fresh observations. -/
def commitRepair
    (capability : RepairCapability)
    (proposal : RepairProposal)
    (head : RepairHead)
    (_proof : CanCommitRepair capability proposal head) : RepairHead :=
  { canonicalIdentity := head.canonicalIdentity
    authorityRevision := head.authorityRevision + 1
    state :=
      { head.state with
        phase := .activating
        declaredGeneration := proposal.targetGeneration
        declaredSchemaVersion := proposal.targetSchemaVersion
        declaredBinaryDigest := proposal.targetBinaryDigest
        hostAuditComplete := false
        subagentStartObserved := false
        dispatchAcknowledged := false } }

def degradedRepairableState : ActivationState :=
  { phase := .degraded
    activationReadable := true
    registryBound := true
    hostReachable := true
    transportBound := true
    repairToolAvailable := true
    emergencyRepairCapability := false
    hostAuditComplete := true
    subagentStartObserved := true
    dispatchAcknowledged := true
    dispatchEvidence := .terminal
    declaredGeneration := 1
    observedGeneration := 1
    declaredSchemaVersion := 1
    runtimeSchemaVersion := 1
    declaredBinaryDigest := 1
    observedBinaryDigest := 1 }

def baseRepairHead : RepairHead :=
  { canonicalIdentity := 1
    authorityRevision := 7
    state := degradedRepairableState }

def baseRepairCapability : RepairCapability :=
  { canonicalIdentity := 1
    allowedSourceGeneration := 1
    maxTargetGeneration := 3
    issuedRevision := 6
    expiresRevision := 9
    authorityDigest := 1
    revoked := false }

def baseRepairProposal : RepairProposal :=
  { canonicalIdentity := 1
    expectedGeneration := 1
    expectedAuthorityRevision := 7
    targetGeneration := 2
    targetSchemaVersion := 2
    targetBinaryDigest := 2
    compatibilityWitness := true
    rollbackCheckpoint := true
    ambiguousDispatchQuarantined := false }

def expiredCapability : RepairCapability :=
  { baseRepairCapability with expiresRevision := 6 }

def revokedCapability : RepairCapability :=
  { baseRepairCapability with revoked := true }

def crossIdentityCapability : RepairCapability :=
  { baseRepairCapability with canonicalIdentity := 2 }

def staleRepairProposal : RepairProposal :=
  { baseRepairProposal with expectedAuthorityRevision := 6 }

def regressionRepairProposal : RepairProposal :=
  { baseRepairProposal with targetGeneration := 1 }

def noRollbackRepairProposal : RepairProposal :=
  { baseRepairProposal with rollbackCheckpoint := false }

def ambiguousDispatchState : ActivationState :=
  { degradedRepairableState with dispatchEvidence := .missingAfterDispatch }

def ambiguousDispatchHead : RepairHead :=
  { baseRepairHead with state := ambiguousDispatchState }

def unquarantinedAmbiguousProposal : RepairProposal :=
  { baseRepairProposal with ambiguousDispatchQuarantined := false }

theorem base_repair_is_committable :
    CanCommitRepair baseRepairCapability baseRepairProposal baseRepairHead := by
  decide

theorem expired_capability_cannot_commit :
    ¬ CanCommitRepair expiredCapability baseRepairProposal baseRepairHead := by
  decide

theorem revoked_capability_cannot_commit :
    ¬ CanCommitRepair revokedCapability baseRepairProposal baseRepairHead := by
  decide

theorem cross_identity_capability_cannot_commit :
    ¬ CanCommitRepair crossIdentityCapability baseRepairProposal baseRepairHead := by
  decide

theorem stale_proposal_cannot_commit :
    ¬ CanCommitRepair baseRepairCapability staleRepairProposal baseRepairHead := by
  decide

theorem generation_regression_cannot_commit :
    ¬ CanCommitRepair baseRepairCapability regressionRepairProposal baseRepairHead := by
  decide

theorem repair_without_rollback_cannot_commit :
    ¬ CanCommitRepair baseRepairCapability noRollbackRepairProposal baseRepairHead := by
  decide

theorem ambiguous_dispatch_requires_quarantine :
    ¬ CanCommitRepair
      baseRepairCapability
      unquarantinedAmbiguousProposal
      ambiguousDispatchHead := by
  decide

theorem committed_repair_advances_generation
    (proof : CanCommitRepair capability proposal head) :
    (commitRepair capability proposal head proof).state.declaredGeneration =
      proposal.targetGeneration := by
  rfl

theorem committed_repair_increments_authority_revision
    (proof : CanCommitRepair capability proposal head) :
    (commitRepair capability proposal head proof).authorityRevision =
      head.authorityRevision + 1 := by
  rfl

theorem committed_repair_preserves_canonical_identity
    (proof : CanCommitRepair capability proposal head) :
    (commitRepair capability proposal head proof).canonicalIdentity =
      head.canonicalIdentity := by
  rfl

theorem committed_repair_requires_fresh_readiness
    (proof : CanCommitRepair capability proposal head) :
    (commitRepair capability proposal head proof).state.phase = .activating := by
  rfl

/-- A proposal that won at a head cannot win again at the committed head:
its expected revision is now stale. This is the one-winner CAS property. -/
theorem committed_head_rejects_same_proposal
    (proof : CanCommitRepair capability proposal head) :
    ¬ CanCommitRepair
      capability
      proposal
      (commitRepair capability proposal head proof) := by
  intro next
  have expectedBefore :
      proposal.expectedAuthorityRevision = head.authorityRevision :=
    proof.2.2.2.2.2.1
  have expectedAfter :
      proposal.expectedAuthorityRevision = head.authorityRevision + 1 :=
    by
      simpa [commitRepair] using next.2.2.2.2.2.1
  have contradiction :
      head.authorityRevision = head.authorityRevision + 1 :=
    expectedBefore.symm.trans expectedAfter
  exact (Nat.ne_of_lt (Nat.lt_succ_self head.authorityRevision)) contradiction

end ASPProof.ActivationRepairTransaction
