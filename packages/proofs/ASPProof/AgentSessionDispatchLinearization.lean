-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionDispatchTransaction

namespace ASPProof.AgentSessionDispatchLinearization

open ASPProof.AgentSessionDispatchTransaction

structure ReservationRecord (intent : DispatchIntent) where
  previousRevision : Revision
  nextRevision : Revision
  sessionReserved : Bool
  outboxDurable : Bool
  commitVisible : Bool
  deriving DecidableEq, Repr

inductive PublishedReservation {intent : DispatchIntent} :
    ReservationRecord intent → Prop where
  | atomic (previousRevision : Revision) :
      PublishedReservation {
        previousRevision := previousRevision
        nextRevision := previousRevision + 1
        sessionReserved := true
        outboxDurable := true
        commitVisible := true
      }

inductive DeliveredCasResult where
  | committed
  | alreadyDelivered
  deriving DecidableEq, Repr

inductive DeliveredCasStep (intent : DispatchIntent) :
    Revision →
    DispatchTransaction intent →
    HostReceipt intent.key intent.payloadDigest →
    DispatchTransaction intent →
    DeliveredCasResult → Prop where
  | commit
      (revision : Revision)
      (receipt : HostReceipt intent.key intent.payloadDigest) :
      DeliveredCasStep intent
        revision
        ({
          revision := revision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        receipt
        ({
          revision := revision + 1
          phase := TransactionPhase.delivered receipt
        } : DispatchTransaction intent)
        .committed
  | replay
      (expectedRevision currentRevision : Revision)
      (receipt : HostReceipt intent.key intent.payloadDigest) :
      DeliveredCasStep intent
        expectedRevision
        ({
          revision := currentRevision
          phase := TransactionPhase.delivered receipt
        } : DispatchTransaction intent)
        receipt
        ({
          revision := currentRevision
          phase := TransactionPhase.delivered receipt
        } : DispatchTransaction intent)
        .alreadyDelivered

inductive LinearizationStage where
  | absent
  | reservationPublished
  | hostAccepted
  | delivered
  deriving DecidableEq, Repr

inductive Advances : LinearizationStage → LinearizationStage → Prop where
  | publish : Advances .absent .reservationPublished
  | accept : Advances .reservationPublished .hostAccepted
  | finalize : Advances .hostAccepted .delivered

def stageRank : LinearizationStage → Nat
  | .absent => 0
  | .reservationPublished => 1
  | .hostAccepted => 2
  | .delivered => 3

theorem atomic_reservation_constructible
    (intent : DispatchIntent)
    (previousRevision : Revision) :
    PublishedReservation (intent := intent) {
      previousRevision := previousRevision
      nextRevision := previousRevision + 1
      sessionReserved := true
      outboxDurable := true
      commitVisible := true
    } := by
  exact PublishedReservation.atomic previousRevision

theorem session_only_write_not_published
    (intent : DispatchIntent)
    (previousRevision nextRevision : Revision) :
    ¬ PublishedReservation (intent := intent) {
      previousRevision := previousRevision
      nextRevision := nextRevision
      sessionReserved := true
      outboxDurable := false
      commitVisible := true
    } := by
  intro published
  cases published

theorem outbox_only_write_not_published
    (intent : DispatchIntent)
    (previousRevision nextRevision : Revision) :
    ¬ PublishedReservation (intent := intent) {
      previousRevision := previousRevision
      nextRevision := nextRevision
      sessionReserved := false
      outboxDurable := true
      commitVisible := true
    } := by
  intro published
  cases published

theorem uncommitted_reservation_not_published
    (intent : DispatchIntent)
    (previousRevision nextRevision : Revision) :
    ¬ PublishedReservation (intent := intent) {
      previousRevision := previousRevision
      nextRevision := nextRevision
      sessionReserved := true
      outboxDurable := true
      commitVisible := false
    } := by
  intro published
  cases published

theorem published_has_session_reservation
    {intent : DispatchIntent}
    {record : ReservationRecord intent}
    (published : PublishedReservation record) :
    record.sessionReserved = true := by
  cases published
  rfl

theorem published_has_durable_outbox
    {intent : DispatchIntent}
    {record : ReservationRecord intent}
    (published : PublishedReservation record) :
    record.outboxDurable = true := by
  cases published
  rfl

theorem published_revision_advances_once
    {intent : DispatchIntent}
    {record : ReservationRecord intent}
    (published : PublishedReservation record) :
    record.nextRevision = record.previousRevision + 1 := by
  cases published
  rfl

theorem delivered_cas_constructible
    (intent : DispatchIntent)
    (revision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    DeliveredCasStep intent
      revision
      ({
        revision := revision
        phase := TransactionPhase.attempting
      } : DispatchTransaction intent)
      receipt
      ({
        revision := revision + 1
        phase := TransactionPhase.delivered receipt
      } : DispatchTransaction intent)
      .committed := by
  exact DeliveredCasStep.commit revision receipt

theorem stale_attempting_revision_rejected
    (intent : DispatchIntent)
    (expectedRevision currentRevision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest)
    (stale : expectedRevision ≠ currentRevision) :
    ¬ ∃ after result,
      DeliveredCasStep intent
        expectedRevision
        ({
          revision := currentRevision
          phase := TransactionPhase.attempting
        } : DispatchTransaction intent)
        receipt
        after
        result := by
  intro witness
  rcases witness with ⟨after, result, step⟩
  cases step
  exact stale rfl

theorem delivered_replay_is_idempotent
    (intent : DispatchIntent)
    (expectedRevision currentRevision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest) :
    DeliveredCasStep intent
      expectedRevision
      ({
        revision := currentRevision
        phase := TransactionPhase.delivered receipt
      } : DispatchTransaction intent)
      receipt
      ({
        revision := currentRevision
        phase := TransactionPhase.delivered receipt
      } : DispatchTransaction intent)
      .alreadyDelivered := by
  exact DeliveredCasStep.replay expectedRevision currentRevision receipt

theorem conflicting_receipt_replay_rejected
    (intent : DispatchIntent)
    (expectedRevision currentRevision : Revision)
    (acceptedReceipt requestedReceipt :
      HostReceipt intent.key intent.payloadDigest)
    (different : requestedReceipt ≠ acceptedReceipt) :
    ¬ ∃ after,
      DeliveredCasStep intent
        expectedRevision
        ({
          revision := currentRevision
          phase := TransactionPhase.delivered acceptedReceipt
        } : DispatchTransaction intent)
        requestedReceipt
        after
        .alreadyDelivered := by
  intro witness
  rcases witness with ⟨after, step⟩
  cases step
  exact different rfl

theorem delivered_commit_advances_revision
    (intent : DispatchIntent)
    (revision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest)
    (after : DispatchTransaction intent)
    (step : DeliveredCasStep intent
      revision
      ({
        revision := revision
        phase := TransactionPhase.attempting
      } : DispatchTransaction intent)
      receipt
      after
      .committed) :
    after.revision = revision + 1 := by
  cases step
  rfl

theorem delivered_replay_preserves_revision
    (intent : DispatchIntent)
    (expectedRevision : Revision)
    (currentRevision : Revision)
    (receipt : HostReceipt intent.key intent.payloadDigest)
    (after : DispatchTransaction intent)
    (step : DeliveredCasStep intent
      expectedRevision
      ({
        revision := currentRevision
        phase := TransactionPhase.delivered receipt
      } : DispatchTransaction intent)
      receipt
      after
      .alreadyDelivered) :
    after.revision = currentRevision := by
  cases step
  rfl

theorem linearization_advances_rank
    {before after : LinearizationStage}
    (advance : Advances before after) :
    stageRank before < stageRank after := by
  cases advance <;> decide

theorem linearization_is_acyclic
    (stage : LinearizationStage) :
    ¬ Advances stage stage := by
  intro advance
  have ranked := linearization_advances_rank advance
  exact Nat.lt_irrefl _ ranked

end ASPProof.AgentSessionDispatchLinearization
