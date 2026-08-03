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

def decide (mode : BuildMode) (currentMaterialization : Bool) : BuildAction :=
  match mode, currentMaterialization with
  | .restoreOnly, true => .publishCommitted
  | .restoreOnly, false => .fail
  | .restoreOrBuild, true => .publishCommitted
  | .restoreOrBuild, false => .runSourceBuilder
  | .rebuildAfterMutation, _ => .runSourceBuilder

def legacyRestore (currentMaterialization : Bool) : BuildAction :=
  if currentMaterialization then .publishCommitted else .runSourceBuilder

theorem legacy_missing_restore_runs_source_builder :
    legacyRestore false = .runSourceBuilder := by
  rfl

theorem restore_only_current_publishes :
    decide .restoreOnly true = .publishCommitted := by
  rfl

theorem restore_only_missing_fails :
    decide .restoreOnly false = .fail := by
  rfl

theorem restore_only_never_runs_source_builder (currentMaterialization : Bool) :
    decide .restoreOnly currentMaterialization ≠ .runSourceBuilder := by
  cases currentMaterialization <;> simp [decide]

theorem source_builder_requires_explicit_build_authority
    (mode : BuildMode)
    (currentMaterialization : Bool)
    (runs : decide mode currentMaterialization = .runSourceBuilder) :
    mode ≠ .restoreOnly := by
  intro restoreOnly
  subst mode
  exact restore_only_never_runs_source_builder currentMaterialization runs

theorem restore_only_result_domain (currentMaterialization : Bool) :
    decide .restoreOnly currentMaterialization = .publishCommitted ∨
      decide .restoreOnly currentMaterialization = .fail := by
  cases currentMaterialization <;> simp [decide]

theorem mutation_never_publishes_existing (currentMaterialization : Bool) :
    decide .rebuildAfterMutation currentMaterialization = .runSourceBuilder := by
  cases currentMaterialization <;> rfl

end ASPProof.RuntimeServerRestoreAuthority
