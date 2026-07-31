import ASPProof.SearchRouteFrontierEvaluation

namespace SearchRouteAdmissionLedger

structure ReplayLedger (Identity : Type) where
  consumedReceipt : Identity → Prop
  committedRun : Identity → Prop

def CanCommit
    {Identity : Type}
    (ledger : ReplayLedger Identity)
    (runIdentity baselineReceiptIdentity extendedReceiptIdentity : Identity) : Prop :=
  baselineReceiptIdentity ≠ extendedReceiptIdentity
    ∧ ¬ledger.committedRun runIdentity
    ∧ ¬ledger.consumedReceipt baselineReceiptIdentity
    ∧ ¬ledger.consumedReceipt extendedReceiptIdentity

def AtomicRunCommit
    {Identity : Type}
    (before after : ReplayLedger Identity)
    (runIdentity baselineReceiptIdentity extendedReceiptIdentity : Identity) : Prop :=
  CanCommit before runIdentity baselineReceiptIdentity extendedReceiptIdentity
    ∧ (∀ identity,
      after.consumedReceipt identity ↔
        before.consumedReceipt identity
          ∨ identity = baselineReceiptIdentity
          ∨ identity = extendedReceiptIdentity)
    ∧ (∀ identity,
      after.committedRun identity ↔
        before.committedRun identity ∨ identity = runIdentity)

theorem atomic_commit_marks_run_and_both_receipts
    {Identity : Type}
    {before after : ReplayLedger Identity}
    {runIdentity baselineReceiptIdentity extendedReceiptIdentity : Identity}
    (commit :
      AtomicRunCommit before after runIdentity baselineReceiptIdentity
        extendedReceiptIdentity) :
    after.committedRun runIdentity
      ∧ after.consumedReceipt baselineReceiptIdentity
      ∧ after.consumedReceipt extendedReceiptIdentity := by
  exact ⟨
    (commit.2.2 runIdentity).2 (Or.inr rfl),
    (commit.2.1 baselineReceiptIdentity).2 (Or.inr (Or.inl rfl)),
    (commit.2.1 extendedReceiptIdentity).2 (Or.inr (Or.inr rfl))
  ⟩

theorem committed_run_rejects_exact_retry
    {Identity : Type}
    {before after : ReplayLedger Identity}
    {runIdentity baselineReceiptIdentity extendedReceiptIdentity : Identity}
    (commit :
      AtomicRunCommit before after runIdentity baselineReceiptIdentity
        extendedReceiptIdentity) :
    ¬CanCommit after runIdentity baselineReceiptIdentity
      extendedReceiptIdentity := by
  intro retry
  exact retry.2.1
    (atomic_commit_marks_run_and_both_receipts commit).1

def CrashConsistent
    {Identity : Type}
    (recovered before after : ReplayLedger Identity) : Prop :=
  recovered = before ∨ recovered = after

theorem crash_recovery_observes_pre_or_post_state
    {Identity : Type}
    {before after recovered : ReplayLedger Identity}
    {runIdentity baselineReceiptIdentity extendedReceiptIdentity : Identity}
    (commit :
      AtomicRunCommit before after runIdentity baselineReceiptIdentity
        extendedReceiptIdentity)
    (recovery : CrashConsistent recovered before after) :
    (¬recovered.committedRun runIdentity
        ∧ ¬recovered.consumedReceipt baselineReceiptIdentity
        ∧ ¬recovered.consumedReceipt extendedReceiptIdentity)
      ∨ (recovered.committedRun runIdentity
        ∧ recovered.consumedReceipt baselineReceiptIdentity
        ∧ recovered.consumedReceipt extendedReceiptIdentity) := by
  rcases recovery with recoveredBefore | recoveredAfter
  · subst recovered
    exact Or.inl ⟨commit.1.2.1, commit.1.2.2.1, commit.1.2.2.2⟩
  · subst recovered
    exact Or.inr (atomic_commit_marks_run_and_both_receipts commit)

inductive LedgerIdentity
  | run
  | baselineReceipt
  | extendedReceipt
  deriving DecidableEq

def emptyLedger : ReplayLedger LedgerIdentity where
  consumedReceipt := fun _ => False
  committedRun := fun _ => False

def partialLedger : ReplayLedger LedgerIdentity where
  consumedReceipt := fun identity => identity = .baselineReceipt
  committedRun := fun _ => False

def committedLedger : ReplayLedger LedgerIdentity where
  consumedReceipt := fun identity =>
    identity = .baselineReceipt ∨ identity = .extendedReceipt
  committedRun := fun identity => identity = .run

theorem partial_ledger_is_not_a_crash_consistent_state :
    ¬CrashConsistent partialLedger emptyLedger committedLedger := by
  intro recovery
  rcases recovery with partialEmpty | partialCommitted
  · have baselineStateEqual :=
      congrArg (fun ledger => ledger.consumedReceipt .baselineReceipt)
        partialEmpty
    simp [partialLedger, emptyLedger] at baselineStateEqual
  · have extensionStateEqual :=
      congrArg (fun ledger => ledger.consumedReceipt .extendedReceipt)
        partialCommitted
    simp [partialLedger, committedLedger] at extensionStateEqual

theorem empty_to_committed_ledger_is_an_atomic_relation :
    AtomicRunCommit emptyLedger committedLedger .run .baselineReceipt
      .extendedReceipt := by
  simp [AtomicRunCommit, CanCommit, emptyLedger, committedLedger]

theorem atomic_relation_alone_does_not_prove_durable_recovery :
    AtomicRunCommit emptyLedger committedLedger .run .baselineReceipt
        .extendedReceipt
      ∧ ¬CrashConsistent partialLedger emptyLedger committedLedger := by
  exact ⟨
    empty_to_committed_ledger_is_an_atomic_relation,
    partial_ledger_is_not_a_crash_consistent_state
  ⟩

theorem read_check_then_partial_write_is_not_atomic_admission :
    CanCommit emptyLedger .run .baselineReceipt .extendedReceipt
      ∧ ¬CrashConsistent partialLedger emptyLedger committedLedger := by
  exact ⟨
    by simp [CanCommit, emptyLedger],
    partial_ledger_is_not_a_crash_consistent_state
  ⟩

end SearchRouteAdmissionLedger
