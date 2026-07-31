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

theorem runtime_server_owns_index_write :
    Owns .runtimeServer .indexWrite := by
  exact .runtimeIndexWrite

theorem runtime_server_owns_generation_publication :
    Owns .runtimeServer .generationPublish := by
  exact .runtimeGenerationPublish

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
