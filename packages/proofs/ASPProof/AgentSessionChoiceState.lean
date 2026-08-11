namespace ASPProof.AgentSessionChoiceState

inductive NamespaceState where
  | registrationRequired
  | registered
  | resumable
  | achieved
  | blocked
deriving DecidableEq, Repr

inductive HostLifecycleSurface where
  | available
  | unavailable
deriving DecidableEq, Repr

inductive ChoiceState where
  | registrationRequired
  | registered
  | resumable
  | achieved
  | blocked
  | hostSurfaceUnavailable
deriving DecidableEq, Repr

def resolve
    (namespaceState : NamespaceState)
    (host : HostLifecycleSurface)
    (_workspaceGeneration _providerGeneration : Option Nat) : ChoiceState :=
  match namespaceState, host with
  | .registrationRequired, .unavailable => .hostSurfaceUnavailable
  | .registrationRequired, .available => .registrationRequired
  | .registered, _ => .registered
  | .resumable, _ => .resumable
  | .achieved, _ => .achieved
  | .blocked, _ => .blocked

theorem unavailable_host_terminates_missing_namespace
    (workspaceGeneration providerGeneration : Option Nat) :
    resolve .registrationRequired .unavailable workspaceGeneration providerGeneration =
      .hostSurfaceUnavailable := by
  rfl

theorem available_host_allows_first_registration
    (workspaceGeneration providerGeneration : Option Nat) :
    resolve .registrationRequired .available workspaceGeneration providerGeneration =
      .registrationRequired := by
  rfl

theorem registered_namespace_is_generation_parametric
    (host : HostLifecycleSurface)
    (workspaceGeneration providerGeneration : Option Nat) :
    resolve .registered host workspaceGeneration providerGeneration = .registered := by
  cases host <;> rfl

theorem workspace_and_provider_generations_do_not_gate_choice
    (namespaceState : NamespaceState)
    (host : HostLifecycleSurface)
    (workspaceA workspaceB providerA providerB : Option Nat) :
    resolve namespaceState host workspaceA providerA =
      resolve namespaceState host workspaceB providerB := by
  cases namespaceState <;> cases host <;> rfl

end ASPProof.AgentSessionChoiceState
