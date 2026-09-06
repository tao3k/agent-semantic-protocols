import Std

namespace ASPProof.SearchQueryIncidentLifecycle

inductive IncidentState where
  | open
  | repairing
  | verificationPending
  | failedVerification
  | resolved
  | superseded
  | compacted
  deriving DecidableEq, Repr

structure ReplayReceipt where
  sameRequest : Bool
  sameWorkspace : Bool
  sameArtifact : Bool
  sameGeneration : Bool
  codeProjectionPresent : Bool
  withinBudget : Bool
  deriving DecidableEq, Repr

def ReplayReceipt.valid (receipt : ReplayReceipt) : Bool :=
  receipt.sameRequest &&
    receipt.sameWorkspace &&
    receipt.sameArtifact &&
    receipt.sameGeneration &&
    receipt.codeProjectionPresent &&
    receipt.withinBudget

structure Incident where
  workspaceIdentity : Nat
  incidentIdentity : Nat
  state : IncidentState
  occurrenceCount : Nat
  transitionSequence : Nat
  verified : Bool
  active : Bool
  deriving DecidableEq, Repr

inductive Event where
  | observeFailure
  | beginRepair
  | requestVerification
  | verificationFailed
  | verificationSucceeded (receipt : ReplayReceipt)
  | supersede
  | compact
  deriving DecidableEq, Repr

def eligibleForCompaction (incident : Incident) : Prop :=
  incident.state = .resolved ∧ incident.verified = true

instance (incident : Incident) : Decidable (eligibleForCompaction incident) :=
  by
    unfold eligibleForCompaction
    infer_instance

def advance (incident : Incident) : Incident :=
  { incident with transitionSequence := incident.transitionSequence + 1 }

def step (incident : Incident) (event : Event) : Incident :=
  match event with
  | .observeFailure =>
      advance { incident with
        state := .open
        occurrenceCount := incident.occurrenceCount + 1
        verified := false
        active := true }
  | .beginRepair =>
      if incident.state = .open then
        advance { incident with state := .repairing }
      else incident
  | .requestVerification =>
      if incident.state = .repairing then
        advance { incident with state := .verificationPending }
      else incident
  | .verificationFailed =>
      if incident.state = .verificationPending then
        advance { incident with state := .open, verified := false, active := true }
      else incident
  | .verificationSucceeded receipt =>
      if incident.state = .verificationPending && receipt.valid then
        advance { incident with state := .resolved, verified := true, active := false }
      else incident
  | .supersede =>
      advance { incident with state := .superseded, verified := false, active := false }
  | .compact =>
      if eligibleForCompaction incident then
        advance { incident with state := .compacted, active := false }
      else incident

def transitionIdentity (incident : Incident) : Nat × Nat :=
  (incident.incidentIdentity, incident.transitionSequence)

theorem workspace_isolation (incident : Incident) (event : Event) :
    (step incident event).workspaceIdentity = incident.workspaceIdentity := by
  cases event with
  | observeFailure => rfl
  | beginRepair =>
      by_cases openState : incident.state = .open <;>
        simp [step, openState, advance]
  | requestVerification =>
      by_cases repairing : incident.state = .repairing <;>
        simp [step, repairing, advance]
  | verificationFailed =>
      by_cases pending : incident.state = .verificationPending <;>
        simp [step, pending, advance]
  | verificationSucceeded receipt =>
      by_cases accepted : incident.state = .verificationPending && receipt.valid <;>
        simp [step, accepted, advance]
  | supersede => rfl
  | compact =>
      by_cases eligible : eligibleForCompaction incident <;>
        simp [step, eligible, advance]

theorem identity_is_stable (incident : Incident) (event : Event) :
    (step incident event).incidentIdentity = incident.incidentIdentity := by
  cases event with
  | observeFailure => rfl
  | beginRepair =>
      by_cases openState : incident.state = .open <;>
        simp [step, openState, advance]
  | requestVerification =>
      by_cases repairing : incident.state = .repairing <;>
        simp [step, repairing, advance]
  | verificationFailed =>
      by_cases pending : incident.state = .verificationPending <;>
        simp [step, pending, advance]
  | verificationSucceeded receipt =>
      by_cases accepted : incident.state = .verificationPending && receipt.valid <;>
        simp [step, accepted, advance]
  | supersede => rfl
  | compact =>
      by_cases eligible : eligibleForCompaction incident <;>
        simp [step, eligible, advance]

theorem transition_sequence_is_monotone (incident : Incident) (event : Event) :
    incident.transitionSequence ≤ (step incident event).transitionSequence := by
  cases event with
  | observeFailure => simp [step, advance]
  | beginRepair =>
      by_cases openState : incident.state = .open <;> simp [step, openState, advance]
  | requestVerification =>
      by_cases repairing : incident.state = .repairing <;> simp [step, repairing, advance]
  | verificationFailed =>
      by_cases pending : incident.state = .verificationPending <;>
        simp [step, pending, advance]
  | verificationSucceeded receipt =>
      by_cases accepted : incident.state = .verificationPending && receipt.valid <;>
        simp [step, accepted, advance]
  | supersede => simp [step, advance]
  | compact =>
      by_cases eligible : eligibleForCompaction incident <;>
        simp [step, eligible, advance]

theorem advanced_transition_identity_is_unique
    (incident : Incident)
    (event : Event)
    (advanced : (step incident event).transitionSequence = incident.transitionSequence + 1) :
    transitionIdentity (step incident event) ≠ transitionIdentity incident := by
  simp [transitionIdentity, advanced]

theorem invalid_replay_preserves_incident
    (incident : Incident)
    (receipt : ReplayReceipt)
    (invalid : receipt.valid = false) :
    step incident (.verificationSucceeded receipt) = incident := by
  simp [step, invalid]

theorem compact_requires_verified (incident : Incident)
    (notAlreadyCompacted : incident.state ≠ .compacted)
    (result : (step incident .compact).state = .compacted) :
    incident.state = .resolved ∧ incident.verified = true := by
  by_cases eligible : eligibleForCompaction incident
  · exact eligible
  · simp [step, eligible] at result
    exact False.elim (notAlreadyCompacted result)

theorem failed_verification_reopens
    (incident : Incident)
    (pending : incident.state = .verificationPending) :
    (step incident .verificationFailed).state = .open ∧
      (step incident .verificationFailed).active = true := by
  simp [step, advance, pending]

theorem observe_deduplicates_identity (incident : Incident) :
    (step incident .observeFailure).incidentIdentity = incident.incidentIdentity ∧
      (step incident .observeFailure).occurrenceCount = incident.occurrenceCount + 1 := by
  simp [step, advance]

theorem resolved_is_not_active
    (incident : Incident)
    (receipt : ReplayReceipt)
    (pending : incident.state = .verificationPending)
    (valid : receipt.valid = true) :
    (step incident (.verificationSucceeded receipt)).active = false := by
  simp [step, advance, pending, valid]

end ASPProof.SearchQueryIncidentLifecycle
