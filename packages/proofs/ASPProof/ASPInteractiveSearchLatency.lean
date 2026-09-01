namespace ASPProof.ASPInteractiveSearchLatency

/-! Lean proves that a Ready request contains only bounded resident work and
    that transports and language providers cannot acquire shared Search
    authority. Executable receipts provide the observed nanoseconds. -/

def readyP50BudgetNs : Nat := 250000
def readyP99BudgetNs : Nat := 700000
def readyHardCeilingNs : Nat := 999999
def controlReplyHardCeilingNs : Nat := 999999

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

inductive TerminalKind where
  | ready
  | failed
  | cancelled
  deriving DecidableEq, Repr

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

theorem oneReadyTerminalIsExactlyOne :
    exactlyOneTerminal [.ready] := by
  rfl

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
