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
