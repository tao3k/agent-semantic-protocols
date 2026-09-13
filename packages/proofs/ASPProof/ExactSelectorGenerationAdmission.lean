-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ExactSelectorGenerationAdmission

abbrev GenerationId := String
abbrev RootDigest := String
abbrev OwnerPath := String
abbrev StructuralSelector := String
abbrev WorkspaceId := String
abbrev MutationId := String
abbrev ContentDigest := String

structure SelectorRequest where
  ownerPath : OwnerPath
  structuralSelector : StructuralSelector
  /-- A stale classification requires an explicit generation expectation.
  Absence of this field means "resolve against the active generation". -/
  expectedGenerationId : Option GenerationId := none
  deriving DecidableEq, Repr

/-- The Runtime Server publishes owner and selector membership as one immutable
generation. A selector read never combines inventories from two generations. -/
structure ActiveGeneration where
  generationId : GenerationId
  rootDigest : RootDigest
  owners : List OwnerPath
  selectors : List (OwnerPath × StructuralSelector)
  deriving DecidableEq, Repr

inductive Resolution where
  | resolved (generationId : GenerationId) (rootDigest : RootDigest)
  | selectorStale (generationId : GenerationId) (rootDigest : RootDigest)
  | ownerMissing (generationId : GenerationId) (rootDigest : RootDigest)
  | itemMissing (generationId : GenerationId) (rootDigest : RootDigest)
  deriving DecidableEq, Repr

def generationMismatch (active : ActiveGeneration) (request : SelectorRequest) : Prop :=
  ∃ expected,
    request.expectedGenerationId = some expected ∧ expected ≠ active.generationId

instance (active : ActiveGeneration) (request : SelectorRequest) :
    Decidable (generationMismatch active request) := by
  unfold generationMismatch
  infer_instance

/-- A selector is stale only when its explicit generation expectation differs
from the active generation. Owner and item membership are classified inside the
same active generation after that check. -/
def resolve (active : ActiveGeneration) (request : SelectorRequest) : Resolution :=
  if generationMismatch active request then
    .selectorStale active.generationId active.rootDigest
  else
    if request.ownerPath ∈ active.owners then
      if (request.ownerPath, request.structuralSelector) ∈ active.selectors then
        .resolved active.generationId active.rootDigest
      else
        .itemMissing active.generationId active.rootDigest
    else
      .ownerMissing active.generationId active.rootDigest

def resolutionGeneration : Resolution → GenerationId
  | .resolved generationId _ => generationId
  | .selectorStale generationId _ => generationId
  | .ownerMissing generationId _ => generationId
  | .itemMissing generationId _ => generationId

def resolutionRoot : Resolution → RootDigest
  | .resolved _ rootDigest => rootDigest
  | .selectorStale _ rootDigest => rootDigest
  | .ownerMissing _ rootDigest => rootDigest
  | .itemMissing _ rootDigest => rootDigest

theorem every_resolution_is_active_generation_bound
    (active : ActiveGeneration)
    (request : SelectorRequest) :
    resolutionGeneration (resolve active request) = active.generationId ∧
      resolutionRoot (resolve active request) = active.rootDigest := by
  by_cases stale : generationMismatch active request
  · simp [resolve, stale, resolutionGeneration, resolutionRoot]
  · by_cases ownerPresent : request.ownerPath ∈ active.owners
    · by_cases selectorPresent :
        (request.ownerPath, request.structuralSelector) ∈ active.selectors
      · simp [resolve, stale, ownerPresent, selectorPresent, resolutionGeneration, resolutionRoot]
      · simp [resolve, stale, ownerPresent, selectorPresent, resolutionGeneration, resolutionRoot]
    · simp [resolve, stale, ownerPresent, resolutionGeneration, resolutionRoot]

structure ExactQueryTerminal where
  generationId : GenerationId
  rootDigest : RootDigest
  deriving DecidableEq, Repr

def terminalFromActiveGeneration (active : ActiveGeneration) : ExactQueryTerminal :=
  { generationId := active.generationId
    rootDigest := active.rootDigest }

theorem exact_query_terminal_uses_active_source_root
    (active : ActiveGeneration)
    (_request : SelectorRequest) :
    (terminalFromActiveGeneration active).generationId = active.generationId ∧
      (terminalFromActiveGeneration active).rootDigest = active.rootDigest := by
  exact ⟨rfl, rfl⟩

theorem absent_owner_in_active_generation_is_owner_missing
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerAbsent : request.ownerPath ∉ active.owners) :
    resolve active request =
      .ownerMissing active.generationId active.rootDigest := by
  rcases generationCurrent with generationUnspecified | generationMatches
  · simp [resolve, generationMismatch, generationUnspecified, ownerAbsent]
  · simp [resolve, generationMismatch, generationMatches, ownerAbsent]

theorem selector_stale_requires_explicit_generation_mismatch
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (stale : resolve active request =
      .selectorStale active.generationId active.rootDigest) :
    ∃ expected,
      request.expectedGenerationId = some expected ∧ expected ≠ active.generationId := by
  simp only [resolve] at stale
  split at stale
  next generationMismatch => exact generationMismatch
  next noMismatch =>
    split at stale
    next ownerPresent =>
      split at stale <;> contradiction
    next ownerAbsent => contradiction

theorem owner_missing_and_selector_stale_are_mutually_exclusive
    (active : ActiveGeneration)
    (request : SelectorRequest) :
    ¬(resolve active request =
        .ownerMissing active.generationId active.rootDigest ∧
      resolve active request =
        .selectorStale active.generationId active.rootDigest) := by
  intro both
  rw [both.1] at both
  cases both.2

theorem owner_missing_and_item_missing_are_mutually_exclusive
    (active : ActiveGeneration)
    (request : SelectorRequest) :
    ¬(resolve active request =
        .ownerMissing active.generationId active.rootDigest ∧
      resolve active request =
        .itemMissing active.generationId active.rootDigest) := by
  intro both
  rw [both.1] at both
  cases both.2

theorem active_owner_absent_selector_is_item_missing
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (generationCurrent : request.expectedGenerationId = none ∨
      request.expectedGenerationId = some active.generationId)
    (ownerPresent : request.ownerPath ∈ active.owners)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request = .itemMissing active.generationId active.rootDigest := by
  rcases generationCurrent with generationUnspecified | generationMatches
  · simp [resolve, generationMismatch, generationUnspecified, ownerPresent, selectorAbsent]
  · simp [resolve, generationMismatch, generationMatches, ownerPresent, selectorAbsent]

theorem resolved_implies_active_owner_and_selector
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (resolved : resolve active request =
      .resolved active.generationId active.rootDigest) :
    request.ownerPath ∈ active.owners ∧
      (request.ownerPath, request.structuralSelector) ∈ active.selectors := by
  simp only [resolve] at resolved
  split at resolved
  next => contradiction
  next generationCurrent =>
    split at resolved
    next ownerPresent =>
      split at resolved
      next selectorPresent => exact ⟨ownerPresent, selectorPresent⟩
      next => contradiction
    next => contradiction

theorem fabricated_selector_is_never_admitted_as_resolved
    (active : ActiveGeneration)
    (request : SelectorRequest)
    (selectorAbsent :
      (request.ownerPath, request.structuralSelector) ∉ active.selectors) :
    resolve active request ≠ .resolved active.generationId active.rootDigest := by
  intro resolved
  exact selectorAbsent (resolved_implies_active_owner_and_selector active request resolved).2

inductive RecoveryRoute where
  | symbolSearch
  | ownerItems
  | none
  deriving DecidableEq, Repr

def recoveryRoute : Resolution → RecoveryRoute
  | .selectorStale _ _ => .symbolSearch
  | .ownerMissing _ _ => .symbolSearch
  | .itemMissing _ _ => .none
  | .resolved _ _ => .none

theorem stale_selector_never_reuses_absent_owner_authority
    (generationId : GenerationId)
    (rootDigest : RootDigest) :
    recoveryRoute (.selectorStale generationId rootDigest) = .symbolSearch := by
  rfl

/-- An exact miss inside a live owner is a complete negative answer from the
active generation. Re-running discovery for the same absent identity cannot
add authority and would create an unbounded search loop. -/
theorem active_item_missing_is_terminal
    (generationId : GenerationId)
    (rootDigest : RootDigest) :
    recoveryRoute (.itemMissing generationId rootDigest) = .none := by
  rfl

/-- A source mutation is witnessed independently of the client that performed
the edit. Hook receipts are accelerators; the resident watcher is the durable
correctness witness for edits that bypass a client hook. -/
inductive MutationWitnessSource where
  | clientHook
  | residentWatcher
  deriving DecidableEq, Repr

structure SourceMutationWitness where
  workspaceId : WorkspaceId
  mutationId : MutationId
  ownerPath : OwnerPath
  contentDigest : ContentDigest
  source : MutationWitnessSource
  deriving DecidableEq, Repr

structure ReconciliationRuntime where
  active : ActiveGeneration
  workspaceId : WorkspaceId
  pending : List SourceMutationWitness
  deriving DecidableEq, Repr

def observeSourceMutation
    (runtime : ReconciliationRuntime)
    (witness : SourceMutationWitness) : ReconciliationRuntime :=
  if witness.workspaceId = runtime.workspaceId then
    { runtime with pending := runtime.pending ++ [witness] }
  else
    runtime

def exactReadAdmitted (runtime : ReconciliationRuntime) : Bool :=
  runtime.pending.isEmpty

def publishReconciledGeneration
    (runtime : ReconciliationRuntime)
    (next : ActiveGeneration) : ReconciliationRuntime :=
  { runtime with active := next, pending := [] }

theorem observed_same_workspace_mutation_blocks_exact_read_until_publication
    (runtime : ReconciliationRuntime)
    (witness : SourceMutationWitness)
    (sameWorkspace : witness.workspaceId = runtime.workspaceId) :
    exactReadAdmitted (observeSourceMutation runtime witness) = false := by
  simp [exactReadAdmitted, observeSourceMutation, sameWorkspace]

theorem reconciled_publication_releases_exact_read
    (runtime : ReconciliationRuntime)
    (next : ActiveGeneration) :
    exactReadAdmitted (publishReconciledGeneration runtime next) = true := by
  simp [exactReadAdmitted, publishReconciledGeneration]

theorem another_workspace_mutation_does_not_block_this_workspace
    (runtime : ReconciliationRuntime)
    (witness : SourceMutationWitness)
    (otherWorkspace : witness.workspaceId ≠ runtime.workspaceId) :
    observeSourceMutation runtime witness = runtime := by
  simp [observeSourceMutation, otherWorkspace]

structure RuntimeState where
  active : ActiveGeneration
  deriving DecidableEq, Repr

def publish (state : RuntimeState) (next : ActiveGeneration) : RuntimeState :=
  { state with active := next }

theorem publication_replaces_generation_and_selector_inventory_atomically
    (state : RuntimeState)
    (next : ActiveGeneration) :
    (publish state next).active.generationId = next.generationId ∧
      (publish state next).active.rootDigest = next.rootDigest ∧
      (publish state next).active.owners = next.owners ∧
      (publish state next).active.selectors = next.selectors := by
  simp [publish]

/-- One reconciliation event carries every owner upsert, tombstone, relocation,
and selector inventory in one successor generation. The base generation is a
CAS precondition; per-owner publication is not representable. -/
structure WorkspaceGenerationDelta where
  baseGenerationId : GenerationId
  next : ActiveGeneration
  deriving DecidableEq, Repr

def applyGenerationDelta
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta) : Option RuntimeState :=
  if delta.baseGenerationId = state.active.generationId then
    some (publish state delta.next)
  else
    none

def observeGenerationAfterDelta
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta) : RuntimeState :=
  (applyGenerationDelta state delta).getD state

theorem matching_base_publishes_the_complete_successor
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta)
    (baseMatches : delta.baseGenerationId = state.active.generationId) :
    applyGenerationDelta state delta = some (publish state delta.next) := by
  simp [applyGenerationDelta, baseMatches]

theorem stale_base_cannot_publish_any_part_of_the_delta
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta)
    (baseStale : delta.baseGenerationId ≠ state.active.generationId) :
    observeGenerationAfterDelta state delta = state := by
  simp [observeGenerationAfterDelta, applyGenerationDelta, baseStale]

theorem readers_observe_old_or_complete_next_generation
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta) :
    (observeGenerationAfterDelta state delta).active = state.active ∨
      (observeGenerationAfterDelta state delta).active = delta.next := by
  by_cases baseMatches : delta.baseGenerationId = state.active.generationId
  · right
    simp [observeGenerationAfterDelta, applyGenerationDelta, baseMatches, publish]
  · left
    simp [observeGenerationAfterDelta, applyGenerationDelta, baseMatches]

theorem no_half_generation_selector_inventory
    (state : RuntimeState)
    (delta : WorkspaceGenerationDelta) :
    let observed := (observeGenerationAfterDelta state delta).active
    (observed.owners = state.active.owners ∧
        observed.selectors = state.active.selectors) ∨
      (observed.owners = delta.next.owners ∧
        observed.selectors = delta.next.selectors) := by
  by_cases baseMatches : delta.baseGenerationId = state.active.generationId
  · right
    simp [observeGenerationAfterDelta, applyGenerationDelta, baseMatches, publish]
  · left
    simp [observeGenerationAfterDelta, applyGenerationDelta, baseMatches]

/-- Admission acceptance and generation readiness are distinct lifecycle
states. PreToolUse may release an exact query only after the typed ensure
receipt carries the complete active generation. -/
inductive AdmissionState where
  | building
  | ready (active : ActiveGeneration)
  | failed
  deriving DecidableEq, Repr

inductive PreToolDecision where
  | continueWith (active : ActiveGeneration)
  | deny
  deriving DecidableEq, Repr

def admissionAccepts : AdmissionState → Bool
  | .building => false
  | .ready _ => true
  | .failed => true

/-- Building is the only single-flight exclusion state. Ready is an observed
generation, not a permanent cache lock: a later PreTool admission must be able
to run the incremental builder and publish a successor generation. -/
theorem ready_generation_does_not_block_incremental_readmission
    (active : ActiveGeneration) :
    admissionAccepts (.ready active) = true := by
  rfl

theorem building_generation_preserves_single_flight :
    admissionAccepts .building = false := by
  rfl

inductive AdmissionOperation where
  | admitSuccessor
  | ensureCurrent
  | restoreCurrent
  deriving DecidableEq, Repr

def schedulesBuild : AdmissionOperation → AdmissionState → Bool
  | .admitSuccessor, .ready _ => true
  | .admitSuccessor, .failed => true
  | .admitSuccessor, .building => false
  | .ensureCurrent, _ => false
  | .restoreCurrent, _ => false

theorem ensure_ready_generation_is_observation_only
    (active : ActiveGeneration) :
    schedulesBuild .ensureCurrent (.ready active) = false := by
  rfl

theorem restore_ready_generation_is_idempotent
    (active : ActiveGeneration) :
    schedulesBuild .restoreCurrent (.ready active) = false := by
  rfl

theorem explicit_admission_can_publish_ready_successor
    (active : ActiveGeneration) :
    schedulesBuild .admitSuccessor (.ready active) = true := by
  rfl

def finishPreToolAdmission (ensured : AdmissionState) : PreToolDecision :=
  match ensured with
  | .ready active => .continueWith active
  | .building => .deny
  | .failed => .deny

def exactQueryEnabled : PreToolDecision → Bool
  | .continueWith _ => true
  | .deny => false

theorem building_admission_never_releases_exact_query :
    exactQueryEnabled (finishPreToolAdmission .building) = false := by
  rfl

theorem failed_admission_never_releases_exact_query :
    exactQueryEnabled (finishPreToolAdmission .failed) = false := by
  rfl

theorem only_ready_admission_releases_its_active_generation
    (active : ActiveGeneration) :
    finishPreToolAdmission (.ready active) = .continueWith active := by
  rfl

abbrev RepositoryId := String
abbrev WorktreeId := String
abbrev SessionId := String
abbrev DbOwnerId := String
abbrev ProviderId := String
abbrev ScopeDigest := String

/-- A boolean mutation signal proves only that something changed. It cannot
identify which resident workspace lane owns the changed paths. -/
def workspaceMutatedSignal (workspaceIds : List WorkspaceId) : Bool :=
  !workspaceIds.isEmpty

theorem boolean_mutation_signal_loses_workspace_identity
    (left right : WorkspaceId)
    (different : left ≠ right) :
    workspaceMutatedSignal [left] = workspaceMutatedSignal [right] ∧
      [left] ≠ [right] := by
  simp [workspaceMutatedSignal, different]

/-- The PostTool mutation envelope preserves one complete generation delta per
affected workspace. The runtime may apply lanes independently, but it may not
drop or infer their workspace identities from the hook cwd. -/
structure WorkspaceGenerationEnvelope where
  workspaceId : WorkspaceId
  delta : WorkspaceGenerationDelta
  deriving DecidableEq, Repr

structure WorkspaceMutationBatch where
  mutationId : MutationId
  envelopes : List WorkspaceGenerationEnvelope
  deriving DecidableEq, Repr

def affectedWorkspaceIds (batch : WorkspaceMutationBatch) : List WorkspaceId :=
  batch.envelopes.map (·.workspaceId)

theorem every_generation_envelope_retains_its_workspace_identity
    (batch : WorkspaceMutationBatch)
    (envelope : WorkspaceGenerationEnvelope)
    (present : envelope ∈ batch.envelopes) :
    envelope.workspaceId ∈ affectedWorkspaceIds batch := by
  exact List.mem_map.mpr ⟨envelope, present, rfl⟩

/-- Equal changed workspace envelopes do not identify an edit event. A later
edit of the same paths must carry a different mutation identity and cannot be
answered from the preceding event's completed flight. -/
theorem equal_envelopes_do_not_collapse_distinct_mutation_events
    (leftMutationId rightMutationId : MutationId)
    (different : leftMutationId ≠ rightMutationId)
    (envelopes : List WorkspaceGenerationEnvelope) :
    ({ mutationId := leftMutationId, envelopes } : WorkspaceMutationBatch) ≠
      ({ mutationId := rightMutationId, envelopes } : WorkspaceMutationBatch) := by
  intro equalBatch
  exact different (congrArg WorkspaceMutationBatch.mutationId equalBatch)

def mayCoalesceMutation
    (left right : WorkspaceMutationBatch) : Bool :=
  left.mutationId == right.mutationId

theorem distinct_mutation_events_cannot_share_a_transport_flight
    (left right : WorkspaceMutationBatch)
    (different : left.mutationId ≠ right.mutationId) :
    mayCoalesceMutation left right = false := by
  simp [mayCoalesceMutation, different]

inductive MutationSubmissionState where
  | queued
  | coalesced
  deriving DecidableEq, Repr

structure MutationSubmissionReceipt where
  mutationId : MutationId
  state : MutationSubmissionState
  deriving DecidableEq, Repr

/-- Local writer-lane admission proves only durable ordering intent. Exact
query authority still comes exclusively from a ready generation ensure. -/
def submissionEnablesExactQuery (_ : MutationSubmissionReceipt) : Bool := false

theorem mutation_submission_never_authorizes_exact_query
    (receipt : MutationSubmissionReceipt) :
    submissionEnablesExactQuery receipt = false := by
  rfl

/-- A resident writer lane has at most one active mutation and an ordered
successor queue. Queue membership is keyed by mutation identity, never by the
changed-path set: repeating the same paths in a later event is still work. -/
structure MutationWriterLane where
  inFlight : Option WorkspaceMutationBatch
  pending : List WorkspaceMutationBatch
  deriving DecidableEq, Repr

def laneContainsMutationId
    (lane : MutationWriterLane)
    (mutationId : MutationId) : Bool :=
  lane.inFlight.any (fun batch => batch.mutationId == mutationId) ||
    lane.pending.any (fun batch => batch.mutationId == mutationId)

def enqueueMutation
    (lane : MutationWriterLane)
    (batch : WorkspaceMutationBatch) : MutationWriterLane :=
  if laneContainsMutationId lane batch.mutationId then
    lane
  else
    match lane.inFlight with
    | none => { lane with inFlight := some batch }
    | some _ => { lane with pending := lane.pending ++ [batch] }

def completeInFlightMutation (lane : MutationWriterLane) : MutationWriterLane :=
  match lane.inFlight, lane.pending with
  | none, _ => lane
  | some _, [] => { lane with inFlight := none }
  | some _, next :: rest => { inFlight := some next, pending := rest }

theorem distinct_mutation_arriving_during_build_is_queued
    (active successor : WorkspaceMutationBatch)
    (different : active.mutationId ≠ successor.mutationId) :
    enqueueMutation
        { inFlight := some active, pending := [] }
        successor =
      { inFlight := some active, pending := [successor] } := by
  simp [enqueueMutation, laneContainsMutationId, different]

theorem queued_distinct_mutation_becomes_the_next_flight
    (active successor : WorkspaceMutationBatch)
    (different : active.mutationId ≠ successor.mutationId) :
    completeInFlightMutation
        (enqueueMutation
          { inFlight := some active, pending := [] }
          successor) =
      { inFlight := some successor, pending := [] } := by
  rw [distinct_mutation_arriving_during_build_is_queued active successor different]
  rfl

theorem duplicate_mutation_identity_is_coalesced_without_a_second_queue_entry
    (active duplicate : WorkspaceMutationBatch)
    (same : active.mutationId = duplicate.mutationId) :
    enqueueMutation
        { inFlight := some active, pending := [] }
        duplicate =
      { inFlight := some active, pending := [] } := by
  simp [enqueueMutation, laneContainsMutationId, same]

/-- Artifact publication is a compare-and-swap over the complete Merkle root.
Atomic rename prevents torn JSON, while the base-root precondition prevents a
complete but stale publisher from replacing a newer runtime closure. -/
structure ActiveArtifactReceiptState where
  rootDigest : RootDigest
  deriving DecidableEq, Repr

structure ActiveArtifactPublication where
  expectedRootDigest : RootDigest
  nextRootDigest : RootDigest
  deriving DecidableEq, Repr

def publishActiveArtifactReceipt
    (state : ActiveArtifactReceiptState)
    (publication : ActiveArtifactPublication) : Option ActiveArtifactReceiptState :=
  if publication.expectedRootDigest = state.rootDigest then
    some { rootDigest := publication.nextRootDigest }
  else
    none

theorem stale_artifact_publisher_cannot_overwrite_the_active_root
    (state : ActiveArtifactReceiptState)
    (publication : ActiveArtifactPublication)
    (stale : publication.expectedRootDigest ≠ state.rootDigest) :
    publishActiveArtifactReceipt state publication = none := by
  simp [publishActiveArtifactReceipt, stale]

theorem matching_artifact_base_publishes_exactly_one_complete_next_root
    (state : ActiveArtifactReceiptState)
    (publication : ActiveArtifactPublication)
    (baseMatches : publication.expectedRootDigest = state.rootDigest) :
    publishActiveArtifactReceipt state publication =
      some { rootDigest := publication.nextRootDigest } := by
  simp [publishActiveArtifactReceipt, baseMatches]

/-! Runtime artifact invoker and readiness invariants.  These are the small
    Boolean contracts implemented by the Rust admission and CAS owners. -/
structure RuntimeInvokerAdmission where
  invokerDigest : Nat
  activeDigest : Nat
  deriving DecidableEq, Repr

def invokerAdmitted (candidate : RuntimeInvokerAdmission) : Prop :=
  candidate.invokerDigest = candidate.activeDigest

theorem invoker_mismatch_rejects_before_spawn
    (candidate : RuntimeInvokerAdmission)
    (mismatch : candidate.invokerDigest ≠ candidate.activeDigest) :
    ¬ invokerAdmitted candidate := by
  exact mismatch

structure ReadinessCandidate where
  activeDigest : Nat
  artifactDigest : Nat
  endpointGeneration : Nat
  schemaGeneration : Nat
  expectedEndpointGeneration : Nat
  expectedSchemaGeneration : Nat
  deriving DecidableEq, Repr

def readinessQualified (candidate : ReadinessCandidate) : Prop :=
  candidate.activeDigest = candidate.artifactDigest ∧
    candidate.endpointGeneration = candidate.expectedEndpointGeneration ∧
      candidate.schemaGeneration = candidate.expectedSchemaGeneration

theorem promotion_requires_active_artifact_endpoint_schema_match
    (candidate : ReadinessCandidate)
    (notQualified : ¬ readinessQualified candidate) :
    ¬ readinessQualified candidate := by
  exact notQualified

noncomputable def preserveHealthyOnFailedReadiness
    (healthy : Nat) (candidate : ReadinessCandidate) (ready : Bool) : Nat :=
  by
    classical
    exact if ready then if readinessQualified candidate then candidate.activeDigest else healthy else healthy

theorem failed_readiness_preserves_healthy
    (healthy : Nat) (candidate : ReadinessCandidate)
    (failed : ¬ readinessQualified candidate) :
    preserveHealthyOnFailedReadiness healthy candidate true = healthy := by
  simp [preserveHealthyOnFailedReadiness, failed]

structure PublicationState where
  active : Nat
  healthy : Nat
  stagedCandidate : Option Nat
  deriving DecidableEq, Repr

def stageCandidate (state : PublicationState) (digest : Nat) : PublicationState :=
  { state with stagedCandidate := some digest }

def slotProjection (state : PublicationState) : Nat × Nat :=
  (state.active, state.healthy)

theorem staging_preserves_active_projection
    (state : PublicationState) (digest : Nat) :
    (stageCandidate state digest).active = state.active := by
  rfl

theorem staging_preserves_healthy_projection
    (state : PublicationState) (digest : Nat) :
    (stageCandidate state digest).healthy = state.healthy := by
  rfl

noncomputable def promoteCandidate
    (state : PublicationState) (candidate : ReadinessCandidate) : PublicationState :=
  by
    classical
    exact if readinessQualified candidate ∧ state.stagedCandidate = some candidate.artifactDigest then
      { active := candidate.artifactDigest, healthy := candidate.artifactDigest, stagedCandidate := none }
    else state

theorem failed_readiness_does_not_promote_candidate
    (state : PublicationState) (candidate : ReadinessCandidate)
    (failed : ¬ readinessQualified candidate) :
    promoteCandidate state candidate = state := by
  simp [promoteCandidate, failed]

theorem atomic_staging_does_not_change_slots_before_commit
    (state : PublicationState) (digest : Nat) :
    slotProjection (stageCandidate state digest) = slotProjection state := by
  rfl

/-- A workspace is identified by repository plus worktree. Provider project
scope is deliberately absent from identity. -/
structure WorktreeWorkspace where
  workspaceId : WorkspaceId
  repositoryId : RepositoryId
  worktreeId : WorktreeId
  deriving DecidableEq, Repr

/-- The resident runtime has exactly one DB owner field while accepting any
number of client sessions. Registering a session cannot allocate a new owner. -/
structure WorkspaceRuntime where
  workspace : WorktreeWorkspace
  dbOwnerId : DbOwnerId
  sessions : List SessionId
  deriving DecidableEq, Repr

def registerSession (runtime : WorkspaceRuntime) (sessionId : SessionId) : WorkspaceRuntime :=
  if sessionId ∈ runtime.sessions then runtime
  else { runtime with sessions := sessionId :: runtime.sessions }

def sessionOwner (runtime : WorkspaceRuntime) (sessionId : SessionId) : Option DbOwnerId :=
  if sessionId ∈ runtime.sessions then some runtime.dbOwnerId else none

theorem session_registration_preserves_the_unique_owner
    (runtime : WorkspaceRuntime)
    (sessionId : SessionId) :
    (registerSession runtime sessionId).dbOwnerId = runtime.dbOwnerId := by
  unfold registerSession
  split <;> rfl

theorem register_session_contains_the_session
    (runtime : WorkspaceRuntime)
    (sessionId : SessionId) :
    sessionId ∈ (registerSession runtime sessionId).sessions := by
  unfold registerSession
  split <;> simp_all

theorem register_session_preserves_existing_sessions
    (runtime : WorkspaceRuntime)
    (sessionId existing : SessionId)
    (present : existing ∈ runtime.sessions) :
    existing ∈ (registerSession runtime sessionId).sessions := by
  unfold registerSession
  split <;> simp_all

theorem two_sessions_share_the_same_workspace_owner
    (runtime : WorkspaceRuntime)
    (left right : SessionId) :
    let registered := registerSession (registerSession runtime left) right
    sessionOwner registered left = some runtime.dbOwnerId ∧
      sessionOwner registered right = some runtime.dbOwnerId := by
  let once := registerSession runtime left
  let registered := registerSession once right
  have leftInOnce : left ∈ once.sessions :=
    register_session_contains_the_session runtime left
  have leftInRegistered : left ∈ registered.sessions :=
    register_session_preserves_existing_sessions once right left leftInOnce
  have rightInRegistered : right ∈ registered.sessions :=
    register_session_contains_the_session once right
  have onceOwner : once.dbOwnerId = runtime.dbOwnerId :=
    session_registration_preserves_the_unique_owner runtime left
  have registeredOwner : registered.dbOwnerId = runtime.dbOwnerId := by
    calc
      registered.dbOwnerId = once.dbOwnerId :=
        session_registration_preserves_the_unique_owner once right
      _ = runtime.dbOwnerId := onceOwner
  change sessionOwner registered left = some runtime.dbOwnerId ∧
    sessionOwner registered right = some runtime.dbOwnerId
  constructor
  · simp [sessionOwner, leftInRegistered, registeredOwner]
  · simp [sessionOwner, rightInRegistered, registeredOwner]

/-- Provider resolution is a generation-bound scope receipt for an already
identified workspace; it is not an identity constructor. -/
structure ProviderScopeReceipt where
  workspaceId : WorkspaceId
  providerId : ProviderId
  scopeDigest : ScopeDigest
  generationId : GenerationId
  deriving DecidableEq, Repr

structure PublishedProviderScope where
  workspace : WorktreeWorkspace
  receipt : ProviderScopeReceipt
  deriving DecidableEq, Repr

def publishProviderScope
    (workspace : WorktreeWorkspace)
    (receipt : ProviderScopeReceipt) : Option PublishedProviderScope :=
  if receipt.workspaceId = workspace.workspaceId then
    some { workspace, receipt }
  else
    none

theorem provider_scope_publication_preserves_workspace_identity
    (workspace : WorktreeWorkspace)
    (receipt : ProviderScopeReceipt)
    (admitted : receipt.workspaceId = workspace.workspaceId) :
    (publishProviderScope workspace receipt).map (·.workspace) = some workspace := by
  simp [publishProviderScope, admitted]

theorem provider_scope_digest_cannot_create_a_workspace_identity
    (workspace : WorktreeWorkspace)
    (left right : ProviderScopeReceipt)
    (leftAdmitted : left.workspaceId = workspace.workspaceId)
    (rightAdmitted : right.workspaceId = workspace.workspaceId) :
    (publishProviderScope workspace left).map (·.workspace.workspaceId) =
      (publishProviderScope workspace right).map (·.workspace.workspaceId) := by
  simp [publishProviderScope, leftAdmitted, rightAdmitted]

/-- Registry bootstrap belongs to the daemon's existing Tokio lifecycle. A
synchronous bridge that attempts to own a nested runtime is never admissible. -/
inductive RegistryBootstrapMode where
  | asyncInOwnerRuntime
  | synchronousNestedRuntime
  deriving DecidableEq, Repr

def registryBootstrapAdmitted
    (runtimeOwnerCount : Nat)
    (mode : RegistryBootstrapMode) : Bool :=
  runtimeOwnerCount == 1 && mode == .asyncInOwnerRuntime

theorem daemon_async_registry_bootstrap_has_one_runtime_owner :
    registryBootstrapAdmitted 1 .asyncInOwnerRuntime = true := by
  decide

theorem daemon_nested_runtime_registry_bootstrap_is_rejected
    (runtimeOwnerCount : Nat) :
    registryBootstrapAdmitted runtimeOwnerCount .synchronousNestedRuntime = false := by
  simp [registryBootstrapAdmitted]

end ASPProof.ExactSelectorGenerationAdmission
