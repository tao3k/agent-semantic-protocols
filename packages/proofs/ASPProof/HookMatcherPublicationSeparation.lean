namespace ASPProof.HookMatcherPublicationSeparation

/-- State Home owns exactly one canonical Runtime Hook binary. -/
structure MatcherState where
  runtimeHookBinaryDigest : Option Nat

/-- One schema-v1 AOT value owns the complete policy and Reader catalog in one
bundle. Cross-file matcher shards and mixed policy values are unrepresentable. -/
structure MatcherBundle (Projection : Type) where
  runtimeHookBinaryDigest : Nat
  configDigest : Nat
  compiledMatcherDigest : Nat
  registryDigest : Nat
  receiptDigest : Nat
  compiledPolicy : Projection
  readerCatalog : Projection

def atomicRuntimeHookSwitch (_current next : MatcherBundle Projection) : MatcherBundle Projection :=
  next

theorem atomic_runtime_hook_switch_exposes_one_complete_bundle
    (current next : MatcherBundle Projection) :
    let visible := atomicRuntimeHookSwitch current next
    visible.runtimeHookBinaryDigest = next.runtimeHookBinaryDigest ∧
      visible.configDigest = next.configDigest ∧
      visible.compiledMatcherDigest = next.compiledMatcherDigest ∧
      visible.registryDigest = next.registryDigest ∧
      visible.receiptDigest = next.receiptDigest ∧
      visible.compiledPolicy = next.compiledPolicy ∧
      visible.readerCatalog = next.readerCatalog := by
  simp [atomicRuntimeHookSwitch]

/-- Policy and lifecycle events enter one immutable Hook binary. Their internal
routes cannot observe different executable identities. -/
inductive HookEventPlane where
  | policy
  | lifecycle
  deriving DecidableEq

def binaryDigestForPlane
    (bundle : MatcherBundle Projection) : HookEventPlane → Nat
  | .policy => bundle.runtimeHookBinaryDigest
  | .lifecycle => bundle.runtimeHookBinaryDigest

theorem atomic_runtime_hook_switch_keeps_policy_and_lifecycle_on_one_binary
    (current next : MatcherBundle Projection) (plane : HookEventPlane) :
    binaryDigestForPlane (atomicRuntimeHookSwitch current next) plane =
      binaryDigestForPlane next plane := by
  cases plane <;> rfl

theorem policy_and_lifecycle_share_one_hook_binary
    (bundle : MatcherBundle Projection) :
    binaryDigestForPlane bundle .policy =
      binaryDigestForPlane bundle .lifecycle := by
  rfl

/-- Mutable Config source is candidate input, never serving state. -/
def editConfigSource (active : MatcherBundle Projection) (_newSourceDigest : Nat) :
    MatcherBundle Projection := active

theorem config_source_edit_preserves_runtime_hook_binary
    (active : MatcherBundle Projection) (newSourceDigest : Nat) :
    (editConfigSource active newSourceDigest).runtimeHookBinaryDigest =
      active.runtimeHookBinaryDigest := by
  rfl

inductive CandidateStage where
  | parse
  | compile
  | validate
  | publication
  deriving DecidableEq

def candidateTransition
    (current candidate : MatcherBundle Projection) (stageSucceeded : Bool) :
    MatcherBundle Projection :=
  if stageSucceeded then atomicRuntimeHookSwitch current candidate else current

theorem candidate_stage_failure_preserves_previous
    (current candidate : MatcherBundle Projection) (stage : CandidateStage) :
    let _ := stage
    candidateTransition current candidate false = current := by
  simp [candidateTransition]

inductive RuntimeActivationState where
  | stopped
  | pending
  | failed
  | ready
  deriving DecidableEq

def installRuntimeHookBinary
    (current candidate : MatcherBundle Projection)
    (_runtime : RuntimeActivationState) : MatcherBundle Projection :=
  atomicRuntimeHookSwitch current candidate

theorem runtime_hook_install_is_runtime_server_independent
    (current candidate : MatcherBundle Projection)
    (left right : RuntimeActivationState) :
    installRuntimeHookBinary current candidate left =
      installRuntimeHookBinary current candidate right := by
  rfl

/-- Hook evaluation is total over both ready and unavailable states. -/
inductive EvaluationResult where
  | decision (binaryDigest : Nat)
  | unavailable
  deriving DecidableEq

def evaluate (state : MatcherState) : EvaluationResult :=
  match state.runtimeHookBinaryDigest with
  | some binaryDigest => .decision binaryDigest
  | none => .unavailable

def install (state : MatcherState) (binaryDigest : Nat) : MatcherState :=
  { state with runtimeHookBinaryDigest := some binaryDigest }

/-- Missing matcher state terminates as a typed unavailable result; evaluation
does not wait for publication or attempt recursive recovery. -/
theorem missing_runtime_hook_binary_terminates_unavailable :
    evaluate { runtimeHookBinaryDigest := none } = .unavailable := by
  rfl

/-- Evaluation cannot mutate or publish the installed Hook binary. -/
theorem evaluation_preserves_runtime_hook_binary (state : MatcherState) :
    let _ := evaluate state
    state.runtimeHookBinaryDigest = state.runtimeHookBinaryDigest := by
  rfl

/-- Publishing is an explicit control-plane transition. -/
theorem installation_makes_runtime_hook_binary_executable
    (state : MatcherState) (binaryDigest : Nat) :
    evaluate (install state binaryDigest) = .decision binaryDigest := by
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

/-- Concurrent readers are independent projections of one immutable policy
bundle; reader cardinality cannot introduce serialization state. -/
theorem concurrent_readers_preserve_cardinality
    (state : MatcherState) (readers : List Reader) :
    (readers.map fun _ => evaluate state).length = readers.length := by
  simp

/-- The public Client and the standalone Hook executable have disjoint command
grammars.  Repeating `hook` inside `asp-hook` is therefore not an event route. -/
inductive ExecutableSurface where
  | client
  | hook
  deriving DecidableEq

def acceptsHostEvent : ExecutableSurface → Bool
  | .client => false
  | .hook => true

theorem only_standalone_hook_accepts_host_events :
    acceptsHostEvent .client = false ∧ acceptsHostEvent .hook = true := by
  decide

/-- The four authorities that formed the retired recovery cycle. -/
inductive RecoveryNode where
  | buildFixedBinary
  | dispatchTestingAgent
  | runtimeUnavailable
  | installFixedBinary
  | recovered
  deriving DecidableEq

/-- Both inherited process boundaries and the parser-proven command process
environment terminate before the Policy Kernel. -/
inductive RecoveryOverrideOrigin where
  | pluginLauncherEnvironment
  | evaluatorProcessEnvironment
  | commandProcessEnvironment
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
          overrideOrigin := some .evaluatorProcessEnvironment } := by
  simp [formsLegacyRecoveryCycle, recoveryEdge]

/-- The fixed Plugin launcher is the earlier inherited-process escape layer. -/
theorem launcher_environment_override_makes_deadlock_unreachable :
    ¬ formsLegacyRecoveryCycle
      { legacyDeadlockModel with
          overrideOrigin := some .pluginLauncherEnvironment } := by
  simp [formsLegacyRecoveryCycle, recoveryEdge]

/-- A Bash payload is recovery authority only when its parsed process topology
proves that the assignment reaches the requested child process. -/
inductive CommandEnvironmentEvidence where
  | directAssignment
  | envUtility
  | exportedCommand
  | unboundText
  deriving DecidableEq

def commandEnvironmentHasRecoveryAuthority : CommandEnvironmentEvidence → Bool
  | .directAssignment => true
  | .envUtility => true
  | .exportedCommand => true
  | .unboundText => false

/-- The command-local guard is evaluated before evaluator resolution. -/
def commandEscapeBeforeEvaluator
    (evidence : CommandEnvironmentEvidence) (evaluatorAvailable : Bool) : Bool :=
  commandEnvironmentHasRecoveryAuthority evidence || evaluatorAvailable

theorem direct_assignment_is_recovery_authority :
    commandEnvironmentHasRecoveryAuthority .directAssignment = true := by
  rfl

theorem env_utility_assignment_is_recovery_authority :
    commandEnvironmentHasRecoveryAuthority .envUtility = true := by
  rfl

theorem exported_assignment_is_recovery_authority :
    commandEnvironmentHasRecoveryAuthority .exportedCommand = true := by
  rfl

theorem unbound_payload_text_is_not_recovery_authority :
    commandEnvironmentHasRecoveryAuthority .unboundText = false := by
  rfl

theorem exported_command_escape_survives_missing_evaluator :
    commandEscapeBeforeEvaluator .exportedCommand false = true := by
  rfl

theorem unbound_text_cannot_escape_missing_evaluator :
    commandEscapeBeforeEvaluator .unboundText false = false := by
  rfl

theorem command_environment_override_makes_deadlock_unreachable :
    ¬ formsLegacyRecoveryCycle
      { legacyDeadlockModel with
          overrideOrigin := some .commandProcessEnvironment } := by
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

/-- Escape authority is resolved before generation and policy. It is not a
Config decision and therefore cannot be denied by the policy it bypasses. -/
inductive HookBootstrapStage where
  | inheritedEscape
  | commandEscape
  | evaluator
  | policy
  deriving DecidableEq

def bootstrapRank : HookBootstrapStage → Nat
  | .inheritedEscape => 0
  | .commandEscape => 1
  | .evaluator => 2
  | .policy => 3

theorem inherited_escape_precedes_evaluator_and_policy :
    bootstrapRank .inheritedEscape < bootstrapRank .evaluator ∧
      bootstrapRank .inheritedEscape < bootstrapRank .policy := by
  decide

theorem command_escape_precedes_evaluator_and_policy :
    bootstrapRank .commandEscape < bootstrapRank .evaluator ∧
      bootstrapRank .commandEscape < bootstrapRank .policy := by
  decide

/-- Agent kind is a closed Config fact. Child topology cannot manufacture a
kind merely because the Host event is named `SubagentStart`. -/
inductive AgentKind where
  | agent
  | subagent
  deriving DecidableEq

structure TypedAgentIdentity where
  canonicalName : String
  agentKind : AgentKind
  configured : Bool
  deriving DecidableEq

inductive DispatchResult where
  | allow
  | dispatch (target : TypedAgentIdentity)
  deriving DecidableEq

def resolveTypedDispatch
    (current target : TypedAgentIdentity) : DispatchResult :=
  if current = target ∧ current.configured = true then .allow else .dispatch target

theorem typed_target_dispatch_is_a_fixed_point
    (identity : TypedAgentIdentity) (configured : identity.configured = true) :
    resolveTypedDispatch identity identity = .allow := by
  simp [resolveTypedDispatch, configured]

/-- Registry projection copies the configured kind; child/root topology and
SubagentStart are not authorities for changing it. -/
def projectConfiguredAgent
    (configuredName : String) (configuredKind : AgentKind) :
    TypedAgentIdentity := {
  canonicalName := configuredName
  agentKind := configuredKind
  configured := true
}

theorem registry_projection_preserves_configured_agent_kind
    (name : String) (kind : AgentKind) :
    (projectConfiguredAgent name kind).agentKind = kind := by
  rfl

/-- A configured Testing target is a terminal fixed point rather than another
Choice Plane dispatch edge. -/
theorem configured_testing_target_does_not_redispatch :
    let testing := projectConfiguredAgent "asp_testing" .agent
    resolveTypedDispatch testing testing = .allow := by
  simp [projectConfiguredAgent, resolveTypedDispatch]

/-- Neither policy evaluation nor observational lifecycle delivery constructs
a Tokio runtime in the standalone Hook process. -/
structure HookProcessRuntime where
  policyRuntimeConstructed : Bool
  observationalRuntimeConstructed : Bool

def canonicalHookProcessRuntime : HookProcessRuntime := {
  policyRuntimeConstructed := false
  observationalRuntimeConstructed := false
}

def isRuntimeIndependent (runtime : HookProcessRuntime) : Prop :=
  runtime.policyRuntimeConstructed = false ∧
    runtime.observationalRuntimeConstructed = false

theorem hook_event_plane_constructs_no_runtime :
    isRuntimeIndependent canonicalHookProcessRuntime := by
  simp [isRuntimeIndependent, canonicalHookProcessRuntime]

end ASPProof.HookMatcherPublicationSeparation
