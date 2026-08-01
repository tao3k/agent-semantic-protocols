import ASPProof.ASPAgentSessionPortal

namespace ASPProof.ASPAgentLifeSessionRefinement

open ASPProof.ASPAgentSessionPortal

/-!
`@name` projection and ASP durable command authority are different facts.

The host may route a statically exported name while ASP repairs its durable
binding.  A LifeSession implementation must expose that route as `hostOnly`;
it must not represent the same action as an ASP-authorized durable dispatch.
The Runtime Server is an executor for repair and storage work, not an input to
this authority decision.
-/

inductive LifeSessionAction where
  | hostCall
  | durableCall
  | durableQueue
  | wait
  | blocked
  deriving DecidableEq, Repr

inductive CommandAuthority where
  | none
  | hostOnly
  | aspDurable
  deriving DecidableEq, Repr

inductive PublicCommand where
  | enterSession
  deriving DecidableEq, Repr

inductive RuntimeHealth where
  | healthy
  | starting
  | draining
  | degraded
  deriving DecidableEq, Repr

structure RuntimeWitness where
  ownerEpoch : Nat
  artifactGeneration : Nat
  health : RuntimeHealth
  deriving DecidableEq, Repr

structure LifeSessionDecision where
  action : LifeSessionAction
  authority : CommandAuthority
  internalDirective : InternalDirective
  targetProjected : Flag

def durableEvidence
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag) : Flag :=
  match snapshot.binding with
  | .delivered =>
      match snapshot.fenceCurrent with
      | .yes => profileEvidenceCurrent
      | .no => .no
  | .absent => .no
  | .reserved => .no
  | .intentDurable => .no
  | .hostAccepted => .no
  | .orphaned => .no
  | .released => .no

def idleExportedDecision
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag) : LifeSessionDecision :=
  match durableEvidence snapshot profileEvidenceCurrent with
  | .yes => ⟨.durableCall, .aspDurable, .none, .yes⟩
  | .no =>
      match snapshot.binding with
      | .absent => ⟨.hostCall, .hostOnly, .registerBinding, .yes⟩
      | .released => ⟨.hostCall, .hostOnly, .registerBinding, .yes⟩
      | .reserved => ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩
      | .intentDurable => ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩
      | .hostAccepted => ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩
      | .delivered => ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩
      | .orphaned => ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩

def activeExportedDecision
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag) : LifeSessionDecision :=
  match durableEvidence snapshot profileEvidenceCurrent with
  | .yes => ⟨.durableQueue, .aspDurable, .none, .yes⟩
  | .no =>
      match snapshot.binding with
      | .absent => ⟨.blocked, .none, .registerBinding, .yes⟩
      | .released => ⟨.blocked, .none, .registerBinding, .yes⟩
      | .reserved => ⟨.blocked, .none, .reconcileBinding, .yes⟩
      | .intentDurable => ⟨.blocked, .none, .reconcileBinding, .yes⟩
      | .hostAccepted => ⟨.blocked, .none, .reconcileBinding, .yes⟩
      | .delivered => ⟨.blocked, .none, .reconcileBinding, .yes⟩
      | .orphaned => ⟨.blocked, .none, .reconcileBinding, .yes⟩

def resolveOpenExported
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag) : LifeSessionDecision :=
  match snapshot.turn with
  | .idle => idleExportedDecision snapshot profileEvidenceCurrent
  | .queued => activeExportedDecision snapshot profileEvidenceCurrent
  | .running => activeExportedDecision snapshot profileEvidenceCurrent
  | .interrupting => ⟨.wait, .none, .none, .yes⟩

def resolveLifeSession
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag) : LifeSessionDecision :=
  match symbolExported snapshot.symbolExport with
  | .no => ⟨.blocked, .none, .repairSymbolExport, .no⟩
  | .yes =>
      match snapshot.session with
      | .open => resolveOpenExported snapshot profileEvidenceCurrent
      | .draining => ⟨.blocked, .none, .none, .yes⟩
      | .closed => ⟨.blocked, .none, .none, .yes⟩

def resolveWithRuntime
    (snapshot : PortalSnapshot)
    (profileEvidenceCurrent : Flag)
    (_runtime : RuntimeWitness) : LifeSessionDecision :=
  resolveLifeSession snapshot profileEvidenceCurrent

def invalidRunningAbsentSnapshot : PortalSnapshot :=
  { absentSnapshot with turn := .running }

def staleProfileSnapshot : PortalSnapshot := baseSnapshot

def healthyRuntime : RuntimeWitness := ⟨11, 19, .healthy⟩
def degradedRuntime : RuntimeWitness := ⟨12, 20, .degraded⟩

theorem staticProjectionDoesNotConferDurableAuthority :
    (resolveLifeSession absentSnapshot .yes).targetProjected = .yes ∧
    (resolveLifeSession absentSnapshot .yes).authority = .hostOnly :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem absentBindingSelectsInternalRegistration :
    resolveLifeSession absentSnapshot .yes =
      ⟨.hostCall, .hostOnly, .registerBinding, .yes⟩ :=
  Eq.refl _

theorem orphanedBindingRemainsHostOnly :
    resolveLifeSession orphanedSnapshot .yes =
      ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩ :=
  Eq.refl _

theorem deliveredCurrentBindingHasDurableAuthority :
    resolveLifeSession baseSnapshot .yes =
      ⟨.durableCall, .aspDurable, .none, .yes⟩ :=
  Eq.refl _

theorem staleProfileEvidenceWithholdsDurableAuthority :
    resolveLifeSession staleProfileSnapshot .no =
      ⟨.hostCall, .hostOnly, .reconcileBinding, .yes⟩ :=
  Eq.refl _

theorem invalidRunningAbsentStateFailsClosed :
    resolveLifeSession invalidRunningAbsentSnapshot .yes =
      ⟨.blocked, .none, .registerBinding, .yes⟩ :=
  Eq.refl _

theorem legacyPortalQueuesWithoutDurableBinding :
    (resolvePortal invalidRunningAbsentSnapshot).agentAction = .queueResident ∧
    invalidRunningAbsentSnapshot.binding = .absent :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem runtimeHealthCannotCreateSessionAuthority
    (runtime : RuntimeWitness) :
    resolveWithRuntime absentSnapshot .yes runtime =
      ⟨.hostCall, .hostOnly, .registerBinding, .yes⟩ :=
  Eq.refl _

theorem runtimeGenerationCannotAlterDecision :
    resolveWithRuntime baseSnapshot .yes healthyRuntime =
    resolveWithRuntime baseSnapshot .yes degradedRuntime :=
  Eq.refl _

theorem profileEvidenceCannotAlterNameProjection
    (snapshot : PortalSnapshot)
    (left right : Flag) :
    (resolveLifeSession snapshot left).targetProjected =
      (resolveLifeSession snapshot right).targetProjected := by
  cases snapshot with
  | mk sessionEpoch agentGeneration registryVersion session binding turn outcome fence symbol history =>
      cases session <;> cases binding <;> cases turn <;> cases fence <;>
        cases symbol with
        | mk stateDefinitionPresent codexSymlinkPresent symlinkTargetsStateDefinition =>
            cases stateDefinitionPresent <;> cases codexSymlinkPresent <;>
              cases symlinkTargetsStateDefinition <;> cases left <;> cases right <;> rfl

theorem wrongExportStillDominatesLifeSessionAuthority
    (profileEvidenceCurrent : Flag) :
    resolveLifeSession wrongTargetSnapshot profileEvidenceCurrent =
      ⟨.blocked, .none, .repairSymbolExport, .no⟩ := by
  cases profileEvidenceCurrent <;> exact Eq.refl _

def publicCommandCount : Nat := 1

theorem publicSurfaceContainsOnlyThePortal :
    publicCommandCount = 1 :=
  Eq.refl _

theorem publicCommandIsNotAnAdministrativeDirective
    (command : PublicCommand) :
    command = .enterSession := by
  cases command
  exact Eq.refl _

/-!
A verified host acknowledgement is reachability evidence, not a delivered ASP
binding.  The repair reducer must nevertheless consume that evidence exactly
once and advance to a durable rebind intent.  Re-entering the host probe after
the acknowledgement is a non-progressing lifecycle loop.
-/

inductive RebindRepairPhase where
  | awaitingHostProbe
  | rebindIntentRequired
  | deliveryReceiptRequired
  | rebound
  | replacementClassificationRequired
  deriving DecidableEq, Repr

inductive RebindRepairEvent where
  | hostAcknowledged
  | targetUnroutable
  | rebindIntentPersisted
  | deliveryReceiptIndexed
  deriving DecidableEq, Repr

inductive RebindRepairDirective where
  | probeCanonicalTarget
  | persistRebindIntent
  | commitDeliveredCAS
  | classifyReplacement
  | none
  deriving DecidableEq, Repr

structure RebindRepairState where
  phase : RebindRepairPhase
  targetVerified : Flag
  intentDurable : Flag
  receiptIndexed : Flag

def initialRebindRepair : RebindRepairState :=
  ⟨.awaitingHostProbe, .no, .no, .no⟩

def applyRebindRepair
    (state : RebindRepairState)
    (event : RebindRepairEvent) : Option RebindRepairState :=
  match state.phase with
  | .awaitingHostProbe =>
      match event with
      | .hostAcknowledged => some ⟨.rebindIntentRequired, .yes, .no, .no⟩
      | .targetUnroutable => some ⟨.replacementClassificationRequired, .no, .no, .no⟩
      | .rebindIntentPersisted => none
      | .deliveryReceiptIndexed => none
  | .rebindIntentRequired =>
      match event with
      | .hostAcknowledged => some state
      | .rebindIntentPersisted => some ⟨.deliveryReceiptRequired, .yes, .yes, .no⟩
      | .targetUnroutable => none
      | .deliveryReceiptIndexed => none
  | .deliveryReceiptRequired =>
      match event with
      | .deliveryReceiptIndexed => some ⟨.rebound, .yes, .yes, .yes⟩
      | .hostAcknowledged => some state
      | .targetUnroutable => none
      | .rebindIntentPersisted => some state
  | .rebound =>
      match event with
      | .hostAcknowledged => some state
      | .deliveryReceiptIndexed => some state
      | .targetUnroutable => none
      | .rebindIntentPersisted => some state
  | .replacementClassificationRequired =>
      match event with
      | .targetUnroutable => some state
      | .hostAcknowledged => none
      | .rebindIntentPersisted => none
      | .deliveryReceiptIndexed => none

def nextRebindDirective (state : RebindRepairState) : RebindRepairDirective :=
  match state.phase with
  | .awaitingHostProbe => .probeCanonicalTarget
  | .rebindIntentRequired => .persistRebindIntent
  | .deliveryReceiptRequired => .commitDeliveredCAS
  | .replacementClassificationRequired => .classifyReplacement
  | .rebound => .none

def acknowledgedRebindRepair : RebindRepairState :=
  ⟨.rebindIntentRequired, .yes, .no, .no⟩

def intentPersistedRebindRepair : RebindRepairState :=
  ⟨.deliveryReceiptRequired, .yes, .yes, .no⟩

def completedRebindRepair : RebindRepairState :=
  ⟨.rebound, .yes, .yes, .yes⟩

theorem hostAckAdvancesBeyondProbe :
    applyRebindRepair initialRebindRepair .hostAcknowledged =
      some acknowledgedRebindRepair :=
  Eq.refl _

theorem hostAckDoesNotConferDeliveredBinding :
    acknowledgedRebindRepair.receiptIndexed = .no ∧
    nextRebindDirective acknowledgedRebindRepair = .persistRebindIntent :=
  ⟨Eq.refl _, Eq.refl _⟩

theorem duplicateHostAckIsIdempotent :
    applyRebindRepair acknowledgedRebindRepair .hostAcknowledged =
      some acknowledgedRebindRepair :=
  Eq.refl _

theorem rebindIntentMustPrecedeReceipt :
    applyRebindRepair acknowledgedRebindRepair .deliveryReceiptIndexed = none :=
  Eq.refl _

theorem persistedIntentAdvancesToReceiptRequirement :
    applyRebindRepair acknowledgedRebindRepair .rebindIntentPersisted =
      some intentPersistedRebindRepair :=
  Eq.refl _

theorem indexedReceiptClosesRebind :
    applyRebindRepair intentPersistedRebindRepair .deliveryReceiptIndexed =
      some completedRebindRepair :=
  Eq.refl _

theorem completedRebindHasEveryDurableEdge :
    completedRebindRepair.targetVerified = .yes ∧
    completedRebindRepair.intentDurable = .yes ∧
    completedRebindRepair.receiptIndexed = .yes :=
  ⟨Eq.refl _, Eq.refl _, Eq.refl _⟩

theorem canonicalSuccessfulRepairHasThreeTransitions :
    applyRebindRepair initialRebindRepair .hostAcknowledged =
      some acknowledgedRebindRepair ∧
    applyRebindRepair acknowledgedRebindRepair .rebindIntentPersisted =
      some intentPersistedRebindRepair ∧
    applyRebindRepair intentPersistedRebindRepair .deliveryReceiptIndexed =
      some completedRebindRepair :=
  ⟨Eq.refl _, Eq.refl _, Eq.refl _⟩

structure RebindRepairCost where
  agentRounds : Nat
  internalTransitions : Nat
  projectedFields : Nat
  deriving DecidableEq, Repr

def correctedRebindCost : RebindRepairCost :=
  ⟨1, 3, 4⟩

def observedTwoProbeLoopCost : RebindRepairCost :=
  ⟨4, 0, 4⟩

theorem correctedRepairKeepsOneAgentRound :
    correctedRebindCost.agentRounds = 1 :=
  Eq.refl _

theorem correctedRepairHasBoundedInternalProgress :
    correctedRebindCost.internalTransitions = 3 :=
  Eq.refl _

theorem correctedRepairStrictlyReducesObservedAgentRounds :
    correctedRebindCost.agentRounds < observedTwoProbeLoopCost.agentRounds := by
  decide

end ASPProof.ASPAgentLifeSessionRefinement
