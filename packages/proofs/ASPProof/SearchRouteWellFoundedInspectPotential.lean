-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean.Elab.Tactic.Omega

namespace ASPProof.SearchRouteWellFoundedInspectPotential

structure InspectPotential where
  ambiguity : Nat
  missingEvidence : Nat
  staleCertificates : Nat
  deriving DecidableEq, Repr

def TotalPotential (state : InspectPotential) : Nat :=
  state.ambiguity + state.missingEvidence + state.staleCertificates

def ComponentwiseProgress
    (current next : InspectPotential) : Prop :=
  next.ambiguity ≤ current.ambiguity
    ∧ next.missingEvidence ≤ current.missingEvidence
    ∧ next.staleCertificates ≤ current.staleCertificates
    ∧ (next.ambiguity < current.ambiguity
      ∨ next.missingEvidence < current.missingEvidence
      ∨ next.staleCertificates < current.staleCertificates)

def Stalled
    (current next : InspectPotential) : Prop :=
  TotalPotential next = TotalPotential current

def Resolved (state : InspectPotential) : Prop :=
  TotalPotential state = 0

theorem componentwise_progress_strictly_decreases
    (current next : InspectPotential)
    (progress : ComponentwiseProgress current next) :
    TotalPotential next < TotalPotential current := by
  rcases progress with ⟨ambiguity, evidence, certificates, strict⟩
  rcases strict with ambiguityStrict | evidenceStrict | certificateStrict
  · simp [TotalPotential]
    omega
  · simp [TotalPotential]
    omega
  · simp [TotalPotential]
    omega

theorem no_productive_self_transition
    (state : InspectPotential) :
    ¬ ComponentwiseProgress state state := by
  intro progress
  have strict :=
    componentwise_progress_strictly_decreases state state progress
  exact (Nat.lt_irrefl (TotalPotential state)) strict

theorem productive_transition_is_not_stalled
    (current next : InspectPotential)
    (progress : ComponentwiseProgress current next) :
    ¬ Stalled current next := by
  intro stalled
  have strict :=
    componentwise_progress_strictly_decreases current next progress
  exact (Nat.ne_of_lt strict) stalled

theorem resolved_state_has_no_productive_successor
    (current next : InspectPotential)
    (resolved : Resolved current) :
    ¬ ComponentwiseProgress current next := by
  intro progress
  have strict :=
    componentwise_progress_strictly_decreases current next progress
  simp [Resolved] at resolved
  omega

theorem productive_trace_budget
    (trace : Nat → InspectPotential)
    (steps : Nat)
    (productive :
      ∀ index,
        index < steps →
        ComponentwiseProgress (trace index) (trace (index + 1))) :
    TotalPotential (trace steps) + steps ≤ TotalPotential (trace 0) := by
  induction steps with
  | zero =>
      simp
  | succ previous inductionHypothesis =>
      have prefixProductive :
          ∀ index,
            index < previous →
            ComponentwiseProgress (trace index) (trace (index + 1)) := by
        intro index beforePrevious
        exact productive index (Nat.lt_trans beforePrevious (Nat.lt_succ_self previous))
      have prefixBound := inductionHypothesis prefixProductive
      have finalProgress :=
        productive previous (Nat.lt_succ_self previous)
      have finalStrict :=
        componentwise_progress_strictly_decreases
          (trace previous) (trace (previous + 1)) finalProgress
      omega

theorem productive_round_count_is_bounded
    (trace : Nat → InspectPotential)
    (steps : Nat)
    (productive :
      ∀ index,
        index < steps →
        ComponentwiseProgress (trace index) (trace (index + 1))) :
    steps ≤ TotalPotential (trace 0) := by
  have budget := productive_trace_budget trace steps productive
  omega

theorem positive_productive_trace_is_not_a_cycle
    (trace : Nat → InspectPotential)
    (steps : Nat)
    (positive : 0 < steps)
    (productive :
      ∀ index,
        index < steps →
        ComponentwiseProgress (trace index) (trace (index + 1))) :
    trace steps ≠ trace 0 := by
  intro cycle
  have budget := productive_trace_budget trace steps productive
  have potentialEquality :
      TotalPotential (trace steps) = TotalPotential (trace 0) :=
    congrArg TotalPotential cycle
  omega

def ambiguityStableCurrent : InspectPotential :=
  ⟨3, 2, 0⟩

def evidenceProgressNext : InspectPotential :=
  ⟨3, 1, 0⟩

theorem ambiguity_need_not_strictly_decrease :
    ComponentwiseProgress ambiguityStableCurrent evidenceProgressNext
      ∧ ¬ evidenceProgressNext.ambiguity
          < ambiguityStableCurrent.ambiguity := by
  simp
    [ ComponentwiseProgress
    , ambiguityStableCurrent
    , evidenceProgressNext
    ]

end ASPProof.SearchRouteWellFoundedInspectPotential
