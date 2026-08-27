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

inductive GenerationTerminalReceipt where
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

inductive GenerationDispatcherExit where
  | joinFailure
  | channelClosed
  | actorAbort
  | runtimeDrain
  deriving DecidableEq, Repr

inductive GenerationAwaitState where
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

def cancelLiveGenerationAwait : GenerationAwaitState := .cancelled

/-- The dispatcher retains this terminal authority beside every admitted task. -/
def terminalReceiptForDispatcherExit :
    GenerationDispatcherExit → GenerationTerminalReceipt
  | .runtimeDrain => .cancelled
  | .joinFailure | .channelClosed | .actorAbort => .failed

inductive Decision where
  | allow
  | deny
  deriving DecidableEq, Repr

/-- Tool normalization is upstream of both policy and durability consumers. -/
inductive NormalizedOperation where
  | applyPatch
  | directRead
  | generic
  deriving DecidableEq, Repr

inductive PolicyMatch where
  | matchedAllow
  | matchedDeny
  | unmatched
  deriving DecidableEq, Repr

inductive CandidateDiscoveryOwner where
  | hookProcess
  | runtimeServer
  deriving DecidableEq, Repr

def hookWaitsForCandidate : CandidateDiscoveryOwner → Bool
  | .hookProcess => true
  | .runtimeServer => false

def backgroundWorkSurvivesHookExit : CandidateDiscoveryOwner → Bool
  | .hookProcess => false
  | .runtimeServer => true

def observesWorkspaceMutation : NormalizedOperation → Bool
  | .applyPatch => true
  | .directRead => false
  | .generic => false

def policyDecision : PolicyMatch → Decision
  | .matchedAllow => .allow
  | .matchedDeny => .deny
  | .unmatched => .allow

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

theorem unmatched_policy_cannot_erase_normalized_apply_patch_mutation :
    policyDecision .unmatched = .allow ∧
      observesWorkspaceMutation .applyPatch = true := by
  constructor <;> rfl

theorem mutation_projection_is_policy_independent
    (policy : PolicyMatch) :
    observesWorkspaceMutation .applyPatch = true := by
  cases policy <;> rfl

theorem runtime_server_submission_does_not_wait_for_candidate :
    hookWaitsForCandidate .runtimeServer = false := by
  rfl

theorem runtime_server_submission_survives_hook_exit :
    backgroundWorkSurvivesHookExit .runtimeServer = true := by
  rfl

theorem admitted_dispatcher_exit_cannot_leave_orphan_building
    (exit : GenerationDispatcherExit) :
    terminalReceiptForDispatcherExit exit = .failed ∨
      terminalReceiptForDispatcherExit exit = .cancelled := by
  cases exit <;> simp [terminalReceiptForDispatcherExit]

theorem cancelling_live_provider_or_source_await_is_terminal :
    cancelLiveGenerationAwait = .cancelled := by
  rfl

theorem client_candidate_discovery_violates_nonblocking_submission :
    hookWaitsForCandidate .hookProcess = true := by
  rfl

end ASPProof.HookGenerationAdmissionNonBlocking
