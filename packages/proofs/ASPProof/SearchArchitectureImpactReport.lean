import ASPProof.ASPInteractiveSearchLatency

namespace ASPProof.SearchArchitectureImpactReport

open ASPProof.ASPInteractiveSearchLatency

/-! The producer-facing inventory remains stringly and neutral.  Only this Lean
    module maps declared owners/capabilities into the architecture model and
    decides whether a legacy path is reachable. -/

structure FactNode where
  nodeId : String
  sourceSelector : String
  declaredOwner : String
  capabilityClaims : List String
  deriving DecidableEq, Repr

structure FactEdge where
  sourceNodeId : String
  targetNodeId : String
  kind : String
  enabled : Bool
  evidenceSelector : String
  deriving DecidableEq, Repr

structure FactInventory where
  inventoryDigest : String
  productionFeatureSet : List String
  productionEntries : List String
  nodes : List FactNode
  edges : List FactEdge
  deriving DecidableEq, Repr

structure CapabilityViolation where
  nodeId : String
  declaredOwner : String
  capability : String
  sourceSelector : String
  deriving DecidableEq, Repr

inductive ProofState where
  | admitted
  | rejected
  deriving DecidableEq, Repr

structure ProofReport where
  inventoryDigest : String
  state : ProofState
  unknownNodes : List String
  capabilityViolations : List CapabilityViolation
  legacyNodes : List String
  productionToLegacyPaths : List (List String)
  legacyToAuthorityPaths : List (List String)
  axioms : List String
  deriving DecidableEq, Repr

def ownerFromId : String -> Option RuntimeOwner
  | "shared-rust-core" => some .sharedRustCore
  | "language-provider" => some .languageProvider
  | "generated-client" => some .generatedClient
  | "grpc-adapter" => some .grpcAdapter
  | "http-debug-adapter" => some .httpDebugAdapter
  | "legacy-cli" => some .legacyCli
  | "legacy-http-server" => some .legacyHttpServer
  | "legacy-provider-cache" => some .legacyProviderCache
  | "legacy-client-fallback" => some .legacyClientFallback
  | "legacy-graph-turbo-executable" => some .legacyGraphTurboExecutable
  | "legacy-targeted-publication" => some .legacyTargetedPublication
  | _ => none

def capabilityFromId : String -> Option RuntimeCapability
  | "fact-publication" => some .factPublication
  | "frame-codec" => some .frameCodec
  | "session-connection" => some .sessionConnection
  | "search-plan" => some .searchPlan
  | "generation-authority" => some .generationAuthority
  | "merkle-authority" => some .merkleAuthority
  | "resident-cache" => some .residentCache
  | "ranking" => some .ranking
  | "retry-policy" => some .retryPolicy
  | "terminal-authority" => some .terminalAuthority
  | _ => none

def FactInventory.node? (inventory : FactInventory) (nodeId : String) : Option FactNode :=
  inventory.nodes.find? fun node => node.nodeId == nodeId

def FactInventory.owner? (inventory : FactInventory) (nodeId : String) : Option RuntimeOwner :=
  inventory.node? nodeId >>= fun node => ownerFromId node.declaredOwner

def FactInventory.enabledSuccessors
    (inventory : FactInventory) (nodeId : String) : List String :=
  inventory.edges.filterMap fun edge =>
    if edge.enabled && edge.sourceNodeId == nodeId then some edge.targetNodeId else none

def pathEndsIn (targets : List String) (path : List String) : Bool :=
  match path.getLast? with
  | some nodeId => targets.contains nodeId
  | none => false

def expandPath (inventory : FactInventory) (path : List String) : List (List String) :=
  match path.getLast? with
  | none => []
  | some nodeId =>
      (inventory.enabledSuccessors nodeId).filterMap fun target =>
        if path.contains target then none else some (path ++ [target])

def shortestPathLoop
    (inventory : FactInventory) (targets : List String) :
    Nat -> List (List String) -> Option (List String)
  | 0, _ => none
  | fuel + 1, frontier =>
      match frontier.find? (pathEndsIn targets) with
      | some path => some path
      | none => shortestPathLoop inventory targets fuel (frontier.flatMap (expandPath inventory))

def shortestPath
    (inventory : FactInventory) (source : String) (targets : List String) :
    Option (List String) :=
  shortestPathLoop inventory targets (inventory.nodes.length + 1) [[source]]

def dedupStrings (values : List String) : List String :=
  values.foldl (fun unique value =>
    if unique.contains value then unique else unique ++ [value]) []

def FactInventory.unknownNodes (inventory : FactInventory) : List String :=
  let unknownDeclarations := inventory.nodes.filterMap fun node =>
    if (ownerFromId node.declaredOwner).isNone ||
        node.capabilityClaims.any fun claim => (capabilityFromId claim).isNone then
      some node.nodeId
    else
      none
  let unknownEntries := inventory.productionEntries.filter fun nodeId =>
    (inventory.node? nodeId).isNone
  let unknownEdges := inventory.edges.flatMap fun edge =>
    [edge.sourceNodeId, edge.targetNodeId].filter fun nodeId =>
      (inventory.node? nodeId).isNone
  dedupStrings (unknownDeclarations ++ unknownEntries ++ unknownEdges)

def FactInventory.capabilityViolations
    (inventory : FactInventory) : List CapabilityViolation :=
  inventory.nodes.flatMap fun node =>
    node.capabilityClaims.filterMap fun claim =>
      match ownerFromId node.declaredOwner with
      | none => none
      | some owner =>
          match capabilityFromId claim with
          | none => none
          | some capability =>
              match ownerMayHold owner capability with
              | true => none
              | false =>
                  some {
                    nodeId := node.nodeId
                    declaredOwner := node.declaredOwner
                    capability := claim
                    sourceSelector := node.sourceSelector
                  }

def FactInventory.legacyNodes (inventory : FactInventory) : List String :=
  inventory.nodes.filterMap fun node =>
    match ownerFromId node.declaredOwner with
    | some owner => if legacyOwner owner then some node.nodeId else none
    | none => none

def FactInventory.sharedAuthorityNodes (inventory : FactInventory) : List String :=
  inventory.nodes.filterMap fun node =>
    match ownerFromId node.declaredOwner with
    | none => none
    | some owner =>
        match runtimeOwnerEq owner .sharedRustCore with
        | true => some node.nodeId
        | false => none

def FactInventory.productionToLegacyPaths
    (inventory : FactInventory) : List (List String) :=
  let targets := inventory.legacyNodes
  inventory.productionEntries.filterMap fun entry =>
    match shortestPath inventory entry targets with
    | some path => if path.length ≥ 2 then some path else none
    | none => none

def FactInventory.legacyToAuthorityPaths
    (inventory : FactInventory) : List (List String) :=
  let targets := inventory.sharedAuthorityNodes
  inventory.legacyNodes.filterMap fun legacy =>
    match shortestPath inventory legacy targets with
    | some path => if path.length ≥ 2 then some path else none
    | none => none

def buildProofReport (inventory : FactInventory) : ProofReport :=
  let unknownNodes := inventory.unknownNodes
  let capabilityViolations := inventory.capabilityViolations
  let legacyNodes := inventory.legacyNodes
  let productionToLegacyPaths := inventory.productionToLegacyPaths
  let legacyToAuthorityPaths := inventory.legacyToAuthorityPaths
  let admitted := unknownNodes.isEmpty && capabilityViolations.isEmpty &&
    legacyNodes.isEmpty && productionToLegacyPaths.isEmpty && legacyToAuthorityPaths.isEmpty
  {
    inventoryDigest := inventory.inventoryDigest
    state := match admitted with
      | true => .admitted
      | false => .rejected
    unknownNodes
    capabilityViolations
    legacyNodes
    productionToLegacyPaths
    legacyToAuthorityPaths
    axioms := []
  }

def directFallbackInventory : FactInventory := {
  inventoryDigest := "blake3-256:fixture"
  productionFeatureSet := ["agent-semantic-client/default"]
  productionEntries := ["client"]
  nodes := [
    {
      nodeId := "client"
      sourceSelector := "rust://client"
      declaredOwner := "generated-client"
      capabilityClaims := ["frame-codec"]
    },
    {
      nodeId := "fallback"
      sourceSelector := "rust://fallback"
      declaredOwner := "legacy-client-fallback"
      capabilityClaims := []
    }
  ]
  edges := [{
    sourceNodeId := "client"
    targetNodeId := "fallback"
    kind := "rust-reexport"
    enabled := true
    evidenceSelector := "rust://client#item/reexport/fallback"
  }]
}

theorem directFallbackProducesRejectedReport :
    (buildProofReport directFallbackInventory).state = .rejected := by decide

theorem directFallbackPreservesShortestCounterexample :
    (buildProofReport directFallbackInventory).productionToLegacyPaths =
      [["client", "fallback"]] := by decide

def orphanLegacyProviderCacheInventory : FactInventory := {
  inventoryDigest := "blake3-256:orphan-legacy-provider-cache"
  productionFeatureSet := ["agent-semantic-client-db/default"]
  productionEntries := ["runtime-server"]
  nodes := [
    {
      nodeId := "runtime-server"
      sourceSelector := "rust://crates/agent-semantic-client-db/src/runtime_server/core.rs"
      declaredOwner := "shared-rust-core"
      capabilityClaims := ["resident-cache"]
    },
    {
      nodeId := "graph-turbo-cache"
      sourceSelector := "rust://crates/agent-semantic-client-db/src/graph_turbo_cache.rs"
      declaredOwner := "legacy-provider-cache"
      capabilityClaims := ["resident-cache"]
    }
  ]
  edges := []
}

theorem orphanLegacyProviderCacheBreaksSourceHardCut :
    (buildProofReport orphanLegacyProviderCacheInventory).state = .rejected := by decide

theorem orphanLegacyProviderCacheIsReportedEvenWhenUnreachable :
    (buildProofReport orphanLegacyProviderCacheInventory).legacyNodes =
      ["graph-turbo-cache"] := by decide

def generatedClientSearchPlannerInventory : FactInventory := {
  inventoryDigest := "blake3-256:generated-client-search-planner"
  productionFeatureSet := ["agent-semantic-client/default"]
  productionEntries := ["client-graph-planner"]
  nodes := [{
    nodeId := "client-graph-planner"
    sourceSelector := "rust://crates/agent-semantic-client/src/command/search_router_graph_state.rs"
    declaredOwner := "generated-client"
    capabilityClaims := ["search-plan", "ranking"]
  }]
  edges := []
}

theorem generatedClientSearchPlannerIsRejected :
    (buildProofReport generatedClientSearchPlannerInventory).state = .rejected := by decide

theorem generatedClientSearchPlannerReportsBothCapabilities :
    (buildProofReport generatedClientSearchPlannerInventory).capabilityViolations.length = 2 := by
  decide

def legacyGraphEvaluateReadyRouteInventory : FactInventory := {
  inventoryDigest := "blake3-256:legacy-graph-evaluate-ready-route"
  productionFeatureSet := ["agent-semantic-runtime-server/default"]
  productionEntries := ["generated-client"]
  nodes := [
    {
      nodeId := "generated-client"
      sourceSelector := "rust://crates/agent-semantic-client/src/runtime_language_client.rs"
      declaredOwner := "generated-client"
      capabilityClaims := ["frame-codec"]
    },
    {
      nodeId := "runtime-resident-search"
      sourceSelector := "rust://crates/agent-semantic-runtime-server/src/runtime_asp_client.rs"
      declaredOwner := "shared-rust-core"
      capabilityClaims := ["search-plan", "generation-authority", "resident-cache", "ranking", "terminal-authority"]
    },
    {
      nodeId := "python-graph-query-executable"
      sourceSelector := "rust://crates/agent-semantic-client/src/server/runtime_server_search_service.rs"
      declaredOwner := "legacy-graph-turbo-executable"
      capabilityClaims := ["resident-cache", "ranking"]
    }
  ]
  edges := [
    {
      sourceNodeId := "generated-client"
      targetNodeId := "runtime-resident-search"
      kind := "grpc-route"
      enabled := true
      evidenceSelector := "asp.graphs.evaluate"
    },
    {
      sourceNodeId := "runtime-resident-search"
      targetNodeId := "python-graph-query-executable"
      kind := "provider-rpc"
      enabled := true
      evidenceSelector := "GraphServer::open_generation_shared_with_token"
    }
  ]
}

theorem legacyGraphEvaluateReadyRouteIsRejected :
    (buildProofReport legacyGraphEvaluateReadyRouteInventory).state = .rejected := by decide

theorem legacyGraphEvaluateReadyRoutePreservesShortestCounterexample :
    (buildProofReport legacyGraphEvaluateReadyRouteInventory).productionToLegacyPaths =
      [["generated-client", "runtime-resident-search", "python-graph-query-executable"]] := by decide

theorem legacyGraphEvaluateReadyRouteReportsPythonExecutable :
    (buildProofReport legacyGraphEvaluateReadyRouteInventory).legacyNodes =
      ["python-graph-query-executable"] := by decide

/-! A client bootstrap is an observation of Runtime authority, not an implicit
    lifecycle loop.  In particular, a live process without the endpoint that
    binds its content identity is divergent authority rather than readiness.
    Every observation maps to one typed terminal decision. -/

inductive RuntimeBootstrapDecision where
  | ready
  | starting
  | endpointAuthorityDiverged
  | cancelled
  deriving DecidableEq, Repr

def decideRuntimeBootstrap
    (ownerAlive endpointBound cancellationObserved : Bool) : RuntimeBootstrapDecision :=
  if cancellationObserved then
    .cancelled
  else if endpointBound && ownerAlive then
    .ready
  else if ownerAlive then
    .endpointAuthorityDiverged
  else
    .starting

def RuntimeBootstrapDecision.isTypedTerminal : RuntimeBootstrapDecision -> Bool
  | .ready | .starting | .endpointAuthorityDiverged | .cancelled => true

theorem missingEndpointWithLiveOwnerFailsClosed :
    decideRuntimeBootstrap true false false = .endpointAuthorityDiverged := by
  decide

theorem missingEndpointWithoutOwnerIsTypedStarting :
    decideRuntimeBootstrap false false false = .starting := by
  decide

theorem cancellationDominatesBootstrapObservation
    (ownerAlive endpointBound : Bool) :
    decideRuntimeBootstrap ownerAlive endpointBound true = .cancelled := by
  simp [decideRuntimeBootstrap]

theorem bootstrapObservationAlwaysProducesTypedTerminal
    (ownerAlive endpointBound cancellationObserved : Bool) :
    (decideRuntimeBootstrap ownerAlive endpointBound cancellationObserved).isTypedTerminal = true := by
  cases ownerAlive <;> cases endpointBound <;> cases cancellationObserved <;> decide

structure CancellationObservation where
  clientCancelled : Bool
  queryProcessAlive : Bool
  deriving DecidableEq, Repr

def CancellationObservation.valid (observation : CancellationObservation) : Bool :=
  !(observation.clientCancelled && observation.queryProcessAlive)

theorem cancelledClientWithLiveQueryProcessIsRejected :
    !(CancellationObservation.valid {
      clientCancelled := true
      queryProcessAlive := true
    }) := by
  decide

theorem propagatedCancellationIsValid :
    CancellationObservation.valid {
      clientCancelled := true
      queryProcessAlive := false
    } := by
  decide

structure RuntimeSupervisionObservation where
  residentOwnerHoldsElection : Bool
  residentOwnerHoldsSupervisorTransaction : Bool
  residentOwnerAwaitsOwnExit : Bool
  deriving DecidableEq, Repr

def RuntimeSupervisionObservation.valid
    (observation : RuntimeSupervisionObservation) : Bool :=
  !(observation.residentOwnerHoldsElection &&
    observation.residentOwnerHoldsSupervisorTransaction) &&
  !observation.residentOwnerAwaitsOwnExit

theorem residentOwnerSelfHandoffIsRejected :
    !(RuntimeSupervisionObservation.valid {
      residentOwnerHoldsElection := true
      residentOwnerHoldsSupervisorTransaction := true
      residentOwnerAwaitsOwnExit := true
    }) := by
  decide

theorem externalSupervisorAfterResidentExitIsValid :
    RuntimeSupervisionObservation.valid {
      residentOwnerHoldsElection := false
      residentOwnerHoldsSupervisorTransaction := true
      residentOwnerAwaitsOwnExit := false
    } := by
  decide

/-! A CompleteGeneration is complete over the Runtime's active provider
    artifacts, not over stale capability declarations.  A capability without a
    content-proven artifact has no execution authority and therefore cannot
    block an unrelated language whose selected artifact is present. -/

inductive ProviderAdmissionDecision where
  | ready
  | selectedCapabilityMissing
  | selectedArtifactMissing
  | selectedAuthorityAmbiguous
  deriving DecidableEq, Repr

def decideProviderAdmission
    (selectedCapabilityPresent selectedArtifactPresent selectedAuthorityAmbiguous
      _unrelatedCapabilityWithoutArtifact : Bool) : ProviderAdmissionDecision :=
  if selectedAuthorityAmbiguous then
    .selectedAuthorityAmbiguous
  else if !selectedCapabilityPresent then
    .selectedCapabilityMissing
  else if !selectedArtifactPresent then
    .selectedArtifactMissing
  else
    .ready

theorem unrelatedCapabilityWithoutArtifactCannotBlockSelectedProvider
    (unrelatedCapabilityWithoutArtifact : Bool) :
    decideProviderAdmission true true false unrelatedCapabilityWithoutArtifact = .ready := by
  simp [decideProviderAdmission]

theorem selectedCapabilityWithoutArtifactFailsClosed
    (unrelatedCapabilityWithoutArtifact : Bool) :
    decideProviderAdmission true false false unrelatedCapabilityWithoutArtifact =
      .selectedArtifactMissing := by
  simp [decideProviderAdmission]

theorem ambiguousSelectedAuthorityFailsClosed
    (selectedCapabilityPresent selectedArtifactPresent
      unrelatedCapabilityWithoutArtifact : Bool) :
    decideProviderAdmission selectedCapabilityPresent selectedArtifactPresent true
      unrelatedCapabilityWithoutArtifact = .selectedAuthorityAmbiguous := by
  simp [decideProviderAdmission]

/-! Interactive read latency and cold generation progress are distinct
    budgets. Applying the read deadline to a correctly progressing cold
    admission manufactures a failure and abandons the only authority that can
    publish the generation. Cancellation remains valid for both classes. -/

inductive RuntimeDispatchClass where
  | interactiveRead
  | coldGenerationAdmission
  deriving DecidableEq, Repr

def RuntimeDispatchClass.hasInteractiveDeadline : RuntimeDispatchClass -> Bool
  | .interactiveRead => true
  | .coldGenerationAdmission => false

theorem coldGenerationCannotBeFailedByInteractiveDeadline :
    RuntimeDispatchClass.coldGenerationAdmission.hasInteractiveDeadline = false := by
  rfl

theorem warmReadRetainsInteractiveDeadline :
    RuntimeDispatchClass.interactiveRead.hasInteractiveDeadline = true := by
  rfl

/-! A transport adapter is not a second deadline authority.  Every layer must
    preserve the method-derived dispatch class; otherwise gRPC or a CLI wrapper
    can reintroduce an interactive timeout after the service correctly admitted
    a cold generation build. -/

inductive RuntimeDispatchLayer where
  | service
  | grpcTransport
  | cliSession
  deriving DecidableEq, Repr

def RuntimeDispatchLayer.hasInteractiveDeadline
    (_layer : RuntimeDispatchLayer) (dispatchClass : RuntimeDispatchClass) : Bool :=
  dispatchClass.hasInteractiveDeadline

theorem coldGenerationDeadlineFreeAtEveryLayer (layer : RuntimeDispatchLayer) :
    layer.hasInteractiveDeadline .coldGenerationAdmission = false := by
  cases layer <;> rfl

theorem warmReadDeadlinePreservedAtEveryLayer (layer : RuntimeDispatchLayer) :
    layer.hasInteractiveDeadline .interactiveRead = true := by
  cases layer <;> rfl

/-! Persisted generation bytes from an incompatible schema are neither an
    active base nor a lifecycle blocker.  They remain inert rollback evidence
    until the sole publisher atomically installs a current-schema generation. -/

inductive PersistedGenerationCompatibility where
  | absent
  | current
  | incompatibleSchema
  deriving DecidableEq, Repr

inductive PersistedGenerationAdmission where
  | reuseCurrent
  | rebuildCurrent
  deriving DecidableEq, Repr

def decidePersistedGenerationAdmission :
    PersistedGenerationCompatibility -> PersistedGenerationAdmission
  | .current => .reuseCurrent
  | .absent | .incompatibleSchema => .rebuildCurrent

theorem incompatibleSchemaCannotBlockCurrentRebuild :
    decidePersistedGenerationAdmission .incompatibleSchema = .rebuildCurrent := by
  rfl

theorem incompatibleSchemaCannotBecomeActiveBase :
    decidePersistedGenerationAdmission .incompatibleSchema != .reuseCurrent := by
  decide

/-! The northbound transport is a stream of bounded wire envelopes, not one
    unbounded unary payload.  A logical response larger than one envelope is
    content-bound and partitioned; only the final verified partition may
    complete the request.  Raising a codec-wide limit cannot become an
    alternative transport authority. -/

structure ResponsePartition where
  index : Nat
  count : Nat
  byteCount : Nat
  deriving DecidableEq, Repr

def ResponsePartition.valid (maximumEnvelopeBytes : Nat)
    (partition : ResponsePartition) : Prop :=
  0 < partition.count ∧
  partition.index < partition.count ∧
  0 < partition.byteCount ∧
  partition.byteCount <= maximumEnvelopeBytes

def ResponsePartition.completes (partition : ResponsePartition) : Prop :=
  partition.index + 1 = partition.count

theorem responsePartitionNeverExceedsEnvelopeBudget
    (maximumEnvelopeBytes : Nat) (partition : ResponsePartition)
    (valid : ResponsePartition.valid maximumEnvelopeBytes partition) :
    partition.byteCount <= maximumEnvelopeBytes := by
  exact valid.2.2.2

theorem nonFinalPartitionCannotComplete
    (partition : ResponsePartition)
    (notFinal : partition.index + 1 != partition.count) :
    ¬ partition.completes := by
  simpa [ResponsePartition.completes] using notFinal

/-! A warm Search receipt preserves the complete bounded lexical candidate
    count, but expensive parser-owned selector projection is admitted only
    after graph ranking and under an independent owner budget.  Candidate
    evidence and projection work therefore cannot be conflated. -/

structure RankedOwnerProjection where
  lexicalCandidateCount : Nat
  selectorProjectionBudget : Nat
  projectedOwnerCount : Nat
  receiptCandidateCount : Nat
  deriving DecidableEq, Repr

def RankedOwnerProjection.valid (projection : RankedOwnerProjection) : Prop :=
  0 < projection.selectorProjectionBudget ∧
  projection.projectedOwnerCount <= projection.selectorProjectionBudget ∧
  projection.receiptCandidateCount = projection.lexicalCandidateCount

theorem rankedOwnerProjectionIsBudgetBounded
    (projection : RankedOwnerProjection)
    (valid : projection.valid) :
    projection.projectedOwnerCount <= projection.selectorProjectionBudget := by
  exact valid.2.1

theorem rankedOwnerProjectionPreservesCandidateEvidence
    (projection : RankedOwnerProjection)
    (valid : projection.valid) :
    projection.receiptCandidateCount = projection.lexicalCandidateCount := by
  exact valid.2.2

structure WarmResidentResultReuse where
  cachedGeneration : Nat
  requestGeneration : Nat
  payloadCloneCount : Nat
  deriving DecidableEq, Repr

def WarmResidentResultReuse.valid (reuse : WarmResidentResultReuse) : Prop :=
  reuse.cachedGeneration = reuse.requestGeneration ∧ reuse.payloadCloneCount = 0

theorem warmResidentReuseCannotDeepClonePayload
    (reuse : WarmResidentResultReuse)
    (valid : reuse.valid) :
    reuse.payloadCloneCount = 0 := by
  exact valid.2

theorem warmResidentReuseCannotCrossGeneration
    (reuse : WarmResidentResultReuse)
    (valid : reuse.valid) :
    reuse.cachedGeneration = reuse.requestGeneration := by
  exact valid.1

theorem proofReportNeverInventsAxioms (inventory : FactInventory) :
    (buildProofReport inventory).axioms = [] := by rfl

end ASPProof.SearchArchitectureImpactReport
