namespace ASPProof.AgentSessionRuntimeServerBoundary

inductive Principal where
  | runtimeServer
  | agentSession
  | hostRuntime
  | residentSubagent
deriving DecidableEq, Repr

inductive Authority where
  | workspaceAdmission
  | indexWrite
  | generationPublish
  | queryLeaseIssue
  | sessionGraph
  | subagentBinding
  | dispatchReceipt
  | nativeSpawn
  | nativeDispatch
  | taskExecutionReceipt
  | sessionRegistryOpen
  | sessionRegistryWrite
deriving DecidableEq, Repr

inductive HookRoute where
  | nativeSubagent
  | parserProjection
  | reject
deriving DecidableEq, Repr

inductive Owns : Principal → Authority → Prop where
  | runtimeWorkspaceAdmission :
      Owns .runtimeServer .workspaceAdmission
  | runtimeIndexWrite :
      Owns .runtimeServer .indexWrite
  | runtimeGenerationPublish :
      Owns .runtimeServer .generationPublish
  | runtimeQueryLeaseIssue :
      Owns .runtimeServer .queryLeaseIssue
  | runtimeSessionRegistryOpen :
      Owns .runtimeServer .sessionRegistryOpen
  | runtimeSessionRegistryWrite :
      Owns .runtimeServer .sessionRegistryWrite
  | sessionGraph :
      Owns .agentSession .sessionGraph
  | sessionSubagentBinding :
      Owns .agentSession .subagentBinding
  | sessionDispatchReceipt :
      Owns .agentSession .dispatchReceipt
  | hostNativeSpawn :
      Owns .hostRuntime .nativeSpawn
  | hostNativeDispatch :
      Owns .hostRuntime .nativeDispatch
  | subagentTaskExecutionReceipt :
      Owns .residentSubagent .taskExecutionReceipt

inductive MayRequestQuery : Principal → Prop where
  | agentSession :
      MayRequestQuery .agentSession
  | residentSubagent :
      MayRequestQuery .residentSubagent

def chooseHookRoute
    (subagentBindingLive : Bool)
    (parserOwned : Bool)
    (exactBounded : Bool)
    (grammarValid : Bool)
    (languageMatches : Bool) : HookRoute :=
  if subagentBindingLive then
    .nativeSubagent
  else if parserOwned && exactBounded && grammarValid && languageMatches then
    .parserProjection
  else
    .reject

def DispatchAdmissible
    (hostAttested : Bool)
    (sessionBound : Bool) : Prop :=
  hostAttested = true ∧ sessionBound = true

inductive RegistryEndpointDescriptor where
  | missing
  | regularFile
  | other
deriving DecidableEq, Repr

inductive RegistryTransportNode where
  | missing
  | unixSocket
  | other
deriving DecidableEq, Repr

inductive RegistryRoute where
  | typedIpc
  | directOpen
  | unavailable
deriving DecidableEq, Repr

def descriptorPresent : RegistryEndpointDescriptor → Bool
  | .missing => false
  | .regularFile => true
  | .other => false

def transportNodePresent : RegistryTransportNode → Bool
  | .missing => false
  | .unixSocket => true
  | .other => false

def chooseRegistryRoute
    (runtimeOwnerProcess : Bool)
    (descriptor : RegistryEndpointDescriptor)
    (transport : RegistryTransportNode) : RegistryRoute :=
  if runtimeOwnerProcess then
    .directOpen
  else if descriptorPresent descriptor && transportNodePresent transport then
    .typedIpc
  else
    .unavailable

def registryStorageOwner (_sessionCount : Nat) : Principal :=
  .runtimeServer

def residentRegistryInstanceCount (_requestCount : Nat) : Nat :=
  1

def perRequestRegistryInstanceCount (requestCount : Nat) : Nat :=
  requestCount

theorem runtime_server_owns_index_write :
    Owns .runtimeServer .indexWrite := by
  exact .runtimeIndexWrite

theorem runtime_server_owns_generation_publication :
    Owns .runtimeServer .generationPublish := by
  exact .runtimeGenerationPublish

theorem runtime_server_owns_session_registry_open :
    Owns .runtimeServer .sessionRegistryOpen := by
  exact .runtimeSessionRegistryOpen

theorem runtime_server_owns_session_registry_write :
    Owns .runtimeServer .sessionRegistryWrite := by
  exact .runtimeSessionRegistryWrite

theorem agent_session_has_no_registry_open_authority :
    ¬ Owns .agentSession .sessionRegistryOpen := by
  intro ownership
  cases ownership

theorem resident_subagent_has_no_registry_open_authority :
    ¬ Owns .residentSubagent .sessionRegistryOpen := by
  intro ownership
  cases ownership

theorem descriptor_and_transport_require_distinct_node_kinds :
    descriptorPresent .regularFile = true ∧
      transportNodePresent .unixSocket = true := by
  constructor <;> rfl

theorem valid_descriptor_and_socket_route_non_owner_through_typed_ipc :
    chooseRegistryRoute false .regularFile .unixSocket = .typedIpc := by
  rfl

theorem non_owner_registry_route_never_direct_opens
    (descriptor : RegistryEndpointDescriptor)
    (transport : RegistryTransportNode) :
    chooseRegistryRoute false descriptor transport ≠ .directOpen := by
  cases descriptor <;> cases transport <;> decide

theorem missing_endpoint_preserves_sole_owner :
    chooseRegistryRoute false .missing .missing = .unavailable := by
  rfl

theorem multiple_sessions_preserve_one_storage_owner
    (sessionCount : Nat) :
    registryStorageOwner sessionCount = .runtimeServer := by
  rfl

theorem resident_registry_reuses_one_database_instance
    (requestCount : Nat) :
    residentRegistryInstanceCount requestCount = 1 := by
  rfl

theorem two_requests_refute_per_request_database_ownership :
    perRequestRegistryInstanceCount 2 = 2 ∧
      perRequestRegistryInstanceCount 2 ≠ residentRegistryInstanceCount 2 := by
  decide

theorem agent_session_has_no_index_write_authority :
    ¬ Owns .agentSession .indexWrite := by
  intro ownership
  cases ownership

theorem resident_subagent_has_no_generation_publish_authority :
    ¬ Owns .residentSubagent .generationPublish := by
  intro ownership
  cases ownership

theorem runtime_server_has_no_subagent_binding_authority :
    ¬ Owns .runtimeServer .subagentBinding := by
  intro ownership
  cases ownership

theorem agent_session_owns_subagent_binding :
    Owns .agentSession .subagentBinding := by
  exact .sessionSubagentBinding

theorem host_runtime_owns_native_dispatch :
    Owns .hostRuntime .nativeDispatch := by
  exact .hostNativeDispatch

theorem query_request_does_not_confer_lease_issue_authority :
    MayRequestQuery .agentSession ∧
      ¬ Owns .agentSession .queryLeaseIssue := by
  constructor
  · exact .agentSession
  · intro impossible
    cases impossible

theorem live_session_subagent_routes_natively
    (parserOwned exactBounded grammarValid languageMatches : Bool) :
    chooseHookRoute true parserOwned exactBounded grammarValid languageMatches =
      .nativeSubagent := by
  rfl

theorem invalid_recommended_next_is_rejected_without_binding
    (parserOwned exactBounded languageMatches : Bool) :
    chooseHookRoute false parserOwned exactBounded false languageMatches =
      .reject := by
  cases parserOwned <;> cases exactBounded <;> cases languageMatches <;> rfl

theorem owner_language_mismatch_is_rejected_without_binding
    (parserOwned exactBounded grammarValid : Bool) :
    chooseHookRoute false parserOwned exactBounded grammarValid false =
      .reject := by
  cases parserOwned <;> cases exactBounded <;> cases grammarValid <;> rfl

theorem dispatch_requires_host_attestation
    (sessionBound : Bool) :
    ¬ DispatchAdmissible false sessionBound := by
  intro hDispatch
  cases hDispatch.1

theorem dispatch_requires_session_binding
    (hostAttested : Bool) :
    ¬ DispatchAdmissible hostAttested false := by
  intro hDispatch
  cases hDispatch.2

end ASPProof.AgentSessionRuntimeServerBoundary
