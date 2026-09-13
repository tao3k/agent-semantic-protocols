-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean

namespace ASPProof.SearchRouteReservationLifecycleConservation

structure LifecycleLedger where
  available : Nat
  held : Nat
  consumed : Nat
  quarantined : Nat
  deriving DecidableEq, Repr

def ledgerTotal (ledger : LifecycleLedger) : Nat :=
  ledger.available + ledger.held + ledger.consumed + ledger.quarantined

def reserve (ledger : LifecycleLedger) (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available - amount
    held := ledger.held + amount
    consumed := ledger.consumed
    quarantined := ledger.quarantined
  }

def consumeHeld
    (ledger : LifecycleLedger)
    (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available
    held := ledger.held - amount
    consumed := ledger.consumed + amount
    quarantined := ledger.quarantined
  }

def quarantineHeld
    (ledger : LifecycleLedger)
    (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available
    held := ledger.held - amount
    consumed := ledger.consumed
    quarantined := ledger.quarantined + amount
  }

def releaseHeld
    (ledger : LifecycleLedger)
    (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available + amount
    held := ledger.held - amount
    consumed := ledger.consumed
    quarantined := ledger.quarantined
  }

def consumeQuarantined
    (ledger : LifecycleLedger)
    (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available
    held := ledger.held
    consumed := ledger.consumed + amount
    quarantined := ledger.quarantined - amount
  }

def releaseQuarantined
    (ledger : LifecycleLedger)
    (amount : Nat) : LifecycleLedger :=
  {
    available := ledger.available + amount
    held := ledger.held
    consumed := ledger.consumed
    quarantined := ledger.quarantined - amount
  }

theorem reserve_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.available) :
    ledgerTotal (reserve ledger amount) = ledgerTotal ledger := by
  simp [ledgerTotal, reserve]
  omega

theorem consume_held_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.held) :
    ledgerTotal (consumeHeld ledger amount) = ledgerTotal ledger := by
  simp [ledgerTotal, consumeHeld]
  omega

theorem quarantine_held_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.held) :
    ledgerTotal (quarantineHeld ledger amount) = ledgerTotal ledger := by
  simp [ledgerTotal, quarantineHeld]
  omega

theorem release_held_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.held) :
    ledgerTotal (releaseHeld ledger amount) = ledgerTotal ledger := by
  simp [ledgerTotal, releaseHeld]
  omega

theorem consume_quarantined_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.quarantined) :
    ledgerTotal (consumeQuarantined ledger amount) =
      ledgerTotal ledger := by
  simp [ledgerTotal, consumeQuarantined]
  omega

theorem release_quarantined_preserves_total
    (ledger : LifecycleLedger)
    (amount : Nat)
    (enough : amount ≤ ledger.quarantined) :
    ledgerTotal (releaseQuarantined ledger amount) =
      ledgerTotal ledger := by
  simp [ledgerTotal, releaseQuarantined]
  omega

inductive LedgerTransition :
    LifecycleLedger → LifecycleLedger → Prop where
  | reserveStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.available) :
      LedgerTransition before (reserve before amount)
  | consumeHeldStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.held) :
      LedgerTransition before (consumeHeld before amount)
  | quarantineHeldStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.held) :
      LedgerTransition before (quarantineHeld before amount)
  | releaseHeldStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.held) :
      LedgerTransition before (releaseHeld before amount)
  | consumeQuarantinedStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.quarantined) :
      LedgerTransition before (consumeQuarantined before amount)
  | releaseQuarantinedStep
      (before : LifecycleLedger)
      (amount : Nat)
      (enough : amount ≤ before.quarantined) :
      LedgerTransition before (releaseQuarantined before amount)

theorem every_ledger_transition_preserves_total
    (before after : LifecycleLedger)
    (transition : LedgerTransition before after) :
    ledgerTotal after = ledgerTotal before := by
  cases transition with
  | reserveStep amount enough =>
      exact reserve_preserves_total before amount enough
  | consumeHeldStep amount enough =>
      exact consume_held_preserves_total before amount enough
  | quarantineHeldStep amount enough =>
      exact quarantine_held_preserves_total before amount enough
  | releaseHeldStep amount enough =>
      exact release_held_preserves_total before amount enough
  | consumeQuarantinedStep amount enough =>
      exact consume_quarantined_preserves_total before amount enough
  | releaseQuarantinedStep amount enough =>
      exact release_quarantined_preserves_total before amount enough

inductive ReservationDisposition where
  | held
  | consumed
  | quarantined
  | released
  deriving DecidableEq, Repr

inductive DispositionTransition :
    ReservationDisposition → ReservationDisposition → Prop where
  | consume :
      DispositionTransition .held .consumed
  | quarantine :
      DispositionTransition .held .quarantined
  | releaseHeld :
      DispositionTransition .held .released
  | resolveCompleted :
      DispositionTransition .quarantined .consumed
  | resolveNoEffect :
      DispositionTransition .quarantined .released

theorem consumed_is_terminal :
    ¬ ∃ next, DispositionTransition .consumed next := by
  intro alleged
  obtain ⟨next, transition⟩ := alleged
  cases transition

theorem released_is_terminal :
    ¬ ∃ next, DispositionTransition .released next := by
  intro alleged
  obtain ⟨next, transition⟩ := alleged
  cases transition

structure ReservationRecord where
  reservationDigest : Nat
  amount : Nat
  disposition : ReservationDisposition
  revision : Nat
  deriving DecidableEq, Repr

def RecordTransition
    (before after : ReservationRecord) : Prop :=
  before.reservationDigest = after.reservationDigest ∧
  before.amount = after.amount ∧
  DispositionTransition before.disposition after.disposition ∧
  after.revision = before.revision + 1

theorem record_transition_advances_revision
    (before after : ReservationRecord)
    (transition : RecordTransition before after) :
    before.revision < after.revision := by
  rw [transition.2.2.2]
  exact Nat.lt_succ_self before.revision

def initialLedger : LifecycleLedger :=
  {
    available := 100
    held := 0
    consumed := 0
    quarantined := 0
  }

def reservedLedger : LifecycleLedger :=
  reserve initialLedger 20

def quarantinedLedger : LifecycleLedger :=
  quarantineHeld reservedLedger 20

def releasedLedger : LifecycleLedger :=
  releaseQuarantined quarantinedLedger 20

def consumedLedger : LifecycleLedger :=
  consumeQuarantined quarantinedLedger 20

theorem quarantine_release_path_preserves_one_hundred :
    ledgerTotal initialLedger = 100 ∧
      ledgerTotal reservedLedger = 100 ∧
      ledgerTotal quarantinedLedger = 100 ∧
      ledgerTotal releasedLedger = 100 := by
  decide

theorem quarantine_consume_path_preserves_one_hundred :
    ledgerTotal initialLedger = 100 ∧
      ledgerTotal reservedLedger = 100 ∧
      ledgerTotal quarantinedLedger = 100 ∧
      ledgerTotal consumedLedger = 100 := by
  decide

end ASPProof.SearchRouteReservationLifecycleConservation
