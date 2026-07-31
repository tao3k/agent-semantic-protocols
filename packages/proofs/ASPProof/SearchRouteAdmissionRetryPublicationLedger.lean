import ASPProof.SearchRouteAdmissionRetryPublication

namespace ASPProof.SearchRouteAdmissionRetryPublicationLedger

universe u v

abbrev PublicationLedger
    (RetryKey : Type u)
    (Certificate : Type v) :=
  RetryKey → Option Certificate

inductive PublishOutcome (Certificate : Type v) where
  | won (winner : Certificate)
  | existing (winner : Certificate)
  | rejected
deriving DecidableEq, Repr

/--
Existing-winner recovery precedes candidate validation. Validation authorizes a
new installation; it does not prevent an idempotent retry from recovering an
already durable winner.
-/
def atomicPublish
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (candidate : Certificate) :
    PublicationLedger RetryKey Certificate × PublishOutcome Certificate :=
  match ledger retryKey with
  | some winner =>
      (ledger, PublishOutcome.existing winner)
  | none =>
      if validate candidate then
        ( fun queryKey =>
            if queryKey = retryKey then some candidate else ledger queryKey
        , PublishOutcome.won candidate )
      else
        (ledger, PublishOutcome.rejected)

theorem existing_winner_is_returned_without_republication
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (candidate winner : Certificate)
    (existing : ledger retryKey = some winner) :
    atomicPublish validate ledger retryKey candidate =
      (ledger, PublishOutcome.existing winner) := by
  simp [atomicPublish, existing]

theorem invalid_candidate_cannot_win_empty_key
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (candidate : Certificate)
    (empty : ledger retryKey = none)
    (invalid : ¬ validate candidate) :
    atomicPublish validate ledger retryKey candidate =
      (ledger, PublishOutcome.rejected) := by
  simp [atomicPublish, empty, invalid]

theorem valid_candidate_wins_empty_key
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (candidate : Certificate)
    (empty : ledger retryKey = none)
    (valid : validate candidate) :
    (atomicPublish validate ledger retryKey candidate).1 retryKey =
        some candidate
      ∧ (atomicPublish validate ledger retryKey candidate).2 =
        PublishOutcome.won candidate := by
  simp [atomicPublish, empty, valid]

theorem installed_winner_is_sticky_for_any_later_candidate
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (firstCandidate laterCandidate : Certificate)
    (empty : ledger retryKey = none)
    (firstValid : validate firstCandidate) :
    let first := atomicPublish validate ledger retryKey firstCandidate
    let later := atomicPublish validate first.1 retryKey laterCandidate
    first.2 = PublishOutcome.won firstCandidate
      ∧ later.1 retryKey = some firstCandidate
      ∧ later.2 = PublishOutcome.existing firstCandidate := by
  simp [atomicPublish, empty, firstValid]

theorem invalid_losing_retry_still_recovers_existing_winner
    {RetryKey : Type u}
    {Certificate : Type v}
    [DecidableEq RetryKey]
    (validate : Certificate → Prop)
    [DecidablePred validate]
    (ledger : PublicationLedger RetryKey Certificate)
    (retryKey : RetryKey)
    (invalidCandidate winner : Certificate)
    (existing : ledger retryKey = some winner)
    (_invalid : ¬ validate invalidCandidate) :
    atomicPublish validate ledger retryKey invalidCandidate =
      (ledger, PublishOutcome.existing winner) :=
  existing_winner_is_returned_without_republication
    validate
    ledger
    retryKey
    invalidCandidate
    winner
    existing

def naiveCheckThenWrite
    {Certificate : Type v}
    (observed : Option Certificate)
    (candidate : Certificate) :
    PublishOutcome Certificate :=
  match observed with
  | none => PublishOutcome.won candidate
  | some winner => PublishOutcome.existing winner

theorem non_atomic_check_then_write_admits_two_distinct_winner_claims :
    naiveCheckThenWrite none true = PublishOutcome.won true
      ∧ naiveCheckThenWrite none false = PublishOutcome.won false
      ∧ true ≠ false := by
  decide

end ASPProof.SearchRouteAdmissionRetryPublicationLedger
