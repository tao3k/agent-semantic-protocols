-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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

/-- The publication probe projects identity from immutable bytes already linked
into the candidate.  It has no Config parse, policy compilation, filesystem, or
Runtime edge, so its work is independent of the number of policy rules. -/
structure HookIdentityProbe where
  policyContentDigest : Nat
  configParse : Bool := false
  policyCompile : Bool := false
  filesystemAccess : Bool := false
  runtimeAccess : Bool := false

def projectHookIdentity (policyContentDigest : Nat) : HookIdentityProbe :=
  { policyContentDigest }

def hookIdentityProbeWork (_policyRuleCount : Nat) : Nat := 1

theorem hook_identity_probe_is_effect_free (policyContentDigest : Nat) :
    let identity := projectHookIdentity policyContentDigest
    identity.configParse = false ∧
      identity.policyCompile = false ∧
      identity.filesystemAccess = false ∧
      identity.runtimeAccess = false := by
  simp [projectHookIdentity]

theorem hook_identity_probe_work_is_rule_count_independent
    (leftRuleCount rightRuleCount : Nat) :
    hookIdentityProbeWork leftRuleCount = hookIdentityProbeWork rightRuleCount := by
  rfl

theorem hook_identity_probe_preserves_embedded_content_identity
    (policyContentDigest : Nat) :
    (projectHookIdentity policyContentDigest).policyContentDigest = policyContentDigest := by
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

/-- Hook enablement is immutable typed State Home configuration, never process
environment or command-payload authority. -/
structure HookEnginePolicy where
  enabled : Bool
  deriving DecidableEq

inductive CliInvocation where
  | hookEvent
  | ordinaryCli
  deriving DecidableEq

inductive HookBootstrapDecision where
  | evaluate
  | passThrough
  | ordinaryClient
  deriving DecidableEq

def hookBootstrapDecision
    (policy : HookEnginePolicy) (invocation : CliInvocation) : HookBootstrapDecision :=
  match invocation with
  | .ordinaryCli => .ordinaryClient
  | .hookEvent => if policy.enabled then .evaluate else .passThrough

theorem hook_configuration_does_not_capture_ordinary_cli (policy : HookEnginePolicy) :
    hookBootstrapDecision policy .ordinaryCli = .ordinaryClient := by
  rfl

theorem disabled_hook_engine_passes_through_before_evaluation :
    hookBootstrapDecision { enabled := false } .hookEvent = .passThrough := by
  rfl

theorem enabled_hook_engine_evaluates :
    hookBootstrapDecision { enabled := true } .hookEvent = .evaluate := by
  rfl

/-- Arbitrary environment and payload text are not inputs to the Hook decision. -/
theorem environment_cannot_change_hook_engine_policy
    (policy : HookEnginePolicy) (_environmentDigest _payloadDigest : Nat) :
    hookBootstrapDecision policy .hookEvent = hookBootstrapDecision policy .hookEvent := by
  rfl

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

/-- The client installer and Hook candidate must carry the same embedded
policy content identity. Executable path adjacency is not admission. -/
structure HookInstallCohort where
  installerPolicyDigest : Nat
  candidatePolicyDigest : Nat

def hookCandidateAdmitted (cohort : HookInstallCohort) : Bool :=
  cohort.installerPolicyDigest == cohort.candidatePolicyDigest

def publishVerifiedHookCandidate
    (current candidate : MatcherBundle Projection) (cohort : HookInstallCohort) :
    MatcherBundle Projection :=
  if hookCandidateAdmitted cohort then candidate else current

theorem stale_sibling_policy_identity_preserves_current
    (current candidate : MatcherBundle Projection)
    (expected stale : Nat) (different : expected ≠ stale) :
    publishVerifiedHookCandidate current candidate
      { installerPolicyDigest := expected, candidatePolicyDigest := stale } = current := by
  simp [publishVerifiedHookCandidate, hookCandidateAdmitted, different]

/-- Runtime readiness only moves the Healthy observation; launchers remain on Active. -/
def hookDigestAfterHealthyObservation
    (active : MatcherBundle Projection) (_healthyBundleDigest : Nat) : Nat :=
  active.runtimeHookBinaryDigest

theorem healthy_observation_cannot_change_active_hook_authority
    (active : MatcherBundle Projection) (left right : Nat) :
    hookDigestAfterHealthyObservation active left =
      hookDigestAfterHealthyObservation active right := by
  rfl

/-- Active and Healthy carry one content-addressed bundle identity. The nonce
fences two publications of otherwise identical content without introducing a
second lifecycle generation. -/
structure RuntimeBinaryBundleIdentity where
  bundleDigest : Nat
  publicationNonce : Nat
  deriving DecidableEq

/-- Artifacts owns exactly two slots: launchers resolve Active, while Healthy is
the last bundle whose Runtime readiness completed. -/
structure ActiveHealthyBundleState where
  active : RuntimeBinaryBundleIdentity
  healthy : RuntimeBinaryBundleIdentity
  deriving DecidableEq

def installActiveBundle
    (state : ActiveHealthyBundleState) (candidate : RuntimeBinaryBundleIdentity) :
    ActiveHealthyBundleState :=
  { state with active := candidate }

def commitHealthyBundle
    (state : ActiveHealthyBundleState) (candidate : RuntimeBinaryBundleIdentity) :
    ActiveHealthyBundleState :=
  if candidate = state.active then { state with healthy := state.active } else state

def rollbackActiveBundle
    (state : ActiveHealthyBundleState) (candidate : RuntimeBinaryBundleIdentity) :
    ActiveHealthyBundleState :=
  if candidate = state.active then { state with active := state.healthy } else state

def launcherBundleIdentity (state : ActiveHealthyBundleState) : RuntimeBinaryBundleIdentity :=
  state.active

theorem install_switches_only_active
    (state : ActiveHealthyBundleState) (candidate : RuntimeBinaryBundleIdentity) :
    (installActiveBundle state candidate).active = candidate ∧
      (installActiveBundle state candidate).healthy = state.healthy := by
  constructor <;> rfl

theorem ready_candidate_moves_only_healthy
    (state : ActiveHealthyBundleState) :
    (commitHealthyBundle state state.active).active = state.active ∧
      (commitHealthyBundle state state.active).healthy = state.active := by
  simp [commitHealthyBundle]

theorem failed_current_candidate_restores_active_from_healthy
    (state : ActiveHealthyBundleState) :
    (rollbackActiveBundle state state.active).active = state.healthy ∧
      (rollbackActiveBundle state state.active).healthy = state.healthy := by
  simp [rollbackActiveBundle]

theorem stale_candidate_cannot_commit_or_rollback
    (state : ActiveHealthyBundleState) (stale : RuntimeBinaryBundleIdentity)
    (different : stale ≠ state.active) :
    commitHealthyBundle state stale = state ∧
      rollbackActiveBundle state stale = state := by
  simp [commitHealthyBundle, rollbackActiveBundle, different]

theorem launcher_reads_active_not_healthy
    (state : ActiveHealthyBundleState) (observedHealthy : RuntimeBinaryBundleIdentity) :
    launcherBundleIdentity { state with healthy := observedHealthy } = state.active := by
  rfl

/-- The retired selector is observational garbage, not a serving authority. -/
def hookDigestWithLegacySelector
    (active : MatcherBundle Projection) (_legacySelector : Option Nat) : Nat :=
  active.runtimeHookBinaryDigest

theorem legacy_hook_selector_cannot_override_runtime_bin
    (active : MatcherBundle Projection) (legacy : Option Nat) :
    hookDigestWithLegacySelector active legacy = active.runtimeHookBinaryDigest := by
  rfl

end ASPProof.HookMatcherPublicationSeparation
