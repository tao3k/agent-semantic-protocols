import ASPProof.ActivationRepairLedger

namespace ASPProof.HookSessionLifecycle

/-- Recovery actions are intentionally distinct from dispatch. There is no
child-facing `bootstrap` or unbound `resume` action. -/
inductive RecoveryAction where
  | rejectUnconfigured
  | materializeResident
  | reconcileRegistry
  | bindMessageTarget
  | replaceGeneration
  deriving Repr, DecidableEq

inductive HookAction where
  | recover (action : RecoveryAction)
  | dispatch
  deriving Repr, DecidableEq

structure ResidentState where
  roleConfigured : Bool
  childObserved : Bool
  registryPresent : Bool
  rootSessionMatches : Bool
  messageTargetBound : Bool
  heartbeatFresh : Bool
  physicalGeneration : Nat
  authorityGeneration : Nat
  authorityRevision : Nat
  rootSessionId : Nat
  childSessionId : Nat
  messageTargetId : Nat
  deriving Repr, DecidableEq

def MaterializedResident (state : ResidentState) : Prop :=
  state.roleConfigured = true ∧
  state.childObserved = true ∧
  state.registryPresent = true ∧
  state.rootSessionMatches = true ∧
  state.physicalGeneration = state.authorityGeneration

instance materializedResidentDecidable (state : ResidentState) :
    Decidable (MaterializedResident state) := by
  unfold MaterializedResident
  infer_instance

def ActiveResident (state : ResidentState) : Prop :=
  MaterializedResident state ∧
  state.messageTargetBound = true ∧
  state.heartbeatFresh = true

instance activeResidentDecidable (state : ResidentState) :
    Decidable (ActiveResident state) := by
  unfold ActiveResident
  infer_instance

def nextRecovery (state : ResidentState) : RecoveryAction :=
  if state.roleConfigured = false then
    .rejectUnconfigured
  else if state.childObserved = false then
    .materializeResident
  else if state.registryPresent = false then
    .reconcileRegistry
  else if state.rootSessionMatches = false then
    .replaceGeneration
  else if state.physicalGeneration != state.authorityGeneration then
    .replaceGeneration
  else if state.heartbeatFresh = false then
    .replaceGeneration
  else if state.messageTargetBound = false then
    .bindMessageTarget
  else
    .replaceGeneration

/-- Dispatch is selected only from a fully active resident. Every other state
is routed through one recovery action. -/
def nextAction (state : ResidentState) : HookAction :=
  if ActiveResident state then
    .dispatch
  else
    .recover (nextRecovery state)

/-- Native spawn materialization publishes child observation, registry
membership, message target binding, and generation identity as one logical
transition. Partial states remain recoverable but are not dispatchable. -/
def materializeResident
    (state : ResidentState)
    (childSessionId messageTargetId : Nat) : ResidentState :=
  { state with
    roleConfigured := true
    childObserved := true
    registryPresent := true
    rootSessionMatches := true
    messageTargetBound := true
    heartbeatFresh := true
    physicalGeneration := state.authorityGeneration
    childSessionId := childSessionId
    messageTargetId := messageTargetId }

/-- Replacement is a new physical generation and authority revision. -/
def replaceResident
    (state : ResidentState)
    (childSessionId messageTargetId : Nat) : ResidentState :=
  { state with
    roleConfigured := true
    childObserved := true
    registryPresent := true
    rootSessionMatches := true
    messageTargetBound := true
    heartbeatFresh := true
    physicalGeneration := state.authorityGeneration + 1
    authorityGeneration := state.authorityGeneration + 1
    authorityRevision := state.authorityRevision + 1
    childSessionId := childSessionId
    messageTargetId := messageTargetId }

structure LifecycleReceipt where
  rootSessionId : Nat
  childSessionId : Nat
  messageTargetId : Nat
  physicalGeneration : Nat
  authorityRevision : Nat
  previousDigest : Nat
  deriving Repr, DecidableEq

def ReceiptBound
    (state : ResidentState)
    (receipt : LifecycleReceipt) : Prop :=
  receipt.rootSessionId = state.rootSessionId ∧
  receipt.childSessionId = state.childSessionId ∧
  receipt.messageTargetId = state.messageTargetId ∧
  receipt.physicalGeneration = state.physicalGeneration ∧
  receipt.authorityRevision = state.authorityRevision

instance receiptBoundDecidable
    (state : ResidentState)
    (receipt : LifecycleReceipt) :
    Decidable (ReceiptBound state receipt) := by
  unfold ReceiptBound
  infer_instance

def CanDispatch
    (state : ResidentState)
    (receipt : LifecycleReceipt) : Prop :=
  ActiveResident state ∧ ReceiptBound state receipt

instance canDispatchDecidable
    (state : ResidentState)
    (receipt : LifecycleReceipt) :
    Decidable (CanDispatch state receipt) := by
  unfold CanDispatch
  infer_instance

def registeredButAbsent : ResidentState :=
  { roleConfigured := true
    childObserved := false
    registryPresent := false
    rootSessionMatches := false
    messageTargetBound := false
    heartbeatFresh := false
    physicalGeneration := 0
    authorityGeneration := 1
    authorityRevision := 10
    rootSessionId := 100
    childSessionId := 0
    messageTargetId := 0 }

def partialSpawn : ResidentState :=
  { registeredButAbsent with
    childObserved := true
    childSessionId := 200 }

def unboundResident : ResidentState :=
  { registeredButAbsent with
    childObserved := true
    registryPresent := true
    rootSessionMatches := true
    heartbeatFresh := true
    physicalGeneration := 1
    childSessionId := 200 }

def staleResident : ResidentState :=
  { unboundResident with
    messageTargetBound := true
    messageTargetId := 200
    physicalGeneration := 1
    authorityGeneration := 2 }

def activeResidentState : ResidentState :=
  materializeResident registeredButAbsent 200 200

def activeReceipt : LifecycleReceipt :=
  { rootSessionId := 100
    childSessionId := 200
    messageTargetId := 200
    physicalGeneration := 1
    authorityRevision := 10
    previousDigest := 9 }

def replacementState : ResidentState :=
  replaceResident activeResidentState 300 300

/-- The behavior being superseded: a registered role was treated as resumable
even when no child or message target existed. -/
inductive LegacyRecoveryAction where
  | resume
  deriving Repr, DecidableEq

def LegacyResumeExecutable (state : ResidentState) : Prop :=
  state.childObserved = true ∧
  state.registryPresent = true ∧
  state.messageTargetBound = true

instance legacyResumeExecutableDecidable (state : ResidentState) :
    Decidable (LegacyResumeExecutable state) := by
  unfold LegacyResumeExecutable
  infer_instance

theorem registered_absence_requires_materialization :
    nextAction registeredButAbsent =
      .recover .materializeResident := by
  decide

theorem legacy_resume_is_not_executable_for_registered_absence :
    ¬ LegacyResumeExecutable registeredButAbsent := by
  decide

theorem partial_spawn_requires_registry_reconciliation :
    nextAction partialSpawn =
      .recover .reconcileRegistry := by
  decide

theorem bound_registry_without_target_requires_target_binding :
    nextAction unboundResident =
      .recover .bindMessageTarget := by
  decide

theorem stale_generation_requires_replacement :
    nextAction staleResident =
      .recover .replaceGeneration := by
  decide

theorem materialization_produces_active_resident
    (state : ResidentState)
    (childSessionId messageTargetId : Nat) :
    ActiveResident
      (materializeResident state childSessionId messageTargetId) := by
  exact ⟨⟨rfl, rfl, rfl, rfl, rfl⟩, rfl, rfl⟩

theorem active_resident_dispatches :
    nextAction activeResidentState = .dispatch := by
  decide

theorem dispatch_action_implies_active
    (selected : nextAction state = .dispatch) :
    ActiveResident state := by
  unfold nextAction at selected
  split at selected
  next active => exact active
  next _inactive => contradiction

theorem bound_receipt_authorizes_dispatch :
    CanDispatch activeResidentState activeReceipt := by
  decide

theorem replacement_increments_generation_and_revision :
    replacementState.physicalGeneration =
        activeResidentState.physicalGeneration + 1 ∧
      replacementState.authorityGeneration =
        activeResidentState.authorityGeneration + 1 ∧
      replacementState.authorityRevision =
        activeResidentState.authorityRevision + 1 := by
  exact ⟨rfl, rfl, rfl⟩

theorem old_receipt_cannot_dispatch_replacement :
    ¬ CanDispatch replacementState activeReceipt := by
  decide

theorem materialization_eliminates_registered_absence_deadlock :
    nextAction
        (materializeResident registeredButAbsent 200 200) =
      .dispatch := by
  decide

structure ProviderExecutionState where
  manifestActivated : Bool
  activationSchemaValid : Bool
  workspaceOwnerReachable : Bool
  activeGenerationPresent : Bool
  parserRouteExecutable : Bool
  deriving Repr, DecidableEq

def ExecutableProvider (state : ProviderExecutionState) : Prop :=
  state.manifestActivated = true ∧
  state.activationSchemaValid = true ∧
  state.workspaceOwnerReachable = true ∧
  state.activeGenerationPresent = true ∧
  state.parserRouteExecutable = true

instance executableProviderDecidable (state : ProviderExecutionState) :
    Decidable (ExecutableProvider state) := by
  unfold ExecutableProvider
  infer_instance

def observedRustProvider : ProviderExecutionState :=
  { manifestActivated := true
    activationSchemaValid := false
    workspaceOwnerReachable := false
    activeGenerationPresent := false
    parserRouteExecutable := false }

structure LifecycleEcosystemState where
  resident : ResidentState
  hookRoutesToResident : Bool
  provider : ProviderExecutionState
  lifecycleReceiptPresent : Bool
  deriving Repr, DecidableEq

def EndToEndExecutable (state : LifecycleEcosystemState) : Prop :=
  ActiveResident state.resident ∧
  state.hookRoutesToResident = true ∧
  ExecutableProvider state.provider ∧
  state.lifecycleReceiptPresent = true

instance endToEndExecutableDecidable (state : LifecycleEcosystemState) :
    Decidable (EndToEndExecutable state) := by
  unfold EndToEndExecutable
  infer_instance

inductive EcosystemRecoveryAction where
  | recoverResident
  | reconcileHookRouting
  | activateProvider
  | repairActivationSchema
  | restoreWorkspaceOwner
  | materializeProviderGeneration
  | reconcileParserRoute
  | reconcileLifecycleReceipt
  deriving Repr, DecidableEq

inductive EcosystemAction where
  | recover (action : EcosystemRecoveryAction)
  | dispatchSearch
  deriving Repr, DecidableEq

def ecosystemRecovery
    (state : LifecycleEcosystemState) : EcosystemRecoveryAction :=
  if ¬ ActiveResident state.resident then
    .recoverResident
  else if state.hookRoutesToResident = false then
    .reconcileHookRouting
  else if state.provider.manifestActivated = false then
    .activateProvider
  else if state.provider.activationSchemaValid = false then
    .repairActivationSchema
  else if state.provider.workspaceOwnerReachable = false then
    .restoreWorkspaceOwner
  else if state.provider.activeGenerationPresent = false then
    .materializeProviderGeneration
  else if state.provider.parserRouteExecutable = false then
    .reconcileParserRoute
  else
    .reconcileLifecycleReceipt

def ecosystemNextAction
    (state : LifecycleEcosystemState) : EcosystemAction :=
  if EndToEndExecutable state then
    .dispatchSearch
  else
    .recover (ecosystemRecovery state)

def observedLifecycleEcosystem : LifecycleEcosystemState :=
  { resident := activeResidentState
    hookRoutesToResident := false
    provider := observedRustProvider
    lifecycleReceiptPresent := true }

def hookRoutedEcosystem : LifecycleEcosystemState :=
  { observedLifecycleEcosystem with hookRoutesToResident := true }

def schemaRepairedEcosystem : LifecycleEcosystemState :=
  { hookRoutedEcosystem with
    provider :=
      { observedRustProvider with activationSchemaValid := true } }

def ownerRestoredEcosystem : LifecycleEcosystemState :=
  { schemaRepairedEcosystem with
    provider :=
      { schemaRepairedEcosystem.provider with
        workspaceOwnerReachable := true } }

def executableLifecycleEcosystem : LifecycleEcosystemState :=
  { observedLifecycleEcosystem with
    hookRoutesToResident := true
    provider :=
      { manifestActivated := true
        activationSchemaValid := true
        workspaceOwnerReachable := true
        activeGenerationPresent := true
        parserRouteExecutable := true }
    lifecycleReceiptPresent := true }

theorem manifest_activation_does_not_imply_provider_execution :
    observedRustProvider.manifestActivated = true ∧
    ¬ ExecutableProvider observedRustProvider := by
  decide

theorem active_resident_does_not_imply_end_to_end_execution :
    ActiveResident observedLifecycleEcosystem.resident ∧
    ¬ EndToEndExecutable observedLifecycleEcosystem := by
  decide

theorem observed_ecosystem_first_repairs_hook_routing :
    ecosystemNextAction observedLifecycleEcosystem =
      .recover .reconcileHookRouting := by
  decide

theorem routed_hook_then_repairs_activation_schema :
    ecosystemNextAction hookRoutedEcosystem =
      .recover .repairActivationSchema := by
  decide

theorem repaired_schema_then_restores_workspace_owner :
    ecosystemNextAction schemaRepairedEcosystem =
      .recover .restoreWorkspaceOwner := by
  decide

theorem restored_owner_then_materializes_provider_generation :
    ecosystemNextAction ownerRestoredEcosystem =
      .recover .materializeProviderGeneration := by
  decide

theorem ecosystem_dispatch_implies_end_to_end_execution
    (selected : ecosystemNextAction state = .dispatchSearch) :
    EndToEndExecutable state := by
  unfold ecosystemNextAction at selected
  split at selected
  next executable => exact executable
  next _incomplete => contradiction

theorem fully_executable_ecosystem_dispatches :
    ecosystemNextAction executableLifecycleEcosystem =
      .dispatchSearch := by
  decide

end ASPProof.HookSessionLifecycle
