import Init

namespace ASPProof.ASPAgentFacingSearchWallBudget

/-- End-to-end wall time is measured in milliseconds at the agent-facing
facade boundary. Admission is strict: one second is already a failure. -/
abbrev wallAdmitted (elapsedMs : Nat) : Prop := elapsedMs < 1000

abbrev wallBudgetExceeded (elapsedMs : Nat) : Prop := 1000 ≤ elapsedMs

theorem subsecond_is_admitted : wallAdmitted 999 := by
  decide

theorem one_second_is_rejected : wallBudgetExceeded 1000 := by
  decide

theorem three_seconds_is_rejected : wallBudgetExceeded 3000 := by
  decide

theorem observed_ten_point_three_seconds_is_rejected :
    wallBudgetExceeded 10300 := by
  decide

theorem evidence_correctness_does_not_override_wall_budget
    (elapsedMs : Nat)
    (evidenceCorrect : Prop) :
    evidenceCorrect ∧ wallAdmitted elapsedMs → wallAdmitted elapsedMs := by
  intro admitted
  exact admitted.2

end ASPProof.ASPAgentFacingSearchWallBudget
