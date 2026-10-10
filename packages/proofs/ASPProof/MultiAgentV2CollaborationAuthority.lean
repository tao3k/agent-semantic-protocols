-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.MultiAgentV2CollaborationAuthority

inductive CollaborationTool where
  | spawnAgent
  | listAgents
  | followupTask
  | sendMessage
  | interruptAgent
  | waitAgent
  deriving DecidableEq, Repr

inductive HostResultKind where
  | spawnedAgent
  | liveAgents
  | emptyActivity
  | previousStatus
  | waitSummary
  deriving DecidableEq, Repr

def resultKind : CollaborationTool → HostResultKind
  | .spawnAgent => .spawnedAgent
  | .listAgents => .liveAgents
  | .followupTask => .emptyActivity
  | .sendMessage => .emptyActivity
  | .interruptAgent => .previousStatus
  | .waitAgent => .waitSummary

def startsTurn : CollaborationTool → Bool
  | .spawnAgent | .followupTask => true
  | .listAgents | .sendMessage | .interruptAgent | .waitAgent => false

def observesOnly : CollaborationTool → Bool
  | .listAgents | .waitAgent => true
  | .spawnAgent | .followupTask | .sendMessage | .interruptAgent => false

structure HostCall where
  hostNamespace : String
  tool : CollaborationTool
  deriving DecidableEq, Repr

def admitted (call : HostCall) : Bool :=
  call.hostNamespace == "collaboration"

theorem collaboration_is_the_only_codex_namespace (tool : CollaborationTool) :
    admitted ⟨"collaboration", tool⟩ = true := by
  rfl

theorem foreign_namespace_fails_closed (hostNamespace : String) (tool : CollaborationTool)
    (different : hostNamespace ≠ "collaboration") :
    admitted ⟨hostNamespace, tool⟩ = false := by
  simp [admitted, different]

theorem only_spawn_and_followup_start_a_required_turn :
    startsTurn .spawnAgent = true ∧
    startsTurn .followupTask = true ∧
    startsTurn .sendMessage = false ∧
    startsTurn .interruptAgent = false := by
  decide

theorem list_and_wait_are_observation_not_dispatch :
    observesOnly .listAgents = true ∧
    observesOnly .waitAgent = true ∧
    startsTurn .listAgents = false ∧
    startsTurn .waitAgent = false := by
  decide

theorem message_tools_have_no_invented_result_envelope :
    resultKind .sendMessage = .emptyActivity ∧
    resultKind .followupTask = .emptyActivity := by
  decide

inductive EmptyActivityEvidence where
  | none
  | hostActivityObserved
  deriving DecidableEq, Repr

def emptyActivityVerifiesTurn
    (tool : CollaborationTool)
    (evidence : EmptyActivityEvidence) : Bool :=
  startsTurn tool && evidence == .hostActivityObserved

theorem void_followup_result_alone_does_not_prove_a_turn :
    emptyActivityVerifiesTurn .followupTask .none = false := by
  rfl

theorem followup_requires_separate_host_activity_evidence :
    emptyActivityVerifiesTurn .followupTask .hostActivityObserved = true := by
  rfl

theorem send_message_never_becomes_dispatch_from_activity_evidence
    (evidence : EmptyActivityEvidence) :
    emptyActivityVerifiesTurn .sendMessage evidence = false := by
  cases evidence <;> rfl

inductive HostPathState where
  | absent
  | present
  deriving DecidableEq, Repr

inductive RegistrationState where
  | missingOrStale
  | current
  deriving DecidableEq, Repr

inductive DispatchAction where
  | spawnAgentAndRegister
  | followupTaskAndRegister
  | followupTask
  deriving DecidableEq, Repr

def chooseDispatch
    (hostPath : HostPathState)
    (registration : RegistrationState) : DispatchAction :=
  match hostPath, registration with
  | .absent, _ => .spawnAgentAndRegister
  | .present, .missingOrStale => .followupTaskAndRegister
  | .present, .current => .followupTask

theorem spawn_depends_only_on_host_path_absence
    (registration : RegistrationState) :
    chooseDispatch .absent registration = .spawnAgentAndRegister := by
  cases registration <;> rfl

theorem stale_db_registration_cannot_invent_host_presence :
    chooseDispatch .absent .missingOrStale = .spawnAgentAndRegister := by
  rfl

theorem present_path_is_reused_and_never_respawned
    (registration : RegistrationState) :
    chooseDispatch .present registration ≠ .spawnAgentAndRegister := by
  cases registration <;> decide

theorem current_binding_uses_one_status_independent_followup :
    chooseDispatch .present .current = .followupTask := by
  rfl

structure RegistrationFact where
  workspaceIdentity : String
  rootSessionId : String
  parentThreadId : String
  childThreadId : String
  canonicalAgentPath : String
  physicalGeneration : Nat
  deriving DecidableEq, Repr

structure ParentDispatchBindings where
  parentThreadId : String
  agentName : String
  canonicalAgentPath : String
  parentTask : String
  deriving DecidableEq, Repr

structure ChildEnvironmentBindings where
  rootSessionId : String
  childThreadId : String
  deriving DecidableEq, Repr

def materializeRegistrationFact
    (workspaceIdentity : String)
    (parent : ParentDispatchBindings)
    (child : ChildEnvironmentBindings)
    (physicalGeneration : Nat) : Option RegistrationFact :=
  if parent.parentThreadId.isEmpty || parent.agentName.isEmpty ||
      parent.canonicalAgentPath.isEmpty || parent.parentTask.isEmpty ||
      child.rootSessionId.isEmpty || child.childThreadId.isEmpty ||
      parent.parentThreadId == child.childThreadId || physicalGeneration = 0 then
    none
  else
    some {
      workspaceIdentity := workspaceIdentity
      rootSessionId := child.rootSessionId
      parentThreadId := parent.parentThreadId
      childThreadId := child.childThreadId
      canonicalAgentPath := parent.canonicalAgentPath
      physicalGeneration := physicalGeneration
    }

theorem parent_task_is_not_child_identity_authority
    (workspaceIdentity : String)
    (parent : ParentDispatchBindings)
    (child : ChildEnvironmentBindings)
    (physicalGeneration : Nat)
    (leftTask rightTask : String)
    (leftPresent : leftTask.isEmpty = false)
    (rightPresent : rightTask.isEmpty = false) :
    materializeRegistrationFact workspaceIdentity { parent with parentTask := leftTask }
        child physicalGeneration =
      materializeRegistrationFact workspaceIdentity { parent with parentTask := rightTask }
        child physicalGeneration := by
  simp [materializeRegistrationFact, leftPresent, rightPresent]

theorem parent_cannot_interpolate_child_identity
    (workspaceIdentity : String)
    (parent : ParentDispatchBindings)
    (physicalGeneration : Nat) :
    materializeRegistrationFact workspaceIdentity parent
      { rootSessionId := "", childThreadId := "" } physicalGeneration = none := by
  simp [materializeRegistrationFact]

def threadIdentitySeparated (fact : RegistrationFact) : Bool :=
  fact.parentThreadId != fact.childThreadId

theorem root_session_does_not_replace_routable_thread_identity
    (fact : RegistrationFact)
    (separated : fact.parentThreadId ≠ fact.childThreadId) :
    threadIdentitySeparated fact = true := by
  simp [threadIdentitySeparated, separated]

def sameChild (host runtime : RegistrationFact) : Bool := host == runtime

theorem runtime_registration_requires_the_exact_host_fact
    (host runtime : RegistrationFact) (different : host ≠ runtime) :
    sameChild host runtime = false := by
  simp [sameChild, different]

end ASPProof.MultiAgentV2CollaborationAuthority
