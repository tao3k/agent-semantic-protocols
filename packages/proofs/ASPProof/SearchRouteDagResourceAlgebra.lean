import ASPProof.SearchRouteFeasibilityFirst

namespace ASPProof.SearchRouteDagResourceAlgebra

structure ConsumableCost where
  uncachedTokens : Nat
  transitions : Nat
  deriving DecidableEq, Repr

structure ScheduleSpan where
  graphHops : Nat
  rounds : Nat
  deriving DecidableEq, Repr

structure DagCost where
  consumable : ConsumableCost
  span : ScheduleSpan
  deriving DecidableEq, Repr

structure DagBudget where
  consumable : ConsumableCost
  span : ScheduleSpan
  deriving DecidableEq, Repr

def sequential (left right : DagCost) : DagCost :=
  {
    consumable := {
      uncachedTokens :=
        left.consumable.uncachedTokens + right.consumable.uncachedTokens
      transitions :=
        left.consumable.transitions + right.consumable.transitions
    }
    span := {
      graphHops := left.span.graphHops + right.span.graphHops
      rounds := left.span.rounds + right.span.rounds
    }
  }

def parallel (left right : DagCost) : DagCost :=
  {
    consumable := {
      uncachedTokens :=
        left.consumable.uncachedTokens + right.consumable.uncachedTokens
      transitions :=
        left.consumable.transitions + right.consumable.transitions
    }
    span := {
      graphHops := max left.span.graphHops right.span.graphHops
      rounds := max left.span.rounds right.span.rounds
    }
  }

def PlanFeasible (budget : DagBudget) (cost : DagCost) : Prop :=
  cost.consumable.uncachedTokens ≤ budget.consumable.uncachedTokens ∧
  cost.consumable.transitions ≤ budget.consumable.transitions ∧
  cost.span.graphHops ≤ budget.span.graphHops ∧
  cost.span.rounds ≤ budget.span.rounds

structure ResourceLedger where
  uncachedTokensRemaining : Nat
  transitionsRemaining : Nat
  deriving DecidableEq, Repr

def CanReserve
    (ledger : ResourceLedger)
    (cost : ConsumableCost) : Prop :=
  cost.uncachedTokens ≤ ledger.uncachedTokensRemaining ∧
  cost.transitions ≤ ledger.transitionsRemaining

def afterReservation
    (ledger : ResourceLedger)
    (cost : ConsumableCost) : ResourceLedger :=
  {
    uncachedTokensRemaining :=
      ledger.uncachedTokensRemaining - cost.uncachedTokens
    transitionsRemaining :=
      ledger.transitionsRemaining - cost.transitions
  }

theorem reservation_preserves_token_accounting
    (ledger : ResourceLedger)
    (cost : ConsumableCost)
    (canReserve : CanReserve ledger cost) :
    (afterReservation ledger cost).uncachedTokensRemaining +
        cost.uncachedTokens =
      ledger.uncachedTokensRemaining := by
  exact Nat.sub_add_cancel canReserve.1

theorem reservation_preserves_transition_accounting
    (ledger : ResourceLedger)
    (cost : ConsumableCost)
    (canReserve : CanReserve ledger cost) :
    (afterReservation ledger cost).transitionsRemaining +
        cost.transitions =
      ledger.transitionsRemaining := by
  exact Nat.sub_add_cancel canReserve.2

def exampleBudget : DagBudget :=
  {
    consumable := {
      uncachedTokens := 20
      transitions := 2
    }
    span := {
      graphHops := 2
      rounds := 2
    }
  }

def branchA : DagCost :=
  {
    consumable := {
      uncachedTokens := 15
      transitions := 1
    }
    span := {
      graphHops := 1
      rounds := 1
    }
  }

def branchB : DagCost :=
  branchA

theorem branch_a_is_individually_feasible :
    PlanFeasible exampleBudget branchA := by
  decide

theorem branch_b_is_individually_feasible :
    PlanFeasible exampleBudget branchB := by
  decide

theorem sequential_plan_is_infeasible :
    ¬ PlanFeasible exampleBudget (sequential branchA branchB) := by
  decide

theorem parallel_plan_is_infeasible :
    ¬ PlanFeasible exampleBudget (parallel branchA branchB) := by
  decide

theorem individual_feasibility_does_not_compose :
    PlanFeasible exampleBudget branchA ∧
      PlanFeasible exampleBudget branchB ∧
      ¬ PlanFeasible exampleBudget (parallel branchA branchB) := by
  exact
    ⟨
      branch_a_is_individually_feasible,
      branch_b_is_individually_feasible,
      parallel_plan_is_infeasible
    ⟩

theorem parallel_and_sequential_span_differ :
    (parallel branchA branchB).span.rounds = 1 ∧
      (sequential branchA branchB).span.rounds = 2 := by
  decide

def exampleLedger : ResourceLedger :=
  {
    uncachedTokensRemaining := 20
    transitionsRemaining := 2
  }

theorem first_branch_can_reserve :
    CanReserve exampleLedger branchA.consumable := by
  decide

theorem first_reservation_leaves_five_tokens :
    (afterReservation exampleLedger branchA.consumable)
        .uncachedTokensRemaining = 5 := by
  decide

theorem second_branch_cannot_double_spend :
    ¬ CanReserve
      (afterReservation exampleLedger branchA.consumable)
      branchB.consumable := by
  decide

end ASPProof.SearchRouteDagResourceAlgebra

