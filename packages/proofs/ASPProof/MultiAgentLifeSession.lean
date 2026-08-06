import ASPProof.AgentSessionLifecycleProduct

namespace ASPProof.MultiAgentLifeSession

open ASPProof.AgentSessionLifecycleProduct

inductive RouteKey where
  | testing
  | explorer
  | configured (value : Nat)
  deriving DecidableEq, Repr

structure PlatformHostAgentName where
  value : Nat
  deriving DecidableEq, Repr

structure ResidentProfile where
  routeKey : RouteKey
  platformHostAgentName : PlatformHostAgentName
  profileDigest : Nat
  modelDigest : Nat
  deriving DecidableEq, Repr

structure CodexHookExecutorIdentity where
  agentId : Option Nat
  agentType : Option PlatformHostAgentName
  deriving DecidableEq, Repr

def hookExecutorMatchesProfile
    (profile : ResidentProfile)
    (identity : CodexHookExecutorIdentity) : Prop :=
  identity.agentId.isSome = true ∧
    identity.agentType = some profile.platformHostAgentName

def requiresResidentRoute
    (profile : ResidentProfile)
    (identity : CodexHookExecutorIdentity) : Prop :=
  ¬ hookExecutorMatchesProfile profile identity

structure VerifiedResidentBinding where
  agentName : PlatformHostAgentName
  profileDigest : Nat
  modelDigest : Nat
  generation : Nat
  targetVerified : Bool
  deriving DecidableEq, Repr

def bindingMatchesProfile
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding) : Bool :=
  binding.agentName == profile.platformHostAgentName &&
    binding.profileDigest == profile.profileDigest &&
    binding.modelDigest == profile.modelDigest

def durableResidentDispatchAuthorized
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding)
    (lifecycle : LifecycleProduct) : Bool :=
  binding.targetVerified &&
    bindingMatchesProfile profile binding &&
    durableDispatchAuthorized lifecycle

inductive ResidentOccupancy where
  | vacant
  | incumbent (lifecycle : LifecycleProduct)
  | replacement (lifecycle : LifecycleProduct)

structure ResidentSlot where
  profile : ResidentProfile
  occupancy : ResidentOccupancy

def residentLiveCount (slot : ResidentSlot) : Nat :=
  match slot.occupancy with
  | .vacant => 0
  | .incumbent _ => 1
  | .replacement _ => 1

structure MultiAgentLifeSession where
  rootSessionId : Nat
  residentSlots : List ResidentSlot
  temporaryAgents : List LifecycleProduct

structure ResidentRegistry where
  resolve : RouteKey → Option ResidentSlot

def installResident
    (registry : ResidentRegistry)
    (slot : ResidentSlot) : ResidentRegistry :=
  { resolve := fun routeKey =>
      if routeKey == slot.profile.routeKey then some slot else registry.resolve routeKey }

def configuredResident
    (registry : ResidentRegistry)
    (routeKey : RouteKey) : Bool :=
  (registry.resolve routeKey).isSome

def emptyRegistry : ResidentRegistry :=
  ⟨fun _ => none⟩

def registryTestingProfile : ResidentProfile :=
  ⟨.testing, ⟨100⟩, 1000, 2000⟩

def testingSlot : ResidentSlot :=
  ⟨registryTestingProfile, .vacant⟩

theorem installedTestingResidentResolvesByConfiguredRouteKey :
    (installResident emptyRegistry testingSlot).resolve registryTestingProfile.routeKey =
      some testingSlot :=
  rfl

theorem installedTestingResidentPreservesExplorerRoute :
    (installResident emptyRegistry testingSlot).resolve .explorer = none :=
  rfl

theorem unverifiedBindingCannotAuthorizeDispatch
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding)
    (lifecycle : LifecycleProduct)
    (unverified : binding.targetVerified = false) :
    durableResidentDispatchAuthorized profile binding lifecycle = false := by
  cases binding with
  | mk agentName profileDigest modelDigest generation targetVerified =>
      cases targetVerified
      · rfl
      · contradiction

def testingProfile : ResidentProfile :=
  ⟨.testing, ⟨100⟩, 1000, 2000⟩

def wrongSessionBinding : VerifiedResidentBinding :=
  ⟨⟨101⟩, 1000, 2000, 7, true⟩

def wrongPlatformTargetBinding : VerifiedResidentBinding :=
  ⟨⟨100⟩, 1001, 2000, 7, true⟩

theorem wrongSessionNameCannotAuthorizeDispatch
    (lifecycle : LifecycleProduct) :
    durableResidentDispatchAuthorized testingProfile wrongSessionBinding lifecycle = false :=
  rfl

theorem wrongPlatformTargetCannotAuthorizeDispatch
    (lifecycle : LifecycleProduct) :
    durableResidentDispatchAuthorized testingProfile wrongPlatformTargetBinding lifecycle = false :=
  rfl

theorem exactCodexHookExecutorCannotRouteToItself
    (profile : ResidentProfile)
    (agentId : Nat) :
    ¬ requiresResidentRoute profile ⟨some agentId, some profile.platformHostAgentName⟩ := by
  intro route
  exact route ⟨rfl, rfl⟩

theorem partialCodexHookExecutorCannotProveResidentIdentity
    (profile : ResidentProfile) :
    ¬ hookExecutorMatchesProfile profile ⟨none, some profile.platformHostAgentName⟩ := by
  intro identity
  cases identity.1

theorem wellFormedResidentSlotHasAtMostOneLiveGeneration
    (slot : ResidentSlot) :
    residentLiveCount slot ≤ 1 := by
  cases slot with
  | mk profile occupancy =>
      cases occupancy
      · exact Nat.zero_le 1
      · exact Nat.le_refl 1
      · exact Nat.le_refl 1

theorem temporaryAgentsDoNotConsumeResidentSlots
    (session : MultiAgentLifeSession)
    (temporary : LifecycleProduct) :
    ({ session with temporaryAgents := temporary :: session.temporaryAgents }).residentSlots =
      session.residentSlots :=
  rfl

inductive VerificationOwner where
  | clientInstallation
  | immutableResidentSnapshot
  | lifecycleTransition
  deriving DecidableEq, Repr

def unitVerificationOwnsHostSupervisor : VerificationOwner → Bool
  | .clientInstallation => false
  | .immutableResidentSnapshot => false
  | .lifecycleTransition => false

theorem unit_verification_never_owns_the_host_supervisor
    (owner : VerificationOwner) :
    unitVerificationOwnsHostSupervisor owner = false := by
  cases owner <;> rfl

inductive HookVerificationAuthority where
  | installedClientConfig
  | localHookProcess
  | runtimeStateApi
  deriving DecidableEq, Repr

def canEvaluateHookPolicy : HookVerificationAuthority → Bool
  | .localHookProcess => true
  | .installedClientConfig | .runtimeStateApi => false

theorem installation_smoke_cannot_replace_local_policy_evaluation :
    canEvaluateHookPolicy .installedClientConfig = false := by
  rfl

theorem local_hook_process_owns_policy_evaluation :
    canEvaluateHookPolicy .localHookProcess = true := by
  rfl

theorem runtime_state_api_cannot_evaluate_hook_policy :
    canEvaluateHookPolicy .runtimeStateApi = false := by
  rfl

inductive AgentIdentityAuthority where
  | platformProfileName
  | agentsConfigRegistry
  | hookConfigOverlay
  | rustLiteral
  deriving DecidableEq, Repr

def canDefineAgentIdentity : AgentIdentityAuthority → Bool
  | .platformProfileName => true
  | .agentsConfigRegistry | .hookConfigOverlay | .rustLiteral => false

theorem only_platform_profile_name_defines_agent_identity
    (authority : AgentIdentityAuthority)
    (authorized : canDefineAgentIdentity authority = true) :
    authority = .platformProfileName := by
  cases authority with
  | platformProfileName => rfl
  | agentsConfigRegistry => cases authorized
  | hookConfigOverlay => cases authorized
  | rustLiteral => cases authorized

theorem hook_config_overlay_cannot_shadow_agent_identity :
    canDefineAgentIdentity .hookConfigOverlay = false := by
  rfl

theorem rust_literal_cannot_shadow_agent_identity :
    canDefineAgentIdentity .rustLiteral = false := by
  rfl

inductive SessionLifetime where
  | resident
  | temporary
  deriving DecidableEq, Repr

def resolvedSessionLifetime
    (registered : Bool)
    (configured : SessionLifetime) : SessionLifetime :=
  if registered then configured else .temporary

def lifecycleAgentType (profile : ResidentProfile) : PlatformHostAgentName :=
  profile.platformHostAgentName

theorem registered_session_preserves_configured_lifetime
    (configured : SessionLifetime) :
    resolvedSessionLifetime true configured = configured := by
  rfl

theorem unregistered_session_cannot_inherit_resident_lifetime :
    resolvedSessionLifetime false .resident = .temporary := by
  rfl

theorem lifecycle_action_preserves_registry_platform_identity
    (profile : ResidentProfile) :
    lifecycleAgentType profile = profile.platformHostAgentName := by
  rfl

inductive AgentControlPlaneChoice where
  | callResume
  | createAndRegister
  | archiveStale
  | createAfterArchive
  | blocked
  deriving DecidableEq, Repr

structure HostLifecycleEvent where
  agentName : PlatformHostAgentName
  rootSessionId : Nat
  parentSessionId : Nat
  childSessionId : Nat
  messageTargetId : Nat
  generation : Nat
  deriving DecidableEq, Repr

structure CodexHostLifecycleIdentityEvidence where
  payloadRootSessionId : Nat
  payloadChildSessionId : Nat
  environmentThreadId : Option Nat
  rolloutRootSessionId : Option Nat
  rolloutParentThreadId : Option Nat
  parentChainReachesPayloadRoot : Bool
  deriving DecidableEq, Repr

def codexHostLifecycleIdentityVerified
    (evidence : CodexHostLifecycleIdentityEvidence) : Bool :=
  !(evidence.payloadRootSessionId == evidence.payloadChildSessionId) &&
    evidence.rolloutParentThreadId.isSome &&
    !(evidence.rolloutParentThreadId == some evidence.payloadChildSessionId) &&
    (evidence.rolloutRootSessionId == some evidence.payloadRootSessionId ||
      (evidence.rolloutRootSessionId.isNone &&
        (evidence.rolloutParentThreadId == some evidence.payloadRootSessionId ||
          evidence.parentChainReachesPayloadRoot)))

theorem missing_root_and_unverified_parent_chain_cannot_authorize_registration :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 2
          environmentThreadId := some 1
          rolloutRootSessionId := none
          rolloutParentThreadId := some 3
          parentChainReachesPayloadRoot := false } = false := by
  rfl

theorem direct_parent_edge_authorizes_missing_redundant_root :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 2
          environmentThreadId := none
          rolloutRootSessionId := none
          rolloutParentThreadId := some 1
          parentChainReachesPayloadRoot := false } = true := by
  rfl

theorem root_identity_mismatch_cannot_authorize_registration :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 3
          environmentThreadId := some 1
          rolloutRootSessionId := some 2
          rolloutParentThreadId := some 1
          parentChainReachesPayloadRoot := false } = false := by
  rfl

theorem nested_parent_is_distinct_from_root_and_still_verifies :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 3
          environmentThreadId := some 99
          rolloutRootSessionId := some 1
          rolloutParentThreadId := some 2
          parentChainReachesPayloadRoot := false } = true := by
  rfl

theorem environment_thread_is_optional_and_not_child_authority :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 3
          environmentThreadId := none
          rolloutRootSessionId := some 1
          rolloutParentThreadId := some 1
          parentChainReachesPayloadRoot := false } = true := by
  rfl

theorem verified_nested_parent_chain_authorizes_missing_redundant_root :
    codexHostLifecycleIdentityVerified
        { payloadRootSessionId := 1
          payloadChildSessionId := 3
          environmentThreadId := some 99
          rolloutRootSessionId := none
          rolloutParentThreadId := some 2
          parentChainReachesPayloadRoot := true } = true := by
  rfl

inductive HostNativeAction where
  | callResume
  | register
  | archive
  | createAfterArchive
  | blocked
  deriving DecidableEq, Repr

def requiredHostNativeAction : AgentControlPlaneChoice → HostNativeAction
  | .callResume => .callResume
  | .createAndRegister => .register
  | .archiveStale => .archive
  | .createAfterArchive => .createAfterArchive
  | .blocked => .blocked

def runtimeCanRecordRegistration : Option HostLifecycleEvent → Bool
  | some _ => true
  | none => false

theorem registration_requires_a_host_lifecycle_event :
    runtimeCanRecordRegistration none = false := by
  rfl

theorem creation_choice_uses_the_host_native_register_action :
    requiredHostNativeAction .createAndRegister = .register := by
  rfl

theorem stale_and_archived_generations_use_distinct_host_actions :
    requiredHostNativeAction .archiveStale = .archive ∧
      requiredHostNativeAction .createAfterArchive = .createAfterArchive := by
  exact ⟨rfl, rfl⟩

inductive LifecycleEventAuthority where
  | runtimeServer
  | cli
  | hook
  deriving DecidableEq, Repr

def canRecordHostLifecycleEvent : LifecycleEventAuthority → Bool
  | .runtimeServer => true
  | .cli | .hook => false

theorem only_runtime_server_records_host_lifecycle_events
    (authority : LifecycleEventAuthority)
    (authorized : canRecordHostLifecycleEvent authority = true) :
    authority = .runtimeServer := by
  cases authority with
  | runtimeServer => rfl
  | cli => cases authorized
  | hook => cases authorized

def registeredGenerationMatchesProfile
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding) : Bool :=
  bindingMatchesProfile profile binding

theorem changed_model_requires_archive_before_replacement
    (profile : ResidentProfile)
    (binding : VerifiedResidentBinding)
    (changed : (binding.modelDigest == profile.modelDigest) = false) :
    registeredGenerationMatchesProfile profile binding = false := by
  unfold registeredGenerationMatchesProfile bindingMatchesProfile
  rw [changed]
  cases binding.agentName == profile.platformHostAgentName <;>
    cases binding.profileDigest == profile.profileDigest <;> rfl

structure RegisteredAgentRole where
  registryOrdinal : Nat
  deriving DecidableEq, Repr

structure RegisteredRoleDescription where
  profileDigest : Nat
  deriving DecidableEq, Repr

inductive HostSessionIdentity where
  | codexSession (value : Nat)
  | codexThread (value : Nat)
  | claudeSession (value : Nat)
  deriving DecidableEq, Repr

structure AgentControlPlaneInvocation where
  hostSessionIdentity : HostSessionIdentity
  deriving DecidableEq, Repr

inductive HostPlatform where
  | codex
  | claude
  deriving DecidableEq, Repr

def hostPlatform : HostSessionIdentity → HostPlatform
  | .codexSession _ | .codexThread _ => .codex
  | .claudeSession _ => .claude

theorem codex_session_and_thread_identify_the_same_host
    (sessionId threadId : Nat) :
    hostPlatform (.codexSession sessionId) =
      hostPlatform (.codexThread threadId) := by
  rfl

structure HookSelectedAgentRoute where
  policyRole : RouteKey
  profile : ResidentProfile
  role : RegisteredAgentRole
  description : RegisteredRoleDescription
  rootSessionId : Nat
  commandDigest : Nat
  deriving DecidableEq, Repr

def selectedRegisteredRole
    (route : HookSelectedAgentRoute)
    (_invocation : AgentControlPlaneInvocation) : RegisteredAgentRole :=
  route.role

def selectedRoleDescription
    (route : HookSelectedAgentRoute)
    (_invocation : AgentControlPlaneInvocation) : RegisteredRoleDescription :=
  route.description

theorem hook_route_controls_registered_role_and_description
    (route : HookSelectedAgentRoute)
    (invocation : AgentControlPlaneInvocation) :
    selectedRegisteredRole route invocation = route.role ∧
      selectedRoleDescription route invocation = route.description := by
  exact ⟨rfl, rfl⟩

theorem cli_invocation_cannot_override_hook_selected_route
    (route : HookSelectedAgentRoute)
    (left right : AgentControlPlaneInvocation) :
    selectedRegisteredRole route left = selectedRegisteredRole route right ∧
      selectedRoleDescription route left = selectedRoleDescription route right := by
  exact ⟨rfl, rfl⟩

def choicePlaneRootSession
    (route : HookSelectedAgentRoute)
    (_invocation : AgentControlPlaneInvocation) : Nat :=
  route.rootSessionId

theorem process_environment_cannot_override_hook_route_root
    (route : HookSelectedAgentRoute)
    (left right : AgentControlPlaneInvocation) :
    choicePlaneRootSession route left = route.rootSessionId ∧
      choicePlaneRootSession route right = route.rootSessionId := by
  exact ⟨rfl, rfl⟩

inductive SessionLookupAuthority where
  | runtimeServer
  | cli
  | hook
  deriving DecidableEq, Repr

def canResolveSessionScope : SessionLookupAuthority → Bool
  | .runtimeServer => true
  | .cli | .hook => false

theorem only_runtime_server_resolves_session_scope
    (authority : SessionLookupAuthority)
    (authorized : canResolveSessionScope authority = true) :
    authority = .runtimeServer := by
  cases authority with
  | runtimeServer => rfl
  | cli => cases authorized
  | hook => cases authorized

inductive ProjectSessionScopeEvidence where
  | projectedProject (identity : Nat)
  | workspaceIdentity (identity : Nat)
  deriving DecidableEq, Repr

def projectSessionScopeIdentity : ProjectSessionScopeEvidence → Nat
  | .projectedProject identity | .workspaceIdentity identity => identity

theorem missing_projected_child_uses_workspace_identity
    (workspaceIdentity : Nat) :
    projectSessionScopeIdentity (.workspaceIdentity workspaceIdentity) = workspaceIdentity := by
  rfl

inductive SessionLookupTransport where
  | runtimeStatusMemory
  | workspaceOwnerSocket
  | directRegistryDatabase
  deriving DecidableEq, Repr

def admittedSessionLookupTransport : SessionLookupTransport → Bool
  | .runtimeStatusMemory => true
  | .workspaceOwnerSocket | .directRegistryDatabase => false

theorem session_lookup_uses_only_runtime_status_memory
    (transport : SessionLookupTransport)
    (admitted : admittedSessionLookupTransport transport = true) :
    transport = .runtimeStatusMemory := by
  cases transport with
  | runtimeStatusMemory => rfl
  | workspaceOwnerSocket => cases admitted
  | directRegistryDatabase => cases admitted

inductive SessionProjectionPublicationAuthority where
  | runtimeServer
  | cli
  | hook
  deriving DecidableEq, Repr

def canPublishSessionProjection : SessionProjectionPublicationAuthority → Bool
  | .runtimeServer => true
  | .cli | .hook => false

theorem only_runtime_server_publishes_session_projection
    (authority : SessionProjectionPublicationAuthority)
    (authorized : canPublishSessionProjection authority = true) :
    authority = .runtimeServer := by
  cases authority with
  | runtimeServer => rfl
  | cli => cases authorized
  | hook => cases authorized

inductive SessionProjectionField where
  | projectIdentity
  | rootSessionIdentity
  | sessionIdentity
  | configuredSessionName
  | physicalGeneration
  | lifecycleState
  | orgChoiceId
  | naturalLanguageInstruction
  | applicabilityPredicate
  | presentationDirective
  deriving DecidableEq, Repr

def isRuntimeSessionFact : SessionProjectionField → Bool
  | .projectIdentity
  | .rootSessionIdentity
  | .sessionIdentity
  | .configuredSessionName
  | .physicalGeneration
  | .lifecycleState => true
  | .orgChoiceId
  | .naturalLanguageInstruction
  | .applicabilityPredicate
  | .presentationDirective => false

theorem runtime_session_projection_excludes_choice_plan_fields
    (field : SessionProjectionField)
    (published : isRuntimeSessionFact field = true) :
    field ≠ .orgChoiceId ∧
      field ≠ .naturalLanguageInstruction ∧
      field ≠ .applicabilityPredicate ∧
      field ≠ .presentationDirective := by
  cases field with
  | projectIdentity => exact ⟨by decide, by decide, by decide, by decide⟩
  | rootSessionIdentity => exact ⟨by decide, by decide, by decide, by decide⟩
  | sessionIdentity => exact ⟨by decide, by decide, by decide, by decide⟩
  | configuredSessionName => exact ⟨by decide, by decide, by decide, by decide⟩
  | physicalGeneration => exact ⟨by decide, by decide, by decide, by decide⟩
  | lifecycleState => exact ⟨by decide, by decide, by decide, by decide⟩
  | orgChoiceId => cases published
  | naturalLanguageInstruction => cases published
  | applicabilityPredicate => cases published
  | presentationDirective => cases published

inductive AgentControlPlaneObservation where
  | registered
  | registrationRequired
  | archiveRequired
  | archived
  | unavailableAuthority
  deriving DecidableEq, Repr

inductive AgentInteractivePresentation where
  | action
  | pane
  deriving DecidableEq, Repr

structure AgentInteractiveContractRow where
  whenState : AgentControlPlaneObservation
  choice : AgentControlPlaneChoice
  presentation : AgentInteractivePresentation
  deriving DecidableEq, Repr

def orgInteractiveContractRows : List AgentInteractiveContractRow :=
  [ ⟨.registered, .callResume, .action⟩
  , ⟨.registrationRequired, .createAndRegister, .pane⟩
  , ⟨.archiveRequired, .archiveStale, .action⟩
  , ⟨.archived, .createAfterArchive, .action⟩
  , ⟨.unavailableAuthority, .blocked, .pane⟩
  ]

def admitContractRows
    (observation : AgentControlPlaneObservation)
    (rows : List AgentInteractiveContractRow) : List AgentControlPlaneChoice :=
  (rows.filter (fun row => row.whenState == observation)).map (·.choice)

def agentControlPlaneChoices (observation : AgentControlPlaneObservation) :
    List AgentControlPlaneChoice :=
  admitContractRows observation orgInteractiveContractRows

def agentControlPlanePresentations (observation : AgentControlPlaneObservation) :
    List AgentInteractivePresentation :=
  (orgInteractiveContractRows.filter (fun row => row.whenState == observation)).map
    (·.presentation)

inductive ChoicePlanAuthority where
  | orgInteractiveContract
  | rustCommand
  | runtimeServer
  deriving DecidableEq, Repr

def canDefineChoicePlan : ChoicePlanAuthority → Bool
  | .orgInteractiveContract => true
  | .rustCommand | .runtimeServer => false

theorem only_org_interactive_contract_defines_choice_plan
    (authority : ChoicePlanAuthority)
    (authorized : canDefineChoicePlan authority = true) :
    authority = .orgInteractiveContract := by
  cases authority with
  | orgInteractiveContract => rfl
  | rustCommand => cases authorized
  | runtimeServer => cases authorized

inductive SessionCommandResponsibility where
  | parseCli
  | detectHostSession
  | resolveConfiguredRoute
  | readRuntimeObservation
  | admitOrgContract
  | defineAgentFacingText
  deriving DecidableEq, Repr

def commandLayerOwns : SessionCommandResponsibility → Bool
  | .parseCli | .detectHostSession => true
  | .resolveConfiguredRoute
  | .readRuntimeObservation
  | .admitOrgContract
  | .defineAgentFacingText => false

theorem command_layer_is_only_a_cli_and_host_context_adapter :
    commandLayerOwns .resolveConfiguredRoute = false ∧
      commandLayerOwns .readRuntimeObservation = false ∧
      commandLayerOwns .admitOrgContract = false ∧
      commandLayerOwns .defineAgentFacingText = false := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem registered_resident_pane_returns_direct_call_resume :
    agentControlPlaneChoices .registered = [.callResume] := by
  rfl

theorem registered_presentation_is_contract_owned_direct_action :
    agentControlPlanePresentations .registered = [.action] := by
  rfl

theorem missing_registration_offers_only_creation :
    agentControlPlaneChoices .registrationRequired =
      [.createAndRegister] := by
  rfl

theorem missing_registration_presentations_are_contract_owned_panes :
    agentControlPlanePresentations .registrationRequired = [.pane] := by
  rfl

theorem expired_or_invalid_generation_requires_archive_before_replacement :
    agentControlPlaneChoices .archiveRequired = [.archiveStale] := by
  rfl

theorem archive_required_presentation_is_a_contract_owned_action :
    agentControlPlanePresentations .archiveRequired = [.action] := by
  rfl

theorem archived_generation_cannot_resume_and_creates_a_new_generation :
    agentControlPlaneChoices .archived = [.createAfterArchive] := by
  rfl

theorem archived_presentation_is_a_contract_owned_action :
    agentControlPlanePresentations .archived = [.action] := by
  rfl

theorem unavailable_authority_never_falls_back_to_agent_creation :
    agentControlPlaneChoices .unavailableAuthority = [.blocked] := by
  rfl

theorem unavailable_authority_presentation_is_a_contract_owned_pane :
    agentControlPlanePresentations .unavailableAuthority = [.pane] := by
  rfl

end ASPProof.MultiAgentLifeSession
