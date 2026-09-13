-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ASPWorkspaceReconcileClosure

namespace ASPProof.ASPWorkspacePublicationRecovery

open ASPProof.ASPWorkspaceReconcileClosure

inductive PublicationPhase where
  | requirement
  | intentOutboxDurable
  | dispatched
  | hostAccepted
  | receiptIndexed
  | delivered
  deriving DecidableEq, Repr

inductive AdvanceOutcome where
  | succeeded
  | timeout
  deriving DecidableEq, Repr

def phaseRank : PublicationPhase -> Nat
  | .requirement => 0
  | .intentOutboxDurable => 1
  | .dispatched => 2
  | .hostAccepted => 3
  | .receiptIndexed => 4
  | .delivered => 5

structure PublicationAggregate where
  dispatchKey : PublicationKey
  phase : PublicationPhase
  revision : Nat
  deriving DecidableEq, Repr

structure DeliveryReceipt where
  dispatchKey : PublicationKey
  generationDigest : Nat
  deriving DecidableEq, Repr

def nextPhase : PublicationPhase -> PublicationPhase
  | .requirement => .intentOutboxDurable
  | .intentOutboxDurable => .dispatched
  | .dispatched => .hostAccepted
  | .hostAccepted => .receiptIndexed
  | .receiptIndexed => .delivered
  | .delivered => .delivered

def advance
    (aggregate : PublicationAggregate) (outcome : AdvanceOutcome) :
    PublicationAggregate :=
  match outcome with
  | .succeeded => { aggregate with phase := nextPhase aggregate.phase }
  | .timeout => aggregate

def advanceFive (aggregate : PublicationAggregate) : PublicationAggregate :=
  advance
    (advance
      (advance
        (advance
          (advance aggregate .succeeded)
          .succeeded)
        .succeeded)
      .succeeded)
    .succeeded

def receiptAdmitted
    (aggregate : PublicationAggregate) (receipt : DeliveryReceipt) : Prop :=
  receipt.dispatchKey = aggregate.dispatchKey

def deliver
    (aggregate : PublicationAggregate) (receipt : DeliveryReceipt)
    (_hAdmitted : receiptAdmitted aggregate receipt) : PublicationAggregate :=
  { aggregate with phase := .delivered }

def indexReceipt
    (current : Option DeliveryReceipt) (incoming : DeliveryReceipt) :
    Option DeliveryReceipt :=
  match current with
  | none => some incoming
  | some existing => some existing

def retarget
    (aggregate : PublicationAggregate) (requirement : WorkspaceRequirement) :
    PublicationAggregate :=
  { dispatchKey := stableKey requirement
    phase := .requirement
    revision := aggregate.revision + 1 }

theorem advanceDoesNotRegress
    (aggregate : PublicationAggregate) (outcome : AdvanceOutcome) :
    phaseRank aggregate.phase ≤ phaseRank (advance aggregate outcome).phase := by
  have nextPhaseDoesNotRegress (phase : PublicationPhase) :
      phaseRank phase ≤ phaseRank (nextPhase phase) := by
    cases phase <;> decide
  cases outcome with
  | succeeded => exact nextPhaseDoesNotRegress aggregate.phase
  | timeout => exact Nat.le_refl _

theorem advancePreservesStableKey
    (aggregate : PublicationAggregate) (outcome : AdvanceOutcome) :
    (advance aggregate outcome).dispatchKey = aggregate.dispatchKey := by
  cases outcome <;> rfl

theorem timeoutPreservesPublicationPhase (aggregate : PublicationAggregate) :
    (advance aggregate .timeout).phase = aggregate.phase := by
  rfl

theorem advanceFivePreservesStableKey (aggregate : PublicationAggregate) :
    (advanceFive aggregate).dispatchKey = aggregate.dispatchKey := by
  rfl

theorem fiveSuccessfulAdvancesDeliver (aggregate : PublicationAggregate) :
    (advanceFive aggregate).phase = .delivered := by
  obtain ⟨dispatchKey, phase, revision⟩ := aggregate
  cases phase <;> rfl

theorem duplicateReceiptIndexKeepsFirst (receipt : DeliveryReceipt) :
    indexReceipt (indexReceipt none receipt) receipt = some receipt := by
  rfl

theorem deliverPreservesStableKey
    (aggregate : PublicationAggregate) (receipt : DeliveryReceipt)
    (hAdmitted : receiptAdmitted aggregate receipt) :
    (deliver aggregate receipt hAdmitted).dispatchKey = aggregate.dispatchKey := by
  rfl

theorem oldDigestReceiptRejectedAfterRetarget
    (aggregate : PublicationAggregate)
    (requirement : WorkspaceRequirement)
    (nextRootDigest : Nat)
    (receipt : DeliveryReceipt)
    (hChanged : requirement.rootDigest ≠ nextRootDigest)
    (hOld : receipt.dispatchKey = stableKey requirement) :
    ¬receiptAdmitted
      (retarget aggregate { requirement with rootDigest := nextRootDigest }) receipt := by
  intro hAdmitted
  have hKeys :
      stableKey requirement = stableKey { requirement with rootDigest := nextRootDigest } :=
    hOld.symm.trans hAdmitted
  have hDigests := congrArg PublicationKey.rootDigest hKeys
  exact hChanged hDigests

end ASPProof.ASPWorkspacePublicationRecovery
