import ASPProof.MultiAgentLifeSession
import ASPProof.MultiAgentHostIdentityBinding

namespace ASPProof.MultiAgentChoicePlane

open ASPProof.MultiAgentLifeSession

structure RuntimeObservation where
  workspaceId : Nat
  rootSessionId : Option Nat
  expectedBinding : ExactAgentBinding
  resident : Option ResidentGenerationFact
  runtimeAvailable : Bool
  deriving DecidableEq, Repr

def residentMatchesExpected
    (observation : RuntimeObservation)
    (resident : ResidentGenerationFact) : Bool :=
  registrationMatches observation.expectedBinding resident

def observedNode (observation : RuntimeObservation) : AgentControlPlaneObservation :=
  if !observation.runtimeAvailable || observation.rootSessionId.isNone then
    .unavailableAuthority
  else
    match observation.resident with
    | none => .registrationRequired
    | some resident =>
        if resident.lifecycle == .archived then
          .archived
        else if residentMatchesExpected observation resident then
          .registered
        else
          .archiveRequired

inductive ChoicePlaneAction where
  | lifecycle (choice : AgentControlPlaneChoice)
  | inspectEvidence (factId : Nat)
  deriving DecidableEq, Repr

structure ChoicePlaneContractRow where
  rowId : Nat
  whenNode : AgentControlPlaneObservation
  action : ChoicePlaneAction
  presentation : AgentInteractivePresentation
  deriving DecidableEq, Repr

def admittedRows
    (rows : List ChoicePlaneContractRow)
    (observation : RuntimeObservation) : List ChoicePlaneContractRow :=
  rows.filter (fun row => row.whenNode == observedNode observation)

def admittedRowsAtNode
    (rows : List ChoicePlaneContractRow)
    (node : AgentControlPlaneObservation) : List ChoicePlaneContractRow :=
  rows.filter (fun row => row.whenNode == node)

def registeredObservation : RuntimeObservation :=
  { workspaceId := 1
    rootSessionId := some 10
    expectedBinding :=
      { host := ⟨10, 11, 12⟩
        asp := ⟨40, 41⟩
        residentId := .aspExplorer
        routeDigest := 13
        profileDigest := 20
        modelDigest := 30
        sandboxMode := .readOnly
        generation := 1 }
    resident := some ⟨
      { host := ⟨10, 11, 12⟩
        asp := ⟨40, 41⟩
        residentId := .aspExplorer
        routeDigest := 13
        profileDigest := 20
        modelDigest := 30
        sandboxMode := .readOnly
        generation := 1 },
      .live,
      true⟩
    runtimeAvailable := true }

def multiChoiceWitnessRows : List ChoicePlaneContractRow :=
  [ ⟨1, .registered, .lifecycle .callResume, .action⟩
  , ⟨2, .registered, .inspectEvidence 99, .pane⟩
  ]

theorem applicability_model_allows_multiple_current_choices :
    admittedRows multiChoiceWitnessRows registeredObservation = multiChoiceWitnessRows := by
  rfl

def currentLifecycleRows : List ChoicePlaneContractRow :=
  [ ⟨1, .registered, .lifecycle .callResume, .action⟩
  , ⟨2, .registrationRequired, .lifecycle .createAndRegister, .pane⟩
  , ⟨3, .archiveRequired, .lifecycle .archiveStale, .action⟩
  , ⟨4, .archived, .lifecycle .createAfterArchive, .action⟩
  , ⟨5, .unavailableAuthority, .lifecycle .blocked, .pane⟩
  ]

def expectedCurrentActions : AgentControlPlaneObservation → List ChoicePlaneAction
  | .registered => [.lifecycle .callResume]
  | .registrationRequired => [.lifecycle .createAndRegister]
  | .archiveRequired => [.lifecycle .archiveStale]
  | .archived => [.lifecycle .createAfterArchive]
  | .unavailableAuthority => [.lifecycle .blocked]

theorem current_contract_admission_is_sound_and_complete
    (node : AgentControlPlaneObservation) :
    (admittedRowsAtNode currentLifecycleRows node).map (·.action) =
      expectedCurrentActions node := by
  cases node <;> rfl

def missingResidentObservation : RuntimeObservation :=
  { registeredObservation with resident := none }

theorem missing_resident_exposes_creation_without_speculative_resume :
    (admittedRows currentLifecycleRows missingResidentObservation).map (·.action) =
      [.lifecycle .createAndRegister] := by
  rfl

theorem registered_resident_exposes_call_resume_without_registration :
    (admittedRows currentLifecycleRows registeredObservation).map (·.action) =
      [.lifecycle .callResume] := by
  rfl

theorem registered_and_missing_resident_actions_are_distinct :
    (admittedRows currentLifecycleRows registeredObservation).map (·.action) ≠
      (admittedRows currentLifecycleRows missingResidentObservation).map (·.action) := by
  decide

theorem unavailable_runtime_cannot_be_registered :
    observedNode { registeredObservation with runtimeAvailable := false } =
      .unavailableAuthority := by
  rfl

def mismatchedResidentObservation : RuntimeObservation :=
  { registeredObservation with
      resident := some ⟨
        { registeredObservation.expectedBinding with profileDigest := 21 },
        .live,
        false⟩ }

theorem mismatched_resident_exposes_archive_before_replacement :
    (admittedRows currentLifecycleRows mismatchedResidentObservation).map (·.action) =
      [.lifecycle .archiveStale] := by
  rfl

def archivedResidentObservation : RuntimeObservation :=
  { registeredObservation with
      resident := some ⟨registeredObservation.expectedBinding, .archived, false⟩ }

theorem archived_resident_exposes_only_new_generation_creation :
    (admittedRows currentLifecycleRows archivedResidentObservation).map (·.action) =
      [.lifecycle .createAfterArchive] := by
  rfl

theorem registered_node_implies_exact_live_binding
    {observation : RuntimeObservation}
    {resident : ResidentGenerationFact}
    (residentPresent : observation.resident = some resident)
    (registeredNode : observedNode observation = .registered) :
    registeredBinding observation.expectedBinding resident := by
  unfold observedNode at registeredNode
  split at registeredNode <;> try contradiction
  rw [residentPresent] at registeredNode
  simp only at registeredNode
  split at registeredNode <;> try contradiction
  split at registeredNode <;> try contradiction
  rename_i bindingMatches
  exact of_decide_eq_true bindingMatches

structure ArchiveRequest where
  resident : ResidentGenerationFact
  deriving DecidableEq, Repr

def archiveRequest (observation : RuntimeObservation) : Option ArchiveRequest :=
  if observedNode observation == AgentControlPlaneObservation.archiveRequired then
    observation.resident.map ArchiveRequest.mk
  else
    none

theorem missing_resident_cannot_form_archive_request :
    archiveRequest missingResidentObservation = none := by
  rfl

theorem archive_request_carries_the_identified_resident :
    archiveRequest mismatchedResidentObservation =
      some ⟨⟨
        { registeredObservation.expectedBinding with profileDigest := 21 },
        .live,
        false⟩⟩ := by
  rfl

structure RecreateGenerationEvidence where
  archivedResidentId : Nat
  archivedGeneration : Nat
  newResidentId : Nat
  newGeneration : Nat
  deriving DecidableEq, Repr

def recreateGenerationValid (evidence : RecreateGenerationEvidence) : Prop :=
  evidence.archivedResidentId ≠ evidence.newResidentId ∧
    evidence.archivedGeneration < evidence.newGeneration

theorem recreate_requires_distinct_identity_and_higher_generation
    (evidence : RecreateGenerationEvidence)
    (valid : recreateGenerationValid evidence) :
    evidence.archivedResidentId ≠ evidence.newResidentId ∧
      evidence.archivedGeneration < evidence.newGeneration :=
  valid

structure ChoicePlaneCursor where
  workspaceId : Nat
  rootSessionId : Option Nat
  generation : Nat
  node : AgentControlPlaneObservation
  deriving DecidableEq, Repr

structure ChoicePlaneSnapshot where
  observation : RuntimeObservation
  cursor : ChoicePlaneCursor
  choices : List ChoicePlaneContractRow
  deriving DecidableEq, Repr

def snapshot
    (rows : List ChoicePlaneContractRow)
    (observation : RuntimeObservation) : ChoicePlaneSnapshot :=
  let generation := observation.resident.map (·.binding.generation) |>.getD 0
  { observation
    cursor :=
      { workspaceId := observation.workspaceId
        rootSessionId := observation.rootSessionId
        generation
        node := observedNode observation }
    choices := admittedRows rows observation }

def reenter
    (rows : List ChoicePlaneContractRow)
    (_history : List ChoicePlaneCursor)
    (observation : RuntimeObservation) : ChoicePlaneSnapshot :=
  snapshot rows observation

theorem reentry_uses_current_observation_without_replaying_history
    (rows : List ChoicePlaneContractRow)
    (leftHistory rightHistory : List ChoicePlaneCursor)
    (observation : RuntimeObservation) :
    reenter rows leftHistory observation = reenter rows rightHistory observation := by
  rfl

structure HostChoiceRequest where
  sourceCursor : ChoicePlaneCursor
  action : ChoicePlaneAction
  deriving DecidableEq, Repr

def requestHostAction
    (plane : ChoicePlaneSnapshot)
    (row : ChoicePlaneContractRow) : HostChoiceRequest :=
  { sourceCursor := plane.cursor, action := row.action }

theorem host_choice_request_carries_no_runtime_state_mutation
    (plane : ChoicePlaneSnapshot)
    (row : ChoicePlaneContractRow) :
    (requestHostAction plane row).sourceCursor = plane.cursor := by
  rfl

inductive ChoicePlaneFact where
  | runtimeAuthority
  | rootIdentity
  | configuredProfile
  | configuredSandbox
  | residentAbsent
  | residentIdentity
  | residentGeneration
  | routable
  | profileMatch
  | mismatchReason
  | archivedGeneration
  | typedFailure
  deriving DecidableEq, Repr

def disclosedFactsForNode : AgentControlPlaneObservation → List ChoicePlaneFact
  | .registered =>
      [.runtimeAuthority, .rootIdentity, .configuredSandbox, .residentIdentity, .residentGeneration,
        .routable, .profileMatch]
  | .registrationRequired =>
      [.runtimeAuthority, .rootIdentity, .configuredProfile, .configuredSandbox, .residentAbsent]
  | .archiveRequired =>
      [.runtimeAuthority, .rootIdentity, .configuredSandbox, .residentIdentity, .residentGeneration,
        .mismatchReason]
  | .archived =>
      [.runtimeAuthority, .rootIdentity, .configuredProfile, .configuredSandbox, .residentIdentity,
        .archivedGeneration]
  | .unavailableAuthority => [.configuredSandbox, .typedFailure]

def requiredFactsForCurrentChoices :
    AgentControlPlaneObservation → List ChoicePlaneFact
  | .registered =>
      [.runtimeAuthority, .rootIdentity, .configuredSandbox, .residentIdentity, .residentGeneration,
        .routable, .profileMatch]
  | .registrationRequired =>
      [.runtimeAuthority, .rootIdentity, .configuredProfile, .configuredSandbox, .residentAbsent]
  | .archiveRequired =>
      [.runtimeAuthority, .rootIdentity, .configuredSandbox, .residentIdentity, .residentGeneration,
        .mismatchReason]
  | .archived =>
      [.runtimeAuthority, .rootIdentity, .configuredProfile, .configuredSandbox, .residentIdentity,
        .archivedGeneration]
  | .unavailableAuthority => [.configuredSandbox, .typedFailure]

theorem current_disclosure_is_exactly_sufficient
    (node : AgentControlPlaneObservation) :
    disclosedFactsForNode node = requiredFactsForCurrentChoices node := by
  cases node <;> rfl

def allLifecycleFacts : List ChoicePlaneFact :=
  [.runtimeAuthority, .rootIdentity, .configuredProfile, .configuredSandbox, .residentAbsent,
    .residentIdentity, .residentGeneration, .routable, .profileMatch,
    .mismatchReason, .archivedGeneration, .typedFailure]

theorem progressive_disclosure_withholds_future_state_facts
    (node : AgentControlPlaneObservation) :
    disclosedFactsForNode node ≠ allLifecycleFacts := by
  cases node <;> decide

theorem every_choice_discloses_the_configured_sandbox
    (node : AgentControlPlaneObservation) :
    (disclosedFactsForNode node).contains ChoicePlaneFact.configuredSandbox = true := by
  cases node <;> rfl

inductive ExpectedHostOutcome where
  | taskControlTransferred
  | residentRegistrationObserved
  | residentArchiveObserved
  | higherGenerationObserved
  | typedRepairOrDefectReview
  deriving DecidableEq, Repr

def expectedOutcomeForAction : ChoicePlaneAction → ExpectedHostOutcome
  | .lifecycle .callResume => .taskControlTransferred
  | .lifecycle .createAndRegister => .residentRegistrationObserved
  | .lifecycle .archiveStale => .residentArchiveObserved
  | .lifecycle .createAfterArchive => .higherGenerationObserved
  | .lifecycle .blocked => .typedRepairOrDefectReview
  | .inspectEvidence _ => .typedRepairOrDefectReview

structure ReasoningChoiceProjection where
  sourceNode : AgentControlPlaneObservation
  action : ChoicePlaneAction
  disclosedFacts : List ChoicePlaneFact
  expectedOutcome : ExpectedHostOutcome
  deriving DecidableEq, Repr

def reasoningProjection
    (node : AgentControlPlaneObservation)
    (action : ChoicePlaneAction) : ReasoningChoiceProjection :=
  { sourceNode := node
    action
    disclosedFacts := disclosedFactsForNode node
    expectedOutcome := expectedOutcomeForAction action }

theorem reasoning_projection_names_a_typed_postcondition
    (node : AgentControlPlaneObservation)
    (action : ChoicePlaneAction) :
    (reasoningProjection node action).expectedOutcome =
      expectedOutcomeForAction action := by
  rfl

inductive HostActionOutcome where
  | completed
  | advanced (cursor : ChoicePlaneCursor)
  | unchanged (cursor : ChoicePlaneCursor)
  deriving DecidableEq, Repr

structure ReentryBudget where
  remaining : Nat
  deriving DecidableEq, Repr

def consumeNonProgress : ReentryBudget → ReentryBudget
  | ⟨0⟩ => ⟨0⟩
  | ⟨remaining + 1⟩ => ⟨remaining⟩

theorem non_progress_consumes_the_finite_budget (remaining : Nat) :
    (consumeNonProgress ⟨remaining + 1⟩).remaining < (ReentryBudget.mk (remaining + 1)).remaining := by
  exact Nat.lt_succ_self remaining

inductive LoopDisposition where
  | completed
  | reenterProgress
  | retry
  | defectReviewRequired
  deriving DecidableEq, Repr

def loopDisposition
    (outcome : HostActionOutcome)
    (budget : ReentryBudget) : LoopDisposition :=
  match outcome with
  | .completed => .completed
  | .advanced _ => .reenterProgress
  | .unchanged _ =>
      if budget.remaining == 0 then .defectReviewRequired else .retry

theorem exhausted_non_progress_cannot_retry (cursor : ChoicePlaneCursor) :
    loopDisposition (.unchanged cursor) ⟨0⟩ = .defectReviewRequired := by
  rfl

inductive ChoicePlaneInvariantDefect where
  | admittedRowStateMismatch
  | missingRequiredDisclosure
  | invalidHistoryCursor
  | exhaustedNonProgressCycle
  deriving DecidableEq, Repr

structure VerifiedChoicePlaneDefectReceipt where
  defect : ChoicePlaneInvariantDefect
  evidenceDigest : Nat
  sourceCursor : ChoicePlaneCursor
  deriving DecidableEq, Repr

inductive LoopExit where
  | continueReasoning
  | completed
  | typedBlocked
  | oneShotPassThrough
  deriving DecidableEq, Repr

def resolveLoopExit
    (disposition : LoopDisposition)
    (defect : Option VerifiedChoicePlaneDefectReceipt) : LoopExit :=
  match disposition with
  | .completed => .completed
  | .reenterProgress | .retry => .continueReasoning
  | .defectReviewRequired =>
      match defect with
      | some _ => .oneShotPassThrough
      | none => .typedBlocked

theorem exhausted_loop_without_verified_defect_is_typed_blocked :
    resolveLoopExit .defectReviewRequired none = .typedBlocked := by
  rfl

theorem pass_through_requires_a_verified_invariant_defect
    (receipt : VerifiedChoicePlaneDefectReceipt) :
    resolveLoopExit .defectReviewRequired (some receipt) = .oneShotPassThrough := by
  rfl

structure OneShotPassThroughAuthorization where
  defect : VerifiedChoicePlaneDefectReceipt
  workspaceId : Nat
  rootSessionId : Nat
  protectedCommandDigest : Nat
  denyEvidenceDigest : Nat
  issuedAtMs : Nat
  ttlMs : Nat
  consumed : Bool
  deriving DecidableEq, Repr

structure PassThroughRequestContext where
  workspaceId : Nat
  rootSessionId : Nat
  protectedCommandDigest : Nat
  denyEvidenceDigest : Nat
  nowMs : Nat
  deriving DecidableEq, Repr

def passThroughBindingsMatch
    (authorization : OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext) : Bool :=
  match authorization.protectedCommandDigest == context.protectedCommandDigest with
  | false => false
  | true =>
      authorization.workspaceId == context.workspaceId &&
      authorization.rootSessionId == context.rootSessionId &&
      authorization.denyEvidenceDigest == context.denyEvidenceDigest

def passThroughTimeIsValid
    (authorization : OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext) : Bool :=
  decide (0 < authorization.ttlMs) &&
  decide (authorization.ttlMs ≤ 60_000) &&
  decide (authorization.issuedAtMs ≤ context.nowMs) &&
  decide (context.nowMs ≤ authorization.issuedAtMs + authorization.ttlMs)

def passThroughAvailable
    (authorization : OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext) : Bool :=
  match authorization.consumed with
  | true => false
  | false =>
      passThroughBindingsMatch authorization context &&
      passThroughTimeIsValid authorization context

def authorizePassThrough
    (authorization : Option OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext) : Bool :=
  match authorization with
  | none => false
  | some capability => passThroughAvailable capability context

def consumePassThrough
    (authorization : OneShotPassThroughAuthorization) : OneShotPassThroughAuthorization :=
  { authorization with consumed := true }

theorem pass_through_cannot_be_reused
    (authorization : OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext) :
    passThroughAvailable (consumePassThrough authorization) context = false := by
  rfl

theorem naked_pass_through_request_is_never_authority
    (context : PassThroughRequestContext) :
    authorizePassThrough none context = false := by
  rfl

theorem command_binding_drift_fails_closed
    (authorization : OneShotPassThroughAuthorization)
    (context : PassThroughRequestContext)
    (drift : (authorization.protectedCommandDigest == context.protectedCommandDigest) = false) :
    authorizePassThrough (some authorization) context = false := by
  cases authorization with
  | mk defect workspace root command evidence issued ttl consumed =>
      cases consumed with
      | false =>
          simp only [authorizePassThrough, passThroughAvailable, passThroughBindingsMatch]
          rw [drift]
          rfl
      | true => rfl

inductive AgentFacingLifecycleCommand where
  | openCurrentChoicePlane
  deriving DecidableEq, Repr

def publicLifecycleCommands : List AgentFacingLifecycleCommand :=
  [.openCurrentChoicePlane]

theorem agent_facing_lifecycle_surface_is_singleton :
    publicLifecycleCommands = [.openCurrentChoicePlane] := by
  rfl

end ASPProof.MultiAgentChoicePlane
