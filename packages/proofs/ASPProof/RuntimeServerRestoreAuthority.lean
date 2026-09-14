-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.RuntimeServerRestoreAuthority

inductive BuildMode where
  | restoreOnly
  | restoreOrBuild
  | rebuildAfterMutation
  deriving DecidableEq, Repr

inductive BuildAction where
  | publishCommitted
  | runSourceBuilder
  | fail
  deriving DecidableEq, Repr

structure RestoreEvidence where
  currentGenerationPointer : Bool
  exactScope : Bool
  exactCandidate : Bool
  exactGenerationDigest : Bool
  exactSourceRootDigest : Bool
  exactProviderBundle : Bool
  deriving DecidableEq, Repr

def restoreAdmitted (evidence : RestoreEvidence) : Bool :=
  evidence.currentGenerationPointer &&
  evidence.exactScope &&
  evidence.exactCandidate &&
  evidence.exactGenerationDigest &&
  evidence.exactSourceRootDigest &&
  evidence.exactProviderBundle

def decide (mode : BuildMode) (evidence : RestoreEvidence) : BuildAction :=
  match mode, restoreAdmitted evidence with
  | .restoreOnly, true => .publishCommitted
  | .restoreOnly, false => .fail
  | .restoreOrBuild, true => .publishCommitted
  | .restoreOrBuild, false => .runSourceBuilder
  | .rebuildAfterMutation, _ => .runSourceBuilder

def completeEvidence : RestoreEvidence where
  currentGenerationPointer := true
  exactScope := true
  exactCandidate := true
  exactGenerationDigest := true
  exactSourceRootDigest := true
  exactProviderBundle := true

def missingPointer : RestoreEvidence :=
  { completeEvidence with currentGenerationPointer := false }

def legacyRestore (currentMaterialization : Bool) : BuildAction :=
  if currentMaterialization then .publishCommitted else .runSourceBuilder

def onDemandPointerRestore (evidence : RestoreEvidence) : BuildAction :=
  decide .restoreOnly evidence

theorem legacy_missing_restore_runs_source_builder :
    legacyRestore false = .runSourceBuilder := by
  rfl

theorem restore_only_current_publishes :
    decide .restoreOnly completeEvidence = .publishCommitted := by
  rfl

theorem restore_only_missing_fails :
    decide .restoreOnly missingPointer = .fail := by
  rfl

theorem restore_only_never_runs_source_builder (evidence : RestoreEvidence) :
    decide .restoreOnly evidence ≠ .runSourceBuilder := by
  cases h : restoreAdmitted evidence <;> simp [decide, h]

theorem on_demand_pointer_restore_uses_restore_only (evidence : RestoreEvidence) :
    onDemandPointerRestore evidence = decide .restoreOnly evidence := by
  rfl

theorem on_demand_pointer_restore_never_runs_source_builder (evidence : RestoreEvidence) :
    onDemandPointerRestore evidence ≠ .runSourceBuilder := by
  exact restore_only_never_runs_source_builder evidence

theorem on_demand_pointer_restore_missing_is_terminal_failure :
    onDemandPointerRestore missingPointer = .fail := by
  rfl

theorem source_builder_requires_explicit_build_authority
    (mode : BuildMode)
    (evidence : RestoreEvidence)
    (runs : decide mode evidence = .runSourceBuilder) :
    mode ≠ .restoreOnly := by
  intro restoreOnly
  subst mode
  exact restore_only_never_runs_source_builder evidence runs

theorem restore_only_result_domain (evidence : RestoreEvidence) :
    decide .restoreOnly evidence = .publishCommitted ∨
      decide .restoreOnly evidence = .fail := by
  cases h : restoreAdmitted evidence <;> simp [decide, h]

theorem mutation_never_publishes_existing (evidence : RestoreEvidence) :
    decide .rebuildAfterMutation evidence = .runSourceBuilder := by
  rfl

theorem exact_binding_restores_without_source_builder :
    decide .restoreOrBuild completeEvidence = .publishCommitted := by
  rfl

theorem candidate_drift_rejects_restore :
    decide .restoreOrBuild { completeEvidence with exactCandidate := false } =
      .runSourceBuilder := by
  rfl

theorem source_root_drift_rejects_restore :
    decide .restoreOrBuild { completeEvidence with exactSourceRootDigest := false } =
      .runSourceBuilder := by
  rfl

theorem provider_drift_rejects_restore :
    decide .restoreOrBuild { completeEvidence with exactProviderBundle := false } =
      .runSourceBuilder := by
  rfl

inductive DurablePublicationPhase where
  | segmentsPending
  | segmentsDurable
  | bindingDurable
  | pointerVisible
  deriving DecidableEq, Repr

def phaseRank : DurablePublicationPhase → Nat
  | .segmentsPending => 0
  | .segmentsDurable => 1
  | .bindingDurable => 2
  | .pointerVisible => 3

def bindingIsDurable (phase : DurablePublicationPhase) : Prop :=
  phaseRank .bindingDurable ≤ phaseRank phase

def pointerIsVisible (phase : DurablePublicationPhase) : Prop :=
  phase = .pointerVisible

theorem visible_pointer_implies_durable_binding
    (phase : DurablePublicationPhase) (visible : pointerIsVisible phase) :
    bindingIsDurable phase := by
  subst phase
  simp [bindingIsDurable, phaseRank]

structure SourceSnapshotRoot where
  value : String
  deriving DecidableEq

structure OwnerMerkleRoot where
  value : String
  deriving DecidableEq

structure RestoredGenerationRoots where
  sourceSnapshotRoot : SourceSnapshotRoot
  ownerMerkleRoot : OwnerMerkleRoot

def sourceRecoveryRoot (roots : RestoredGenerationRoots) : SourceSnapshotRoot :=
  roots.sourceSnapshotRoot

def ownerProofRoot (roots : RestoredGenerationRoots) : OwnerMerkleRoot :=
  roots.ownerMerkleRoot

theorem source_recovery_uses_only_source_snapshot_root (roots : RestoredGenerationRoots) :
    sourceRecoveryRoot roots = roots.sourceSnapshotRoot := by
  rfl

theorem owner_proof_uses_only_owner_merkle_root (roots : RestoredGenerationRoots) :
    ownerProofRoot roots = roots.ownerMerkleRoot := by
  rfl

end ASPProof.RuntimeServerRestoreAuthority
