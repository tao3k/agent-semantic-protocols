import ASPProof.SearchRouteReservationLifecycleConservation

namespace ASPProof.SearchRouteReservationCasLinearizability

open ASPProof.SearchRouteReservationLifecycleConservation

structure CasProposal where
  reservationDigest : Nat
  expectedRevision : Nat
  target : ReservationDisposition
  deriving DecidableEq, Repr

def CanCommit
    (current : ReservationRecord)
    (proposal : CasProposal) : Prop :=
  proposal.reservationDigest = current.reservationDigest ∧
  proposal.expectedRevision = current.revision ∧
  DispositionTransition current.disposition proposal.target

def commit
    (current : ReservationRecord)
    (proposal : CasProposal) : ReservationRecord :=
  {
    reservationDigest := current.reservationDigest
    amount := current.amount
    disposition := proposal.target
    revision := current.revision + 1
  }

theorem commit_preserves_reservation_identity
    (current : ReservationRecord)
    (proposal : CasProposal) :
    (commit current proposal).reservationDigest =
      current.reservationDigest := by
  rfl

theorem commit_preserves_amount
    (current : ReservationRecord)
    (proposal : CasProposal) :
    (commit current proposal).amount = current.amount := by
  rfl

theorem commit_advances_revision
    (current : ReservationRecord)
    (proposal : CasProposal) :
    (commit current proposal).revision = current.revision + 1 := by
  rfl

theorem competing_old_revision_is_rejected
    (current : ReservationRecord)
    (winner loser : CasProposal)
    (winnerAllowed : CanCommit current winner)
    (sameExpectedRevision :
      loser.expectedRevision = winner.expectedRevision) :
    ¬ CanCommit (commit current winner) loser := by
  intro loserAllowed
  have winnerExpected := winnerAllowed.2.1
  have loserExpected := loserAllowed.2.1
  simp [commit] at loserExpected
  omega

def currentRecord : ReservationRecord :=
  {
    reservationDigest := 141
    amount := 20
    disposition := .held
    revision := 5
  }

def consumeProposal : CasProposal :=
  {
    reservationDigest := 141
    expectedRevision := 5
    target := .consumed
  }

def quarantineProposal : CasProposal :=
  {
    reservationDigest := 141
    expectedRevision := 5
    target := .quarantined
  }

theorem consume_proposal_is_initially_allowed :
    CanCommit currentRecord consumeProposal := by
  exact ⟨rfl, rfl, .consume⟩

theorem quarantine_proposal_is_initially_allowed :
    CanCommit currentRecord quarantineProposal := by
  exact ⟨rfl, rfl, .quarantine⟩

theorem quarantine_proposal_is_stale_after_consume :
    ¬ CanCommit
      (commit currentRecord consumeProposal)
      quarantineProposal := by
  exact competing_old_revision_is_rejected
    currentRecord
    consumeProposal
    quarantineProposal
    consume_proposal_is_initially_allowed
    rfl

def MovementMatches
    (beforeLedger afterLedger : LifecycleLedger)
    (beforeRecord afterRecord : ReservationRecord) : Prop :=
  match beforeRecord.disposition, afterRecord.disposition with
  | .held, .consumed =>
      afterLedger = consumeHeld beforeLedger beforeRecord.amount
  | .held, .quarantined =>
      afterLedger = quarantineHeld beforeLedger beforeRecord.amount
  | .held, .released =>
      afterLedger = releaseHeld beforeLedger beforeRecord.amount
  | .quarantined, .consumed =>
      afterLedger =
        consumeQuarantined beforeLedger beforeRecord.amount
  | .quarantined, .released =>
      afterLedger =
        releaseQuarantined beforeLedger beforeRecord.amount
  | _, _ => False

structure AtomicLifecycleCommit where
  beforeLedger : LifecycleLedger
  afterLedger : LifecycleLedger
  beforeRecord : ReservationRecord
  afterRecord : ReservationRecord
  ledgerTransition :
    LedgerTransition beforeLedger afterLedger
  recordTransition :
    RecordTransition beforeRecord afterRecord
  movementMatches :
    MovementMatches
      beforeLedger afterLedger beforeRecord afterRecord

theorem atomic_commit_preserves_ledger_total
    (transaction : AtomicLifecycleCommit) :
    ledgerTotal transaction.afterLedger =
      ledgerTotal transaction.beforeLedger :=
  every_ledger_transition_preserves_total
    transaction.beforeLedger
    transaction.afterLedger
    transaction.ledgerTransition

theorem atomic_commit_advances_record_revision
    (transaction : AtomicLifecycleCommit) :
    transaction.beforeRecord.revision <
      transaction.afterRecord.revision :=
  record_transition_advances_revision
    transaction.beforeRecord
    transaction.afterRecord
    transaction.recordTransition

theorem consumed_record_rejects_every_proposal
    (current : ReservationRecord)
    (consumed : current.disposition = .consumed)
    (proposal : CasProposal) :
    ¬ CanCommit current proposal := by
  intro allowed
  have transition := allowed.2.2
  rw [consumed] at transition
  exact consumed_is_terminal ⟨proposal.target, transition⟩

theorem released_record_rejects_every_proposal
    (current : ReservationRecord)
    (released : current.disposition = .released)
    (proposal : CasProposal) :
    ¬ CanCommit current proposal := by
  intro allowed
  have transition := allowed.2.2
  rw [released] at transition
  exact released_is_terminal ⟨proposal.target, transition⟩

end ASPProof.SearchRouteReservationCasLinearizability

