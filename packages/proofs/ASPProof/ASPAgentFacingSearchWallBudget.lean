import Init

namespace ASPProof.ASPAgentFacingSearchWallBudget

/-- The strict end-to-end wall budget exposed at the agent-facing facade. -/
abbrev wallBudgetMs : Nat := 1000

/-- The process supervisor must observe runtime completion and a typed reply
before this boundary, leaving the final slice for cleanup and caller I/O. -/
abbrev supervisorBoundaryMs : Nat := 900

/-- End-to-end wall time is measured in milliseconds at the agent-facing
facade boundary. Admission is strict: one second is already a failure. -/
abbrev wallAdmitted (elapsedMs : Nat) : Prop := elapsedMs < wallBudgetMs

abbrev wallBudgetExceeded (elapsedMs : Nat) : Prop := wallBudgetMs ≤ elapsedMs

/-- Runtime awaits use only the execution slice of the wall budget. The next
100 ms is reserved for a typed reply and the final 100 ms for supervisor
cleanup and caller-visible I/O. -/
abbrev runtimeWaitAdmitted (elapsedMs : Nat) : Prop := elapsedMs < 800

abbrev semanticEvidenceAdmitted
    (currentGeneration requestedProjection : Bool) : Prop :=
  currentGeneration = true ∧ requestedProjection = true

abbrev overallEvidenceAdmitted
    (elapsedMs : Nat)
    (currentGeneration requestedProjection : Bool) : Prop :=
  wallAdmitted elapsedMs ∧
    semanticEvidenceAdmitted currentGeneration requestedProjection

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

theorem runtime_wait_keeps_reply_reserve : runtimeWaitAdmitted 799 := by
  decide

theorem runtime_wait_boundary_is_not_admitted : ¬ runtimeWaitAdmitted 800 := by
  decide

abbrev typedReplyAdmitted (elapsedMs : Nat) : Prop := elapsedMs < 100

abbrev supervisorCleanupAdmitted (elapsedMs : Nat) : Prop := elapsedMs < 100

abbrev supervisorCompletionAdmitted (elapsedMs : Nat) : Prop :=
  elapsedMs < supervisorBoundaryMs

abbrev stagedWallAdmitted
    (runtimeMs replyMs cleanupMs : Nat) : Prop :=
  runtimeWaitAdmitted runtimeMs ∧
    typedReplyAdmitted replyMs ∧
    supervisorCompletionAdmitted (runtimeMs + replyMs) ∧
    supervisorCleanupAdmitted cleanupMs ∧
    runtimeMs + replyMs + cleanupMs < wallBudgetMs

theorem supervisor_last_millisecond_is_admitted :
    supervisorCompletionAdmitted 899 := by
  decide

theorem supervisor_boundary_is_not_admitted :
    ¬ supervisorCompletionAdmitted 900 := by
  decide

theorem strict_stage_maxima_leave_wall_slack :
    stagedWallAdmitted 799 99 99 := by
  decide

theorem shared_nine_hundred_millisecond_boundary_is_rejected :
    ¬ stagedWallAdmitted 900 0 0 := by
  decide

theorem fast_stale_evidence_is_rejected :
    ¬ overallEvidenceAdmitted 72 false true := by
  decide

theorem late_current_evidence_is_rejected :
    ¬ overallEvidenceAdmitted 1000 true true := by
  decide

theorem overall_evidence_requires_wall_and_semantic_admission
    (elapsedMs : Nat)
    (currentGeneration requestedProjection : Bool) :
    overallEvidenceAdmitted elapsedMs currentGeneration requestedProjection →
      wallAdmitted elapsedMs ∧
        semanticEvidenceAdmitted currentGeneration requestedProjection := by
  intro admitted
  exact admitted

end ASPProof.ASPAgentFacingSearchWallBudget
