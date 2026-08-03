import ASPProof.ASPAgentLifeSessionRuntimeRepairDeadline

/-!
Audit correspondence:
docs/10-19-rfcs/10.15.02.06-codex-multi-agent-v2-state-lifecycle/
05-bounded-runtime-repair-and-agent-facing-deadline.org

This module contains no fallback axioms.  It imports the executable lifecycle
model and exposes the obligations that the Rust refinement receipt must satisfy.
-/

namespace ASPProof.Audit.AgentLifeSessionRuntimeRepairDeadline

open ASPProof.AgentLifeSessionRuntimeRepairDeadline

theorem audit_runtime_strict_acceptance_boundary :
    runtimeAccepts 799 ∧ ¬ runtimeAccepts 800 := by
  exact ⟨runtime_accepts_last_strict_millisecond, runtime_rejects_exact_boundary⟩

theorem audit_supervisor_strict_acceptance_boundary :
    supervisorAccepts 899 ∧ ¬ supervisorAccepts 900 := by
  exact ⟨supervisor_accepts_last_strict_millisecond, supervisor_rejects_exact_boundary⟩

theorem audit_outer_timeout_requires_inner_evidence :
    ∃ operation,
      outerDeadlineDeclared operation
          ASPProof.ASPAgentFacingSearchWallBudget.supervisorBoundaryMs ∧
      ¬ boundedBy operation
          ASPProof.ASPAgentFacingSearchWallBudget.supervisorBoundaryMs :=
  outer_deadline_alone_is_insufficient

end ASPProof.Audit.AgentLifeSessionRuntimeRepairDeadline
