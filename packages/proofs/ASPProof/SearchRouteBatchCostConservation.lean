-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteBatchCostConservation

structure ClosureState where
  undiscoveredRoutes : Nat
  activeInspectPotential : Nat
  deriving DecidableEq

def closureMeasure (maxInspectPerRoute : Nat) (state : ClosureState) : Nat :=
  state.undiscoveredRoutes * (maxInspectPerRoute + 1) +
  state.activeInspectPotential

structure CertifiedBatchTransition
    (maxInspectPerRoute : Nat)
    (before after : ClosureState) where
  discoveredRoutes : Nat
  discoversPositiveBatch : 0 < discoveredRoutes
  removesDiscoveredRoutes :
    before.undiscoveredRoutes =
      after.undiscoveredRoutes + discoveredRoutes
  boundedNewInspect :
    after.activeInspectPotential ≤
      before.activeInspectPotential +
        discoveredRoutes * maxInspectPerRoute

theorem batch_measure_decreases_by_discovered_count
    {maxInspectPerRoute : Nat}
    {before after : ClosureState}
    (transition :
      CertifiedBatchTransition maxInspectPerRoute before after) :
    closureMeasure maxInspectPerRoute after +
        transition.discoveredRoutes ≤
      closureMeasure maxInspectPerRoute before := by
  calc
    closureMeasure maxInspectPerRoute after +
          transition.discoveredRoutes =
        (after.undiscoveredRoutes * (maxInspectPerRoute + 1) +
          after.activeInspectPotential) +
          transition.discoveredRoutes := rfl
    _ ≤
        (after.undiscoveredRoutes * (maxInspectPerRoute + 1) +
          (before.activeInspectPotential +
            transition.discoveredRoutes * maxInspectPerRoute)) +
          transition.discoveredRoutes :=
            Nat.add_le_add_right
              (Nat.add_le_add_left transition.boundedNewInspect _)
              _
    _ =
        (after.undiscoveredRoutes + transition.discoveredRoutes) *
            (maxInspectPerRoute + 1) +
          before.activeInspectPotential := by
            simp only
              [ Nat.add_mul
              , Nat.mul_add
              , Nat.mul_one
              , Nat.add_assoc
              , Nat.add_comm
              , Nat.add_left_comm
              ]
    _ = closureMeasure maxInspectPerRoute before := by
          simp [closureMeasure, transition.removesDiscoveredRoutes]

theorem certified_batch_transition_decreases
    {maxInspectPerRoute : Nat}
    {before after : ClosureState}
    (transition :
      CertifiedBatchTransition maxInspectPerRoute before after) :
    closureMeasure maxInspectPerRoute after <
      closureMeasure maxInspectPerRoute before := by
  have positiveIncrease :
      closureMeasure maxInspectPerRoute after <
        closureMeasure maxInspectPerRoute after +
          transition.discoveredRoutes := by
    have increased :=
      Nat.add_lt_add_left transition.discoversPositiveBatch
        (closureMeasure maxInspectPerRoute after)
    simpa using increased
  exact Nat.lt_of_lt_of_le positiveIncrease
    (batch_measure_decreases_by_discovered_count transition)

inductive CertifiedBatchRun
    (maxInspectPerRoute : Nat) :
    ClosureState → ClosureState → Nat → Prop where
  | zero {state : ClosureState} :
      CertifiedBatchRun maxInspectPerRoute state state 0
  | next {start middle finish : ClosureState} {rounds : Nat}
      (transition :
        CertifiedBatchTransition maxInspectPerRoute start middle)
      (remaining :
        CertifiedBatchRun maxInspectPerRoute middle finish rounds) :
      CertifiedBatchRun maxInspectPerRoute start finish (rounds + 1)

theorem certified_batch_run_round_bound
    {maxInspectPerRoute rounds : Nat}
    {start finish : ClosureState}
    (run :
      CertifiedBatchRun maxInspectPerRoute start finish rounds) :
    rounds ≤ closureMeasure maxInspectPerRoute start := by
  induction run with
  | zero =>
      exact Nat.zero_le _
  | next transition _ inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (certified_batch_transition_decreases transition)

structure BatchCost where
  discoveredRoutes : Nat
  toolCalls : Nat
  branchTokens : Nat
  synthesisTokens : Nat
  llmRounds : Nat
  criticalPathLatency : Nat
  deriving DecidableEq

def BatchCost.totalTokens (cost : BatchCost) : Nat :=
  cost.branchTokens + cost.synthesisTokens

def ValidBatchAccounting
    (minBranchTokensPerRoute : Nat)
    (cost : BatchCost) : Prop :=
  cost.discoveredRoutes ≤ cost.toolCalls ∧
  cost.discoveredRoutes * minBranchTokensPerRoute ≤ cost.branchTokens ∧
  (0 < cost.discoveredRoutes → 0 < cost.llmRounds)

theorem valid_batch_conserves_tool_work
    {minBranchTokensPerRoute : Nat}
    {cost : BatchCost}
    (valid : ValidBatchAccounting minBranchTokensPerRoute cost) :
    cost.discoveredRoutes ≤ cost.toolCalls :=
  valid.1

theorem valid_batch_conserves_branch_tokens
    {minBranchTokensPerRoute : Nat}
    {cost : BatchCost}
    (valid : ValidBatchAccounting minBranchTokensPerRoute cost) :
    cost.discoveredRoutes * minBranchTokensPerRoute ≤ cost.totalTokens := by
  exact Nat.le_trans valid.2.1
    (Nat.le_add_right cost.branchTokens cost.synthesisTokens)

theorem valid_nonempty_batch_consumes_round
    {minBranchTokensPerRoute : Nat}
    {cost : BatchCost}
    (valid : ValidBatchAccounting minBranchTokensPerRoute cost)
    (nonempty : 0 < cost.discoveredRoutes) :
    0 < cost.llmRounds :=
  valid.2.2 nonempty

def sequentialBatch : BatchCost where
  discoveredRoutes := 1
  toolCalls := 1
  branchTokens := 10
  synthesisTokens := 5
  llmRounds := 1
  criticalPathLatency := 10

def parallelBatch : BatchCost where
  discoveredRoutes := 4
  toolCalls := 4
  branchTokens := 40
  synthesisTokens := 8
  llmRounds := 1
  criticalPathLatency := 12

def zeroRoundBatch : BatchCost where
  discoveredRoutes := 1
  toolCalls := 1
  branchTokens := 10
  synthesisTokens := 5
  llmRounds := 0
  criticalPathLatency := 10

theorem parallel_batch_accounting_is_valid :
    ValidBatchAccounting 10 parallelBatch := by
  exact ⟨by decide, by decide, fun _ => by decide⟩

theorem same_round_count_does_not_determine_work :
    sequentialBatch.llmRounds = parallelBatch.llmRounds ∧
    sequentialBatch.toolCalls < parallelBatch.toolCalls ∧
    sequentialBatch.totalTokens < parallelBatch.totalTokens := by
  exact ⟨rfl, by decide, by decide⟩

theorem round_only_cost_underreports_parallel_batch :
    parallelBatch.llmRounds < parallelBatch.toolCalls ∧
    parallelBatch.llmRounds < parallelBatch.totalTokens := by
  exact ⟨by decide, by decide⟩

theorem zero_round_nonempty_batch_is_invalid :
    ¬ ValidBatchAccounting 10 zeroRoundBatch := by
  intro valid
  exact Nat.lt_irrefl 0 (valid.2.2 (by decide))

def batchBefore : ClosureState where
  undiscoveredRoutes := 4
  activeInspectPotential := 0

def batchAfter : ClosureState where
  undiscoveredRoutes := 0
  activeInspectPotential := 12

def batchDiscovery : CertifiedBatchTransition 3 batchBefore batchAfter where
  discoveredRoutes := 4
  discoversPositiveBatch := by decide
  removesDiscoveredRoutes := rfl
  boundedNewInspect := by decide

theorem four_route_batch_decreases_measure_by_four :
    closureMeasure 3 batchAfter + 4 ≤ closureMeasure 3 batchBefore :=
  batch_measure_decreases_by_discovered_count batchDiscovery

end ASPProof.SearchRouteBatchCostConservation
