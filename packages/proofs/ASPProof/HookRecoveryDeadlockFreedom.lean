-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.HookRecoveryDeadlockFreedom

inductive HookOutcome where
  | allow
  | deny
  | dispatch
  deriving DecidableEq

structure HookInput where
  recoveryOverride : Bool
  currentIsTarget : Bool
  policyWouldDeny : Bool

/-- Recovery is evaluated before identity and ordinary policy.  Target
identity is the dispatch fixed point. -/
def evaluate (input : HookInput) : HookOutcome :=
  if input.recoveryOverride then .allow
  else if input.currentIsTarget then .allow
  else if input.policyWouldDeny then .dispatch
  else .allow

theorem recovery_override_dominates_every_state
    (currentIsTarget policyWouldDeny : Bool) :
    evaluate {
      recoveryOverride := true
      currentIsTarget := currentIsTarget
      policyWouldDeny := policyWouldDeny
    } = .allow := by
  rfl

theorem target_identity_is_dispatch_fixed_point (policyWouldDeny : Bool) :
    evaluate {
      recoveryOverride := false
      currentIsTarget := true
      policyWouldDeny := policyWouldDeny
    } = .allow := by
  rfl

theorem non_target_denial_requires_one_dispatch :
    evaluate {
      recoveryOverride := false
      currentIsTarget := false
      policyWouldDeny := true
    } = .dispatch := by
  rfl

/-- The legacy transition ignored typed target identity. -/
def legacyEvaluate (policyWouldDeny : Bool) : HookOutcome :=
  if policyWouldDeny then .dispatch else .allow

/-- Counterexample discovered by the model: after dispatching into the target,
the same deny rule dispatches to the same target again. -/
theorem legacy_dispatch_recurses_inside_target :
    legacyEvaluate true = .dispatch := by
  rfl

inductive StatefulDependency where
  | runtime
  | configuration
  | lock
  | telemetry
  deriving DecidableEq

def recoveryDependencies : List StatefulDependency := []

theorem recovery_override_has_no_stateful_dependency
    (dependency : StatefulDependency) :
    dependency ∉ recoveryDependencies := by
  simp [recoveryDependencies]

/-- A broken binary cannot be repaired if both the caller and target recursively
deny the repair command. -/
def legacyRepairReachable
    (binaryHealthy callerAllows targetAllows : Bool) : Bool :=
  binaryHealthy || (callerAllows && targetAllows)

theorem legacy_broken_binary_recursive_deny_is_deadlocked :
    legacyRepairReachable false false false = false := by
  rfl

/-- The entry-plane recovery override adds a transition that does not depend on
the broken policy/runtime path. -/
def repairReachable
    (recoveryOverride binaryHealthy callerAllows targetAllows : Bool) : Bool :=
  recoveryOverride || legacyRepairReachable binaryHealthy callerAllows targetAllows

theorem recovery_override_breaks_recursive_repair_deadlock :
    repairReachable true false false false = true := by
  rfl

inductive RuntimeAuthority where
  | missing
  | legacyV1Stale
  | malformed
  | currentV1Validated
  deriving DecidableEq

inductive ExecutionMode where
  | normal
  | processRecovery
  deriving DecidableEq

/-- Only the fully validated stable-v1 launcher observation is Runtime health
authority. A legacy stable-v1 shape is evidence that re-observation is needed,
not permission to assert health. -/
def runtimeHealthy : RuntimeAuthority → Bool
  | .currentV1Validated => true
  | _ => false

theorem legacy_v1_stale_does_not_assert_runtime_health :
    runtimeHealthy .legacyV1Stale = false := by
  rfl

theorem malformed_receipt_does_not_assert_runtime_health :
    runtimeHealthy .malformed = false := by
  rfl

/-- The process recovery edge is decided without consulting Runtime authority.
This is the formal non-interference boundary that keeps a broken observation
from disabling the repair process itself. -/
def entryRecoveryAllows
    (mode : ExecutionMode) (_runtime : RuntimeAuthority) : Bool :=
  mode == .processRecovery

theorem runtime_authority_cannot_change_process_recovery
    (left right : RuntimeAuthority) :
    entryRecoveryAllows .processRecovery left =
      entryRecoveryAllows .processRecovery right := by
  rfl

theorem process_recovery_is_reachable_from_every_runtime_state
    (runtime : RuntimeAuthority) :
    entryRecoveryAllows .processRecovery runtime = true := by
  rfl

theorem normal_mode_cannot_consume_recovery_authority
    (runtime : RuntimeAuthority) :
    entryRecoveryAllows .normal runtime = false := by
  rfl

/-- A validated stable-v1 candidate restores normal Runtime authority without
changing the public receipt schema version. -/
def validateCandidate : RuntimeAuthority := .currentV1Validated

theorem validated_candidate_restores_normal_runtime_authority :
    runtimeHealthy validateCandidate = true := by
  rfl

inductive RecoveryPhase where
  | bootstrapFailed
  | processOverrideObserved
  | repairLaunched
  | normalAuthorityRestored
  deriving DecidableEq

def recoveryRank : RecoveryPhase → Nat
  | .bootstrapFailed => 0
  | .processOverrideObserved => 1
  | .repairLaunched => 2
  | .normalAuthorityRestored => 3

def recoveryStep : RecoveryPhase → RecoveryPhase → Prop
  | .bootstrapFailed, .processOverrideObserved => True
  | .processOverrideObserved, .repairLaunched => True
  | .repairLaunched, .normalAuthorityRestored => True
  | _, _ => False

theorem recovery_step_strictly_advances
    {left right : RecoveryPhase}
    (step : recoveryStep left right) :
    recoveryRank left < recoveryRank right := by
  cases left <;> cases right <;> simp_all [recoveryStep, recoveryRank]

theorem recovery_transition_is_acyclic
    (phase : RecoveryPhase) :
    ¬ recoveryStep phase phase := by
  cases phase <;> simp [recoveryStep]

/-- Publication and one-shot readers must derive authority from the same
normalized workspace identity. Comparing raw lexical paths admits aliases such
as `root` and `root/.` as different authorities. -/
def rawAuthorityHit [DecidableEq RawWorkspace]
    (published observed : RawWorkspace) : Bool :=
  decide (published = observed)

theorem lexical_alias_can_miss_raw_authority
    [DecidableEq RawWorkspace]
    (canonical alias : RawWorkspace)
    (differentRaw : canonical ≠ alias) :
    rawAuthorityHit canonical alias = false := by
  simp [rawAuthorityHit, differentRaw]

def normalizedAuthorityHit [DecidableEq WorkspaceKey]
    (normalize : RawWorkspace → WorkspaceKey)
    (published observed : RawWorkspace) : Bool :=
  decide (normalize published = normalize observed)

theorem normalized_aliases_share_authority
    [DecidableEq WorkspaceKey]
    (normalize : RawWorkspace → WorkspaceKey)
    (canonical alias : RawWorkspace)
    (sameIdentity : normalize canonical = normalize alias) :
    normalizedAuthorityHit normalize canonical alias = true := by
  simp [normalizedAuthorityHit, sameIdentity]

def withinDecisionBudget (cpuMicros : Nat) : Prop :=
  cpuMicros < 1000

def observedWallMicros (cpuMicros schedulerDelayMicros : Nat) : Nat :=
  cpuMicros + schedulerDelayMicros

def withinObservedWallBoundary (elapsedMicros : Nat) : Prop :=
  elapsedMicros < 1000

theorem last_admitted_microsecond_is_within_budget :
    withinDecisionBudget 999 := by
  simp [withinDecisionBudget]

theorem one_millisecond_is_rejected :
    ¬ withinDecisionBudget 1000 := by
  simp [withinDecisionBudget]

/-- Scheduler delay is observable but not Hook-owned work. A universal
wall-clock max cannot refine the local decision budget on a preemptive Host. -/
theorem scheduler_preemption_refutes_universal_wall_max :
    withinDecisionBudget 240 ∧
      ¬ withinObservedWallBoundary (observedWallMicros 240 6000) := by
  simp [withinDecisionBudget, withinObservedWallBoundary, observedWallMicros]

/-- 500us is the engineering headroom target, distinct from the 1ms terminal
boundary. A decision may remain admissible while already representing a
performance regression. -/
def meetsTypicalTarget (elapsedMicros : Nat) : Prop :=
  elapsedMicros ≤ 500

theorem five_hundred_microseconds_meets_typical_target :
    meetsTypicalTarget 500 := by
  simp [meetsTypicalTarget]

theorem eight_hundred_twenty_three_is_admitted_but_not_safe_headroom :
    withinDecisionBudget 823 ∧ ¬ meetsTypicalTarget 823 := by
  simp [withinDecisionBudget, meetsTypicalTarget]

/-- "Typical" is an executable distribution contract: at least three of four
concurrent decisions must meet the 500us target. -/
def hasTypicalQuorum (sampleCount typicalCount : Nat) : Prop :=
  typicalCount * 4 ≥ sampleCount * 3

theorem twenty_four_of_thirty_two_has_typical_quorum :
    hasTypicalQuorum 32 24 := by
  simp [hasTypicalQuorum]

theorem twenty_three_of_thirty_two_lacks_typical_quorum :
    ¬ hasTypicalQuorum 32 23 := by
  simp [hasTypicalQuorum]

/-- Reader concurrency is not an input to the decision function: adding any
number of simultaneous one-shot Hook readers cannot change policy authority. -/
def evaluateWithReaders (input : HookInput) (_concurrentReaders : Nat) : HookOutcome :=
  evaluate input

theorem concurrent_readers_cannot_change_decision
    (input : HookInput) (leftReaders rightReaders : Nat) :
    evaluateWithReaders input leftReaders = evaluateWithReaders input rightReaders := by
  rfl

/-- A serialized reader queue is an architectural counterexample: a policy
decision that is locally fast can miss the strict boundary solely because
other Hook readers exist. -/
def serializedElapsed
    (localMicros queueMicros concurrentReaders : Nat) : Nat :=
  localMicros + queueMicros * concurrentReaders

theorem serialized_reader_queue_violates_budget :
    ¬ withinObservedWallBoundary (serializedElapsed 600 250 2) := by
  simp [withinObservedWallBoundary, serializedElapsed]

/-- Lock-free immutable readers do not inherit work from sibling readers. -/
def lockFreeElapsed (localMicros : Nat) (_concurrentReaders : Nat) : Nat :=
  localMicros

theorem lock_free_reader_latency_is_concurrency_invariant
    (localMicros leftReaders rightReaders : Nat) :
    lockFreeElapsed localMicros leftReaders =
      lockFreeElapsed localMicros rightReaders := by
  rfl

theorem lock_free_reader_preserves_budget
    (localMicros readers : Nat)
    (within : withinDecisionBudget localMicros) :
    withinDecisionBudget (lockFreeElapsed localMicros readers) := by
  exact within

inductive CompleteGeneration where
  | prior
  | next
  deriving DecidableEq

/-- Atomic rename exposes one complete immutable generation. There is no
constructor for a partially published matcher. -/
def observeAtomicGeneration (published : Bool) : CompleteGeneration :=
  if published then .next else .prior

theorem atomic_publication_never_exposes_torn_generation (published : Bool) :
    observeAtomicGeneration published = .prior ∨
      observeAtomicGeneration published = .next := by
  cases published <;> simp [observeAtomicGeneration]

end ASPProof.HookRecoveryDeadlockFreedom
