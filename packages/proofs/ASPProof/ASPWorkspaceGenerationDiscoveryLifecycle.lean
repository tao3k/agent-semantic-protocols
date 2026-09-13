-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.WorkspaceGenerationDiscovery

inductive State where
  | absent
  | discovering
  | candidateReady
  | building
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

inductive Step : State → State → Prop where
  | beginDiscovery : Step .absent .discovering
  | publishCandidate : Step .discovering .candidateReady
  | publishFailure : Step .discovering .failed
  | publishCancellation : Step .discovering .cancelled
  | beginBuild : Step .candidateReady .building
  | publishReady : Step .building .ready
  | publishBuildFailure : Step .building .failed
  | publishBuildCancellation : Step .building .cancelled

inductive Terminal : State → Prop where
  | failed : Terminal .failed
  | cancelled : Terminal .cancelled

structure TimedObservation where
  state : State
  nowMs : Nat
  deadlineMs : Nat

def deadlineSafe (observation : TimedObservation) : Prop :=
  observation.state ≠ .discovering ∨ observation.nowMs < observation.deadlineMs

def buildDeadlineSafe (observation : TimedObservation) : Prop :=
  observation.state ≠ .building ∨ observation.nowMs < observation.deadlineMs

theorem discovering_cannot_silently_return_to_absent
    (transition : Step .discovering .absent) : False := by
  cases transition

theorem terminal_receipt_is_not_discovering
    {state : State}
    (isTerminal : Terminal state) : state ≠ .discovering := by
  cases isTerminal <;> intro equality <;> cases equality

theorem expired_observation_is_not_discovering
    (observation : TimedObservation)
    (safe : deadlineSafe observation)
    (expired : observation.deadlineMs ≤ observation.nowMs) :
    observation.state ≠ .discovering := by
  rcases safe with stateIsTerminal | beforeDeadline
  · exact stateIsTerminal
  · intro _
    exact (Nat.not_lt_of_ge expired) beforeDeadline

theorem every_discovery_step_is_progressive
    {next : State}
    (transition : Step .discovering next) :
    next = .candidateReady ∨ next = .failed ∨ next = .cancelled := by
  cases transition with
  | publishCandidate => exact Or.inl rfl
  | publishFailure => exact Or.inr (Or.inl rfl)
  | publishCancellation => exact Or.inr (Or.inr rfl)

theorem expired_build_is_not_building
    (observation : TimedObservation)
    (safe : buildDeadlineSafe observation)
    (expired : observation.deadlineMs ≤ observation.nowMs) :
    observation.state ≠ .building := by
  rcases safe with stateIsTerminal | beforeDeadline
  · exact stateIsTerminal
  · intro _
    exact (Nat.not_lt_of_ge expired) beforeDeadline

theorem every_build_step_is_progressive
    {next : State}
    (transition : Step .building next) :
    next = .ready ∨ next = .failed ∨ next = .cancelled := by
  cases transition with
  | publishReady => exact Or.inl rfl
  | publishBuildFailure => exact Or.inr (Or.inl rfl)
  | publishBuildCancellation => exact Or.inr (Or.inr rfl)

end ASPProof.WorkspaceGenerationDiscovery
