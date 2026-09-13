-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ASPWorkspaceReconcileClosure

structure ProjectionRequirements where
  source : Bool
  callableSkeleton : Bool
  seeds : Bool
  deriving DecidableEq, Repr

structure WorkspaceRequirement where
  workspaceIdentity : Nat
  rootDigest : Nat
  projections : ProjectionRequirements
  deriving DecidableEq, Repr

structure PublicationReceipt where
  workspaceIdentity : Nat
  rootDigest : Nat
  generationDigest : Nat
  committed : Bool
  sourceProjection : Bool
  callableSkeletonProjection : Bool
  seedsProjection : Bool
  deriving DecidableEq, Repr

structure Satisfies
    (requirement : WorkspaceRequirement)
    (receipt : PublicationReceipt) : Prop where
  workspaceIdentity :
    receipt.workspaceIdentity = requirement.workspaceIdentity
  rootDigest : receipt.rootDigest = requirement.rootDigest
  committed : receipt.committed = true
  sourceProjection :
    requirement.projections.source = true -> receipt.sourceProjection = true
  callableSkeletonProjection :
    requirement.projections.callableSkeleton = true ->
      receipt.callableSkeletonProjection = true
  seedsProjection :
    requirement.projections.seeds = true -> receipt.seedsProjection = true

def WorkspaceClosed
    (requirement : WorkspaceRequirement)
    (receipt : Option PublicationReceipt) : Prop :=
  ∃ publication, receipt = some publication ∧ Satisfies requirement publication

def FleetClosed
    (requirements : Nat -> Option WorkspaceRequirement)
    (receipts : Nat -> Option PublicationReceipt) : Prop :=
  ∀ slot requirement,
    requirements slot = some requirement -> WorkspaceClosed requirement (receipts slot)

structure ReconcileSummary where
  serverHealthy : Bool
  reportedWorkspaceCount : Nat
  deriving DecidableEq, Repr

structure PublicationKey where
  workspaceIdentity : Nat
  rootDigest : Nat
  projections : ProjectionRequirements
  deriving DecidableEq, Repr

def stableKey (requirement : WorkspaceRequirement) : PublicationKey :=
  { workspaceIdentity := requirement.workspaceIdentity
    rootDigest := requirement.rootDigest
    projections := requirement.projections }

def canonicalReceipt
    (requirement : WorkspaceRequirement) (generationDigest : Nat) :
    PublicationReceipt :=
  { workspaceIdentity := requirement.workspaceIdentity
    rootDigest := requirement.rootDigest
    generationDigest := generationDigest
    committed := true
    sourceProjection := true
    callableSkeletonProjection := true
    seedsProjection := true }

theorem noReceiptCannotClose (requirement : WorkspaceRequirement) :
    ¬WorkspaceClosed requirement none := by
  intro hClosed
  obtain ⟨receipt, hReceipt, _⟩ := hClosed
  cases hReceipt

theorem canonicalReceiptCloses
    (requirement : WorkspaceRequirement) (generationDigest : Nat) :
    WorkspaceClosed requirement (some (canonicalReceipt requirement generationDigest)) := by
  refine ⟨canonicalReceipt requirement generationDigest, rfl, ?_⟩
  constructor
  · rfl
  · rfl
  · rfl
  · intro _
    rfl
  · intro _
    rfl
  · intro _
    rfl

theorem wrongWorkspaceIdentityRejected
    (requirement : WorkspaceRequirement) (receipt : PublicationReceipt)
    (hWrong : receipt.workspaceIdentity ≠ requirement.workspaceIdentity) :
    ¬Satisfies requirement receipt := by
  intro hSatisfies
  exact hWrong hSatisfies.workspaceIdentity

theorem wrongRootDigestRejected
    (requirement : WorkspaceRequirement) (receipt : PublicationReceipt)
    (hWrong : receipt.rootDigest ≠ requirement.rootDigest) :
    ¬Satisfies requirement receipt := by
  intro hSatisfies
  exact hWrong hSatisfies.rootDigest

theorem uncommittedReceiptRejected
    (requirement : WorkspaceRequirement) (receipt : PublicationReceipt)
    (hUncommitted : receipt.committed = false) :
    ¬Satisfies requirement receipt := by
  intro hSatisfies
  have hImpossible : false = true := hUncommitted.symm.trans hSatisfies.committed
  cases hImpossible

theorem missingSourceProjectionRejected
    (requirement : WorkspaceRequirement) (receipt : PublicationReceipt)
    (hRequired : requirement.projections.source = true)
    (hMissing : receipt.sourceProjection = false) :
    ¬Satisfies requirement receipt := by
  intro hSatisfies
  have hDelivered : receipt.sourceProjection = true :=
    hSatisfies.sourceProjection hRequired
  have hImpossible : false = true := hMissing.symm.trans hDelivered
  cases hImpossible

theorem healthOnlyCannotCloseRequiredWorkspace
    (requirement : WorkspaceRequirement) (summary : ReconcileSummary)
    (_hHealthy : summary.serverHealthy = true) :
    ¬FleetClosed (fun _ => some requirement) (fun _ => none) := by
  intro hClosed
  have hSlot := hClosed 0 requirement rfl
  exact noReceiptCannotClose requirement hSlot

theorem stableKeyDeterministic (requirement : WorkspaceRequirement) :
    stableKey requirement = stableKey requirement := by
  rfl

theorem changedRootDigestChangesStableKey
    (requirement : WorkspaceRequirement) (nextRootDigest : Nat)
    (hChanged : requirement.rootDigest ≠ nextRootDigest) :
    stableKey requirement ≠ stableKey { requirement with rootDigest := nextRootDigest } := by
  intro hKeys
  have hDigests := congrArg PublicationKey.rootDigest hKeys
  exact hChanged hDigests

end ASPProof.ASPWorkspaceReconcileClosure
