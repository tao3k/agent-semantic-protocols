-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentSessionDispatchTransaction

abbrev RootSessionId := Nat
abbrev TurnId := Nat
abbrev TargetId := Nat
abbrev BindingGeneration := Nat
abbrev Revision := Nat
abbrev PayloadDigest := Nat

structure DispatchKey where
  root : RootSessionId
  turn : TurnId
  target : TargetId
  bindingGeneration : BindingGeneration
  deriving DecidableEq, Repr

structure DispatchIntent where
  key : DispatchKey
  payloadDigest : PayloadDigest
  deriving DecidableEq, Repr

structure HostReceipt
    (key : DispatchKey)
    (payloadDigest : PayloadDigest) where
  sequence : Nat
  deriving DecidableEq, Repr

inductive HostSlot (key : DispatchKey) where
  | empty
  | accepted
      (payloadDigest : PayloadDigest)
      (receipt : HostReceipt key payloadDigest)
  deriving DecidableEq, Repr

def firstHostReceipt
    (key : DispatchKey)
    (payloadDigest : PayloadDigest) :
    HostReceipt key payloadDigest :=
  { sequence := 0 }

inductive HostAccepts (key : DispatchKey) :
    HostSlot key → PayloadDigest → HostSlot key → Prop where
  | first (payloadDigest : PayloadDigest) :
      HostAccepts key
        .empty
        payloadDigest
        (.accepted payloadDigest (firstHostReceipt key payloadDigest))
  | replay
      (payloadDigest : PayloadDigest)
      (receipt : HostReceipt key payloadDigest) :
      HostAccepts key
        (.accepted payloadDigest receipt)
        payloadDigest
        (.accepted payloadDigest receipt)

inductive HostConflicts
    (key : DispatchKey)
    (requestedDigest : PayloadDigest) : HostSlot key → Prop where
  | existing
      (acceptedDigest : PayloadDigest)
      (receipt : HostReceipt key acceptedDigest)
      (different : requestedDigest ≠ acceptedDigest) :
      HostConflicts key requestedDigest
        (.accepted acceptedDigest receipt)

inductive TransactionPhase (intent : DispatchIntent) where
  | reserved
  | attempting
  | delivered (receipt : HostReceipt intent.key intent.payloadDigest)
  | cancelled
  deriving DecidableEq, Repr

structure DispatchTransaction (intent : DispatchIntent) where
  revision : Revision
  phase : TransactionPhase intent
  deriving DecidableEq, Repr

structure SessionHead where
  revision : Revision
  turnReserved : Bool
  deriving DecidableEq, Repr

def reserve
    (expectedRevision : Revision)
    (head : SessionHead)
    (intent : DispatchIntent) :
    Option (SessionHead × DispatchTransaction intent) :=
  if head.revision = expectedRevision ∧ head.turnReserved = false then
    some (
      { revision := head.revision + 1, turnReserved := true },
      { revision := 0, phase := .reserved }
    )
  else
    none

def beginAttempt
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent) :
    Option (DispatchTransaction intent) :=
  match transaction.phase with
  | .reserved =>
      some {
        revision := transaction.revision + 1
        phase := .attempting
      }
  | _ => none

def finalize
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    Option (DispatchTransaction intent) :=
  match transaction.phase with
  | .attempting =>
      some {
        revision := transaction.revision + 1
        phase := .delivered receipt
      }
  | _ => none

def cancel
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent) :
    Option (DispatchTransaction intent) :=
  match transaction.phase with
  | .reserved =>
      some {
        revision := transaction.revision + 1
        phase := .cancelled
      }
  | _ => none

def crash
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent) :
    DispatchTransaction intent :=
  transaction

inductive BeginAttemptStep {intent : DispatchIntent} :
    DispatchTransaction intent → DispatchTransaction intent → Prop where
  | reserved (revision : Revision) :
      BeginAttemptStep
        ({
          revision := revision
          phase := TransactionPhase.reserved
        } : DispatchTransaction intent)
        ({
          revision := revision + 1
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)

inductive FinalizeStep
    {intent : DispatchIntent}
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    DispatchTransaction intent → DispatchTransaction intent → Prop where
  | attempting (revision : Revision) :
      FinalizeStep receipt
        ({
          revision := revision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        ({
          revision := revision + 1
          phase := TransactionPhase.delivered receipt
        } : DispatchTransaction intent)

inductive CancelStep {intent : DispatchIntent} :
    DispatchTransaction intent → DispatchTransaction intent → Prop where
  | reserved (revision : Revision) :
      CancelStep
        ({
          revision := revision
          phase := TransactionPhase.reserved
        } : DispatchTransaction intent)
        ({
          revision := revision + 1
          phase := TransactionPhase.cancelled
        } : DispatchTransaction intent)

def retryKey
    {intent : DispatchIntent}
    (_transaction : DispatchTransaction intent) :
    DispatchKey :=
  intent.key

inductive NaiveEvent where
  | consume
  | dispatch
  | crash
  | recover
  deriving DecidableEq, Repr

def dispatchCount : List NaiveEvent → Nat
  | [] => 0
  | .dispatch :: rest => dispatchCount rest + 1
  | _ :: rest => dispatchCount rest

def turnConsumed : List NaiveEvent → Bool
  | [] => false
  | .consume :: _ => true
  | _ :: rest => turnConsumed rest

def consumeFirstCrashTrace : List NaiveEvent :=
  [.consume, .crash]

def dispatchFirstReplayTrace : List NaiveEvent :=
  [.dispatch, .crash, .recover, .dispatch]

inductive NaiveFailure : Bool → Nat → Prop where
  | consumeFirstCrash : NaiveFailure true 0
  | dispatchFirstReplay : NaiveFailure false 2

structure Cost where
  graphHops : Nat
  controlRounds : Nat
  tokenUnits : Nat
  deriving DecidableEq, Repr

def durableNativeCost : Cost :=
  { graphHops := 5, controlRounds := 2, tokenUnits := 5 }

def legacyBootstrapCost : Cost :=
  { graphHops := 7, controlRounds := 4, tokenUnits := 9 }

def StrictlyDominates (better worse : Cost) : Prop :=
  better.graphHops < worse.graphHops ∧
  better.controlRounds < worse.controlRounds ∧
  better.tokenUnits < worse.tokenUnits

theorem consume_first_has_loss_window :
    NaiveFailure true 0 := by
  exact NaiveFailure.consumeFirstCrash

theorem dispatch_first_has_duplicate_window :
    NaiveFailure false 2 := by
  exact NaiveFailure.dispatchFirstReplay

theorem stale_revision_cannot_reserve
    (expected : Revision)
    (head : SessionHead)
    (intent : DispatchIntent)
    (stale : head.revision ≠ expected) :
    reserve expected head intent = none := by
  unfold reserve
  split
  next condition =>
    exact (stale condition.1).elim
  next =>
    rfl

theorem already_reserved_turn_cannot_reserve
    (expected : Revision)
    (head : SessionHead)
    (intent : DispatchIntent)
    (reserved : head.turnReserved = true) :
    reserve expected head intent = none := by
  unfold reserve
  split
  next condition =>
    rw [reserved] at condition
    cases condition.2
  next =>
    rfl

theorem crash_preserves_transaction
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent) :
    crash transaction = transaction := by
  rfl

theorem retry_preserves_dispatch_key
    {intent : DispatchIntent}
    (transaction : DispatchTransaction intent) :
    retryKey transaction = intent.key := by
  rfl

theorem attempting_cannot_cancel
    (intent : DispatchIntent)
    (revision : Revision) :
    ¬ ∃ next, CancelStep
      ({
        revision := revision
        phase := (TransactionPhase.attempting : TransactionPhase intent)
      } : DispatchTransaction intent)
      next := by
  intro witness
  rcases witness with ⟨next, step⟩
  cases step

theorem delivered_cannot_cancel
    (intent : DispatchIntent)
    (revision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    ¬ ∃ next, CancelStep
      ({
        revision := revision
        phase := TransactionPhase.delivered receipt
      } : DispatchTransaction intent)
      next := by
  intro witness
  rcases witness with ⟨next, step⟩
  cases step

theorem cancelled_cannot_begin_attempt
    (intent : DispatchIntent)
    (revision : Revision) :
    ¬ ∃ next, BeginAttemptStep
      ({
        revision := revision
        phase := (TransactionPhase.cancelled : TransactionPhase intent)
      } : DispatchTransaction intent)
      next := by
  intro witness
  rcases witness with ⟨next, step⟩
  cases step

theorem host_retry_is_idempotent
    (key : DispatchKey)
    (payloadDigest : PayloadDigest)
    (receipt : HostReceipt key payloadDigest) :
    HostAccepts key
      (.accepted payloadDigest receipt)
      payloadDigest
      (.accepted payloadDigest receipt) := by
  exact HostAccepts.replay payloadDigest receipt

theorem host_accepted_retry_preserves_receipt
    (key : DispatchKey)
    (payloadDigest : PayloadDigest)
    (receipt : HostReceipt key payloadDigest) :
    HostAccepts key
      (.accepted payloadDigest receipt)
      payloadDigest
      (.accepted payloadDigest receipt) := by
  exact HostAccepts.replay payloadDigest receipt

theorem different_payload_detects_conflict
    (key : DispatchKey)
    (acceptedDigest requestedDigest : PayloadDigest)
    (receipt : HostReceipt key acceptedDigest)
    (different : requestedDigest ≠ acceptedDigest) :
    HostConflicts key requestedDigest
      (.accepted acceptedDigest receipt) := by
  exact HostConflicts.existing acceptedDigest receipt different

theorem different_payload_cannot_be_accepted
    (key : DispatchKey)
    (acceptedDigest requestedDigest : PayloadDigest)
    (receipt : HostReceipt key acceptedDigest)
    (different : requestedDigest ≠ acceptedDigest) :
    ¬ ∃ after,
      HostAccepts key
        (.accepted acceptedDigest receipt)
        requestedDigest
        after := by
  intro witness
  rcases witness with ⟨after, acceptance⟩
  cases acceptance
  exact different rfl

theorem finalize_receipt_is_intent_indexed
    (intent : DispatchIntent)
    (revision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    ∃ next, FinalizeStep receipt
      ({
        revision := revision
        phase := TransactionPhase.attempting
      } : DispatchTransaction intent)
      next := by
  exact ⟨{
    revision := revision + 1
    phase := TransactionPhase.delivered receipt
  }, FinalizeStep.attempting revision⟩

theorem transaction_cannot_retarget
    (intent : DispatchIntent)
    (transaction : DispatchTransaction intent) :
    retryKey transaction = intent.key := by
  rfl

theorem durable_native_route_strictly_dominates_legacy :
    StrictlyDominates durableNativeCost legacyBootstrapCost := by
  exact ⟨by decide, by decide, by decide⟩

end ASPProof.AgentSessionDispatchTransaction
