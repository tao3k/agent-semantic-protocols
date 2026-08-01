namespace ASPProof.SearchRouteCertifiedCompletion

structure ObjectiveIdentity where
  objectiveDigest : Nat
  objectiveVersion : Nat
  evidenceRootDigest : Nat
  costContextDigest : Nat
  deriving DecidableEq, Repr

structure SearchProblem (Candidate : Type) where
  objectiveIdentity : ObjectiveIdentity
  feasible : Candidate → Prop
  objective : Candidate → Nat

def GlobalLowerBound
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (lowerBound : Nat) : Prop :=
  ∀ candidate, problem.feasible candidate →
    lowerBound ≤ problem.objective candidate

def ExactOptimal
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (chosen : Candidate) : Prop :=
  problem.feasible chosen ∧
  ∀ candidate, problem.feasible candidate →
    problem.objective chosen ≤ problem.objective candidate

def FactorOptimal
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (factor : Nat)
    (chosen : Candidate) : Prop :=
  problem.feasible chosen ∧
  ∀ candidate, problem.feasible candidate →
    problem.objective chosen ≤ factor * problem.objective candidate

structure BoundCertificate
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (factor : Nat)
    (chosen : Candidate) where
  chosenFeasible : problem.feasible chosen
  lowerBound : Nat
  globallyLowerBounded : GlobalLowerBound problem lowerBound
  incumbentWithinBound :
    problem.objective chosen ≤ factor * lowerBound

theorem bound_certificate_is_sound
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (factor : Nat)
    (chosen : Candidate)
    (certificate : BoundCertificate problem factor chosen) :
    FactorOptimal problem factor chosen := by
  constructor
  · exact certificate.chosenFeasible
  · intro candidate feasible
    exact Nat.le_trans
      certificate.incumbentWithinBound
      (Nat.mul_le_mul_left
        factor
        (certificate.globallyLowerBounded candidate feasible))

theorem factor_one_certificate_is_exact
    {Candidate : Type}
    (problem : SearchProblem Candidate)
    (chosen : Candidate)
    (certificate : BoundCertificate problem 1 chosen) :
    ExactOptimal problem chosen := by
  constructor
  · exact certificate.chosenFeasible
  · intro candidate feasible
    have factorOptimal :=
      bound_certificate_is_sound problem 1 chosen certificate
    have bounded := factorOptimal.2 candidate feasible
    simpa using bounded

inductive ExampleCandidate where
  | best
  | heuristic
  deriving DecidableEq, Repr

def exampleObjective : ExampleCandidate → Nat
  | .best => 10
  | .heuristic => 12

def exampleProblem : SearchProblem ExampleCandidate :=
  {
    objectiveIdentity := {
      objectiveDigest := 41
      objectiveVersion := 1
      evidenceRootDigest := 42
      costContextDigest := 43
    }
    feasible := fun _ => True
    objective := exampleObjective
  }

theorem example_six_is_global_lower_bound :
    GlobalLowerBound exampleProblem 6 := by
  intro candidate feasible
  cases candidate <;> decide

def exampleFactorTwoCertificate :
    BoundCertificate exampleProblem 2 .heuristic :=
  {
    chosenFeasible := True.intro
    lowerBound := 6
    globallyLowerBounded := example_six_is_global_lower_bound
    incumbentWithinBound := by decide
  }

theorem heuristic_is_factor_two_optimal :
    FactorOptimal exampleProblem 2 .heuristic :=
  bound_certificate_is_sound
    exampleProblem
    2
    .heuristic
    exampleFactorTwoCertificate

theorem heuristic_is_not_exact :
    ¬ ExactOptimal exampleProblem .heuristic := by
  intro exact
  have noWorseThanBest := exact.2 .best True.intro
  change 12 ≤ 10 at noWorseThanBest
  exact (by decide : ¬ 12 ≤ 10) noWorseThanBest

theorem bounded_does_not_imply_exact :
    FactorOptimal exampleProblem 2 .heuristic ∧
      ¬ ExactOptimal exampleProblem .heuristic :=
  ⟨heuristic_is_factor_two_optimal, heuristic_is_not_exact⟩

end ASPProof.SearchRouteCertifiedCompletion
