import ASPProof.SearchRouteAdmissionRetryCacheRejoinAtomicJointDecisionPublication

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

structure RecoveryLease (HolderId SnapshotId : Type) where
  holderId : HolderId
  leaseEpoch : Nat
  publicationRevision : Nat
  issuedAt : Nat
  expiresAt : Nat
  verificationSnapshotId : SnapshotId

def LeaseActive
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (now : Nat) : Prop :=
  now < lease.expiresAt

def LeaseExpired
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (now : Nat) : Prop :=
  lease.expiresAt ≤ now

def LeaseDurationBound
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (maximumDuration : Nat) : Prop :=
  lease.expiresAt ≤ lease.issuedAt + maximumDuration

def PublicationLeaseFence
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (candidateHolder : HolderId)
    (candidateLeaseEpoch expectedRevision now : Nat)
    (candidateSnapshot : SnapshotId) : Prop :=
  candidateHolder = lease.holderId ∧
  candidateLeaseEpoch = lease.leaseEpoch ∧
  expectedRevision = lease.publicationRevision ∧
  candidateSnapshot = lease.verificationSnapshotId ∧
  LeaseActive lease now

def TakeoverAuthorized
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (expectedLeaseEpoch expectedRevision now : Nat)
    (expectedSnapshot : SnapshotId) : Prop :=
  LeaseExpired lease now ∧
  expectedLeaseEpoch = lease.leaseEpoch ∧
  expectedRevision = lease.publicationRevision ∧
  expectedSnapshot = lease.verificationSnapshotId

def takeoverLease
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (newHolder : HolderId)
    (now newExpiry : Nat) :
    RecoveryLease HolderId SnapshotId := {
  holderId := newHolder
  leaseEpoch := lease.leaseEpoch + 1
  publicationRevision := lease.publicationRevision + 1
  issuedAt := now
  expiresAt := newExpiry
  verificationSnapshotId := lease.verificationSnapshotId
}

structure RecoveryCommandKey
    (TransitionId HolderId SnapshotId DecisionReceiptId : Type) where
  transitionId : TransitionId
  holderId : HolderId
  leaseEpoch : Nat
  expectedPublicationRevision : Nat
  verificationSnapshotId : SnapshotId
  jointDecisionReceiptId : DecisionReceiptId

def RecoveryCommandCompatible
    {TransitionId HolderId SnapshotId DecisionReceiptId : Type}
    (left right :
      RecoveryCommandKey
        TransitionId HolderId SnapshotId DecisionReceiptId) : Prop :=
  left.transitionId = right.transitionId ∧
  left.holderId = right.holderId ∧
  left.leaseEpoch = right.leaseEpoch ∧
  left.expectedPublicationRevision =
      right.expectedPublicationRevision ∧
  left.verificationSnapshotId =
      right.verificationSnapshotId ∧
  left.jointDecisionReceiptId =
      right.jointDecisionReceiptId

theorem active_and_expired_are_disjoint
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (now : Nat)
    (active : LeaseActive lease now) :
    ¬ LeaseExpired lease now := by
  intro expired
  exact (Nat.not_lt_of_ge expired) active

theorem maximum_duration_bounds_active_ownership
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (maximumDuration now : Nat)
    (durationBound :
      LeaseDurationBound lease maximumDuration)
    (maximumElapsed :
      lease.issuedAt + maximumDuration ≤ now) :
    ¬ LeaseActive lease now := by
  intro active
  have expired : lease.expiresAt ≤ now :=
    Nat.le_trans durationBound maximumElapsed
  exact (Nat.not_lt_of_ge expired) active

theorem active_lease_blocks_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (expectedLeaseEpoch expectedRevision now : Nat)
    (expectedSnapshot : SnapshotId)
    (active : LeaseActive lease now) :
    ¬ TakeoverAuthorized
        lease expectedLeaseEpoch expectedRevision
        now expectedSnapshot := by
  intro authorized
  exact
    (active_and_expired_are_disjoint
      lease now active)
      authorized.1

theorem expired_lease_with_exact_fences_authorizes_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (now : Nat)
    (expired : LeaseExpired lease now) :
    TakeoverAuthorized
      lease
      lease.leaseEpoch
      lease.publicationRevision
      now
      lease.verificationSnapshotId :=
  ⟨expired, rfl, rfl, rfl⟩

theorem stale_lease_epoch_blocks_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (expectedLeaseEpoch expectedRevision now : Nat)
    (expectedSnapshot : SnapshotId)
    (epochChanged :
      expectedLeaseEpoch ≠ lease.leaseEpoch) :
    ¬ TakeoverAuthorized
        lease expectedLeaseEpoch expectedRevision
        now expectedSnapshot := by
  intro authorized
  exact epochChanged authorized.2.1

theorem stale_publication_revision_blocks_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (expectedLeaseEpoch expectedRevision now : Nat)
    (expectedSnapshot : SnapshotId)
    (revisionChanged :
      expectedRevision ≠ lease.publicationRevision) :
    ¬ TakeoverAuthorized
        lease expectedLeaseEpoch expectedRevision
        now expectedSnapshot := by
  intro authorized
  exact revisionChanged authorized.2.2.1

theorem changed_verification_snapshot_blocks_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (expectedLeaseEpoch expectedRevision now : Nat)
    (expectedSnapshot : SnapshotId)
    (snapshotChanged :
      expectedSnapshot ≠ lease.verificationSnapshotId) :
    ¬ TakeoverAuthorized
        lease expectedLeaseEpoch expectedRevision
        now expectedSnapshot := by
  intro authorized
  exact snapshotChanged authorized.2.2.2

theorem takeover_increments_epoch_and_revision
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (newHolder : HolderId)
    (now newExpiry : Nat) :
    (takeoverLease lease newHolder now newExpiry).leaseEpoch =
        lease.leaseEpoch + 1 ∧
      (takeoverLease lease newHolder now newExpiry).publicationRevision =
        lease.publicationRevision + 1 :=
  ⟨rfl, rfl⟩

theorem old_lease_epoch_is_fenced_after_takeover
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (newHolder candidateHolder : HolderId)
    (now newExpiry expectedRevision : Nat)
    (candidateSnapshot : SnapshotId) :
    ¬ PublicationLeaseFence
        (takeoverLease lease newHolder now newExpiry)
        candidateHolder
        lease.leaseEpoch
        expectedRevision
        now
        candidateSnapshot := by
  intro fenced
  have impossible :
      lease.leaseEpoch = lease.leaseEpoch + 1 :=
    fenced.2.1
  rw [Nat.add_one] at impossible
  exact (Nat.ne_of_lt (Nat.lt_succ_self lease.leaseEpoch))
    impossible

theorem changed_snapshot_blocks_active_holder_command
    {HolderId SnapshotId : Type}
    (lease : RecoveryLease HolderId SnapshotId)
    (candidateHolder : HolderId)
    (candidateLeaseEpoch expectedRevision now : Nat)
    (candidateSnapshot : SnapshotId)
    (snapshotChanged :
      candidateSnapshot ≠ lease.verificationSnapshotId) :
    ¬ PublicationLeaseFence
        lease candidateHolder candidateLeaseEpoch
        expectedRevision now candidateSnapshot := by
  intro fenced
  exact snapshotChanged fenced.2.2.2.1

theorem recovery_command_compatibility_binds_lease_epoch
    {TransitionId HolderId SnapshotId DecisionReceiptId : Type}
    (left right :
      RecoveryCommandKey
        TransitionId HolderId SnapshotId DecisionReceiptId)
    (compatible : RecoveryCommandCompatible left right) :
    left.leaseEpoch = right.leaseEpoch :=
  compatible.2.2.1

theorem changed_lease_epoch_rejects_recovery_replay
    {TransitionId HolderId SnapshotId DecisionReceiptId : Type}
    (left right :
      RecoveryCommandKey
        TransitionId HolderId SnapshotId DecisionReceiptId)
    (epochChanged :
      left.leaseEpoch ≠ right.leaseEpoch) :
    ¬ RecoveryCommandCompatible left right := by
  intro compatible
  exact
    epochChanged
      (recovery_command_compatibility_binds_lease_epoch
        left right compatible)

end ASPProof.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease
