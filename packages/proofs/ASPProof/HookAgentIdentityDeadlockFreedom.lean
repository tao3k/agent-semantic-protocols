namespace ASPProof.HookAgentIdentityDeadlockFreedom

inductive AgentRoute where
  | explorer
  | testing
  | coding
  deriving DecidableEq

def routeName : AgentRoute → String
  | .explorer => "asp_explorer"
  | .testing => "asp_testing"
  | .coding => "asp_coding"

def canonicalPath (route : AgentRoute) : String :=
  "/root/" ++ routeName route

/-- Host-owned identity available in Codex PreToolUse and SubagentStart. -/
structure HostPreToolIdentity where
  agentId : String
  agentRole : String
  deriving DecidableEq

def bindsRoute (identity : HostPreToolIdentity) (route : AgentRoute) : Bool :=
  identity.agentId != "" &&
    identity.agentRole == routeName route

/-- Canonical path is a distinct Host lifecycle fact returned by
collaboration.list_agents. Codex does not include it in PreToolUse. -/
structure HostLifecycleIdentity where
  agentPath : String
  deriving DecidableEq

def addressesRoute (identity : HostLifecycleIdentity) (route : AgentRoute) : Bool :=
  identity.agentPath == canonicalPath route

inductive DispatchDecision where
  | allowDirect
  | spawnConfiguredAgent
  | typedIdentityFailure
  deriving DecidableEq

/-- A root call without a child identity may dispatch once. A child identity
either reaches the target fixed point or fails closed; it never dispatches
again. -/
def evaluate
    (route : AgentRoute)
    (policyWouldDeny : Bool)
    (identity : Option HostPreToolIdentity) : DispatchDecision :=
  if !policyWouldDeny then .allowDirect
  else match identity with
    | none => .spawnConfiguredAgent
    | some observed =>
        if bindsRoute observed route then .allowDirect else .typedIdentityFailure

theorem root_denial_dispatches_once (route : AgentRoute) :
    evaluate route true none = .spawnConfiguredAgent := by
  rfl

theorem exact_child_identity_is_dispatch_fixed_point
    (route : AgentRoute) (agentId : String)
    (agentPresent : agentId ≠ "") :
    evaluate route true (some {
      agentId := agentId
      agentRole := routeName route
    }) = .allowDirect := by
  simp [evaluate, bindsRoute, agentPresent]

theorem canonical_lifecycle_path_addresses_target (route : AgentRoute) :
    addressesRoute { agentPath := canonicalPath route } route = true := by
  simp [addressesRoute]

theorem wrong_child_role_fails_without_recursive_dispatch
    (route : AgentRoute) (identity : HostPreToolIdentity)
    (roleMismatch : identity.agentRole ≠ routeName route) :
    evaluate route true (some identity) = .typedIdentityFailure := by
  simp [evaluate, bindsRoute, roleMismatch]

/-- The parent identity carried by the spawn message is a typed command
argument. The explanatory prose is deliberately absent from this type. -/
structure ChildRegistrationCommand where
  parentSessionId : String
  route : AgentRoute
  deriving DecidableEq

/-- The child identity is obtained only from the Codex child process
environment when the delivered command runs. -/
structure ChildProcessIdentity where
  childSessionId : String
  parentSessionId : String
  deriving DecidableEq

structure ParentChildBinding where
  parentSessionId : String
  childSessionId : String
  route : AgentRoute
  deriving DecidableEq

/-- Child registration is a Runtime Server method carried by the canonical
ClientFrame transport. The removed raw WorkspaceDb socket cannot publish an
AgentSession binding. -/
inductive ChildRegistrationTransport where
  | grpcClientFrame
  | legacyRawWorkspaceDb
  deriving DecidableEq

def registerChild
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (_explanatoryProse : String) : Option ParentChildBinding :=
  if command.parentSessionId.isEmpty || child.childSessionId.isEmpty then none
  else if command.parentSessionId != child.parentSessionId then none
  else if command.parentSessionId == child.childSessionId then none
  else if command.route != configuredRoute then none
  else some {
    parentSessionId := command.parentSessionId
    childSessionId := child.childSessionId
    route := configuredRoute
  }

def registerChildVia
    (transport : ChildRegistrationTransport)
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (explanatoryProse : String) : Option ParentChildBinding :=
  match transport with
    | .grpcClientFrame =>
        registerChild command child configuredRoute explanatoryProse
    | .legacyRawWorkspaceDb => none

theorem legacy_raw_workspace_db_never_materializes_binding
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (prose : String) :
    registerChildVia .legacyRawWorkspaceDb command child configuredRoute prose = none := by
  rfl

theorem registration_ignores_explanatory_prose
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (left right : String) :
    registerChild command child configuredRoute left =
      registerChild command child configuredRoute right := by
  rfl

theorem exact_parent_child_command_materializes_binding
    (route : AgentRoute)
    (parent child : String)
    (parentPresent : parent.isEmpty = false)
    (childPresent : child.isEmpty = false)
    (distinct : parent ≠ child)
    (prose : String) :
    registerChild
      { parentSessionId := parent, route := route }
      { childSessionId := child, parentSessionId := parent }
      route
      prose = some {
        parentSessionId := parent
        childSessionId := child
        route := route
      } := by
  simp [registerChild, parentPresent, childPresent, distinct]

theorem grpc_client_frame_materializes_exact_binding
    (route : AgentRoute)
    (parent child : String)
    (parentPresent : parent.isEmpty = false)
    (childPresent : child.isEmpty = false)
    (distinct : parent ≠ child)
    (prose : String) :
    registerChildVia
      .grpcClientFrame
      { parentSessionId := parent, route := route }
      { childSessionId := child, parentSessionId := parent }
      route
      prose = some {
        parentSessionId := parent
        childSessionId := child
        route := route
      } := by
  simp [registerChildVia, registerChild, parentPresent, childPresent, distinct]

/-- A durable receipt is produced only after the Runtime-owned transport has
materialized one exact binding and allocated a nonzero physical generation. -/
structure ChildRegistrationReceipt where
  binding : ParentChildBinding
  transport : ChildRegistrationTransport
  physicalGeneration : Nat
  deriving DecidableEq

def publishRegistrationReceipt
    (transport : ChildRegistrationTransport)
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (prose : String)
    (physicalGeneration : Nat) : Option ChildRegistrationReceipt :=
  match transport with
    | .legacyRawWorkspaceDb => none
    | .grpcClientFrame =>
        match registerChild command child configuredRoute prose with
          | none => none
          | some binding =>
              if physicalGeneration = 0 then none
              else some {
                binding := binding
                transport := .grpcClientFrame
                physicalGeneration := physicalGeneration
              }

theorem receipt_implies_grpc_and_nonzero_generation
    (transport : ChildRegistrationTransport)
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (prose : String)
    (physicalGeneration : Nat)
    (receipt : ChildRegistrationReceipt)
    (published : publishRegistrationReceipt transport command child configuredRoute prose
      physicalGeneration = some receipt) :
    receipt.transport = .grpcClientFrame ∧ receipt.physicalGeneration ≠ 0 := by
  cases transport with
  | legacyRawWorkspaceDb =>
      simp [publishRegistrationReceipt] at published
  | grpcClientFrame =>
      cases materialized : registerChild command child configuredRoute prose with
      | none =>
          simp [publishRegistrationReceipt, materialized] at published
      | some binding =>
          by_cases zero : physicalGeneration = 0
          · simp [publishRegistrationReceipt, materialized, zero] at published
          · simp [publishRegistrationReceipt, materialized, zero] at published
            subst receipt
            exact ⟨rfl, zero⟩

theorem receipt_implies_exact_materialized_binding
    (transport : ChildRegistrationTransport)
    (command : ChildRegistrationCommand)
    (child : ChildProcessIdentity)
    (configuredRoute : AgentRoute)
    (prose : String)
    (physicalGeneration : Nat)
    (receipt : ChildRegistrationReceipt)
    (published : publishRegistrationReceipt transport command child configuredRoute prose
      physicalGeneration = some receipt) :
    registerChild command child configuredRoute prose = some receipt.binding := by
  cases transport with
  | legacyRawWorkspaceDb =>
      simp [publishRegistrationReceipt] at published
  | grpcClientFrame =>
      cases materialized : registerChild command child configuredRoute prose with
      | none =>
          simp [publishRegistrationReceipt, materialized] at published
      | some binding =>
          by_cases zero : physicalGeneration = 0
          · simp [publishRegistrationReceipt, materialized, zero] at published
          · simp [publishRegistrationReceipt, materialized, zero] at published
            subst receipt
            rfl

/-- A registry receipt is necessary evidence, but Host lifecycle presence is
still required. This prevents a stale DB row from inventing a live Agent. -/
def receiptAdmitsLiveRoute
    (receipt : ChildRegistrationReceipt)
    (lifecycle : Option HostLifecycleIdentity)
    (route : AgentRoute) : Bool :=
  match lifecycle with
    | none => false
    | some live =>
        receipt.transport == .grpcClientFrame &&
          receipt.physicalGeneration != 0 &&
          receipt.binding.route == route &&
          addressesRoute live route

theorem receipt_without_host_lifecycle_never_admits
    (receipt : ChildRegistrationReceipt)
    (route : AgentRoute) :
    receiptAdmitsLiveRoute receipt none route = false := by
  rfl

theorem wrong_host_path_never_admits
    (receipt : ChildRegistrationReceipt)
    (live : HostLifecycleIdentity)
    (route : AgentRoute)
    (wrongPath : live.agentPath ≠ canonicalPath route) :
    receiptAdmitsLiveRoute receipt (some live) route = false := by
  simp [receiptAdmitsLiveRoute, addressesRoute, wrongPath]

inductive RegistryObservation where
  | unavailable
  | absent
  | current
  | archived
  deriving DecidableEq

inductive HostAgentState where
  | absent
  | running
  | reusable
  deriving DecidableEq

inductive CollaborationOperation where
  | spawnAgentAndRegister
  | followupTaskAndRegister
  | followupTask
  deriving DecidableEq

inductive CollaborationPrimitive where
  | listAgents
  | spawnAgent
  | sendMessage
  | followupTask
  | interruptAgent
  | waitAgent
  deriving DecidableEq

def startsTurn : CollaborationPrimitive → Bool
  | .spawnAgent | .followupTask => true
  | .listAgents | .sendMessage | .interruptAgent | .waitAgent => false

def observesOnly : CollaborationPrimitive → Bool
  | .listAgents | .waitAgent => true
  | .spawnAgent | .sendMessage | .followupTask | .interruptAgent => false

def afterInterrupt : HostAgentState → HostAgentState
  | .absent => .absent
  | .running | .reusable => .reusable

theorem list_and_wait_never_start_a_turn :
    startsTurn CollaborationPrimitive.listAgents = false ∧
      startsTurn CollaborationPrimitive.waitAgent = false := by
  exact ⟨rfl, rfl⟩

theorem send_message_never_starts_a_turn :
    startsTurn .sendMessage = false := by
  rfl

theorem followup_task_starts_a_turn :
    startsTurn .followupTask = true := by
  rfl

theorem interrupt_preserves_running_agent_for_followup :
    afterInterrupt .running = .reusable := by
  rfl

def chooseCollaborationOperation
    (host : HostAgentState)
    (registry : RegistryObservation) : CollaborationOperation :=
  match host, registry with
    | .absent, _ => .spawnAgentAndRegister
    | .running, .current | .reusable, .current => .followupTask
    | .running, _ | .reusable, _ => .followupTaskAndRegister

theorem current_reusable_agent_resumes_without_registration :
    chooseCollaborationOperation .reusable .current = .followupTask := by
  rfl

theorem stale_reusable_agent_repairs_binding_without_respawn :
    chooseCollaborationOperation .reusable .archived = .followupTaskAndRegister := by
  rfl

theorem required_dispatch_is_stable_across_running_to_reusable_race
    (registry : RegistryObservation) :
    chooseCollaborationOperation .running registry =
      chooseCollaborationOperation .reusable registry := by
  cases registry <;> rfl

theorem registry_cannot_resurrect_absent_host_agent
    (registry : RegistryObservation) :
    chooseCollaborationOperation .absent registry = .spawnAgentAndRegister := by
  rfl

/-- Registry state is diagnostic and retained for queries. It cannot grant or
remove permission from the same Host-authenticated PreTool call. -/
def evaluateWithRegistry
    (route : AgentRoute)
    (policyWouldDeny : Bool)
    (identity : Option HostPreToolIdentity)
    (_registry : RegistryObservation) : DispatchDecision :=
  evaluate route policyWouldDeny identity

theorem registry_never_grants_permission
    (route : AgentRoute)
    (policyWouldDeny : Bool)
    (identity : Option HostPreToolIdentity)
    (left right : RegistryObservation) :
    evaluateWithRegistry route policyWouldDeny identity left =
      evaluateWithRegistry route policyWouldDeny identity right := by
  rfl

end ASPProof.HookAgentIdentityDeadlockFreedom
