import ASPProof.ASPAgentFacingSearchWallBudget

/-!
RFC correspondence:
docs/10-19-rfcs/10.15.02.06-codex-multi-agent-v2-state-lifecycle/
05-bounded-runtime-repair-and-agent-facing-deadline.org

This model separates cold preparation from the bounded warm acceptance path.
It also records the counterexample that a caller timeout alone cannot establish
bounded repair for a non-cancellation-safe operation.

Numeric staging correspondence:
ASPProof/ASPAgentFacingSearchWallBudget.lean owns the canonical 800/100/100
search-wall contract.  This module refines those boundaries into the Agent
Session repair lifecycle and must remain definitionally aligned with it.
-/

namespace ASPProof.AgentLifeSessionRuntimeRepairDeadline

def runtimeAccepts : Nat → Prop :=
  ASPProof.ASPAgentFacingSearchWallBudget.runtimeWaitAdmitted

def supervisorAccepts : Nat → Prop :=
  ASPProof.ASPAgentFacingSearchWallBudget.supervisorCompletionAdmitted

theorem runtime_accepts_last_strict_millisecond : runtimeAccepts 799 := by
  exact ASPProof.ASPAgentFacingSearchWallBudget.runtime_wait_keeps_reply_reserve

theorem runtime_rejects_exact_boundary : ¬ runtimeAccepts 800 := by
  exact ASPProof.ASPAgentFacingSearchWallBudget.runtime_wait_boundary_is_not_admitted

theorem supervisor_accepts_last_strict_millisecond : supervisorAccepts 899 := by
  exact ASPProof.ASPAgentFacingSearchWallBudget.supervisor_last_millisecond_is_admitted

theorem supervisor_rejects_exact_boundary : ¬ supervisorAccepts 900 := by
  exact ASPProof.ASPAgentFacingSearchWallBudget.supervisor_boundary_is_not_admitted

structure RepairOperation where
  durationMs : Nat
  cancellationSafe : Bool

def boundedBy (operation : RepairOperation) (deadlineMs : Nat) : Prop :=
  operation.cancellationSafe = true ∧ operation.durationMs ≤ deadlineMs

def outerDeadlineDeclared (_operation : RepairOperation) (_deadlineMs : Nat) : Prop :=
  True

theorem outer_deadline_alone_is_insufficient :
    ∃ operation,
      outerDeadlineDeclared operation
          ASPProof.ASPAgentFacingSearchWallBudget.supervisorBoundaryMs ∧
      ¬ boundedBy operation
          ASPProof.ASPAgentFacingSearchWallBudget.supervisorBoundaryMs := by
  refine ⟨{ durationMs := 901, cancellationSafe := false }, trivial, ?_⟩
  intro bounded
  exact Bool.false_ne_true bounded.1

inductive RepairPhase where
  | coldPrepare
  | warmReconcile
  | reply
  deriving DecidableEq

structure RepairTimeline where
  coldPrepareMs : Nat
  warmReconcileMs : Nat
  replyMs : Nat

def warmPathAdmitted (timeline : RepairTimeline) : Prop :=
  runtimeAccepts timeline.warmReconcileMs

theorem cold_preparation_is_not_charged_to_warm_acceptance
    (coldMs : Nat) :
    warmPathAdmitted
        { coldPrepareMs := coldMs, warmReconcileMs := 799, replyMs := 100 } := by
  exact runtime_accepts_last_strict_millisecond

end ASPProof.AgentLifeSessionRuntimeRepairDeadline
