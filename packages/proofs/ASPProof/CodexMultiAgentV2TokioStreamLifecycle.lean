-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.CodexMultiAgentV2DurableDelegation

namespace ASPProof

structure TokioStreamLifecycle (Event : Type) where
  capacity : Nat
  admissionOpen : Bool
  actorRunning : Bool
  queue : List Event
  completed : List Event
  cancelled : List Event

def TokioStreamLifecycle.bounded (state : TokioStreamLifecycle Event) : Prop :=
  state.queue.length ≤ state.capacity

def TokioStreamLifecycle.canAdmit (state : TokioStreamLifecycle Event) : Prop :=
  state.admissionOpen = true ∧ state.queue.length < state.capacity

def TokioStreamLifecycle.stopAdmission
    (state : TokioStreamLifecycle Event) : TokioStreamLifecycle Event :=
  { state with admissionOpen := false }

def TokioStreamLifecycle.drainAndJoin
    (state : TokioStreamLifecycle Event) : TokioStreamLifecycle Event :=
  { state with
      actorRunning := false
      queue := []
      completed := state.completed ++ state.queue }

def TokioStreamLifecycle.stopDrainJoin
    (state : TokioStreamLifecycle Event) : TokioStreamLifecycle Event :=
  state.stopAdmission.drainAndJoin

theorem stopped_stream_rejects_new_admission
    (state : TokioStreamLifecycle Event) :
    ¬ state.stopAdmission.canAdmit := by
  simp [TokioStreamLifecycle.canAdmit, TokioStreamLifecycle.stopAdmission]

theorem joined_stream_has_no_queued_event
    (state : TokioStreamLifecycle Event) :
    state.stopDrainJoin.queue = [] := by
  rfl

theorem joined_stream_actor_is_stopped
    (state : TokioStreamLifecycle Event) :
    state.stopDrainJoin.actorRunning = false := by
  rfl

theorem drain_preserves_every_admitted_event
    (state : TokioStreamLifecycle Event) :
    state.stopDrainJoin.completed = state.completed ++ state.queue := by
  rfl

theorem drain_does_not_invent_cancellation
    (state : TokioStreamLifecycle Event) :
    state.stopDrainJoin.cancelled = state.cancelled := by
  rfl

theorem joined_stream_remains_bounded
    (state : TokioStreamLifecycle Event) :
    state.stopDrainJoin.bounded := by
  simp [TokioStreamLifecycle.bounded, TokioStreamLifecycle.stopDrainJoin,
    TokioStreamLifecycle.drainAndJoin, TokioStreamLifecycle.stopAdmission]

end ASPProof
