-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ASPInteractiveSearchLatency

/-! Lean proves that a Ready request contains only bounded resident work and
    that transports and language providers cannot acquire shared Search
    authority. Executable receipts provide the observed nanoseconds. -/

def readyP50BudgetNs : Nat := 250000
def readyP99BudgetNs : Nat := 700000
def readyHardCeilingNs : Nat := 999999
def controlReplyHardCeilingNs : Nat := 999999
def coldBasePublicationHardCeilingMicros : Nat := 999999

inductive GenerationState where
  | missing
  | building
  | readyCurrent
  | readyStale
  | projectionMissing
  | failed
  deriving DecidableEq, Repr

inductive ReplyKind where
  | evidence
  | notReady
  | failed
  | cancelled
  deriving DecidableEq, Repr

structure SearchReply where
  kind : ReplyKind
  retryable : Bool
  deriving DecidableEq, Repr

def replyFor : GenerationState -> SearchReply
  | .missing => { kind := .notReady, retryable := true }
  | .building => { kind := .notReady, retryable := true }
  | .readyCurrent => { kind := .evidence, retryable := false }
  | .readyStale => { kind := .notReady, retryable := true }
  | .projectionMissing => { kind := .notReady, retryable := true }
  | .failed => { kind := .failed, retryable := false }

inductive ReadyEffect where
  | frameCodec
  | sessionGenerationLease
  | compileBoundedPlan
  | residentExactRead
  | residentLexicalRead
  | residentGraphRead
  | deterministicMerge
  | terminalEncode
  | processSpawn
  | providerRpc
  | workspaceScan
  | durableDbRead
  | durableDbWrite
  | generationMutation
  | endpointDiscovery
  | clientRetry
  | pythonGraphRpc
  | ripgrepProcess
  | crossProcessLaneJoin
  deriving DecidableEq, Repr

def readyEffectAllowed : ReadyEffect -> Bool
  | .frameCodec => true
  | .sessionGenerationLease => true
  | .compileBoundedPlan => true
  | .residentExactRead => true
  | .residentLexicalRead => true
  | .residentGraphRead => true
  | .deterministicMerge => true
  | .terminalEncode => true
  | .processSpawn => false
  | .providerRpc => false
  | .workspaceScan => false
  | .durableDbRead => false
  | .durableDbWrite => false
  | .generationMutation => false
  | .endpointDiscovery => false
  | .clientRetry => false
  | .pythonGraphRpc => false
  | .ripgrepProcess => false
  | .crossProcessLaneJoin => false

def readyTraceAdmitted (effects : List ReadyEffect) : Prop :=
  ∀ effect ∈ effects, readyEffectAllowed effect = true

def readyTraceCheck (effects : List ReadyEffect) : Bool :=
  effects.all readyEffectAllowed

structure SearchPlanBudget where
  exactEntries : Nat
  lexicalPostings : Nat
  graphNodes : Nat
  graphEdges : Nat
  outputBytes : Nat
  deriving DecidableEq, Repr

structure SearchPlanWork where
  exactEntries : Nat
  lexicalPostings : Nat
  graphNodes : Nat
  graphEdges : Nat
  outputBytes : Nat
  deriving DecidableEq, Repr

def workWithinBudget (work : SearchPlanWork) (budget : SearchPlanBudget) : Prop :=
  work.exactEntries ≤ budget.exactEntries ∧
  work.lexicalPostings ≤ budget.lexicalPostings ∧
  work.graphNodes ≤ budget.graphNodes ∧
  work.graphEdges ≤ budget.graphEdges ∧
  work.outputBytes ≤ budget.outputBytes

inductive ResidentRecallEffect where
  | normalizeQuery
  | visitIntegerPosting
  | updateBoundedTopK
  | materializeWinningOwner
  | cloneFullOwnerVocabulary
  | buildStringScoreMap
  | sortFullCandidateSet
  | scanAllOwners
  deriving DecidableEq, Repr

def residentRecallEffectAllowed : ResidentRecallEffect -> Bool
  | .normalizeQuery => true
  | .visitIntegerPosting => true
  | .updateBoundedTopK => true
  | .materializeWinningOwner => true
  | .cloneFullOwnerVocabulary => false
  | .buildStringScoreMap => false
  | .sortFullCandidateSet => false
  | .scanAllOwners => false

def residentRecallTraceAdmitted (effects : List ResidentRecallEffect) : Prop :=
  ∀ effect ∈ effects, residentRecallEffectAllowed effect = true

structure ResidentPostingPlan where
  normalizedTerms : Nat
  postingVisits : Nat
  topK : Nat
  materializedOwners : Nat
  matchedTermsPerOwner : Nat
  deriving DecidableEq, Repr

def residentPostingPlanBounded
    (termBudget postingBudget topKBudget matchedTermBudget : Nat)
    (plan : ResidentPostingPlan) : Prop :=
  plan.normalizedTerms ≤ termBudget ∧
  plan.postingVisits ≤ postingBudget ∧
  plan.topK ≤ topKBudget ∧
  plan.materializedOwners ≤ plan.topK ∧
  plan.matchedTermsPerOwner ≤ matchedTermBudget

theorem bounded_frontier_materializes_no_more_than_top_k
    (termBudget postingBudget topKBudget matchedTermBudget : Nat)
    (plan : ResidentPostingPlan)
    (bounded : residentPostingPlanBounded
      termBudget postingBudget topKBudget matchedTermBudget plan) :
    plan.materializedOwners ≤ plan.topK := by
  exact bounded.2.2.2.1

theorem legacy_string_sort_effects_are_not_admitted :
    residentRecallEffectAllowed .cloneFullOwnerVocabulary = false ∧
    residentRecallEffectAllowed .buildStringScoreMap = false ∧
    residentRecallEffectAllowed .sortFullCandidateSet = false ∧
    residentRecallEffectAllowed .scanAllOwners = false := by
  decide

structure ResidentQueryIdentity where
  project : Nat
  workspace : Nat
  generation : Nat
  sourceRoot : Nat
  provider : Nat
  indexArtifact : Nat
  normalizedQuery : Nat
  authority : Nat
  limit : Nat
  deriving DecidableEq, Repr

def residentQueryReusable
    (cached current : ResidentQueryIdentity) : Bool :=
  cached == current

theorem resident_query_reuse_requires_exact_generation_identity
    (cached current : ResidentQueryIdentity)
    (reused : residentQueryReusable cached current = true) :
    cached.generation = current.generation ∧
      cached.sourceRoot = current.sourceRoot ∧
      cached.indexArtifact = current.indexArtifact := by
  have equal : cached = current := by
    simpa [residentQueryReusable] using reused
  simp [equal]

/-! File discovery and immutable bytes alone publish the cold-searchable
    content generation. Native syntax, Tantivy, and graph artifacts may become
    visible later, independently, only under the exact content-generation identity. -/

inductive BaseGenerationEffect where
  | repositoryCandidateFilter
  | sourceByteRead
  | contentMerkle
  | canonicalResidentCommit
  | readyNotification
  | sourceIndexDbCommit
  | walFlush
  | providerProjectResolution
  | providerRpc
  | nativeSyntax
  | tantivy
  | graph
  deriving DecidableEq, Repr

def baseGenerationEffectAllowed : BaseGenerationEffect -> Bool
  | .repositoryCandidateFilter => true
  | .sourceByteRead => true
  | .contentMerkle => true
  | .canonicalResidentCommit => true
  | .readyNotification => true
  | .sourceIndexDbCommit => false
  | .walFlush => false
  | .providerProjectResolution => false
  | .providerRpc => false
  | .nativeSyntax => false
  | .tantivy => false
  | .graph => false

theorem base_publication_cannot_wait_for_semantic_enrichment :
    baseGenerationEffectAllowed .sourceIndexDbCommit = false ∧
    baseGenerationEffectAllowed .walFlush = false ∧
    baseGenerationEffectAllowed .providerProjectResolution = false ∧
    baseGenerationEffectAllowed .providerRpc = false ∧
    baseGenerationEffectAllowed .nativeSyntax = false ∧
    baseGenerationEffectAllowed .tantivy = false ∧
    baseGenerationEffectAllowed .graph = false := by
  decide

inductive DurabilityAttachmentState where
  | absent
  | building
  | ready (contentDigest : Nat)
  | failed (contentDigest : Nat)
  deriving DecidableEq, Repr

def residentGenerationRemainsQueryable
    (activeContentDigest : Nat)
    (_durability : DurabilityAttachmentState) : Nat :=
  activeContentDigest

def durabilityMayReplace
    (activeContentDigest : Nat)
    (durability : DurabilityAttachmentState) : Bool :=
  match durability with
  | .ready contentDigest => contentDigest == activeContentDigest
  | _ => false

theorem failed_durability_cannot_revoke_resident_generation
    (activeContentDigest failedDigest : Nat) :
    residentGenerationRemainsQueryable activeContentDigest
      (.failed failedDigest) = activeContentDigest := by
  rfl

theorem stale_durability_cannot_replace_generation
    (activeContentDigest durableDigest : Nat)
    (stale : durableDigest ≠ activeContentDigest) :
    durabilityMayReplace activeContentDigest (.ready durableDigest) = false := by
  simp [durabilityMayReplace, stale]

structure BaseGenerationTiming where
  candidateFilterMicros : Nat
  byteReadMicros : Nat
  merkleMicros : Nat
  residentCommitMicros : Nat
  readyNotificationMicros : Nat
  totalMicros : Nat
  deriving DecidableEq, Repr

def baseGenerationTimingAdmitted (timing : BaseGenerationTiming) : Prop :=
  timing.totalMicros = timing.candidateFilterMicros + timing.byteReadMicros +
    timing.merkleMicros + timing.residentCommitMicros +
    timing.readyNotificationMicros ∧
  timing.totalMicros ≤ coldBasePublicationHardCeilingMicros

theorem admitted_base_generation_is_subsecond
    (timing : BaseGenerationTiming)
    (admitted : baseGenerationTimingAdmitted timing) :
    timing.totalMicros < 1000000 := by
  unfold baseGenerationTimingAdmitted coldBasePublicationHardCeilingMicros at admitted
  omega

inductive SearchConstructionLane where
  | fdInventory
  | sourceBytes
  | nativeSyntax
  deriving DecidableEq, Repr

structure SearchLaneIdentity where
  project : Nat
  workspace : Nat
  sourceRoot : Nat
  providerSet : Nat
  schemaCatalog : Nat
  algorithmCatalog : Nat
  generationCandidate : Nat
  ownerSet : Nat
  deriving DecidableEq, Repr

structure ContentSearchGeneration where
  inventory : SearchLaneIdentity
  bytes : SearchLaneIdentity
  deriving DecidableEq, Repr

def contentGenerationAdmitted (generation : ContentSearchGeneration) : Bool :=
  generation.inventory == generation.bytes

theorem content_generation_requires_exact_lane_identity
    (generation : ContentSearchGeneration)
    (admitted : contentGenerationAdmitted generation = true) :
    generation.inventory = generation.bytes := by
  simp [contentGenerationAdmitted] at admitted
  exact admitted

inductive InteractiveGenerationReadKind where
  | search
  | exactQuery
  deriving DecidableEq, Repr

structure InteractiveGenerationReadiness where
  kind : InteractiveGenerationReadKind
  generationReady : Bool
  readinessDemandSubmitted : Bool
  foregroundWaitMicros : Nat
  searchWaitBudgetMicros : Nat
  deriving DecidableEq, Repr

def interactiveGenerationReadinessAdmitted
    (route : InteractiveGenerationReadiness) : Bool :=
  if route.generationReady then true
  else route.readinessDemandSubmitted &&
    match route.kind with
    | .search => route.foregroundWaitMicros <= route.searchWaitBudgetMicros
    | .exactQuery => route.foregroundWaitMicros == 0

theorem absent_exact_query_submits_without_foreground_wait
    (route : InteractiveGenerationReadiness)
    (exactQuery : route.kind = .exactQuery)
    (absent : route.generationReady = false)
    (admitted : interactiveGenerationReadinessAdmitted route = true) :
    route.readinessDemandSubmitted = true ∧ route.foregroundWaitMicros = 0 := by
  simp [interactiveGenerationReadinessAdmitted, exactQuery, absent] at admitted
  exact admitted

theorem absent_search_wait_is_bounded_by_its_dispatch_budget
    (route : InteractiveGenerationReadiness)
    (search : route.kind = .search)
    (absent : route.generationReady = false)
    (admitted : interactiveGenerationReadinessAdmitted route = true) :
    route.readinessDemandSubmitted = true ∧
      route.foregroundWaitMicros <= route.searchWaitBudgetMicros := by
  simp [interactiveGenerationReadinessAdmitted, search, absent] at admitted
  exact admitted

inductive DerivedSearchAttachmentKind where
  | nativeSyntax
  | tantivy
  | graph
  deriving DecidableEq, Repr

inductive SearchGenerationBuilderOwner where
  | aspServer
  | client
  | generationObject
  | provider
  deriving DecidableEq, Repr

def generationBuilderOwnerAdmitted : SearchGenerationBuilderOwner -> Bool
  | .aspServer => true
  | .client => false
  | .generationObject => false
  | .provider => false

theorem only_the_asp_server_may_own_the_generation_builder
    (owner : SearchGenerationBuilderOwner)
    (admitted : generationBuilderOwnerAdmitted owner = true) :
    owner = .aspServer := by
  cases owner <;> simp [generationBuilderOwnerAdmitted] at admitted ⊢

theorem generation_objects_cannot_own_detached_builders :
    generationBuilderOwnerAdmitted .generationObject = false := by
  rfl

inductive DerivedBuildStrategy where
  | tantivySingleSegment
  | tantivyParallelSegments
  | graph
  deriving DecidableEq, Repr

structure DerivedBuildObservationKey where
  strategy : DerivedBuildStrategy
  lexicalByteBucket : Nat
  ownerCountBucket : Nat
  changedOwnerRatioBucket : Nat
  deriving DecidableEq, Repr

def observedThroughputReusable
    (observed current : DerivedBuildObservationKey) : Bool :=
  observed == current

theorem throughput_history_requires_the_same_workload_and_strategy
    (observed current : DerivedBuildObservationKey)
    (reused : observedThroughputReusable observed current = true) :
    observed.strategy = current.strategy ∧
      observed.lexicalByteBucket = current.lexicalByteBucket ∧
      observed.ownerCountBucket = current.ownerCountBucket ∧
      observed.changedOwnerRatioBucket = current.changedOwnerRatioBucket := by
  have equal : observed = current := by
    simpa [observedThroughputReusable] using reused
  simp [equal]

structure DerivedBuildCalibrationIdentity where
  engine : Nat
  machineCpuPermits : Nat
  machineMemoryBytes : Nat
  workload : DerivedBuildObservationKey
  deriving DecidableEq, Repr

structure DerivedBuildCalibrationObservation where
  generation : Nat
  strategy : DerivedBuildStrategy
  workers : Nat
  memoryBytes : Nat
  measuredThroughput : Nat
  deriving DecidableEq, Repr

def productionObservationReusable
    (observed : DerivedBuildCalibrationObservation)
    (incomingGeneration : Nat) : Prop :=
  observed.generation < incomingGeneration

theorem a_generation_cannot_rebuild_from_its_own_calibration
    (observed : DerivedBuildCalibrationObservation)
    (generation : Nat)
    (reused : productionObservationReusable observed generation) :
    observed.generation ≠ generation := by
  exact Nat.ne_of_lt reused

def calibrationReceiptReusable
    (stored current : DerivedBuildCalibrationIdentity) : Bool :=
  stored == current

theorem calibration_reuse_requires_engine_machine_and_workload_identity
    (stored current : DerivedBuildCalibrationIdentity)
    (reused : calibrationReceiptReusable stored current = true) :
    stored.engine = current.engine ∧
      stored.machineCpuPermits = current.machineCpuPermits ∧
      stored.machineMemoryBytes = current.machineMemoryBytes ∧
      stored.workload = current.workload := by
  have equal : stored = current := by
    simpa [calibrationReceiptReusable] using reused
  simp [equal]

structure DerivedBuildResourceEnvelope where
  cpuCapacity : Nat
  memoryCapacity : Nat
  cpuPermits : Nat
  memoryPermits : Nat
  deriving DecidableEq, Repr

def derivedBuildResourceEnvelopeAdmitted
    (resources : DerivedBuildResourceEnvelope) : Prop :=
  0 < resources.cpuPermits ∧
    resources.cpuPermits ≤ resources.cpuCapacity ∧
    0 < resources.memoryPermits ∧
    resources.memoryPermits ≤ resources.memoryCapacity

theorem admitted_derived_build_cannot_oversubscribe_daemon_capacity
    (resources : DerivedBuildResourceEnvelope)
    (admitted : derivedBuildResourceEnvelopeAdmitted resources) :
    resources.cpuPermits ≤ resources.cpuCapacity ∧
      resources.memoryPermits ≤ resources.memoryCapacity := by
  exact ⟨admitted.2.1, admitted.2.2.2⟩

structure DerivedProductionBuildWork where
  attachmentBuilds : Nat
  calibrationSampleBuilds : Nat
  deriving DecidableEq, Repr

def derivedProductionBuildWorkAdmitted
    (work : DerivedProductionBuildWork) : Prop :=
  work.attachmentBuilds = 1 ∧ work.calibrationSampleBuilds = 0

theorem cold_calibration_adds_no_materialization
    (work : DerivedProductionBuildWork)
    (admitted : derivedProductionBuildWorkAdmitted work) :
    work.attachmentBuilds = 1 ∧ work.calibrationSampleBuilds = 0 := by
  exact admitted

inductive DerivedAttachmentStreamState where
  | queued
  | building
  | ready
  | failed
  deriving DecidableEq, Repr

def derivedAttachmentTransitionAdmitted
    (before after : DerivedAttachmentStreamState) : Bool :=
  match before, after with
  | .queued, .building => true
  | .building, .ready => true
  | .building, .failed => true
  | _, _ => false

theorem derived_attachment_stream_has_no_ready_shortcut
    (before : DerivedAttachmentStreamState) :
    derivedAttachmentTransitionAdmitted before .ready = true →
      before = .building := by
  cases before <;> simp [derivedAttachmentTransitionAdmitted]

structure DerivedAttachmentStreamIdentity where
  project : Nat
  workspace : Nat
  generationToken : Nat
  contentGeneration : Nat
  attachment : DerivedSearchAttachmentKind
  deriving DecidableEq, Repr

def derivedAttachmentEventNotOlder
    (stored incoming : DerivedAttachmentStreamIdentity) : Prop :=
  stored.generationToken ≤ incoming.generationToken

theorem late_derived_attachment_event_cannot_replace_current_generation
    (stored incoming : DerivedAttachmentStreamIdentity)
    (older : incoming.generationToken < stored.generationToken) :
    ¬ derivedAttachmentEventNotOlder stored incoming := by
  exact Nat.not_le_of_gt older

def derivedAttachmentStreamUpdateAdmitted
    (stored incoming : DerivedAttachmentStreamIdentity)
    (before after : DerivedAttachmentStreamState) : Bool :=
  stored == incoming && derivedAttachmentTransitionAdmitted before after

theorem derived_attachment_stream_cannot_cross_generation
    (stored incoming : DerivedAttachmentStreamIdentity)
    (before after : DerivedAttachmentStreamState)
    (admitted : derivedAttachmentStreamUpdateAdmitted stored incoming before after = true) :
    stored.contentGeneration = incoming.contentGeneration := by
  simp only [derivedAttachmentStreamUpdateAdmitted, Bool.and_eq_true, beq_iff_eq] at admitted
  simp [admitted.1]

structure DerivedAttachmentMaterializationReceipt where
  requestIdentity : DerivedAttachmentStreamIdentity
  buildCount : Nat
  digestProjectionCount : Nat
  retainedObjectCount : Nat
  deriving DecidableEq, Repr

def derivedAttachmentMaterializationAdmitted
    (receipt : DerivedAttachmentMaterializationReceipt) : Prop :=
  receipt.buildCount = 1 ∧
    receipt.digestProjectionCount = 1 ∧
    receipt.retainedObjectCount = 1

theorem admitted_attachment_cannot_materialize_once_for_digest_and_again_for_publication
    (receipt : DerivedAttachmentMaterializationReceipt)
    (admitted : derivedAttachmentMaterializationAdmitted receipt) :
    receipt.buildCount = 1 := by
  exact admitted.1

structure DerivedSearchAttachment where
  kind : DerivedSearchAttachmentKind
  contentIdentity : SearchLaneIdentity
  artifactIdentity : Nat
  complete : Bool
  deriving DecidableEq, Repr

def derivedAttachmentAdmitted
    (content : ContentSearchGeneration)
    (attachment : DerivedSearchAttachment) : Bool :=
  contentGenerationAdmitted content &&
    attachment.contentIdentity == content.inventory &&
    attachment.complete

theorem cold_admission_is_independent_of_derived_attachments
    (content : ContentSearchGeneration)
    (admitted : contentGenerationAdmitted content = true)
    (_nativeSyntax _tantivy _graph : DerivedSearchAttachment) :
    contentGenerationAdmitted content = true := by
  exact admitted

theorem derived_attachment_requires_exact_content_identity
    (content : ContentSearchGeneration)
    (attachment : DerivedSearchAttachment)
    (admitted : derivedAttachmentAdmitted content attachment = true) :
    attachment.contentIdentity = content.inventory := by
  simp only [derivedAttachmentAdmitted, Bool.and_eq_true, beq_iff_eq] at admitted
  exact admitted.1.2

theorem stale_derived_attachment_fails_closed
    (content : ContentSearchGeneration)
    (attachment : DerivedSearchAttachment)
    (drift : attachment.contentIdentity ≠ content.inventory) :
    derivedAttachmentAdmitted content attachment = false := by
  simp [derivedAttachmentAdmitted, drift]

structure LexicalAccelerator where
  contentIdentity : SearchLaneIdentity
  artifactIdentity : Nat
  rgTantivyEquivalent : Bool
  deriving DecidableEq, Repr

def lexicalAcceleratorAdmitted
    (content : ContentSearchGeneration)
    (accelerator : LexicalAccelerator) : Bool :=
  contentGenerationAdmitted content &&
    accelerator.contentIdentity == content.inventory &&
    accelerator.rgTantivyEquivalent

theorem content_generation_admission_does_not_require_an_accelerator
    (content : ContentSearchGeneration)
    (admitted : contentGenerationAdmitted content = true) :
    contentGenerationAdmitted content = true := admitted

theorem accelerator_requires_exact_content_identity_and_rg_equivalence
    (content : ContentSearchGeneration)
    (accelerator : LexicalAccelerator)
    (admitted : lexicalAcceleratorAdmitted content accelerator = true) :
    accelerator.contentIdentity = content.inventory ∧
      accelerator.rgTantivyEquivalent = true := by
  simp only [lexicalAcceleratorAdmitted, Bool.and_eq_true, beq_iff_eq] at admitted
  exact ⟨admitted.1.2, admitted.2⟩

theorem accelerator_content_drift_fails_closed
    (content : ContentSearchGeneration)
    (accelerator : LexicalAccelerator)
    (drift : accelerator.contentIdentity ≠ content.inventory) :
    lexicalAcceleratorAdmitted content accelerator = false := by
  simp [lexicalAcceleratorAdmitted, drift]

structure PythonCalibrationReceipt where
  identity : SearchLaneIdentity
  retainedQueryGenerations : Nat
  productionLaneFragments : Nat
  deriving DecidableEq, Repr

def pythonCalibrationReceiptAdmitted
    (receipt : PythonCalibrationReceipt) : Bool :=
  receipt.retainedQueryGenerations == 0 &&
    receipt.productionLaneFragments == 0

theorem admitted_python_calibration_owns_no_production_generation
    (receipt : PythonCalibrationReceipt)
    (admitted : pythonCalibrationReceiptAdmitted receipt = true) :
    receipt.retainedQueryGenerations = 0 ∧
      receipt.productionLaneFragments = 0 := by
  simpa [pythonCalibrationReceiptAdmitted] using admitted

inductive ReadyResidentLane where
  | lexicalRead
  | graphRead
  | byteEvidenceRead
  deriving DecidableEq, Repr

def readyResidentLaneEffects : ReadyResidentLane -> List ReadyEffect
  | .lexicalRead => [.residentLexicalRead]
  | .graphRead => [.residentGraphRead]
  | .byteEvidenceRead => [.residentExactRead]

theorem every_accelerated_ready_lane_is_resident
    (lane : ReadyResidentLane) :
    readyTraceAdmitted (readyResidentLaneEffects lane) := by
  intro effect member
  cases lane <;> simp [readyResidentLaneEffects] at member
  all_goals simp_all [readyEffectAllowed]

theorem python_graph_rpc_is_not_a_ready_lane :
    readyEffectAllowed .pythonGraphRpc = false := by rfl

theorem ripgrep_process_is_not_a_ready_lane :
    readyEffectAllowed .ripgrepProcess = false := by rfl

inductive ResidentByteEvidenceState where
  | exactMatches
  | completeAbsence
  | queryNotReady
  deriving DecidableEq, Repr

structure ResidentByteEvidenceBudget where
  gramWidth : Nat
  candidateLimit : Nat
  deriving DecidableEq, Repr

def residentByteEvidenceState
    (budget : ResidentByteEvidenceBudget)
    (queryBytes candidateOwners verifiedOwners exactMatches : Nat) :
    ResidentByteEvidenceState :=
  if queryBytes < budget.gramWidth ∨ budget.candidateLimit < candidateOwners then
    .queryNotReady
  else if verifiedOwners = candidateOwners ∧ exactMatches = 0 then
    .completeAbsence
  else
    .exactMatches

theorem complete_absence_requires_every_candidate_verified
    (budget : ResidentByteEvidenceBudget)
    (queryBytes candidateOwners verifiedOwners exactMatches : Nat)
    (absence : residentByteEvidenceState budget queryBytes candidateOwners
      verifiedOwners exactMatches = .completeAbsence) :
    verifiedOwners = candidateOwners ∧ exactMatches = 0 := by
  unfold residentByteEvidenceState at absence
  split at absence
  · contradiction
  · split at absence
    · assumption
    · contradiction

structure LanguageCapabilityDemand where
  requestedLanguage : Nat
  admittedLanguages : List Nat
  deriving DecidableEq, Repr

def languageWorkAdmitted
    (demand : LanguageCapabilityDemand)
    (language : Nat) : Bool :=
  language == demand.requestedLanguage &&
    demand.admittedLanguages.contains language

theorem unrelated_language_is_lazy
    (demand : LanguageCapabilityDemand)
    (language : Nat)
    (unrelated : language ≠ demand.requestedLanguage) :
    languageWorkAdmitted demand language = false := by
  simp [languageWorkAdmitted, unrelated]

/-! LSP is used as a countermodel, not as an execution template. -/

inductive LspFailureMode where
  | clientProcessInitializationAuthority
  | advertisedMethodWithoutTerminalRoute
  | requestTimeServerSpawn
  | mutableDocumentCacheAsMembership
  | detachedCancellation
  | dynamicCompatibilityDispatcher
  | perRequestCrossProcessFeatureChain
  deriving DecidableEq, Repr

def lspFailureModeAdmitted (_failure : LspFailureMode) : Bool := false

theorem lsp_failure_modes_are_rejected (failure : LspFailureMode) :
    lspFailureModeAdmitted failure = false := by rfl

structure PythonToolEnvironment where
  lockedDistributions : List String
  installedDistributions : List String
  deriving DecidableEq, Repr

def pythonToolEnvironmentExact (environment : PythonToolEnvironment) : Prop :=
  environment.installedDistributions = environment.lockedDistributions

theorem legacy_distribution_makes_python_tool_environment_inexact
    (locked installed : List String)
    (legacyDistribution : String)
    (legacyMissingFromLock : legacyDistribution ∉ locked) :
    ¬ pythonToolEnvironmentExact
      { lockedDistributions := locked
        installedDistributions := legacyDistribution :: installed } := by
  intro exact
  have present : legacyDistribution ∈ locked := by
    have exact' : legacyDistribution :: installed = locked := by
      simpa [pythonToolEnvironmentExact] using exact
    rw [← exact']
    simp
  exact legacyMissingFromLock present

structure ReadySegmentCost where
  frameCodecNs : Nat
  generationLeaseNs : Nat
  planNs : Nat
  residentReadNs : Nat
  mergeEncodeNs : Nat
  deriving DecidableEq, Repr

def ReadySegmentCost.totalNs (cost : ReadySegmentCost) : Nat :=
  cost.frameCodecNs + cost.generationLeaseNs + cost.planNs +
    cost.residentReadNs + cost.mergeEncodeNs

def readyCostAdmitted (cost : ReadySegmentCost) : Prop :=
  cost.totalNs ≤ readyHardCeilingNs

inductive RuntimeOwner where
  | sharedRustCore
  | languageProvider
  | generatedClient
  | grpcAdapter
  | httpDebugAdapter
  | legacyCli
  | legacyHttpServer
  | legacyProviderCache
  | legacyClientFallback
  | legacyGraphTurboExecutable
  | legacyTargetedPublication
  deriving DecidableEq, Repr

inductive RuntimeCapability where
  | factPublication
  | frameCodec
  | sessionConnection
  | searchPlan
  | generationAuthority
  | merkleAuthority
  | residentCache
  | ranking
  | retryPolicy
  | terminalAuthority
  deriving DecidableEq, Repr

def sharedRustCoreMayHold : RuntimeCapability -> Bool
  | .factPublication => true
  | .frameCodec => true
  | .sessionConnection => true
  | .searchPlan => true
  | .generationAuthority => true
  | .merkleAuthority => true
  | .residentCache => true
  | .ranking => true
  | .retryPolicy => true
  | .terminalAuthority => true

def languageProviderMayHold : RuntimeCapability -> Bool
  | .factPublication => true
  | .frameCodec => false
  | .sessionConnection => false
  | .searchPlan => false
  | .generationAuthority => false
  | .merkleAuthority => false
  | .residentCache => false
  | .ranking => false
  | .retryPolicy => false
  | .terminalAuthority => false

def transportMayHold : RuntimeCapability -> Bool
  | .factPublication => false
  | .frameCodec => true
  | .sessionConnection => true
  | .searchPlan => false
  | .generationAuthority => false
  | .merkleAuthority => false
  | .residentCache => false
  | .ranking => false
  | .retryPolicy => false
  | .terminalAuthority => false

def legacyMayHold : RuntimeCapability -> Bool
  | .factPublication => false
  | .frameCodec => false
  | .sessionConnection => false
  | .searchPlan => false
  | .generationAuthority => false
  | .merkleAuthority => false
  | .residentCache => false
  | .ranking => false
  | .retryPolicy => false
  | .terminalAuthority => false

def ownerMayHold : RuntimeOwner -> RuntimeCapability -> Bool
  | .sharedRustCore, capability => sharedRustCoreMayHold capability
  | .languageProvider, capability => languageProviderMayHold capability
  | .generatedClient, capability => transportMayHold capability
  | .grpcAdapter, capability => transportMayHold capability
  | .httpDebugAdapter, capability => transportMayHold capability
  | .legacyCli, capability => legacyMayHold capability
  | .legacyHttpServer, capability => legacyMayHold capability
  | .legacyProviderCache, capability => legacyMayHold capability
  | .legacyClientFallback, capability => legacyMayHold capability
  | .legacyGraphTurboExecutable, capability => legacyMayHold capability
  | .legacyTargetedPublication, capability => legacyMayHold capability

structure ArchitectureSurface where
  owner : RuntimeOwner
  capability : RuntimeCapability
  deriving DecidableEq, Repr

def architectureSurfaceAdmitted (surface : ArchitectureSurface) : Prop :=
  ownerMayHold surface.owner surface.capability = true

def architectureInventoryAdmitted (inventory : List ArchitectureSurface) : Prop :=
  ∀ surface ∈ inventory, architectureSurfaceAdmitted surface

def architectureSurfaceCheck (surface : ArchitectureSurface) : Bool :=
  ownerMayHold surface.owner surface.capability

def architectureInventoryCheck (inventory : List ArchitectureSurface) : Bool :=
  inventory.all architectureSurfaceCheck

/-! Architecture edges are generated from the real Cargo feature/dependency
    graph, Rust re-exports and calls, and the Runtime method catalog. Keeping
    these edge kinds in the proof prevents a removed direct call from hiding an
    equivalent legacy path behind a feature, facade, or route registration. -/

inductive ArchitectureEdgeKind where
  | cargoFeature
  | cargoDependency
  | rustReexport
  | runtimeRoute
  | rustCall
  deriving DecidableEq, Repr

structure ArchitectureEdge where
  source : RuntimeOwner
  target : RuntimeOwner
  kind : ArchitectureEdgeKind
  enabled : Bool
  deriving DecidableEq, Repr

inductive ArchitectureReachable (edges : List ArchitectureEdge) :
    RuntimeOwner -> RuntimeOwner -> Prop where
  | direct (edge : ArchitectureEdge)
      (hMember : edge ∈ edges)
      (hEnabled : edge.enabled = true) :
      ArchitectureReachable edges edge.source edge.target
  | trans {source middle target : RuntimeOwner}
      (head : ArchitectureReachable edges source middle)
      (tail : ArchitectureReachable edges middle target) :
      ArchitectureReachable edges source target

def productionEntry : RuntimeOwner -> Bool
  | .sharedRustCore => false
  | .languageProvider => false
  | .generatedClient => true
  | .grpcAdapter => true
  | .httpDebugAdapter => false
  | .legacyCli => false
  | .legacyHttpServer => false
  | .legacyProviderCache => false
  | .legacyClientFallback => false
  | .legacyGraphTurboExecutable => false
  | .legacyTargetedPublication => false

def legacyOwner : RuntimeOwner -> Bool
  | .sharedRustCore => false
  | .languageProvider => false
  | .generatedClient => false
  | .grpcAdapter => false
  | .httpDebugAdapter => false
  | .legacyCli => true
  | .legacyHttpServer => true
  | .legacyProviderCache => true
  | .legacyClientFallback => true
  | .legacyGraphTurboExecutable => true
  | .legacyTargetedPublication => true

def RuntimeOwner.all : List RuntimeOwner := [
  .sharedRustCore,
  .languageProvider,
  .generatedClient,
  .grpcAdapter,
  .httpDebugAdapter,
  .legacyCli,
  .legacyHttpServer,
  .legacyProviderCache,
  .legacyClientFallback,
  .legacyGraphTurboExecutable,
  .legacyTargetedPublication
]

def RuntimeOwner.code : RuntimeOwner -> Nat
  | .sharedRustCore => 0
  | .languageProvider => 1
  | .generatedClient => 2
  | .grpcAdapter => 3
  | .httpDebugAdapter => 4
  | .legacyCli => 5
  | .legacyHttpServer => 6
  | .legacyProviderCache => 7
  | .legacyClientFallback => 8
  | .legacyGraphTurboExecutable => 9
  | .legacyTargetedPublication => 10

def runtimeOwnerEq (left right : RuntimeOwner) : Bool :=
  left.code == right.code

structure ArchitectureInventory where
  owners : List RuntimeOwner
  surfaces : List ArchitectureSurface
  edges : List ArchitectureEdge
  deriving Repr

def sourceInventoryHardCut (inventory : ArchitectureInventory) : Prop :=
  ∀ owner ∈ inventory.owners, legacyOwner owner = false

def legacyReachableFromProduction (inventory : ArchitectureInventory) : Prop :=
  ∃ entry legacy,
    productionEntry entry = true ∧
    legacyOwner legacy = true ∧
    ArchitectureReachable inventory.edges entry legacy

def legacyCanReachSharedAuthority (inventory : ArchitectureInventory) : Prop :=
  ∃ legacy,
    legacyOwner legacy = true ∧
    ArchitectureReachable inventory.edges legacy .sharedRustCore

def architectureImpactClosed (inventory : ArchitectureInventory) : Prop :=
  sourceInventoryHardCut inventory ∧
  architectureInventoryAdmitted inventory.surfaces ∧
  ¬legacyReachableFromProduction inventory ∧
  ¬legacyCanReachSharedAuthority inventory

def architectureReachableWithin :
    Nat -> List ArchitectureEdge -> RuntimeOwner -> RuntimeOwner -> Bool
  | 0, _, _, _ => false
  | fuel + 1, edges, source, target =>
      edges.any fun edge =>
        edge.enabled && runtimeOwnerEq edge.source source &&
          (runtimeOwnerEq edge.target target ||
            architectureReachableWithin fuel edges edge.target target)

def sourceInventoryHardCutCheck (inventory : ArchitectureInventory) : Bool :=
  inventory.owners.all fun owner => !(legacyOwner owner)

def legacyReachableFromProductionCheck (inventory : ArchitectureInventory) : Bool :=
  RuntimeOwner.all.any fun entry =>
    productionEntry entry && RuntimeOwner.all.any fun legacy =>
      legacyOwner legacy &&
        architectureReachableWithin RuntimeOwner.all.length inventory.edges entry legacy

def legacyCanReachSharedAuthorityCheck (inventory : ArchitectureInventory) : Bool :=
  RuntimeOwner.all.any fun legacy =>
    legacyOwner legacy &&
      architectureReachableWithin RuntimeOwner.all.length inventory.edges legacy .sharedRustCore

def architectureImpactCheck (inventory : ArchitectureInventory) : Bool :=
  sourceInventoryHardCutCheck inventory &&
  architectureInventoryCheck inventory.surfaces &&
  !(legacyReachableFromProductionCheck inventory) &&
  !(legacyCanReachSharedAuthorityCheck inventory)

inductive Transport where
  | directCore
  | grpc
  | httpDebug
  deriving DecidableEq, Repr

inductive PublicEntryPoint where
  | languageFacade
  | formalQualification
  deriving DecidableEq, Repr

def entersRuntimeAdmission (_entryPoint : PublicEntryPoint) : Bool := true

structure RuntimeActivationObservation where
  endpointHealthy : Bool
  transactionIdentityBound : Bool
  deriving DecidableEq, Repr

def runtimeActivationReady (observation : RuntimeActivationObservation) : Bool :=
  observation.endpointHealthy && observation.transactionIdentityBound

inductive TerminalKind where
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

inductive TerminalDiagnosticKind where
  | none
  | failure
  | cancellation
  deriving DecidableEq, Repr

structure TerminalFrameShape where
  terminal : TerminalKind
  diagnostic : TerminalDiagnosticKind
  deriving DecidableEq, Repr

def terminalFrameShapeAdmitted (frame : TerminalFrameShape) : Bool :=
  match frame.terminal, frame.diagnostic with
  | .ready, .none => true
  | .failed, .failure => true
  | .cancelled, .cancellation => true
  | _, _ => false

structure SemanticReceipt where
  workspaceIdentity : Nat
  sourceRootDigest : Nat
  generationDigest : Nat
  schemaCatalogDigest : Nat
  algorithmDigest : Nat
  resultDigest : Nat
  terminal : TerminalKind
  deriving DecidableEq, Repr

def transportProjection (_transport : Transport) (receipt : SemanticReceipt) :
    SemanticReceipt := receipt

def exactlyOneTerminal (terminals : List TerminalKind) : Prop :=
  terminals.length = 1

def exactlyOneAdmittedTerminalFrame (frames : List TerminalFrameShape) : Prop :=
  frames.length = 1 ∧ frames.all terminalFrameShapeAdmitted = true

def firstReceiptAdmitted (elapsedNs : Nat) : Prop :=
  elapsedNs ≤ controlReplyHardCeilingNs

def warmExactAdmitted (elapsedNs : Nat) : Prop :=
  elapsedNs ≤ readyHardCeilingNs

def warmSearchAdmitted (elapsedNs : Nat) : Prop :=
  elapsedNs ≤ readyHardCeilingNs

theorem fortySecondForegroundWaitRejected :
    ¬firstReceiptAdmitted 40000000000 := by
  unfold firstReceiptAdmitted controlReplyHardCeilingNs
  exact Nat.not_le_of_gt (by decide)

theorem everyGenerationStateReturnsProtocolReply (state : GenerationState) :
    (replyFor state).kind = .evidence ∨
    (replyFor state).kind = .notReady ∨
    (replyFor state).kind = .failed := by
  cases state with
  | missing => exact Or.inr (Or.inl rfl)
  | building => exact Or.inr (Or.inl rfl)
  | readyCurrent => exact Or.inl rfl
  | readyStale => exact Or.inr (Or.inl rfl)
  | projectionMissing => exact Or.inr (Or.inl rfl)
  | failed => exact Or.inr (Or.inr rfl)

theorem evidenceRequiresCurrentGeneration
    (state : GenerationState)
    (hEvidence : (replyFor state).kind = .evidence) :
    state = .readyCurrent := by
  cases state with
  | missing => cases hEvidence
  | building => cases hEvidence
  | readyCurrent => rfl
  | readyStale => cases hEvidence
  | projectionMissing => cases hEvidence
  | failed => cases hEvidence

theorem missingGenerationReturnsNotReady :
    (replyFor .missing).kind = .notReady := by
  rfl

theorem buildingGenerationReturnsNotReady :
    (replyFor .building).kind = .notReady := by
  rfl

theorem staleGenerationReturnsNotReady :
    (replyFor .readyStale).kind = .notReady := by
  rfl

theorem terminalFailureReturnsFailed :
    (replyFor .failed).kind = .failed := by
  rfl

theorem providerRpcForbiddenFromReadyTrace :
    readyEffectAllowed .providerRpc = false := by rfl

theorem workspaceScanForbiddenFromReadyTrace :
    readyEffectAllowed .workspaceScan = false := by rfl

theorem generationMutationForbiddenFromReadyTrace :
    readyEffectAllowed .generationMutation = false := by rfl

theorem clientRetryForbiddenFromReadyTrace :
    readyEffectAllowed .clientRetry = false := by rfl

theorem providerCannotOwnSearchPlan :
    ownerMayHold .languageProvider .searchPlan = false := by rfl

theorem clientCannotOwnGeneration :
    ownerMayHold .generatedClient .generationAuthority = false := by rfl

theorem httpAdapterCannotOwnCache :
    ownerMayHold .httpDebugAdapter .residentCache = false := by rfl

theorem grpcAdapterCannotOwnTerminal :
    ownerMayHold .grpcAdapter .terminalAuthority = false := by rfl

theorem directLegacyRouteExposesProductionImpact :
    legacyReachableFromProductionCheck {
      owners := [.generatedClient, .legacyCli]
      surfaces := []
      edges := [{
        source := .generatedClient
        target := .legacyCli
        kind := .runtimeRoute
        enabled := true
      }]
    } = true := by decide

theorem transitiveFeatureAndReexportExposeLegacyImpact :
    legacyReachableFromProductionCheck {
      owners := [.grpcAdapter, .generatedClient, .legacyClientFallback]
      surfaces := []
      edges := [
        {
          source := .grpcAdapter
          target := .generatedClient
          kind := .cargoFeature
          enabled := true
        },
        {
          source := .generatedClient
          target := .legacyClientFallback
          kind := .rustReexport
          enabled := true
        }
      ]
    } = true := by decide

theorem disabledLegacyFeatureDoesNotCreateReachability :
    architectureReachableWithin RuntimeOwner.all.length [{
      source := .generatedClient
      target := .legacyCli
      kind := .cargoFeature
      enabled := false
    }] .generatedClient .legacyCli = false := by decide

theorem legacyInventoryRowBreaksHardCut :
    legacyOwner .legacyGraphTurboExecutable = true := by rfl

theorem legacyAuthorityIngressBreaksImpactClosure :
    legacyOwner .legacyTargetedPublication = true := by rfl

theorem transportAblationPreservesSemanticReceipt
    (transport : Transport) (receipt : SemanticReceipt) :
    transportProjection transport receipt = receipt := by
  rfl

theorem formalQualificationCannotBypassRuntimeAdmission :
    entersRuntimeAdmission .formalQualification = true := by
  rfl

theorem languageFacadeCannotBypassRuntimeAdmission :
    entersRuntimeAdmission .languageFacade = true := by
  rfl

theorem healthyEndpointBeforeTransactionCommitIsNotReady :
    runtimeActivationReady {
      endpointHealthy := true
      transactionIdentityBound := false
    } = false := by
  rfl

theorem healthyBoundTransactionIsReady :
    runtimeActivationReady {
      endpointHealthy := true
      transactionIdentityBound := true
    } = true := by
  rfl

theorem oneReadyTerminalIsExactlyOne :
    exactlyOneTerminal [.ready] := by
  rfl

theorem cancelledWithDiagnosticIsExactlyOneTerminal :
    exactlyOneAdmittedTerminalFrame [{
      terminal := .cancelled
      diagnostic := .cancellation
    }] := by
  simp [exactlyOneAdmittedTerminalFrame, terminalFrameShapeAdmitted]

theorem bareCancelledTerminalRejected :
    ¬exactlyOneAdmittedTerminalFrame [{
      terminal := .cancelled
      diagnostic := .none
    }] := by
  simp [exactlyOneAdmittedTerminalFrame, terminalFrameShapeAdmitted]

theorem errorLabelledCancellationRejected :
    ¬exactlyOneAdmittedTerminalFrame [{
      terminal := .failed
      diagnostic := .cancellation
    }] := by
  simp [exactlyOneAdmittedTerminalFrame, terminalFrameShapeAdmitted]

theorem duplicateTerminalRejected (first second : TerminalKind) :
    ¬ exactlyOneTerminal [first, second] := by
  intro duplicate
  have twoIsNotOne : (2 : Nat) ≠ 1 := by decide
  exact twoIsNotOne duplicate

theorem referenceReadySegmentsFitHardCeiling :
    readyCostAdmitted {
      frameCodecNs := 120000
      generationLeaseNs := 80000
      planNs := 50000
      residentReadNs := 300000
      mergeEncodeNs := 150000
    } := by
  unfold readyCostAdmitted ReadySegmentCost.totalNs readyHardCeilingNs
  decide

theorem oldTenMillisecondWarmSearchBudgetRejected :
    ¬warmSearchAdmitted 10000000 := by
  unfold warmSearchAdmitted readyHardCeilingNs
  exact Nat.not_le_of_gt (by decide)

theorem backgroundDeadlineCannotReplaceFirstReceiptBudget
    (background : Nat)
    (hSlow : controlReplyHardCeilingNs < background) :
    ¬firstReceiptAdmitted background := by
  exact Nat.not_le_of_gt hSlow

/-- A provider target constrains readiness, never workspace publication
membership.  Active publication is representable only for complete registry
coverage. -/
inductive GenerationBuildScope where
  | completeGeneration
  | targetProvider
  deriving DecidableEq, Repr

structure WorkspaceProviderCoverage where
  registeredProviderCount : Nat
  projectedProviderCount : Nat
  deriving DecidableEq, Repr

def workspaceGenerationPublishable
    (scope : GenerationBuildScope)
    (coverage : WorkspaceProviderCoverage) : Bool :=
  scope == .completeGeneration &&
    coverage.projectedProviderCount == coverage.registeredProviderCount

def buildScopeForQueryDemand (_targetProviderPresent : Bool) : GenerationBuildScope :=
  .completeGeneration

theorem queryDemandTargetCannotNarrowGenerationBuild
    (targetProviderPresent : Bool) :
    buildScopeForQueryDemand targetProviderPresent = .completeGeneration := by
  rfl

theorem targetProviderGenerationCannotBecomeActive
    (coverage : WorkspaceProviderCoverage) :
    workspaceGenerationPublishable .targetProvider coverage = false := by
  simp [workspaceGenerationPublishable]

theorem incompleteProviderCoverageCannotBecomeActive
    (coverage : WorkspaceProviderCoverage)
    (incomplete : coverage.projectedProviderCount ≠ coverage.registeredProviderCount) :
    workspaceGenerationPublishable .completeGeneration coverage = false := by
  simp [workspaceGenerationPublishable, incomplete]

end ASPProof.ASPInteractiveSearchLatency
