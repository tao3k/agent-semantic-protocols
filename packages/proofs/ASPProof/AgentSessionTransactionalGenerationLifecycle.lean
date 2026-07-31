namespace ASPProof.AgentSessionTransactionalGenerationLifecycle

structure ResidentSlot where
  projectId : Nat
  rootSessionId : Nat
  residentName : Nat
  deriving DecidableEq

structure ResidentProfile where
  agentType : Nat
  model : Nat
  reasoning : Nat
  deriving DecidableEq

structure GenerationReservation where
  slot : ResidentSlot
  generation : Nat
  revision : Nat
  intentKey : Nat
  profile : ResidentProfile
  deriving DecidableEq

structure HostStartReceipt where
  slot : ResidentSlot
  generation : Nat
  intentKey : Nat
  childId : Nat
  canonicalTarget : Nat
  observedProfile : ResidentProfile
  typedStart : Bool
  deriving DecidableEq

def ValidHostStart
    (reservation : GenerationReservation)
    (receipt : HostStartReceipt) : Prop :=
  receipt.slot = reservation.slot ∧
  receipt.generation = reservation.generation ∧
  receipt.intentKey = reservation.intentKey ∧
  receipt.observedProfile = reservation.profile ∧
  receipt.typedStart = true

structure BindingLease where
  slot : ResidentSlot
  generation : Nat
  childId : Nat
  canonicalTarget : Nat
  verified : Bool
  deriving DecidableEq

def ValidBinding
    (reservation : GenerationReservation)
    (start : HostStartReceipt)
    (lease : BindingLease) : Prop :=
  ValidHostStart reservation start ∧
  lease.slot = reservation.slot ∧
  lease.generation = reservation.generation ∧
  lease.childId = start.childId ∧
  lease.canonicalTarget = start.canonicalTarget ∧
  lease.verified = true

structure PresenceObservation where
  canonicalTargetPresent : Bool
  deriving DecidableEq

structure FollowupAcknowledgement where
  canonicalTarget : Nat
  acknowledged : Bool
  deriving DecidableEq

structure HostCapabilities where
  typedSpawn : Bool
  retire : Bool
  releaseCanonicalPath : Bool
  deriving DecidableEq

structure RepairObservation where
  pathOccupied : Bool
  bindingVerified : Bool
  profileMatches : Bool
  retirementReceipt : Bool
  pathReleased : Bool
  deriving DecidableEq

inductive RepairDecision where
  | keepReady
  | commitRetirementIntent
  | commitReleaseIntent
  | commitReplacementIntent
  | blockedHostCapability
  deriving DecidableEq

def classifyRepair
    (capabilities : HostCapabilities)
    (observation : RepairObservation) : RepairDecision :=
  if observation.bindingVerified && observation.profileMatches then
    .keepReady
  else if observation.pathOccupied then
    if observation.retirementReceipt then
      if capabilities.releaseCanonicalPath then .commitReleaseIntent
      else .blockedHostCapability
    else if capabilities.retire then .commitRetirementIntent
    else .blockedHostCapability
  else if capabilities.typedSpawn then
    .commitReplacementIntent
  else
    .blockedHostCapability

inductive TransactionPhase where
  | observedInvalid
  | retirementIntentDurable
  | retiredReceiptIndexed
  | releaseIntentDurable
  | pathReleased
  | replacementIntentDurable
  | startReceiptIndexed
  | bindingCommitted
  | ready
  | blockedCapability
  deriving DecidableEq

def phaseRank : TransactionPhase → Nat
  | .observedInvalid => 8
  | .retirementIntentDurable => 7
  | .retiredReceiptIndexed => 6
  | .releaseIntentDurable => 5
  | .pathReleased => 4
  | .replacementIntentDurable => 3
  | .startReceiptIndexed => 2
  | .bindingCommitted => 1
  | .ready => 0
  | .blockedCapability => 0

structure TransactionalRepairStep where
  current : TransactionPhase
  next : TransactionPhase
  rankDecreases : phaseRank next < phaseRank current

structure DispatchIntent where
  slot : ResidentSlot
  generation : Nat
  dispatchKey : Nat
  commandDigest : Nat
  deriving DecidableEq

structure HostAcceptReceipt where
  slot : ResidentSlot
  generation : Nat
  dispatchKey : Nat
  commandDigest : Nat
  accepted : Bool
  deriving DecidableEq

def AcceptsIntent (intent : DispatchIntent) (receipt : HostAcceptReceipt) : Prop :=
  receipt.slot = intent.slot ∧
  receipt.generation = intent.generation ∧
  receipt.dispatchKey = intent.dispatchKey ∧
  receipt.commandDigest = intent.commandDigest ∧
  receipt.accepted = true

theorem presence_does_not_establish_registration :
    ∃ observation : PresenceObservation,
      observation.canonicalTargetPresent = true :=
  ⟨⟨true⟩, rfl⟩

theorem acknowledgement_does_not_establish_registration :
    ∃ acknowledgement : FollowupAcknowledgement,
      acknowledgement.acknowledged = true :=
  ⟨⟨1, true⟩, rfl⟩

theorem valid_start_binds_slot
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (valid : ValidHostStart reservation receipt) :
    receipt.slot = reservation.slot :=
  valid.1

theorem valid_start_binds_generation
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (valid : ValidHostStart reservation receipt) :
    receipt.generation = reservation.generation :=
  valid.2.1

theorem valid_start_binds_intent
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (valid : ValidHostStart reservation receipt) :
    receipt.intentKey = reservation.intentKey :=
  valid.2.2.1

theorem valid_start_requires_profile_match
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (valid : ValidHostStart reservation receipt) :
    receipt.observedProfile = reservation.profile :=
  valid.2.2.2.1

theorem valid_start_requires_typed_start
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (valid : ValidHostStart reservation receipt) :
    receipt.typedStart = true :=
  valid.2.2.2.2

theorem stale_generation_rejects_start
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (stale : receipt.generation ≠ reservation.generation) :
    ¬ ValidHostStart reservation receipt := by
  intro valid
  exact stale valid.2.1

theorem profile_drift_rejects_start
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (drift : receipt.observedProfile ≠ reservation.profile) :
    ¬ ValidHostStart reservation receipt := by
  intro valid
  exact drift valid.2.2.2.1

theorem untyped_start_rejects_registration
    {reservation : GenerationReservation} {receipt : HostStartReceipt}
    (untyped : receipt.typedStart ≠ true) :
    ¬ ValidHostStart reservation receipt := by
  intro valid
  exact untyped valid.2.2.2.2

theorem valid_binding_binds_generation
    {reservation : GenerationReservation} {start : HostStartReceipt}
    {lease : BindingLease}
    (valid : ValidBinding reservation start lease) :
    lease.generation = reservation.generation :=
  valid.2.2.1

theorem valid_binding_requires_verified_lease
    {reservation : GenerationReservation} {start : HostStartReceipt}
    {lease : BindingLease}
    (valid : ValidBinding reservation start lease) :
    lease.verified = true :=
  valid.2.2.2.2.2

theorem occupied_unbound_drift_begins_retirement
    (capabilities : HostCapabilities)
    (canRetire : capabilities.retire = true) :
    classifyRepair capabilities ⟨true, false, false, false, false⟩ =
      .commitRetirementIntent := by
  cases capabilities with
  | mk typedSpawn retire releaseCanonicalPath =>
      cases retire <;> cases canRetire
      rfl

theorem occupied_unbound_without_retire_is_explicitly_blocked
    (capabilities : HostCapabilities)
    (cannotRetire : capabilities.retire = false) :
    classifyRepair capabilities ⟨true, false, false, false, false⟩ =
      .blockedHostCapability := by
  cases capabilities with
  | mk typedSpawn retire releaseCanonicalPath =>
      cases retire <;> cases cannotRetire
      rfl

theorem retired_occupied_generation_requires_release
    (capabilities : HostCapabilities)
    (canRelease : capabilities.releaseCanonicalPath = true) :
    classifyRepair capabilities ⟨true, false, false, true, false⟩ =
      .commitReleaseIntent := by
  cases capabilities with
  | mk typedSpawn retire releaseCanonicalPath =>
      cases releaseCanonicalPath <;> cases canRelease
      rfl

theorem released_slot_with_typed_spawn_creates_replacement
    (capabilities : HostCapabilities)
    (canSpawn : capabilities.typedSpawn = true) :
    classifyRepair capabilities ⟨false, false, false, true, true⟩ =
      .commitReplacementIntent := by
  cases capabilities with
  | mk typedSpawn retire releaseCanonicalPath =>
      cases typedSpawn <;> cases canSpawn
      rfl

theorem unbound_drift_never_returns_ready
    (capabilities : HostCapabilities) :
    classifyRepair capabilities ⟨true, false, false, false, false⟩ ≠
      .keepReady := by
  cases capabilities with
  | mk typedSpawn retire releaseCanonicalPath =>
      cases retire <;> intro equality <;> cases equality

theorem transactional_repair_has_no_self_loop
    (step : TransactionalRepairStep) : step.current ≠ step.next := by
  intro same
  exact (Nat.ne_of_lt step.rankDecreases) (congrArg phaseRank same).symm

theorem transactional_repair_has_no_two_cycle
    (forward backward : TransactionalRepairStep)
    (linked : forward.next = backward.current)
    (returned : backward.next = forward.current) : False := by
  have lessForward : phaseRank forward.next < phaseRank forward.current :=
    forward.rankDecreases
  have lessBackward : phaseRank forward.current < phaseRank forward.next := by
    simpa [linked, returned] using backward.rankDecreases
  exact (Nat.not_lt_of_ge (Nat.le_of_lt lessBackward)) lessForward

theorem accepted_receipt_binds_stable_dispatch_key
    {intent : DispatchIntent} {receipt : HostAcceptReceipt}
    (accepted : AcceptsIntent intent receipt) :
    receipt.dispatchKey = intent.dispatchKey :=
  accepted.2.2.1

theorem accepted_receipt_binds_generation
    {intent : DispatchIntent} {receipt : HostAcceptReceipt}
    (accepted : AcceptsIntent intent receipt) :
    receipt.generation = intent.generation :=
  accepted.2.1

theorem command_digest_conflict_rejects_acceptance
    {intent : DispatchIntent} {receipt : HostAcceptReceipt}
    (conflict : receipt.commandDigest ≠ intent.commandDigest) :
    ¬ AcceptsIntent intent receipt := by
  intro accepted
  exact conflict accepted.2.2.2.1

end ASPProof.AgentSessionTransactionalGenerationLifecycle
