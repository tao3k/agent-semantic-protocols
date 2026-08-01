import ASPProof.SearchRouteReservationLifecycleConservation

namespace ASPProof.SearchRouteAtomicMultiResourceReservation

open ASPProof.SearchRouteReservationLifecycleConservation

structure ResourceDemand where
  tokens : Nat
  money : Nat
  providerQuota : Nat
  deriving DecidableEq, Repr

structure MultiResourceLedger where
  tokens : LifecycleLedger
  money : LifecycleLedger
  providerQuota : LifecycleLedger
  deriving DecidableEq, Repr

abbrev CanReserveAll
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand) : Prop :=
  demand.tokens ≤ ledger.tokens.available ∧
  demand.money ≤ ledger.money.available ∧
  demand.providerQuota ≤ ledger.providerQuota.available

def reserveAll
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand) : MultiResourceLedger :=
  {
    tokens := reserve ledger.tokens demand.tokens
    money := reserve ledger.money demand.money
    providerQuota :=
      reserve ledger.providerQuota demand.providerQuota
  }

inductive AtomicReservationOutcome where
  | admitted
  | rejected
  deriving DecidableEq, Repr

def reserveAtomically
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand) :
    MultiResourceLedger × AtomicReservationOutcome :=
  if CanReserveAll ledger demand then
    (reserveAll ledger demand, .admitted)
  else
    (ledger, .rejected)

theorem admitted_iff_all_dimensions_fit
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand) :
    (reserveAtomically ledger demand).2 = .admitted ↔
      CanReserveAll ledger demand := by
  unfold reserveAtomically
  split
  case isTrue fits =>
    constructor
    · intro _
      exact fits
    · intro _
      rfl
  case isFalse doesNotFit =>
    constructor
    · intro impossible
      cases impossible
    · intro fits
      exact False.elim (doesNotFit fits)

theorem rejected_result_preserves_input_ledger
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand)
    (rejected :
      (reserveAtomically ledger demand).2 = .rejected) :
    (reserveAtomically ledger demand).1 = ledger := by
  unfold reserveAtomically at rejected ⊢
  split
  case isTrue fits =>
    have impossible :
        AtomicReservationOutcome.admitted = .rejected := by
      simpa only [if_pos fits] using rejected
    cases impossible
  case isFalse =>
    rfl

theorem successful_reservation_preserves_all_totals
    (ledger : MultiResourceLedger)
    (demand : ResourceDemand)
    (fits : CanReserveAll ledger demand) :
    ledgerTotal (reserveAll ledger demand).tokens =
        ledgerTotal ledger.tokens ∧
      ledgerTotal (reserveAll ledger demand).money =
        ledgerTotal ledger.money ∧
      ledgerTotal (reserveAll ledger demand).providerQuota =
        ledgerTotal ledger.providerQuota := by
  exact
    ⟨
      reserve_preserves_total ledger.tokens demand.tokens fits.1,
      reserve_preserves_total ledger.money demand.money fits.2.1,
      reserve_preserves_total
        ledger.providerQuota
        demand.providerQuota
        fits.2.2
    ⟩

def emptyResourceLedger (available : Nat) : LifecycleLedger :=
  {
    available := available
    held := 0
    consumed := 0
    quarantined := 0
  }

def exampleLedger : MultiResourceLedger :=
  {
    tokens := emptyResourceLedger 100
    money := emptyResourceLedger 5
    providerQuota := emptyResourceLedger 2
  }

def exampleDemand : ResourceDemand :=
  {
    tokens := 20
    money := 10
    providerQuota := 1
  }

theorem example_tokens_fit :
    exampleDemand.tokens ≤ exampleLedger.tokens.available := by
  decide

theorem example_money_does_not_fit :
    ¬ exampleDemand.money ≤ exampleLedger.money.available := by
  decide

theorem example_provider_quota_fits :
    exampleDemand.providerQuota ≤
      exampleLedger.providerQuota.available := by
  decide

theorem example_multi_resource_admission_is_rejected :
    (reserveAtomically exampleLedger exampleDemand).2 = .rejected := by
  decide

theorem example_rejection_leaves_every_ledger_unchanged :
    (reserveAtomically exampleLedger exampleDemand).1 =
      exampleLedger := by
  decide

theorem partial_component_feasibility_does_not_imply_admission :
    exampleDemand.tokens ≤ exampleLedger.tokens.available ∧
      exampleDemand.providerQuota ≤
        exampleLedger.providerQuota.available ∧
      ¬ CanReserveAll exampleLedger exampleDemand := by
  decide

end ASPProof.SearchRouteAtomicMultiResourceReservation
