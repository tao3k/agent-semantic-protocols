-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

universe u

namespace ASPProof.SearchRouteCertifiedPotentialCompletion

structure SearchProblem (Candidate : Type u) where
  feasible : Candidate → Prop
  cost : Candidate → Nat

def GlobalOptimal
    {Candidate : Type u}
    (problem : SearchProblem Candidate)
    (chosen : Candidate) : Prop :=
  problem.feasible chosen
    ∧ ∀ candidate,
        problem.feasible candidate →
        problem.cost chosen ≤ problem.cost candidate

def GapOptimal
    {Candidate : Type u}
    (problem : SearchProblem Candidate)
    (chosen : Candidate)
    (gap : Nat) : Prop :=
  problem.feasible chosen
    ∧ ∀ candidate,
        problem.feasible candidate →
        problem.cost chosen ≤ problem.cost candidate + gap

structure OptimalityCertificate
    {Candidate : Type u}
    (problem : SearchProblem Candidate)
    (chosen : Candidate)
    (gap : Nat) where
  generation : Nat
  covered : Candidate → Prop
  coversEveryFeasible :
    ∀ candidate, problem.feasible candidate → covered candidate
  chosenFeasible : problem.feasible chosen
  chosenBound :
    ∀ candidate, covered candidate →
      problem.cost chosen ≤ problem.cost candidate + gap

structure LoopState where
  generation : Nat
  potential : Nat
  deriving DecidableEq, Repr

def ExactAdmissible
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen 0) : Prop :=
  state.potential = 0
    ∧ certificate.generation = state.generation

def BoundedAdmissible
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    {gap : Nat}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen gap) : Prop :=
  0 < gap
    ∧ certificate.generation = state.generation

structure IncompleteReceipt where
  generation : Nat
  coverageCommitted : Bool
  reasonDigest : String
  deriving DecidableEq, Repr

def IncompleteAdmissible
    (state : LoopState)
    (receipt : IncompleteReceipt) : Prop :=
  receipt.generation = state.generation
    ∧ receipt.coverageCommitted = false

theorem bounded_certificate_is_sound
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    {gap : Nat}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen gap)
    (_admissible : BoundedAdmissible state certificate) :
    GapOptimal problem chosen gap := by
  constructor
  · exact certificate.chosenFeasible
  · intro candidate feasible
    exact certificate.chosenBound
      candidate
      (certificate.coversEveryFeasible candidate feasible)

theorem exact_certificate_is_sound
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen 0)
    (_admissible : ExactAdmissible state certificate) :
    GlobalOptimal problem chosen := by
  constructor
  · exact certificate.chosenFeasible
  · intro candidate feasible
    have bounded :=
      certificate.chosenBound
        candidate
        (certificate.coversEveryFeasible candidate feasible)
    simpa using bounded

theorem exact_admission_requires_zero_potential
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen 0)
    (admissible : ExactAdmissible state certificate) :
    state.potential = 0 :=
  admissible.1

theorem exact_admission_requires_current_generation
    {Candidate : Type u}
    {problem : SearchProblem Candidate}
    {chosen : Candidate}
    (state : LoopState)
    (certificate : OptimalityCertificate problem chosen 0)
    (admissible : ExactAdmissible state certificate) :
    certificate.generation = state.generation :=
  admissible.2

inductive ExampleCandidate
  | cheap
  | expensive
  deriving DecidableEq, Repr

def exampleProblem : SearchProblem ExampleCandidate where
  feasible := fun _ => True
  cost
    | .cheap => 1
    | .expensive => 4

def currentState : LoopState :=
  ⟨8, 0⟩

def currentExactCertificate :
    OptimalityCertificate exampleProblem .cheap 0 where
  generation := 8
  covered := fun _ => True
  coversEveryFeasible := by
    intro _ _
    trivial
  chosenFeasible := trivial
  chosenBound := by
    intro candidate _
    cases candidate <;> decide

def staleExactCertificate :
    OptimalityCertificate exampleProblem .cheap 0 :=
  { currentExactCertificate with generation := 7 }

def localZeroIncomplete : IncompleteReceipt where
  generation := 8
  coverageCommitted := false
  reasonDigest := "uncommitted-universe"

theorem stale_exact_certificate_is_rejected :
    ¬ ExactAdmissible currentState staleExactCertificate := by
  simp
    [ ExactAdmissible
    , currentState
    , staleExactCertificate
    , currentExactCertificate
    ]

theorem expensive_candidate_is_not_global_optimum :
    ¬ GlobalOptimal exampleProblem .expensive := by
  intro optimal
  have contradiction :=
    optimal.2 .cheap trivial
  simp [exampleProblem] at contradiction

theorem zero_potential_with_incomplete_coverage_is_not_exact :
    currentState.potential = 0
      ∧ IncompleteAdmissible currentState localZeroIncomplete
      ∧ ¬ GlobalOptimal exampleProblem .expensive := by
  constructor
  · rfl
  · constructor
    · constructor <;> rfl
    · exact expensive_candidate_is_not_global_optimum

theorem current_exact_certificate_is_admitted :
    ExactAdmissible currentState currentExactCertificate := by
  constructor <;> rfl

theorem current_exact_completion_is_optimal :
    GlobalOptimal exampleProblem .cheap := by
  exact exact_certificate_is_sound
    currentState
    currentExactCertificate
    current_exact_certificate_is_admitted

end ASPProof.SearchRouteCertifiedPotentialCompletion
