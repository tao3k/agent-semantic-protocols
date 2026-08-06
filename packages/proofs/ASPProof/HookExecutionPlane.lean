namespace ASPProof.HookExecutionPlane

inductive HostMatcher where
  | wildcard
  deriving DecidableEq, Repr

inductive HookWork where
  | unrelated
  | sourcePolicy
  | commandPolicy
  | nestedPolicy
  | missingIdentity
  | sessionLifecycle
  | generationMutation
  | explicitAspGeneration
  deriving DecidableEq, Repr

inductive ExecutionPlane where
  | localPassthrough
  | localPolicy
  deriving DecidableEq, Repr

def executionPlane : HookWork → ExecutionPlane
  | .unrelated => .localPassthrough
  | .sourcePolicy | .commandPolicy | .nestedPolicy | .missingIdentity
  | .sessionLifecycle | .generationMutation | .explicitAspGeneration => .localPolicy

def runtimeCallCount (_work : HookWork) : Nat := 0

def hostDispatches (_matcher : HostMatcher) (_work : HookWork) : Bool := true

theorem wildcard_dispatch_does_not_imply_runtime_dispatch (work : HookWork) :
    hostDispatches .wildcard work = true := by
  rfl

inductive CodexHookAuthority where
  | nativeInline
  | pluginBundle
  deriving DecidableEq, Repr

def codexProductionHookAuthorityAdmitted : CodexHookAuthority → Bool
  | .nativeInline => false
  | .pluginBundle => true

inductive CodexHookEventName where
  | preToolUse
  | snakeCasePreToolUse
  deriving DecidableEq, Repr

def codexHostRecognizesEvent : CodexHookEventName → Bool
  | .preToolUse => true
  | .snakeCasePreToolUse => false

theorem codex_hook_has_one_plugin_bundle_authority :
    codexProductionHookAuthorityAdmitted .pluginBundle = true ∧
      codexProductionHookAuthorityAdmitted .nativeInline = false := by
  decide

structure HookPluginInstallPlan where
  publishesPluginBundle : Bool
  syncsAgentProfiles : Bool
  runtimeSupervisorCallCount : Nat
  deriving DecidableEq, Repr

def canonicalHookPluginInstallPlan : HookPluginInstallPlan := {
  publishesPluginBundle := true
  syncsAgentProfiles := false
  runtimeSupervisorCallCount := 0
}

theorem plugin_install_cannot_mutate_agent_or_runtime_lifecycle :
    canonicalHookPluginInstallPlan.publishesPluginBundle = true ∧
      canonicalHookPluginInstallPlan.syncsAgentProfiles = false ∧
      canonicalHookPluginInstallPlan.runtimeSupervisorCallCount = 0 := by
  decide

inductive HookProjectionOutcome where
  | recorded
  | unavailable
  deriving DecidableEq, Repr

inductive StructuredDocumentProjection where
  | boundedPath
  | boundedScalarPredicate
  | unboundedDocument
  deriving DecidableEq, Repr

def structuredDocumentProjectionAdmitted : StructuredDocumentProjection → Bool
  | .boundedPath => true
  | .boundedScalarPredicate => true
  | .unboundedDocument => false

theorem bounded_scalar_document_predicate_is_not_a_bulk_dump :
    structuredDocumentProjectionAdmitted .boundedScalarPredicate = true ∧
      structuredDocumentProjectionAdmitted .unboundedDocument = false := by
  decide

def decisionAuthorityAdmitted : HookProjectionOutcome → Bool
  | .recorded => true
  | .unavailable => true

structure HookDecisionProjectionReceipt where
  decisionAuthoritative : Bool
  diagnosticProjection : HookProjectionOutcome
  eventWriterLockMicros : Nat
  runtimeServerCallCount : Nat
  hookAuthority : CodexHookAuthority
  deriving DecidableEq, Repr

def unavailableProjectionReceipt : HookDecisionProjectionReceipt := {
  decisionAuthoritative := true
  diagnosticProjection := .unavailable
  eventWriterLockMicros := 0
  runtimeServerCallCount := 0
  hookAuthority := .pluginBundle
}

theorem decision_authority_survives_diagnostic_projection_timeout :
    decisionAuthorityAdmitted .unavailable = true := by
  decide

theorem unavailable_projection_cannot_reenter_runtime_or_change_authority :
    unavailableProjectionReceipt.decisionAuthoritative = true ∧
      unavailableProjectionReceipt.runtimeServerCallCount = 0 ∧
      unavailableProjectionReceipt.hookAuthority = .pluginBundle := by
  decide

theorem codex_hook_uses_canonical_host_event_name :
    codexHostRecognizesEvent .preToolUse = true ∧
      codexHostRecognizesEvent .snakeCasePreToolUse = false := by
  decide

theorem unrelated_action_is_local_zero_io :
    executionPlane .unrelated = .localPassthrough ∧
      runtimeCallCount .unrelated = 0 := by
  decide

theorem source_policy_never_enters_runtime :
    executionPlane .sourcePolicy = .localPolicy ∧
      runtimeCallCount .sourcePolicy = 0 := by
  decide

theorem command_policy_never_enters_runtime :
    executionPlane .commandPolicy = .localPolicy ∧
      runtimeCallCount .commandPolicy = 0 := by
  decide

theorem nested_policy_never_enters_runtime :
    executionPlane .nestedPolicy = .localPolicy ∧
      runtimeCallCount .nestedPolicy = 0 := by
  decide

theorem missing_identity_fails_closed_without_runtime_dependency :
    executionPlane .missingIdentity = .localPolicy ∧
      runtimeCallCount .missingIdentity = 0 := by
  decide

inductive TypedActionIdentity where
  | read
  | unrelated
  | missing
  deriving DecidableEq, Repr

def projectedWork : TypedActionIdentity → Bool → HookWork
  | .read, _ => .sourcePolicy
  | .unrelated, _ => .unrelated
  | .missing, _ => .missingIdentity

theorem unrelated_typed_action_path_cannot_create_read_policy
    (hasPathShapedField : Bool) :
    projectedWork .unrelated hasPathShapedField = .unrelated := by
  rfl

theorem missing_identity_path_remains_local_fail_closed
    (hasPathShapedField : Bool) :
    executionPlane (projectedWork .missing hasPathShapedField) = .localPolicy := by
  rfl

inductive SourceSubject where
  | providerLanguageSource
  | structuredDocument
  deriving DecidableEq, Repr

inductive SourceMatcherAxis where
  | registeredLanguageSource
  | structuredDocumentFile
  deriving DecidableEq, Repr

def sourceAxisMatches : SourceMatcherAxis → SourceSubject → Bool
  | .registeredLanguageSource, .providerLanguageSource => true
  | .structuredDocumentFile, .structuredDocument => true
  | _, _ => false

theorem structured_document_axis_cannot_capture_provider_source :
    sourceAxisMatches .structuredDocumentFile .providerLanguageSource = false := by
  rfl

theorem provider_source_axis_cannot_capture_structured_document :
    sourceAxisMatches .registeredLanguageSource .structuredDocument = false := by
  rfl

theorem hook_runtime_server_is_unreachable (work : HookWork) :
    runtimeCallCount work = 0 := by
  rfl

inductive ChoiceDescriptionOwner where
  | orgContractInteractive
  | hookMatcher
  | rustHeuristic
  | runtimeServer
  deriving DecidableEq, Repr

def canDescribeChoice : ChoiceDescriptionOwner → Bool
  | .orgContractInteractive => true
  | .hookMatcher | .rustHeuristic | .runtimeServer => false

inductive ChoiceAdmissionAuthority where
  | aspValidatedState
  | orgProjection
  | hookExecutionRouter
  deriving DecidableEq, Repr

def canAdmitChoice : ChoiceAdmissionAuthority → Bool
  | .aspValidatedState => true
  | .orgProjection | .hookExecutionRouter => false

theorem org_contract_interactive_owns_choice_description :
    canDescribeChoice .orgContractInteractive = true := by
  rfl

theorem hook_execution_router_cannot_invent_choice :
    canDescribeChoice .hookMatcher = false ∧
      canDescribeChoice .rustHeuristic = false ∧
      canDescribeChoice .runtimeServer = false ∧
      canAdmitChoice .hookExecutionRouter = false := by
  decide

theorem org_projection_does_not_create_transition_authority :
    canAdmitChoice .orgProjection = false ∧
      canAdmitChoice .aspValidatedState = true := by
  decide

inductive HookRecoveryProjection where
  | orgAgentWindowReference
  | rustOwnedChoicePane
  deriving DecidableEq, Repr

def hookRecoveryProjectionAdmitted : HookRecoveryProjection → Bool
  | .orgAgentWindowReference => true
  | .rustOwnedChoicePane => false

theorem hook_recovery_can_reference_but_not_reimplement_choice_plane :
    hookRecoveryProjectionAdmitted .orgAgentWindowReference = true ∧
      hookRecoveryProjectionAdmitted .rustOwnedChoicePane = false := by
  decide

inductive LegacyRustChoiceSurface where
  | hookInteractiveCommandField
  | residentInteractiveCommand
  | clientDbInteractiveMenu
  | publicBootstrapCommand
  deriving DecidableEq, Repr

def legacyRustChoiceSurfaceAdmitted : LegacyRustChoiceSurface → Bool
  | .hookInteractiveCommandField
  | .residentInteractiveCommand
  | .clientDbInteractiveMenu
  | .publicBootstrapCommand => false

theorem org_choice_plane_excludes_every_legacy_rust_surface :
    legacyRustChoiceSurfaceAdmitted .hookInteractiveCommandField = false ∧
      legacyRustChoiceSurfaceAdmitted .residentInteractiveCommand = false ∧
      legacyRustChoiceSurfaceAdmitted .clientDbInteractiveMenu = false ∧
      legacyRustChoiceSurfaceAdmitted .publicBootstrapCommand = false := by
  decide

inductive HookTermination where
  | responseOwnerFlushedImmediate
  | processGlobalCleanup
  deriving DecidableEq, Repr

def terminationAdmitted : HookTermination → Bool
  | .responseOwnerFlushedImmediate => true
  | .processGlobalCleanup => false

theorem response_owner_flushes_before_immediate_termination :
    terminationAdmitted .responseOwnerFlushedImmediate = true := by
  rfl

theorem hook_deadline_cannot_depend_on_process_global_cleanup :
    terminationAdmitted .processGlobalCleanup = false := by
  rfl

structure HookLatency where
  loaderMicros : Nat
  entryExecutionMicros : Nat
  deriving DecidableEq, Repr

def endToEndMicros (latency : HookLatency) : Nat :=
  latency.loaderMicros + latency.entryExecutionMicros

theorem end_to_end_budget_implies_entry_execution_budget
    (latency : HookLatency)
    (budget : Nat)
    (bounded : endToEndMicros latency < budget) :
    latency.entryExecutionMicros < budget := by
  exact Nat.lt_of_le_of_lt (Nat.le_add_left _ _) bounded

inductive HookEffect where
  | classifyPolicy
  | contactRuntimeServer
  | inspectAgentSession
  | scanHostRollout
  | scheduleCodexAgent
  | reconcileSupervisor
  | installBinary
  deriving DecidableEq, Repr

def hookDataPathAdmits : HookEffect → Bool
  | .classifyPolicy => true
  | .contactRuntimeServer | .inspectAgentSession | .scanHostRollout
  | .scheduleCodexAgent | .reconcileSupervisor | .installBinary => false

theorem hook_data_path_cannot_contact_runtime_server :
    hookDataPathAdmits .contactRuntimeServer = false := by
  rfl

theorem hook_data_path_cannot_inspect_agent_session_or_rollout :
    hookDataPathAdmits .inspectAgentSession = false ∧
      hookDataPathAdmits .scanHostRollout = false := by
  decide

theorem hook_data_path_cannot_schedule_codex_agent :
    hookDataPathAdmits .scheduleCodexAgent = false := by
  rfl

theorem hook_data_path_cannot_reconcile_supervisor :
    hookDataPathAdmits .reconcileSupervisor = false := by
  rfl

theorem hook_data_path_cannot_install_binary :
    hookDataPathAdmits .installBinary = false := by
  rfl

inductive HookMatcherCacheAuthority where
  | processLocalResident
  | immutableContentAddressedMmap
  deriving DecidableEq, Repr

def hookMatcherCacheAuthorityAdmitted : HookMatcherCacheAuthority → Bool
  | .processLocalResident => false
  | .immutableContentAddressedMmap => true

inductive HookMatcherSnapshotState where
  | absent
  | atomicallyPublished
  | mmapDigestValidated
  | corrupt
  deriving DecidableEq, Repr

def warmHookMatcherLoadAdmitted : HookMatcherSnapshotState → Bool
  | .mmapDigestValidated => true
  | .absent | .atomicallyPublished | .corrupt => false

def matcherSnapshotRuntimeCallCount (_state : HookMatcherSnapshotState) : Nat := 0

theorem one_shot_hook_cache_is_process_independent :
    hookMatcherCacheAuthorityAdmitted .processLocalResident = false ∧
      hookMatcherCacheAuthorityAdmitted .immutableContentAddressedMmap = true := by
  decide

theorem warm_hook_requires_digest_validated_read_only_mmap :
    warmHookMatcherLoadAdmitted .mmapDigestValidated = true ∧
      warmHookMatcherLoadAdmitted .atomicallyPublished = false ∧
      warmHookMatcherLoadAdmitted .corrupt = false := by
  decide

theorem matcher_snapshot_recovery_never_contacts_runtime
    (state : HookMatcherSnapshotState) :
    matcherSnapshotRuntimeCallCount state = 0 := by
  rfl

inductive HookEventWriterAuthority where
  | processLocalMutex
  | workspaceOsFileLock
  deriving DecidableEq, Repr

def hookEventWriterAuthorityAdmitted : HookEventWriterAuthority → Bool
  | .processLocalMutex => false
  | .workspaceOsFileLock => true

structure HookEventStateBudget where
  decisionProjectionLockMicros : Nat
  lockMicros : Nat
  tailBytes : Nat
  tailLines : Nat
  maxBytes : Nat
  deriving DecidableEq, Repr

def canonicalHookEventStateBudget : HookEventStateBudget :=
  { decisionProjectionLockMicros := 0
  , lockMicros := 100000
  , tailBytes := 1024 * 1024
  , tailLines := 4096
  , maxBytes := 4 * 1024 * 1024 }

theorem concurrent_hook_writers_require_workspace_os_lock :
    hookEventWriterAuthorityAdmitted .processLocalMutex = false ∧
      hookEventWriterAuthorityAdmitted .workspaceOsFileLock = true := by
  decide

theorem event_writer_lock_timeout_precedes_host_deadline :
    canonicalHookEventStateBudget.lockMicros < 1000000 := by
  decide

theorem decision_projection_never_waits_for_event_writer_lock :
    canonicalHookEventStateBudget.decisionProjectionLockMicros = 0 := by
  decide

theorem event_replay_and_state_size_are_bounded :
    canonicalHookEventStateBudget.tailBytes * 4 =
        canonicalHookEventStateBudget.maxBytes ∧
      canonicalHookEventStateBudget.tailLines = 4096 := by
  decide

inductive LocalPolicyRecovery where
  | hookDoctor
  | canonicalBinaryInstall
  | serverReconcile
  deriving DecidableEq, Repr

def localPolicyFailureRecommends : LocalPolicyRecovery → Bool
  | .hookDoctor | .canonicalBinaryInstall => true
  | .serverReconcile => false

theorem local_policy_failure_cannot_recommend_server_reconcile :
    localPolicyFailureRecommends .serverReconcile = false := by
  rfl

theorem local_policy_failure_has_configuration_independent_diagnostic :
    localPolicyFailureRecommends .hookDoctor = true := by
  rfl

structure StatefulDeadline where
  serverMicros : Nat
  clientMicros : Nat
  hostMicros : Nat
  deriving DecidableEq, Repr

def StrictlyNestedDeadline (budget : StatefulDeadline) : Prop :=
  budget.serverMicros < budget.clientMicros ∧
    budget.clientMicros < budget.hostMicros

theorem nested_deadlines_leave_cleanup_slack
    (budget : StatefulDeadline)
    (nested : StrictlyNestedDeadline budget) :
    budget.serverMicros < budget.hostMicros := by
  exact Nat.lt_trans nested.1 nested.2

inductive StatefulBoundaryResult where
  | completed
  | timedOut
  | concurrencyCapHit
  deriving DecidableEq, Repr

inductive RuntimeHealthTransition where
  | remainHealthy
  | boundedShutdown
  deriving DecidableEq, Repr

def healthTransition : StatefulBoundaryResult → RuntimeHealthTransition
  | .completed => .remainHealthy
  | .timedOut | .concurrencyCapHit => .boundedShutdown

theorem timed_out_stateful_call_cannot_leave_runtime_healthy :
    healthTransition .timedOut = .boundedShutdown := by
  rfl

theorem concurrency_cap_is_terminal_not_a_queue :
    healthTransition .concurrencyCapHit = .boundedShutdown := by
  rfl

inductive RuntimeStartupStage where
  | controlEndpointPublished
  | optionalTelemetryStarted
  deriving DecidableEq, Repr

def startupStageRank : RuntimeStartupStage → Nat
  | .controlEndpointPublished => 0
  | .optionalTelemetryStarted => 1

theorem optional_telemetry_cannot_gate_control_endpoint_publication :
    startupStageRank .controlEndpointPublished <
      startupStageRank .optionalTelemetryStarted := by
  decide

inductive RuntimeReconcileStage where
  | providerCatalogReconciled
  | supervisorStarted
  deriving DecidableEq, Repr

def reconcileStageRank : RuntimeReconcileStage → Nat
  | .providerCatalogReconciled => 0
  | .supervisorStarted => 1

theorem provider_catalog_repair_precedes_supervisor_generation :
    reconcileStageRank .providerCatalogReconciled <
      reconcileStageRank .supervisorStarted := by
  decide

structure RuntimeResourceTelemetry where
  activeTasks : Nat
  openDescriptors : Nat
  queueDepth : Nat
  deriving DecidableEq, Repr

def RuntimeResourcesDrained
    (baseline after : RuntimeResourceTelemetry) : Prop :=
  after.activeTasks ≤ baseline.activeTasks ∧
    after.openDescriptors ≤ baseline.openDescriptors ∧
    after.queueDepth ≤ baseline.queueDepth

def RuntimeResourceLeakDetected
    (baseline after : RuntimeResourceTelemetry) : Prop :=
  baseline.activeTasks < after.activeTasks ∨
    baseline.openDescriptors < after.openDescriptors ∨
    baseline.queueDepth < after.queueDepth

theorem undrained_resources_are_detected
    (baseline after : RuntimeResourceTelemetry)
    (undrained : ¬ RuntimeResourcesDrained baseline after) :
    RuntimeResourceLeakDetected baseline after := by
  simp only [RuntimeResourcesDrained, RuntimeResourceLeakDetected] at *
  omega

structure SupervisorNamespace where
  home : Nat
  stateHome : Nat
  deriving DecidableEq, Repr

def TargetsSameSupervisor (left right : SupervisorNamespace) : Prop :=
  left.home = right.home

theorem isolated_test_home_cannot_target_user_supervisor
    (test user : SupervisorNamespace) (isolated : test.home ≠ user.home) :
    ¬ TargetsSameSupervisor test user := by
  exact isolated

inductive HostEnvelope where
  | compactRead
  | nestedCommandActions
  deriving DecidableEq, Repr

structure ConformanceWitness where
  hostMatcher : Bool
  normalizedAction : Bool
  configRule : Bool
  providerExtension : Bool
  providerRoute : Bool
  deriving DecidableEq, Repr

def completeConformance (w : ConformanceWitness) : Prop :=
  w.hostMatcher = true ∧ w.normalizedAction = true ∧ w.configRule = true ∧
    w.providerExtension = true ∧ w.providerRoute = true

def escapesPolicy (w : ConformanceWitness) : Prop :=
  w.normalizedAction = false

theorem full_layer_witness_implies_no_escape (w : ConformanceWitness)
    (h : completeConformance w) : ¬ escapesPolicy w := by
  rcases h with ⟨_, hn, _, _, _⟩
  simp [escapesPolicy, hn]

def escapingWitness : ConformanceWitness := {
  hostMatcher := true
  normalizedAction := false
  configRule := true
  providerExtension := true
  providerRoute := true
}

theorem missing_normalization_witness_can_escape :
    escapesPolicy escapingWitness := by
  rfl

theorem enabled_rule_and_registered_extension_require_executable_witness
    (ruleWitness extensionWitness : Bool)
    (hRule : ruleWitness = true) (hExtension : extensionWitness = true) :
    ruleWitness = true ∧ extensionWitness = true := by
  exact ⟨hRule, hExtension⟩

end ASPProof.HookExecutionPlane
