namespace ASPProof.AgentSessionRegistryOwnerLifetime

inductive Caller where
  | runtimeServer
  | client
  | residentHook
  deriving DecidableEq, Repr

inductive Route where
  | localOwner
  | typedIpc
  | unavailable
  deriving DecidableEq, Repr

inductive RuntimeLane where
  | registryWriter
  | hookEvaluation
  deriving DecidableEq, Repr

structure OwnerHandle where
  holder : Caller
  holder_is_runtime_server : holder = Caller.runtimeServer

def correctedRoute
    (caller : Caller)
    (owner : Option OwnerHandle)
    (endpointReady : Bool) : Route :=
  match owner with
  | some handle =>
      if caller = handle.holder then Route.localOwner
      else if endpointReady then Route.typedIpc else Route.unavailable
  | none =>
      if endpointReady then Route.typedIpc else Route.unavailable

def legacyRoute
    (processOwnerFlag endpointReady : Bool) : Route :=
  if processOwnerFlag then Route.localOwner
  else if endpointReady then Route.typedIpc else Route.unavailable

theorem legacy_process_flag_grants_client_local_open :
    legacyRoute true true = Route.localOwner := by
  rfl

theorem client_never_uses_resident_owner_locally
    (owner : OwnerHandle)
    (endpointReady : Bool) :
    correctedRoute Caller.client (some owner) endpointReady ≠ Route.localOwner := by
  cases endpointReady <;> simp [correctedRoute, owner.holder_is_runtime_server]

theorem resident_hook_never_uses_resident_owner_locally
    (owner : OwnerHandle)
    (endpointReady : Bool) :
    correctedRoute Caller.residentHook (some owner) endpointReady ≠ Route.localOwner := by
  cases endpointReady <;> simp [correctedRoute, owner.holder_is_runtime_server]

theorem runtime_server_owner_does_not_self_route
    (owner : OwnerHandle)
    (endpointReady : Bool) :
    correctedRoute Caller.runtimeServer (some owner) endpointReady = Route.localOwner := by
  simp [correctedRoute, owner.holder_is_runtime_server]

theorem client_routes_typed_ipc_when_endpoint_ready
    (owner : OwnerHandle) :
    correctedRoute Caller.client (some owner) true = Route.typedIpc := by
  simp [correctedRoute, owner.holder_is_runtime_server]

theorem missing_owner_and_endpoint_is_unavailable :
    correctedRoute Caller.client none false = Route.unavailable := by
  rfl

theorem hook_evaluation_is_not_registry_writer :
    RuntimeLane.hookEvaluation ≠ RuntimeLane.registryWriter := by
  decide

end ASPProof.AgentSessionRegistryOwnerLifetime
