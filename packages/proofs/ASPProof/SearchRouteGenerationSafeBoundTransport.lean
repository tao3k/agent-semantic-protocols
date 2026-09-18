-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteGenerationSafeBoundTransport

structure ProblemIdentity where
  evidenceRootDigest : Nat
  sourceIndexGeneration : Nat
  providerSetDigest : Nat
  feasibilityContextDigest : Nat
  objectiveDigest : Nat
  objectiveVersion : Nat
  semanticCacheContextDigest : Nat
  modelPrefixCostContextDigest : Nat
  candidateSchemaVersion : Nat
  deriving DecidableEq, Repr

structure SearchProblem (Candidate : Type) where
  identity : ProblemIdentity
  feasible : Candidate → Prop
  cost : Candidate → Nat

def GlobalLowerBound
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (lowerBound : Nat) : Prop :=
  ∀ candidate, problem.feasible candidate →
    lowerBound ≤ problem.cost candidate

structure BoundTransport
    {OldCandidate NewCandidate : Type}
    (oldProblem : SearchProblem OldCandidate)
    (newProblem : SearchProblem NewCandidate) where
  map : NewCandidate → OldCandidate
  mapsFeasible :
    ∀ candidate, newProblem.feasible candidate →
      oldProblem.feasible (map candidate)
  costMonotone :
    ∀ candidate, newProblem.feasible candidate →
      oldProblem.cost (map candidate) ≤ newProblem.cost candidate

theorem bound_transport_is_sound
    {OldCandidate NewCandidate : Type}
    (oldProblem : SearchProblem OldCandidate)
    (newProblem : SearchProblem NewCandidate)
    (lowerBound : Nat)
    (oldBound : GlobalLowerBound oldProblem lowerBound)
    (transport : BoundTransport oldProblem newProblem) :
    GlobalLowerBound newProblem lowerBound := by
  intro candidate feasible
  exact Nat.le_trans
    (oldBound
      (transport.map candidate)
      (transport.mapsFeasible candidate feasible))
    (transport.costMonotone candidate feasible)

inductive OldCandidate where
  | existing
  deriving DecidableEq, Repr

inductive DriftedCandidate where
  | existing
  | newCheap
  deriving DecidableEq, Repr

def oldIdentity : ProblemIdentity :=
  {
    evidenceRootDigest := 61
    sourceIndexGeneration := 7
    providerSetDigest := 62
    feasibilityContextDigest := 63
    objectiveDigest := 64
    objectiveVersion := 1
    semanticCacheContextDigest := 66
    modelPrefixCostContextDigest := 67
    candidateSchemaVersion := 1
  }

def newIdentity : ProblemIdentity :=
  {
    oldIdentity with
    evidenceRootDigest := 65
    sourceIndexGeneration := 8
  }

def oldProblem : SearchProblem OldCandidate :=
  {
    identity := oldIdentity
    feasible := fun _ => True
    cost := fun _ => 10
  }

def driftedProblem : SearchProblem DriftedCandidate :=
  {
    identity := newIdentity
    feasible := fun _ => True
    cost := fun
      | .existing => 10
      | .newCheap => 4
  }

theorem old_problem_has_lower_bound_ten :
    GlobalLowerBound oldProblem 10 := by
  intro candidate feasible
  cases candidate
  decide

theorem cheaper_alternative_invalidates_old_bound :
    ¬ GlobalLowerBound driftedProblem 10 := by
  intro alleged
  have invalid := alleged .newCheap True.intro
  change 10 ≤ 4 at invalid
  exact (by decide : ¬ 10 ≤ 4) invalid

inductive MonotoneCandidate where
  | existing
  deriving DecidableEq, Repr

def monotoneProblem : SearchProblem MonotoneCandidate :=
  {
    identity := newIdentity
    feasible := fun _ => True
    cost := fun _ => 12
  }

def monotoneTransport :
    BoundTransport oldProblem monotoneProblem :=
  {
    map := fun _ => .existing
    mapsFeasible := by
      intro candidate feasible
      exact True.intro
    costMonotone := by
      intro candidate feasible
      cases candidate
      decide
  }

theorem monotone_cost_increase_preserves_lower_bound :
    GlobalLowerBound monotoneProblem 10 :=
  bound_transport_is_sound
    oldProblem
    monotoneProblem
    10
    old_problem_has_lower_bound_ten
    monotoneTransport

theorem old_bound_does_not_imply_new_bound :
    GlobalLowerBound oldProblem 10 ∧
      ¬ GlobalLowerBound driftedProblem 10 :=
  ⟨
    old_problem_has_lower_bound_ten,
    cheaper_alternative_invalidates_old_bound
  ⟩

end ASPProof.SearchRouteGenerationSafeBoundTransport
