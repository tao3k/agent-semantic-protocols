import Lean.Elab.Tactic.Omega

namespace ASPProof.SearchRouteAppendOnlyCompensation

inductive CompensationKind
  | credit
  | debit
  deriving DecidableEq, Repr

inductive CompensationStatus
  | pending
  | applied
  deriving DecidableEq, Repr

structure CompensationLedger where
  capacity : Nat
  available : Nat
  held : Nat
  quarantined : Nat
  grossConsumed : Nat
  credits : Nat
  deriving DecidableEq, Repr

def Balanced (ledger : CompensationLedger) : Prop :=
  ledger.available
    + ledger.held
    + ledger.quarantined
    + ledger.grossConsumed
    = ledger.capacity + ledger.credits

def NetConsumed (ledger : CompensationLedger) : Nat :=
  ledger.grossConsumed - ledger.credits

def CanAdjust
    (ledger : CompensationLedger)
    (amount : Nat)
    (kind : CompensationKind) : Prop :=
  match kind with
  | .credit => ledger.credits + amount ≤ ledger.grossConsumed
  | .debit => amount ≤ ledger.available

def applyAdjustment
    (ledger : CompensationLedger)
    (amount : Nat)
    (kind : CompensationKind) : CompensationLedger :=
  match kind with
  | .credit =>
      { ledger with
        available := ledger.available + amount
        credits := ledger.credits + amount }
  | .debit =>
      { ledger with
        available := ledger.available - amount
        grossConsumed := ledger.grossConsumed + amount }

theorem adjustment_preserves_balance
    (ledger : CompensationLedger)
    (amount : Nat)
    (kind : CompensationKind)
    (balanced : Balanced ledger)
    (allowed : CanAdjust ledger amount kind) :
    Balanced (applyAdjustment ledger amount kind) := by
  cases kind <;>
    simp [Balanced, CanAdjust, applyAdjustment] at balanced allowed ⊢ <;>
    omega

theorem gross_consumed_is_append_only
    (ledger : CompensationLedger)
    (amount : Nat)
    (kind : CompensationKind) :
    ledger.grossConsumed ≤ (applyAdjustment ledger amount kind).grossConsumed := by
  cases kind <;> simp [applyAdjustment]

theorem credits_are_append_only
    (ledger : CompensationLedger)
    (amount : Nat)
    (kind : CompensationKind) :
    ledger.credits ≤ (applyAdjustment ledger amount kind).credits := by
  cases kind <;> simp [applyAdjustment]

theorem allowed_credit_remains_bounded_by_history
    (ledger : CompensationLedger)
    (amount : Nat)
    (allowed : CanAdjust ledger amount .credit) :
    (applyAdjustment ledger amount .credit).credits
      ≤ (applyAdjustment ledger amount .credit).grossConsumed := by
  simpa [CanAdjust, applyAdjustment] using allowed

structure CompensationRecord where
  correctionDigest : String
  authorityDigest : String
  dimensionDigest : String
  amount : Nat
  revision : Nat
  status : CompensationStatus
  deriving DecidableEq, Repr

structure CompensationProposal where
  correctionDigest : String
  authorityDigest : String
  dimensionDigest : String
  amount : Nat
  expectedRevision : Nat
  kind : CompensationKind
  deriving DecidableEq, Repr

def CanCommit
    (record : CompensationRecord)
    (proposal : CompensationProposal) : Prop :=
  proposal.correctionDigest = record.correctionDigest
    ∧ proposal.authorityDigest = record.authorityDigest
    ∧ proposal.dimensionDigest = record.dimensionDigest
    ∧ proposal.amount = record.amount
    ∧ proposal.expectedRevision = record.revision
    ∧ record.status = .pending

def commitRecord
    (record : CompensationRecord)
    (_proposal : CompensationProposal) : CompensationRecord :=
  { record with
    revision := record.revision + 1
    status := .applied }

def applyAuthorizedCompensation
    (ledger : CompensationLedger)
    (record : CompensationRecord)
    (proposal : CompensationProposal)
    (_authorized : CanCommit record proposal)
    (_allowed : CanAdjust ledger proposal.amount proposal.kind) :
    CompensationLedger × CompensationRecord :=
  (applyAdjustment ledger proposal.amount proposal.kind,
   commitRecord record proposal)

theorem authorized_compensation_preserves_balance
    (ledger : CompensationLedger)
    (record : CompensationRecord)
    (proposal : CompensationProposal)
    (authorized : CanCommit record proposal)
    (allowed : CanAdjust ledger proposal.amount proposal.kind)
    (balanced : Balanced ledger) :
    Balanced
      (applyAuthorizedCompensation
        ledger record proposal authorized allowed).1 := by
  exact adjustment_preserves_balance
    ledger proposal.amount proposal.kind balanced allowed

theorem compensation_commit_advances_revision
    (ledger : CompensationLedger)
    (record : CompensationRecord)
    (proposal : CompensationProposal)
    (authorized : CanCommit record proposal)
    (allowed : CanAdjust ledger proposal.amount proposal.kind) :
    (applyAuthorizedCompensation
      ledger record proposal authorized allowed).2.revision
      = record.revision + 1 := by
  rfl

theorem committed_record_rejects_replay
    (record : CompensationRecord)
    (proposal : CompensationProposal) :
    ¬ CanCommit (commitRecord record proposal) proposal := by
  simp [CanCommit, commitRecord]

def consumedTwenty : CompensationLedger where
  capacity := 100
  available := 80
  held := 0
  quarantined := 0
  grossConsumed := 20
  credits := 0

example : Balanced consumedTwenty := by
  rfl

example : CanAdjust consumedTwenty 20 .credit := by
  simp [CanAdjust, consumedTwenty]

example : Balanced (applyAdjustment consumedTwenty 20 .credit) := by
  exact adjustment_preserves_balance
    consumedTwenty 20 .credit
    (by rfl)
    (by simp [CanAdjust, consumedTwenty])

example :
    (applyAdjustment consumedTwenty 20 .credit).grossConsumed
      = consumedTwenty.grossConsumed := by
  rfl

example :
    ¬ CanAdjust
      (applyAdjustment consumedTwenty 20 .credit)
      20
      .credit := by
  simp [CanAdjust, applyAdjustment, consumedTwenty]

end ASPProof.SearchRouteAppendOnlyCompensation
