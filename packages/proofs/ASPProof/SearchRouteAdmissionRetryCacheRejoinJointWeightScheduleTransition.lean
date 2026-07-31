import ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteWeightedCut

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

inductive TransitionPhase
  | oldOnly
  | joint
  | newOnly
  deriving DecidableEq

def phaseRank : TransitionPhase → Nat
  | .oldOnly => 0
  | .joint => 1
  | .newOnly => 2

def ValidPhaseTransition
    (fromPhase toPhase : TransitionPhase) : Prop :=
  phaseRank fromPhase ≤ phaseRank toPhase

def ScheduleAuthorized
    {ScheduleId : Type}
    (phase : TransitionPhase)
    (oldSchedule newSchedule candidate : ScheduleId) : Prop :=
  match phase with
  | .oldOnly => candidate = oldSchedule
  | .joint =>
      candidate = oldSchedule ∨ candidate = newSchedule
  | .newOnly => candidate = newSchedule

structure WeightedReceiptKey
    (SnapshotId ScheduleDigest Epoch ReportSetDigest : Type) where
  authoritySetSnapshotId : SnapshotId
  weightScheduleDigest : ScheduleDigest
  epoch : Epoch
  admittedReportSetDigest : ReportSetDigest
  totalCollectedWeight : VotingWeight
  faultWeightBudget : VotingWeight
  cutWeight : VotingWeight
  selectedValue : VotingWeight

def ReceiptCompatible
    {SnapshotId ScheduleDigest Epoch ReportSetDigest : Type}
    (left right :
      WeightedReceiptKey
        SnapshotId ScheduleDigest Epoch ReportSetDigest) : Prop :=
  left.authoritySetSnapshotId =
      right.authoritySetSnapshotId ∧
  left.weightScheduleDigest =
      right.weightScheduleDigest ∧
  left.epoch = right.epoch ∧
  left.admittedReportSetDigest =
      right.admittedReportSetDigest ∧
  left.totalCollectedWeight =
      right.totalCollectedWeight ∧
  left.faultWeightBudget =
      right.faultWeightBudget ∧
  left.cutWeight = right.cutWeight ∧
  left.selectedValue = right.selectedValue

structure JointTransitionReceiptPair
    (SnapshotId ScheduleDigest Epoch ReportSetDigest : Type)
    (oldSnapshot newSnapshot : SnapshotId)
    (oldSchedule newSchedule : ScheduleDigest) where
  oldReceipt :
    WeightedReceiptKey
      SnapshotId ScheduleDigest Epoch ReportSetDigest
  newReceipt :
    WeightedReceiptKey
      SnapshotId ScheduleDigest Epoch ReportSetDigest
  oldSnapshotBound :
    oldReceipt.authoritySetSnapshotId = oldSnapshot
  newSnapshotBound :
    newReceipt.authoritySetSnapshotId = newSnapshot
  oldScheduleBound :
    oldReceipt.weightScheduleDigest = oldSchedule
  newScheduleBound :
    newReceipt.weightScheduleDigest = newSchedule

theorem valid_phase_progression :
    ValidPhaseTransition
        TransitionPhase.oldOnly TransitionPhase.joint ∧
      ValidPhaseTransition
        TransitionPhase.joint TransitionPhase.newOnly := by
  unfold ValidPhaseTransition
  unfold phaseRank
  exact ⟨by decide, by decide⟩

theorem new_only_cannot_regress_to_old_only :
    ¬ ValidPhaseTransition
        TransitionPhase.newOnly TransitionPhase.oldOnly := by
  unfold ValidPhaseTransition
  unfold phaseRank
  decide

theorem old_only_rejects_new_schedule
    {ScheduleId : Type}
    (oldSchedule newSchedule : ScheduleId)
    (schedulesDiffer : oldSchedule ≠ newSchedule) :
    ¬ ScheduleAuthorized
        TransitionPhase.oldOnly
        oldSchedule newSchedule newSchedule := by
  intro authorized
  exact schedulesDiffer authorized.symm

theorem new_only_rejects_old_schedule
    {ScheduleId : Type}
    (oldSchedule newSchedule : ScheduleId)
    (schedulesDiffer : oldSchedule ≠ newSchedule) :
    ¬ ScheduleAuthorized
        TransitionPhase.newOnly
        oldSchedule newSchedule oldSchedule := by
  intro authorized
  exact schedulesDiffer authorized

theorem joint_authorizes_both_schedules
    {ScheduleId : Type}
    (oldSchedule newSchedule : ScheduleId) :
    ScheduleAuthorized
        TransitionPhase.joint
        oldSchedule newSchedule oldSchedule ∧
      ScheduleAuthorized
        TransitionPhase.joint
        oldSchedule newSchedule newSchedule := by
  exact ⟨Or.inl rfl, Or.inr rfl⟩

theorem receipt_compatibility_binds_weight_schedule
    {SnapshotId ScheduleDigest Epoch ReportSetDigest : Type}
    (left right :
      WeightedReceiptKey
        SnapshotId ScheduleDigest Epoch ReportSetDigest)
    (compatible : ReceiptCompatible left right) :
    left.weightScheduleDigest =
      right.weightScheduleDigest :=
  compatible.2.1

theorem changed_schedule_invalidates_receipt_compatibility
    {SnapshotId ScheduleDigest Epoch ReportSetDigest : Type}
    (left right :
      WeightedReceiptKey
        SnapshotId ScheduleDigest Epoch ReportSetDigest)
    (scheduleChanged :
      left.weightScheduleDigest ≠
        right.weightScheduleDigest) :
    ¬ ReceiptCompatible left right := by
  intro compatible
  exact
    scheduleChanged
      (receipt_compatibility_binds_weight_schedule
        left right compatible)

theorem same_selected_value_does_not_imply_compatibility :
    ∀ {SnapshotId ScheduleDigest Epoch ReportSetDigest : Type}
      (left right :
        WeightedReceiptKey
          SnapshotId ScheduleDigest Epoch ReportSetDigest),
      left.selectedValue = right.selectedValue →
      left.weightScheduleDigest ≠ right.weightScheduleDigest →
      left.selectedValue = right.selectedValue ∧
        ¬ ReceiptCompatible left right := by
  intro SnapshotId ScheduleDigest Epoch ReportSetDigest
  intro left right sameSelectedValue scheduleChanged
  exact ⟨
    sameSelectedValue,
    changed_schedule_invalidates_receipt_compatibility
      left right scheduleChanged
  ⟩

theorem joint_pair_remains_two_receipts_when_selected_values_match
    {SnapshotId ScheduleDigest Epoch ReportSetDigest : Type}
    {oldSnapshot newSnapshot : SnapshotId}
    {oldSchedule newSchedule : ScheduleDigest}
    (receiptPair :
      JointTransitionReceiptPair
        SnapshotId ScheduleDigest Epoch ReportSetDigest
        oldSnapshot newSnapshot oldSchedule newSchedule)
    (snapshotsDiffer : oldSnapshot ≠ newSnapshot)
    (sameSelectedValue :
      receiptPair.oldReceipt.selectedValue =
        receiptPair.newReceipt.selectedValue) :
    receiptPair.oldReceipt.selectedValue =
        receiptPair.newReceipt.selectedValue ∧
      ¬ ReceiptCompatible
          receiptPair.oldReceipt receiptPair.newReceipt := by
  constructor
  · exact sameSelectedValue
  · intro compatible
    apply snapshotsDiffer
    calc
      oldSnapshot =
          receiptPair.oldReceipt.authoritySetSnapshotId :=
        receiptPair.oldSnapshotBound.symm
      _ = receiptPair.newReceipt.authoritySetSnapshotId :=
        compatible.1
      _ = newSnapshot :=
        receiptPair.newSnapshotBound

theorem one_materialized_weight_cannot_realize_two_different_schedule_weights
    (materializedWeight oldScheduleWeight newScheduleWeight :
      VotingWeight)
    (realizesOld :
      materializedWeight = oldScheduleWeight)
    (realizesNew :
      materializedWeight = newScheduleWeight)
    (weightsDiffer :
      oldScheduleWeight ≠ newScheduleWeight) :
    False := by
  apply weightsDiffer
  calc
    oldScheduleWeight = materializedWeight :=
      realizesOld.symm
    _ = newScheduleWeight :=
      realizesNew

end ASPProof.SearchRouteAdmissionRetryCacheRejoinJointWeightScheduleTransition
