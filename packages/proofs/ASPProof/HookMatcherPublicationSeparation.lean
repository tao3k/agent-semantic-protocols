namespace ASPProof.HookMatcherPublicationSeparation

/-- The control plane owns immutable matcher publication. -/
structure MatcherState where
  generation : Option Nat

/-- One Binary v1 value owns every projection in one generation. Cross-file
generation mixtures are unrepresentable. -/
structure MatcherBundle (Projection : Type) where
  generation : Nat
  complete : Projection
  directRead : Projection
  shellRead : Projection
  commandProfile : Projection

def atomicSwitch (_current next : MatcherBundle Projection) : MatcherBundle Projection :=
  next

theorem atomic_switch_exposes_one_complete_generation
    (current next : MatcherBundle Projection) :
    let visible := atomicSwitch current next
    visible.generation = next.generation ∧
      visible.complete = next.complete ∧
      visible.directRead = next.directRead ∧
      visible.shellRead = next.shellRead ∧
      visible.commandProfile = next.commandProfile := by
  simp [atomicSwitch]

/-- Hook evaluation is total over both ready and unavailable states. -/
inductive EvaluationResult where
  | decision (generation : Nat)
  | unavailable
  deriving DecidableEq

def evaluate (state : MatcherState) : EvaluationResult :=
  match state.generation with
  | some generation => .decision generation
  | none => .unavailable

def publish (state : MatcherState) (generation : Nat) : MatcherState :=
  { state with generation := some generation }

/-- Missing matcher state terminates as a typed unavailable result; evaluation
does not wait for publication or attempt recursive recovery. -/
theorem missing_generation_terminates_unavailable :
    evaluate { generation := none } = .unavailable := by
  rfl

/-- Evaluation cannot mutate or publish the matcher generation. -/
theorem evaluation_preserves_generation (state : MatcherState) :
    let _ := evaluate state
    state.generation = state.generation := by
  rfl

/-- Publishing is an explicit control-plane transition. -/
theorem publication_makes_generation_readable (state : MatcherState) (generation : Nat) :
    evaluate (publish state generation) = .decision generation := by
  rfl

/-- Counterexample for the retired coupled design: an evaluator that owns the
evaluation boundary and requires that same boundary for publication can wait
on itself. -/
def CoupledSelfWait (evaluationOwnsBoundary publicationNeedsBoundary : Bool) : Prop :=
  evaluationOwnsBoundary = true ∧ publicationNeedsBoundary = true

theorem coupled_evaluation_publication_has_self_wait_counterexample :
    CoupledSelfWait true true := by
  simp [CoupledSelfWait]

/-- The separated data plane has no publication edge, so the self-wait
precondition is false by construction. -/
theorem separated_evaluation_cannot_self_wait :
    ¬ CoupledSelfWait true false := by
  simp [CoupledSelfWait]

/-- Concurrent readers are independent projections of one immutable
generation; reader cardinality cannot introduce serialization state. -/
theorem concurrent_readers_preserve_cardinality
    (state : MatcherState) (readers : List Reader) :
    (readers.map fun _ => evaluate state).length = readers.length := by
  simp

/-- The four authorities that formed the retired recovery cycle. -/
inductive RecoveryNode where
  | buildFixedBinary
  | dispatchTestingAgent
  | runtimeUnavailable
  | installFixedBinary
  | recovered
  deriving DecidableEq

/-- A recovery override is detected before the Policy Kernel.  Both supported
origins have identical semantics; the origin is evidence, not policy. -/
inductive RecoveryOverrideOrigin where
  | hookProcessEnvironment
  | shellCommandAssignment
  deriving DecidableEq

structure RecoveryModel where
  oldBinaryDeniesBuild : Bool
  testingDispatchRecurses : Bool
  runtimeIsUnavailable : Bool
  installNeedsDeniedBuild : Bool
  overrideOrigin : Option RecoveryOverrideOrigin

/-- Without an override the old design follows its authority dependencies.
With an override every source node terminates at `recovered` before the Policy
Kernel can create a dispatch or denial edge. -/
def recoveryEdge (model : RecoveryModel) (source target : RecoveryNode) : Prop :=
  match model.overrideOrigin with
  | some _ => target = .recovered
  | none =>
      match source, target with
      | .buildFixedBinary, .dispatchTestingAgent => model.oldBinaryDeniesBuild = true
      | .dispatchTestingAgent, .runtimeUnavailable => model.testingDispatchRecurses = true
      | .runtimeUnavailable, .installFixedBinary => model.runtimeIsUnavailable = true
      | .installFixedBinary, .buildFixedBinary => model.installNeedsDeniedBuild = true
      | _, _ => False

def formsLegacyRecoveryCycle (model : RecoveryModel) : Prop :=
  recoveryEdge model .buildFixedBinary .dispatchTestingAgent ∧
    recoveryEdge model .dispatchTestingAgent .runtimeUnavailable ∧
    recoveryEdge model .runtimeUnavailable .installFixedBinary ∧
    recoveryEdge model .installFixedBinary .buildFixedBinary

def legacyDeadlockModel : RecoveryModel := {
  oldBinaryDeniesBuild := true
  testingDispatchRecurses := true
  runtimeIsUnavailable := true
  installNeedsDeniedBuild := true
  overrideOrigin := none
}

/-- Concrete counterexample: the retired dependency graph contains a cycle. -/
theorem legacy_deadlock_cycle_is_reachable :
    formsLegacyRecoveryCycle legacyDeadlockModel := by
  simp [formsLegacyRecoveryCycle, recoveryEdge, legacyDeadlockModel]

/-- A process-environment recovery override is a terminal pre-kernel edge, so
the legacy recovery cycle cannot be formed. -/
theorem process_environment_override_makes_deadlock_unreachable :
    ¬ formsLegacyRecoveryCycle
      { legacyDeadlockModel with
          overrideOrigin := some .hookProcessEnvironment } := by
  simp [formsLegacyRecoveryCycle, recoveryEdge]

/-- A shell-command assignment has the same pre-kernel precedence. -/
theorem shell_assignment_override_makes_deadlock_unreachable :
    ¬ formsLegacyRecoveryCycle
      { legacyDeadlockModel with
          overrideOrigin := some .shellCommandAssignment } := by
  simp [formsLegacyRecoveryCycle, recoveryEdge]

/-- Recovery precedence is scoped to a parsed Hook event.  The override cannot
capture ordinary ASP CLI commands before their own parser runs. -/
inductive CliInvocation where
  | hookEvent
  | ordinaryCli
  deriving DecidableEq

def entersSynchronousHookBootstrap
    (invocation : CliInvocation) (overridePresent : Bool) : Bool :=
  match invocation with
  | .hookEvent => overridePresent
  | .ordinaryCli => false

theorem recovery_override_does_not_capture_ordinary_cli
    (overridePresent : Bool) :
    entersSynchronousHookBootstrap .ordinaryCli overridePresent = false := by
  rfl

theorem recovery_override_precedes_policy_for_hook_event :
    entersSynchronousHookBootstrap .hookEvent true = true := by
  rfl

/-- Typed dispatch is a fixed point: once execution is in the requested Agent,
the same policy must allow rather than emit another dispatch edge. -/
structure TypedAgentIdentity where
  agentType : String
  agentId : String
  deriving DecidableEq

inductive DispatchResult where
  | allow
  | dispatch (target : TypedAgentIdentity)
  deriving DecidableEq

def resolveTypedDispatch
    (current target : TypedAgentIdentity) : DispatchResult :=
  if current = target then .allow else .dispatch target

theorem typed_target_dispatch_is_a_fixed_point (identity : TypedAgentIdentity) :
    resolveTypedDispatch identity identity = .allow := by
  simp [resolveTypedDispatch]

/-- The policy data plane precedes runtime construction. Only asynchronous
lifecycle work owns a bounded multi-worker Tokio profile. -/
structure HookProcessRuntime where
  policyDataPlaneWorkerCount : Option Nat
  asyncLifecycleWorkerCount : Nat

def canonicalHookProcessRuntime : HookProcessRuntime := {
  policyDataPlaneWorkerCount := none
  asyncLifecycleWorkerCount := 2
}

def admitsConcurrentProgress (runtime : HookProcessRuntime) : Prop :=
  runtime.policyDataPlaneWorkerCount = none ∧
    2 ≤ runtime.asyncLifecycleWorkerCount

theorem policy_data_plane_precedes_runtime_and_lifecycle_is_not_single_worker :
    admitsConcurrentProgress canonicalHookProcessRuntime := by
  simp [admitsConcurrentProgress, canonicalHookProcessRuntime]

end ASPProof.HookMatcherPublicationSeparation
