import ASPProof.AgentSessionLifecycleFencingEpoch

namespace ASPProof.AgentSessionLifecycleLeaseRenewalClockAuthority

open AgentSessionTransactionalGenerationLifecycle

structure LeaseState where
  slot : ResidentSlot
  generation : Nat
  fencingEpoch : Nat
  revision : Nat
  issuedAt : Nat
  expiresAt : Nat
  clockEpoch : Nat
  capabilityDigest : Nat
  deriving DecidableEq

structure RenewalRequest where
  slot : ResidentSlot
  generation : Nat
  fencingEpoch : Nat
  expectedRevision : Nat
  previousExpiry : Nat
  requestedExpiry : Nat
  clockEpoch : Nat
  capabilityDigest : Nat
  renewalNonce : Nat
  deriving DecidableEq

structure RenewalReceipt where
  slot : ResidentSlot
  generation : Nat
  fencingEpoch : Nat
  renewalNonce : Nat
  commitRevision : Nat
  oldExpiry : Nat
  newExpiry : Nat
  authorityTime : Nat
  clockEpoch : Nat
  capabilityDigest : Nat
  deriving DecidableEq

structure ValidRenewal
    (current : LeaseState)
    (request : RenewalRequest)
    (receipt : RenewalReceipt) : Prop where
  requestSlot : request.slot = current.slot
  requestGeneration : request.generation = current.generation
  requestEpoch : request.fencingEpoch = current.fencingEpoch
  requestRevision : request.expectedRevision = current.revision
  requestPreviousExpiry : request.previousExpiry = current.expiresAt
  requestClockEpoch : request.clockEpoch = current.clockEpoch
  requestCapability : request.capabilityDigest = current.capabilityDigest
  expiryExtends : current.expiresAt < request.requestedExpiry
  authorityBeforeExpiry : receipt.authorityTime < current.expiresAt
  receiptSlot : receipt.slot = current.slot
  receiptGeneration : receipt.generation = current.generation
  receiptEpoch : receipt.fencingEpoch = current.fencingEpoch
  receiptNonce : receipt.renewalNonce = request.renewalNonce
  receiptRevision : receipt.commitRevision = current.revision + 1
  receiptOldExpiry : receipt.oldExpiry = current.expiresAt
  receiptNewExpiry : receipt.newExpiry = request.requestedExpiry
  receiptClockEpoch : receipt.clockEpoch = current.clockEpoch
  receiptCapability : receipt.capabilityDigest = current.capabilityDigest

def renewedLease (current : LeaseState) (receipt : RenewalReceipt) : LeaseState :=
  { slot := current.slot
    generation := current.generation
    fencingEpoch := current.fencingEpoch
    revision := receipt.commitRevision
    issuedAt := receipt.authorityTime
    expiresAt := receipt.newExpiry
    clockEpoch := current.clockEpoch
    capabilityDigest := current.capabilityDigest }

def ValidRenewalAtLocalClock
    (current : LeaseState)
    (request : RenewalRequest)
    (receipt : RenewalReceipt)
    (_localTime : Nat) : Prop :=
  ValidRenewal current request receipt

def RenewalLedgerLinearizable (receipts : List RenewalReceipt) : Prop :=
  ∀ left, left ∈ receipts → ∀ right, right ∈ receipts →
    left.slot = right.slot →
    left.fencingEpoch = right.fencingEpoch →
    left.renewalNonce = right.renewalNonce →
    left = right

structure InFlightDispatchLease where
  dispatchKey : Nat
  generation : Nat
  fencingEpoch : Nat
  validUntil : Nat
  deriving DecidableEq

def DispatchValidAt (dispatch : InFlightDispatchLease) (time : Nat) : Prop :=
  time < dispatch.validUntil

theorem valid_renewal_binds_epoch
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    request.fencingEpoch = current.fencingEpoch :=
  valid.requestEpoch

theorem valid_renewal_binds_revision
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    request.expectedRevision = current.revision :=
  valid.requestRevision

theorem valid_renewal_binds_clock_epoch
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    request.clockEpoch = current.clockEpoch :=
  valid.requestClockEpoch

theorem valid_renewal_binds_capability_digest
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    request.capabilityDigest = current.capabilityDigest :=
  valid.requestCapability

theorem valid_renewal_strictly_extends_expiry
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    current.expiresAt < request.requestedExpiry :=
  valid.expiryExtends

theorem valid_renewal_uses_authority_time_before_expiry
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    receipt.authorityTime < current.expiresAt :=
  valid.authorityBeforeExpiry

theorem renewed_lease_preserves_generation
    {current request receipt}
    (_valid : ValidRenewal current request receipt) :
    (renewedLease current receipt).generation = current.generation :=
  rfl

theorem renewed_lease_preserves_fencing_epoch
    {current request receipt}
    (_valid : ValidRenewal current request receipt) :
    (renewedLease current receipt).fencingEpoch = current.fencingEpoch :=
  rfl

theorem renewed_lease_advances_revision
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    (renewedLease current receipt).revision = current.revision + 1 :=
  valid.receiptRevision

theorem renewed_lease_uses_requested_expiry
    {current request receipt}
    (valid : ValidRenewal current request receipt) :
    (renewedLease current receipt).expiresAt = request.requestedExpiry :=
  valid.receiptNewExpiry

theorem stale_epoch_rejects_renewal
    {current request receipt}
    (stale : request.fencingEpoch ≠ current.fencingEpoch) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact stale valid.requestEpoch

theorem stale_revision_rejects_renewal
    {current request receipt}
    (stale : request.expectedRevision ≠ current.revision) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact stale valid.requestRevision

theorem stale_clock_epoch_rejects_renewal
    {current request receipt}
    (stale : request.clockEpoch ≠ current.clockEpoch) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact stale valid.requestClockEpoch

theorem capability_drift_rejects_renewal
    {current request receipt}
    (drift : request.capabilityDigest ≠ current.capabilityDigest) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact drift valid.requestCapability

theorem expired_lease_cannot_be_renewed
    {current request receipt}
    (expired : current.expiresAt ≤ receipt.authorityTime) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact (Nat.not_lt_of_ge expired) valid.authorityBeforeExpiry

theorem replayed_old_revision_receipt_is_rejected
    {current request receipt}
    (replayed : receipt.commitRevision ≠ current.revision + 1) :
    ¬ ValidRenewal current request receipt := by
  intro valid
  exact replayed valid.receiptRevision

theorem local_clock_cannot_change_renewal_validity
    (current : LeaseState) (request : RenewalRequest)
    (receipt : RenewalReceipt) (leftLocal rightLocal : Nat) :
    ValidRenewalAtLocalClock current request receipt leftLocal ↔
    ValidRenewalAtLocalClock current request receipt rightLocal :=
  Iff.rfl

theorem renewal_ledger_has_one_receipt_per_nonce
    {receipts : List RenewalReceipt}
    (linearizable : RenewalLedgerLinearizable receipts)
    {left right : RenewalReceipt}
    (leftMember : left ∈ receipts) (rightMember : right ∈ receipts)
    (sameSlot : left.slot = right.slot)
    (sameEpoch : left.fencingEpoch = right.fencingEpoch)
    (sameNonce : left.renewalNonce = right.renewalNonce) : left = right :=
  linearizable left leftMember right rightMember sameSlot sameEpoch sameNonce

theorem renewal_ledger_rejects_expiry_conflict
    {receipts : List RenewalReceipt}
    (linearizable : RenewalLedgerLinearizable receipts)
    {left right : RenewalReceipt}
    (leftMember : left ∈ receipts) (rightMember : right ∈ receipts)
    (sameSlot : left.slot = right.slot)
    (sameEpoch : left.fencingEpoch = right.fencingEpoch)
    (sameNonce : left.renewalNonce = right.renewalNonce)
    (conflict : left.newExpiry ≠ right.newExpiry) : False := by
  have same := linearizable left leftMember right rightMember
    sameSlot sameEpoch sameNonce
  exact conflict (congrArg RenewalReceipt.newExpiry same)

theorem valid_inflight_dispatch_survives_expiry_extension
    (dispatch : InFlightDispatchLease) (time newExpiry : Nat)
    (validNow : DispatchValidAt dispatch time)
    (extended : dispatch.validUntil < newExpiry) : time < newExpiry :=
  Nat.lt_trans validNow extended

theorem renewal_does_not_change_dispatch_identity
    (dispatch : InFlightDispatchLease) (newExpiry : Nat) :
    ({ dispatch with validUntil := newExpiry }).dispatchKey = dispatch.dispatchKey :=
  rfl

theorem renewal_does_not_change_dispatch_generation
    (dispatch : InFlightDispatchLease) (newExpiry : Nat) :
    ({ dispatch with validUntil := newExpiry }).generation = dispatch.generation :=
  rfl

theorem renewal_does_not_change_dispatch_fencing_epoch
    (dispatch : InFlightDispatchLease) (newExpiry : Nat) :
    ({ dispatch with validUntil := newExpiry }).fencingEpoch = dispatch.fencingEpoch :=
  rfl

end ASPProof.AgentSessionLifecycleLeaseRenewalClockAuthority
