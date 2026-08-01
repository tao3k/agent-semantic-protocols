namespace ASPProof.HookGenerationAdmissionNonBlocking

/-- Hook classification and generation-backed execution are separate planes. -/
inductive Action where
  | genericCommand
  | controlPlaneRecovery
  | directSourceRead
  | generationQuery
  deriving DecidableEq, Repr

inductive GenerationAdmission where
  | building
  | ready
  | failed
  deriving DecidableEq, Repr

inductive Decision where
  | allow
  | deny
  deriving DecidableEq, Repr

/-- Admission is observed by the hook but cannot replace policy matching. -/
def hookDecision (_admission : GenerationAdmission) : Action → Decision
  | .directSourceRead => .deny
  | .genericCommand => .allow
  | .controlPlaneRecovery => .allow
  | .generationQuery => .allow

/-- A generation consumer independently requires a ready data plane. -/
def generationQueryEnabled : GenerationAdmission → Bool
  | .ready => true
  | .building => false
  | .failed => false

def GloballyDeadlocked (admission : GenerationAdmission) : Prop :=
  ∀ action, hookDecision admission action = .deny

theorem failed_admission_preserves_generic_commands :
    hookDecision .failed .genericCommand = .allow := by
  rfl

theorem failed_admission_preserves_control_plane_recovery :
    hookDecision .failed .controlPlaneRecovery = .allow := by
  rfl

theorem failed_admission_does_not_release_direct_source_read :
    hookDecision .failed .directSourceRead = .deny := by
  rfl

theorem failed_generation_query_remains_data_plane_fail_closed :
    generationQueryEnabled .failed = false := by
  rfl

theorem failed_admission_cannot_globally_deadlock_the_hook :
    ¬ GloballyDeadlocked .failed := by
  intro deadlocked
  have denied := deadlocked .controlPlaneRecovery
  cases denied

theorem hook_allow_does_not_imply_generation_query_execution
    (admission : GenerationAdmission)
    (notReady : admission ≠ .ready) :
    hookDecision admission .generationQuery = .allow ∧
      generationQueryEnabled admission = false := by
  constructor
  · rfl
  · cases admission with
    | building => rfl
    | ready => contradiction
    | failed => rfl

end ASPProof.HookGenerationAdmissionNonBlocking
