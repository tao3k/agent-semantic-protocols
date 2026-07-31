import ASPProof.SearchRouteAdmissionRetryCacheRejoinJointDecisionFusion

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

open ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

inductive DecisionDurability
  | absent
  | intent
  | durable
  deriving DecidableEq

structure AtomicPublicationState where
  phase : TransitionPhase
  decisionDurability : DecisionDurability
  revision : Nat
  deriving DecidableEq

inductive PublicationStep :
    AtomicPublicationState →
    AtomicPublicationState → Prop
  | prepare (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.absent,
          revision⟩
        ⟨TransitionPhase.joint,
          DecisionDurability.intent,
          revision + 1⟩
  | persist (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.intent,
          revision⟩
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision + 1⟩
  | abortIntent (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.intent,
          revision⟩
        ⟨TransitionPhase.joint,
          DecisionDurability.absent,
          revision + 1⟩
  | advance (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision⟩
        ⟨TransitionPhase.newOnly,
          DecisionDurability.durable,
          revision + 1⟩
  | replayDurableJoint (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision⟩
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision⟩
  | replayAdvanced (revision : Nat) :
      PublicationStep
        ⟨TransitionPhase.newOnly,
          DecisionDurability.durable,
          revision⟩
        ⟨TransitionPhase.newOnly,
          DecisionDurability.durable,
          revision⟩

def InitialPublicationState : AtomicPublicationState :=
  ⟨TransitionPhase.joint, DecisionDurability.absent, 0⟩

def SafePublicationState
    (state : AtomicPublicationState) : Prop :=
  state.phase = TransitionPhase.newOnly →
    state.decisionDurability = DecisionDurability.durable

inductive ReachablePublicationState :
    AtomicPublicationState → Prop
  | initial :
      ReachablePublicationState InitialPublicationState
  | next
      {before after : AtomicPublicationState}
      (beforeReachable : ReachablePublicationState before)
      (step : PublicationStep before after) :
      ReachablePublicationState after

def PhaseCasAuthorized
    (state : AtomicPublicationState)
    (expectedRevision : Nat) : Prop :=
  expectedRevision = state.revision ∧
  state.phase = TransitionPhase.joint ∧
  state.decisionDurability = DecisionDurability.durable

def VerificationSnapshotCurrent
    {SnapshotId : Type}
    (decisionSnapshot currentSnapshot : SnapshotId) : Prop :=
  decisionSnapshot = currentSnapshot

structure PublicationCommandKey
    (TransitionId DecisionReceiptId SnapshotId : Type) where
  transitionId : TransitionId
  jointDecisionReceiptId : DecisionReceiptId
  expectedRevision : Nat
  verificationSnapshotId : SnapshotId

def PublicationCommandCompatible
    {TransitionId DecisionReceiptId SnapshotId : Type}
    (left right :
      PublicationCommandKey
        TransitionId DecisionReceiptId SnapshotId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.jointDecisionReceiptId =
      right.jointDecisionReceiptId ∧
  left.expectedRevision = right.expectedRevision ∧
  left.verificationSnapshotId =
      right.verificationSnapshotId

theorem publication_step_preserves_safety
    {before after : AtomicPublicationState}
    (beforeSafe : SafePublicationState before)
    (step : PublicationStep before after) :
    SafePublicationState after := by
  cases step with
  | prepare revision =>
      intro impossible
      cases impossible
  | persist revision =>
      intro impossible
      cases impossible
  | abortIntent revision =>
      intro impossible
      cases impossible
  | advance revision =>
      intro phaseIsNew
      rfl
  | replayDurableJoint revision =>
      intro impossible
      cases impossible
  | replayAdvanced revision =>
      intro phaseIsNew
      rfl

theorem every_reachable_publication_state_is_safe
    {state : AtomicPublicationState}
    (reachable : ReachablePublicationState state) :
    SafePublicationState state := by
  induction reachable with
  | initial =>
      intro impossible
      cases impossible
  | next beforeReachable step inductionHypothesis =>
      exact
        publication_step_preserves_safety
          inductionHypothesis step

theorem reachable_new_only_implies_durable_decision
    {state : AtomicPublicationState}
    (reachable : ReachablePublicationState state)
    (isNewOnly :
      state.phase = TransitionPhase.newOnly) :
    state.decisionDurability =
      DecisionDurability.durable :=
  (every_reachable_publication_state_is_safe reachable)
    isNewOnly

theorem intent_cannot_advance_phase
    (revision : Nat) :
    ¬ PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.intent,
          revision⟩
        ⟨TransitionPhase.newOnly,
          DecisionDurability.intent,
          revision + 1⟩ := by
  intro step
  cases step

theorem durable_joint_state_can_advance
    (revision : Nat) :
    PublicationStep
      ⟨TransitionPhase.joint,
        DecisionDurability.durable,
        revision⟩
      ⟨TransitionPhase.newOnly,
        DecisionDurability.durable,
        revision + 1⟩ :=
  PublicationStep.advance revision

theorem intent_can_abort_without_phase_advance
    (revision : Nat) :
    PublicationStep
      ⟨TransitionPhase.joint,
        DecisionDurability.intent,
        revision⟩
      ⟨TransitionPhase.joint,
        DecisionDurability.absent,
        revision + 1⟩ :=
  PublicationStep.abortIntent revision

theorem publication_step_revision_is_monotone
    {before after : AtomicPublicationState}
    (step : PublicationStep before after) :
    before.revision ≤ after.revision := by
  cases step with
  | prepare revision =>
      exact Nat.le_succ revision
  | persist revision =>
      exact Nat.le_succ revision
  | abortIntent revision =>
      exact Nat.le_succ revision
  | advance revision =>
      exact Nat.le_succ revision
  | replayDurableJoint revision =>
      exact Nat.le_refl revision
  | replayAdvanced revision =>
      exact Nat.le_refl revision

theorem stale_revision_blocks_phase_cas
    (state : AtomicPublicationState)
    (expectedRevision : Nat)
    (revisionChanged :
      expectedRevision ≠ state.revision) :
    ¬ PhaseCasAuthorized state expectedRevision := by
  intro authorized
  exact revisionChanged authorized.1

theorem non_durable_state_blocks_phase_cas
    (state : AtomicPublicationState)
    (expectedRevision : Nat)
    (notDurable :
      state.decisionDurability ≠
        DecisionDurability.durable) :
    ¬ PhaseCasAuthorized state expectedRevision := by
  intro authorized
  exact notDurable authorized.2.2

theorem changed_verification_snapshot_blocks_phase_advance
    {SnapshotId : Type}
    (decisionSnapshot currentSnapshot : SnapshotId)
    (snapshotChanged :
      decisionSnapshot ≠ currentSnapshot) :
    ¬ VerificationSnapshotCurrent
        decisionSnapshot currentSnapshot := by
  intro current
  exact snapshotChanged current

theorem durable_replay_is_idempotent
    (revision : Nat) :
    PublicationStep
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision⟩
        ⟨TransitionPhase.joint,
          DecisionDurability.durable,
          revision⟩ ∧
      PublicationStep
        ⟨TransitionPhase.newOnly,
          DecisionDurability.durable,
          revision⟩
        ⟨TransitionPhase.newOnly,
          DecisionDurability.durable,
          revision⟩ :=
  ⟨PublicationStep.replayDurableJoint revision,
    PublicationStep.replayAdvanced revision⟩

theorem publication_command_compatibility_binds_decision
    {TransitionId DecisionReceiptId SnapshotId : Type}
    (left right :
      PublicationCommandKey
        TransitionId DecisionReceiptId SnapshotId)
    (compatible :
      PublicationCommandCompatible left right) :
    left.jointDecisionReceiptId =
      right.jointDecisionReceiptId :=
  compatible.2.1

theorem changed_decision_rejects_publication_replay
    {TransitionId DecisionReceiptId SnapshotId : Type}
    (left right :
      PublicationCommandKey
        TransitionId DecisionReceiptId SnapshotId)
    (decisionChanged :
      left.jointDecisionReceiptId ≠
        right.jointDecisionReceiptId) :
    ¬ PublicationCommandCompatible left right := by
  intro compatible
  exact
    decisionChanged
      (publication_command_compatibility_binds_decision
        left right compatible)

end ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication
