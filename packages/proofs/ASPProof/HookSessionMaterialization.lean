import ASPProof.HookSessionLifecycle

namespace ASPProof.HookSessionMaterialization

inductive MaterializationPhase where
  | absent
  | claimed
  | spawned
  | bound
  | active
  | failed
  | retired
  deriving Repr, DecidableEq

structure MaterializationState where
  phase : MaterializationPhase
  rootSessionId : Nat
  authorityRevision : Nat
  physicalGeneration : Nat
  claimToken : Nat
  childSessionId : Nat
  messageTargetId : Nat
  registryPresent : Bool
  messageTargetBound : Bool
  heartbeatObservedRevision : Nat
  heartbeatExpiresRevision : Nat
  deriving Repr, DecidableEq

structure MaterializationProposal where
  expectedAuthorityRevision : Nat
  targetGeneration : Nat
  claimToken : Nat
  deriving Repr, DecidableEq

def CanClaim
    (state : MaterializationState)
    (proposal : MaterializationProposal) : Prop :=
  (state.phase = .absent ∨ state.phase = .failed) ∧
  proposal.expectedAuthorityRevision = state.authorityRevision ∧
  proposal.targetGeneration = state.physicalGeneration + 1 ∧
  proposal.claimToken ≠ 0

instance canClaimDecidable
    (state : MaterializationState)
    (proposal : MaterializationProposal) :
    Decidable (CanClaim state proposal) := by
  unfold CanClaim
  infer_instance

def claimMaterialization
    (state : MaterializationState)
    (proposal : MaterializationProposal)
    (_admissible : CanClaim state proposal) : MaterializationState :=
  { state with
    phase := .claimed
    authorityRevision := state.authorityRevision + 1
    physicalGeneration := proposal.targetGeneration
    claimToken := proposal.claimToken
    childSessionId := 0
    messageTargetId := 0
    registryPresent := false
    messageTargetBound := false
    heartbeatObservedRevision := 0
    heartbeatExpiresRevision := 0 }

/-- External native-host idempotency ledger. -/
structure NativeHostLedger where
  acceptedClaimToken : Option Nat
  childSessionId : Nat
  deriving Repr, DecidableEq

def spawnAtHost
    (host : NativeHostLedger)
    (claimToken proposedChildSessionId : Nat) : NativeHostLedger :=
  match host.acceptedClaimToken with
  | none =>
      { acceptedClaimToken := some claimToken
        childSessionId := proposedChildSessionId }
  | some accepted =>
      if accepted = claimToken then host else host

structure SpawnReceipt where
  rootSessionId : Nat
  claimToken : Nat
  physicalGeneration : Nat
  childSessionId : Nat
  deriving Repr, DecidableEq

def CanRecordSpawn
    (state : MaterializationState)
    (receipt : SpawnReceipt) : Prop :=
  state.phase = .claimed ∧
  receipt.rootSessionId = state.rootSessionId ∧
  receipt.claimToken = state.claimToken ∧
  receipt.physicalGeneration = state.physicalGeneration ∧
  receipt.childSessionId ≠ 0

instance canRecordSpawnDecidable
    (state : MaterializationState)
    (receipt : SpawnReceipt) :
    Decidable (CanRecordSpawn state receipt) := by
  unfold CanRecordSpawn
  infer_instance

def recordSpawn
    (state : MaterializationState)
    (receipt : SpawnReceipt)
    (_admissible : CanRecordSpawn state receipt) : MaterializationState :=
  { state with
    phase := .spawned
    childSessionId := receipt.childSessionId }

structure BindingReceipt where
  claimToken : Nat
  physicalGeneration : Nat
  childSessionId : Nat
  messageTargetId : Nat
  deriving Repr, DecidableEq

def CanBind
    (state : MaterializationState)
    (receipt : BindingReceipt) : Prop :=
  state.phase = .spawned ∧
  receipt.claimToken = state.claimToken ∧
  receipt.physicalGeneration = state.physicalGeneration ∧
  receipt.childSessionId = state.childSessionId ∧
  receipt.messageTargetId ≠ 0

instance canBindDecidable
    (state : MaterializationState)
    (receipt : BindingReceipt) :
    Decidable (CanBind state receipt) := by
  unfold CanBind
  infer_instance

def bindResident
    (state : MaterializationState)
    (receipt : BindingReceipt)
    (_admissible : CanBind state receipt) : MaterializationState :=
  { state with
    phase := .bound
    messageTargetId := receipt.messageTargetId
    registryPresent := true
    messageTargetBound := true }

structure HeartbeatReceipt where
  physicalGeneration : Nat
  observedRevision : Nat
  expiresRevision : Nat
  deriving Repr, DecidableEq

def CanActivate
    (state : MaterializationState)
    (receipt : HeartbeatReceipt) : Prop :=
  state.phase = .bound ∧
  state.registryPresent = true ∧
  state.messageTargetBound = true ∧
  receipt.physicalGeneration = state.physicalGeneration ∧
  receipt.observedRevision ≤ state.authorityRevision ∧
  state.authorityRevision ≤ receipt.expiresRevision

instance canActivateDecidable
    (state : MaterializationState)
    (receipt : HeartbeatReceipt) :
    Decidable (CanActivate state receipt) := by
  unfold CanActivate
  infer_instance

def activateResident
    (state : MaterializationState)
    (receipt : HeartbeatReceipt)
    (_admissible : CanActivate state receipt) : MaterializationState :=
  { state with
    phase := .active
    heartbeatObservedRevision := receipt.observedRevision
    heartbeatExpiresRevision := receipt.expiresRevision }

def LeaseFresh (state : MaterializationState) : Prop :=
  state.phase = .active ∧
  state.heartbeatObservedRevision ≤ state.authorityRevision ∧
  state.authorityRevision ≤ state.heartbeatExpiresRevision

instance leaseFreshDecidable (state : MaterializationState) :
    Decidable (LeaseFresh state) := by
  unfold LeaseFresh
  infer_instance

structure MaterializedDispatchReceipt where
  rootSessionId : Nat
  claimToken : Nat
  physicalGeneration : Nat
  authorityRevision : Nat
  childSessionId : Nat
  messageTargetId : Nat
  deriving Repr, DecidableEq

def CanDispatch
    (state : MaterializationState)
    (receipt : MaterializedDispatchReceipt) : Prop :=
  LeaseFresh state ∧
  state.registryPresent = true ∧
  state.messageTargetBound = true ∧
  receipt.rootSessionId = state.rootSessionId ∧
  receipt.claimToken = state.claimToken ∧
  receipt.physicalGeneration = state.physicalGeneration ∧
  receipt.authorityRevision = state.authorityRevision ∧
  receipt.childSessionId = state.childSessionId ∧
  receipt.messageTargetId = state.messageTargetId

instance canDispatchDecidable
    (state : MaterializationState)
    (receipt : MaterializedDispatchReceipt) :
    Decidable (CanDispatch state receipt) := by
  unfold CanDispatch
  infer_instance

inductive MaterializationAction where
  | claim
  | createNativeChild
  | reconcileBinding
  | observeHeartbeat
  | dispatch
  | replaceExpired
  | rejectRetired
  deriving Repr, DecidableEq

def nextMaterializationAction
    (state : MaterializationState) : MaterializationAction :=
  match state.phase with
  | .absent => .claim
  | .claimed => .createNativeChild
  | .spawned => .reconcileBinding
  | .bound => .observeHeartbeat
  | .active => if LeaseFresh state then .dispatch else .replaceExpired
  | .failed => .claim
  | .retired => .rejectRetired

def absentState : MaterializationState :=
  { phase := .absent
    rootSessionId := 100
    authorityRevision := 10
    physicalGeneration := 0
    claimToken := 0
    childSessionId := 0
    messageTargetId := 0
    registryPresent := false
    messageTargetBound := false
    heartbeatObservedRevision := 0
    heartbeatExpiresRevision := 0 }

def proposalOne : MaterializationProposal :=
  { expectedAuthorityRevision := 10
    targetGeneration := 1
    claimToken := 501 }

def competingProposal : MaterializationProposal :=
  { expectedAuthorityRevision := 10
    targetGeneration := 1
    claimToken := 502 }

def claimedState : MaterializationState :=
  claimMaterialization absentState proposalOne (by decide)

def emptyHost : NativeHostLedger :=
  { acceptedClaimToken := none
    childSessionId := 0 }

def spawnedHost : NativeHostLedger :=
  spawnAtHost emptyHost 501 200

def spawnReceipt : SpawnReceipt :=
  { rootSessionId := 100
    claimToken := 501
    physicalGeneration := 1
    childSessionId := 200 }

def spawnedState : MaterializationState :=
  recordSpawn claimedState spawnReceipt (by decide)

def bindingReceipt : BindingReceipt :=
  { claimToken := 501
    physicalGeneration := 1
    childSessionId := 200
    messageTargetId := 200 }

def boundState : MaterializationState :=
  bindResident spawnedState bindingReceipt (by decide)

def heartbeatReceipt : HeartbeatReceipt :=
  { physicalGeneration := 1
    observedRevision := 11
    expiresRevision := 12 }

def activeState : MaterializationState :=
  activateResident boundState heartbeatReceipt (by decide)

def dispatchReceipt : MaterializedDispatchReceipt :=
  { rootSessionId := 100
    claimToken := 501
    physicalGeneration := 1
    authorityRevision := 11
    childSessionId := 200
    messageTargetId := 200 }

def expiredActiveState : MaterializationState :=
  { activeState with authorityRevision := 13 }

theorem first_claim_is_admissible :
    CanClaim absentState proposalOne := by
  decide

theorem committed_claim_rejects_competing_writer :
    ¬ CanClaim claimedState competingProposal := by
  decide

theorem claim_advances_generation_and_revision :
    claimedState.physicalGeneration = 1 ∧
    claimedState.authorityRevision = 11 := by
  exact ⟨rfl, rfl⟩

theorem duplicate_host_spawn_is_idempotent :
    spawnAtHost spawnedHost 501 999 = spawnedHost := by
  decide

theorem conflicting_host_claim_cannot_replace_winner :
    spawnAtHost spawnedHost 502 300 = spawnedHost := by
  decide

theorem crash_after_host_spawn_reuses_same_child :
    (spawnAtHost spawnedHost 501 999).childSessionId = 200 := by
  decide

theorem recorded_spawn_reconciles_instead_of_respawning :
    nextMaterializationAction spawnedState = .reconcileBinding := by
  decide

theorem binding_requires_exact_generation_and_child :
    CanBind spawnedState bindingReceipt := by
  decide

theorem bound_state_requires_heartbeat :
    nextMaterializationAction boundState = .observeHeartbeat := by
  decide

theorem fresh_active_state_dispatches :
    nextMaterializationAction activeState = .dispatch := by
  decide

theorem bound_dispatch_receipt_is_admissible :
    CanDispatch activeState dispatchReceipt := by
  decide

theorem expired_heartbeat_blocks_dispatch :
    ¬ CanDispatch expiredActiveState dispatchReceipt := by
  decide

theorem expired_active_state_requires_replacement :
    nextMaterializationAction expiredActiveState = .replaceExpired := by
  decide

end ASPProof.HookSessionMaterialization
