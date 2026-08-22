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

def decide (mode : BuildMode) (currentGenerationPointer : Bool) : BuildAction :=
  match mode, currentGenerationPointer with
  | .restoreOnly, true => .publishCommitted
  | .restoreOnly, false => .fail
  | .restoreOrBuild, true => .publishCommitted
  | .restoreOrBuild, false => .runSourceBuilder
  | .rebuildAfterMutation, _ => .runSourceBuilder

def legacyRestore (currentMaterialization : Bool) : BuildAction :=
  if currentMaterialization then .publishCommitted else .runSourceBuilder

def onDemandPointerRestore (currentGenerationPointer : Bool) : BuildAction :=
  decide .restoreOnly currentGenerationPointer

theorem legacy_missing_restore_runs_source_builder :
    legacyRestore false = .runSourceBuilder := by
  rfl

theorem restore_only_current_publishes :
    decide .restoreOnly true = .publishCommitted := by
  rfl

theorem restore_only_missing_fails :
    decide .restoreOnly false = .fail := by
  rfl

theorem restore_only_never_runs_source_builder (currentGenerationPointer : Bool) :
    decide .restoreOnly currentGenerationPointer ≠ .runSourceBuilder := by
  cases currentGenerationPointer <;> simp [decide]

theorem on_demand_pointer_restore_uses_restore_only (currentGenerationPointer : Bool) :
    onDemandPointerRestore currentGenerationPointer =
      decide .restoreOnly currentGenerationPointer := by
  rfl

theorem on_demand_pointer_restore_never_runs_source_builder (currentGenerationPointer : Bool) :
    onDemandPointerRestore currentGenerationPointer ≠ .runSourceBuilder := by
  exact restore_only_never_runs_source_builder currentGenerationPointer

theorem on_demand_pointer_restore_missing_is_terminal_failure :
    onDemandPointerRestore false = .fail := by
  rfl

theorem source_builder_requires_explicit_build_authority
    (mode : BuildMode)
    (currentGenerationPointer : Bool)
    (runs : decide mode currentGenerationPointer = .runSourceBuilder) :
    mode ≠ .restoreOnly := by
  intro restoreOnly
  subst mode
  exact restore_only_never_runs_source_builder currentGenerationPointer runs

theorem restore_only_result_domain (currentGenerationPointer : Bool) :
    decide .restoreOnly currentGenerationPointer = .publishCommitted ∨
      decide .restoreOnly currentGenerationPointer = .fail := by
  cases currentGenerationPointer <;> simp [decide]

theorem mutation_never_publishes_existing (currentGenerationPointer : Bool) :
    decide .rebuildAfterMutation currentGenerationPointer = .runSourceBuilder := by
  cases currentGenerationPointer <;> rfl

structure RestoredGenerationRoots where
  sourceSnapshotRoot : String
  ownerMerkleRoot : String

inductive RootConsumer where
  | sourceRecovery
  | ownerProof
  deriving DecidableEq

def rootFor (roots : RestoredGenerationRoots) : RootConsumer → String
  | .sourceRecovery => roots.sourceSnapshotRoot
  | .ownerProof => roots.ownerMerkleRoot

theorem source_recovery_uses_only_source_snapshot_root (roots : RestoredGenerationRoots) :
    rootFor roots .sourceRecovery = roots.sourceSnapshotRoot := by
  rfl

theorem owner_proof_uses_only_owner_merkle_root (roots : RestoredGenerationRoots) :
    rootFor roots .ownerProof = roots.ownerMerkleRoot := by
  rfl

end ASPProof.RuntimeServerRestoreAuthority
