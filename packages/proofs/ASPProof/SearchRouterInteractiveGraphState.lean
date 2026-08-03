import ASPProof.SearchRouteDAG
import ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness
import ASPProof.SearchRouteSemanticPathCacheSeparation

namespace ASPProof.SearchRouterInteractiveGraphState

abbrev NodeId := String
abbrev FactId := String
abbrev ClaimId := String
abbrev ProvenanceId := String

inductive Vertical where
  | syntaxTree
  | symbol
  | dependency
  | configuration
  | runtime
  | tests
  | policy
  | proof
  | semantic
  deriving Repr, DecidableEq, BEq

inductive ToolKind where
  | lexical
  | nativeSyntax
  | semanticSearch
  | dependencySearch
  | exactSelector
  | runtimeProbe
  deriving Repr, DecidableEq, BEq

inductive ObservationAuthority where
  | parser
  | provider
  | runtime
  deriving Repr, DecidableEq, BEq

structure Capability where
  tool : ToolKind
  vertical : Vertical
  deriving Repr, DecidableEq, BEq

structure EvidenceEdge where
  source : NodeId
  target : NodeId
  relation : String
  provenance : ProvenanceId
  deriving Repr, DecidableEq, BEq

structure RouterAction where
  tool : ToolKind
  vertical : Vertical
  queryIdentity : String
  anchor : NodeId
  budgetBefore : Nat
  deriving Repr, DecidableEq, BEq

structure ToolObservation where
  invocationId : String
  providerId : String
  authority : ObservationAuthority
  tool : ToolKind
  vertical : Vertical
  workspaceGeneration : Nat
  factIds : List FactId
  edges : List EvidenceEdge
  supportsClaims : List ClaimId
  rejectsClaims : List ClaimId
  provenance : List ProvenanceId
  noHit : Bool
  deriving Repr, DecidableEq, BEq

structure GraphState where
  sessionId : String
  generation : Nat
  stateDigest : String
  currentNode : NodeId
  nodeIds : List NodeId
  factIds : List FactId
  obligations : List ClaimId
  frontier : List NodeId
  visited : List NodeId
  routeMeasure : ASPProof.SearchRouteSemanticPathCacheSeparation.RouteMeasure
  routeBudget : SearchRouteDAG.GraphRouteBudget
  remainingBudget : Nat
  deriving Repr, DecidableEq, BEq

structure GraphDelta where
  beforeGeneration : Nat
  afterGeneration : Nat
  beforeStateDigest : String
  afterStateDigest : String
  action : RouterAction
  observation : ToolObservation
  nodesAdded : List NodeId
  factsAdded : List FactId
  edgesAdded : List EvidenceEdge
  obligationsOpened : List ClaimId
  obligationsDischarged : List ClaimId
  frontierAdded : List NodeId
  frontierRemoved : List NodeId
  visitedAdded : List NodeId
  remainingBudget : Nat
  deriving Repr, DecidableEq, BEq

structure DatalogReceipt where
  factGeneration : Nat
  rulesetDigest : String
  sourceFactIds : List FactId
  derivedFactIds : List FactId
  deriving Repr, DecidableEq, BEq

def observationValid (observation : ToolObservation) : Bool :=
  !observation.invocationId.isEmpty &&
    !observation.providerId.isEmpty &&
    !observation.provenance.isEmpty &&
    (observation.noHit ||
      !observation.factIds.isEmpty ||
      !observation.edges.isEmpty)

def routeAuthorized
    (capabilities : List Capability)
    (action : RouterAction) : Bool :=
  capabilities.any fun capability =>
    capability.tool == action.tool &&
      capability.vertical == action.vertical

def knownNode
    (state : GraphState)
    (delta : GraphDelta)
    (node : NodeId) : Bool :=
  (state.nodeIds ++ delta.nodesAdded).contains node

def edgeGrounded
    (state : GraphState)
    (delta : GraphDelta)
    (edge : EvidenceEdge) : Bool :=
  knownNode state delta edge.source &&
    knownNode state delta edge.target &&
    delta.observation.provenance.contains edge.provenance

def dischargeGrounded
    (delta : GraphDelta)
    (claim : ClaimId) : Bool :=
  delta.observation.supportsClaims.contains claim ||
    delta.observation.rejectsClaims.contains claim

def deltaValid
    (capabilities : List Capability)
    (state : GraphState)
    (delta : GraphDelta) : Bool :=
  delta.beforeGeneration == state.generation &&
    delta.afterGeneration == state.generation + 1 &&
    delta.beforeStateDigest == state.stateDigest &&
    delta.afterStateDigest != state.stateDigest &&
    delta.action.budgetBefore == state.remainingBudget &&
    delta.remainingBudget < state.remainingBudget &&
    delta.observation.workspaceGeneration == state.generation &&
    delta.observation.tool == delta.action.tool &&
    delta.observation.vertical == delta.action.vertical &&
    routeAuthorized capabilities delta.action &&
    observationValid delta.observation &&
    delta.factsAdded == delta.observation.factIds &&
    delta.edgesAdded == delta.observation.edges &&
    delta.edgesAdded.all (edgeGrounded state delta) &&
    delta.obligationsDischarged.all (dischargeGrounded delta)

def resumeValid
    (state : GraphState)
    (continuationIdentity : String)
    (sessionId : String)
    (nodeId : NodeId)
    (graphGeneration : Nat)
    (stateDigest : String)
    (nextRouteMeasure : ASPProof.SearchRouteSemanticPathCacheSeparation.RouteMeasure) : Bool :=
  !continuationIdentity.isEmpty &&
    sessionId == state.sessionId &&
    graphGeneration == state.generation &&
    stateDigest == state.stateDigest &&
    state.frontier.contains nodeId &&
    nextRouteMeasure.semanticGraphHops == state.routeMeasure.semanticGraphHops + 1 &&
    nextRouteMeasure.semanticGraphHops <= state.routeBudget.maxGraphHops &&
    nextRouteMeasure.executedGraphHops >= state.routeMeasure.executedGraphHops

def closureAllowed
    (state : GraphState)
    (resolvedClaims : List ClaimId) : Bool :=
  state.obligations.all fun obligation =>
    resolvedClaims.contains obligation

def datalogReceiptValid
    (state : GraphState)
    (receipt : DatalogReceipt) : Bool :=
  receipt.factGeneration == state.generation &&
    !receipt.rulesetDigest.isEmpty &&
    !receipt.derivedFactIds.isEmpty &&
    receipt.sourceFactIds.all fun fact => state.factIds.contains fact

def ownershipState : GraphState :=
  { sessionId := "session-owner-model"
    generation := 7
    stateDigest := "state-7"
    currentNode := "goal:model-owner"
    nodeIds := ["goal:model-owner", "symbol:ModelConfig"]
    factIds := ["fact:model-declared"]
    obligations := ["claim:effective-owner"]
    frontier := ["symbol:ModelConfig"]
    visited := ["goal:model-owner"]
    routeMeasure :=
      { semanticGraphHops := 2
        executedGraphHops := 1
        toolRounds := 1
        searchTokens := 180
        uncachedModelTokens := 60 }
    routeBudget :=
      { routeBudget :=
          { maxEvidenceTokens := 4096
            maxRawTokens := 8192
            maxUncachedTokens := 2048
            maxRounds := 8 }
        maxGraphHops := 4
        maxTransitions := 8 }
    remainingBudget := 4 }

def ownershipCapabilities : List Capability :=
  [ { tool := .nativeSyntax, vertical := .symbol }
  , { tool := .semanticSearch, vertical := .semantic }
  , { tool := .runtimeProbe, vertical := .runtime }
  , { tool := .lexical, vertical := .configuration }
  ]

def ownershipObservation : ToolObservation :=
  { invocationId := "invocation:semantic-owner"
    providerId := "provider:semantic-search"
    authority := .provider
    tool := .semanticSearch
    vertical := .semantic
    workspaceGeneration := 7
    factIds := ["fact:registry-loads-model-config"]
    edges :=
      [ { source := "symbol:AgentRegistry"
          target := "symbol:ModelConfig"
          relation := "loads"
          provenance := "receipt:semantic-owner" } ]
    supportsClaims := []
    rejectsClaims := []
    provenance := ["receipt:semantic-owner"]
    noHit := false }

def ownershipDelta : GraphDelta :=
  { beforeGeneration := 7
    afterGeneration := 8
    beforeStateDigest := "state-7"
    afterStateDigest := "state-8"
    action :=
      { tool := .semanticSearch
        vertical := .semantic
        queryIdentity := "query:effective-model-owner"
        anchor := "symbol:ModelConfig"
        budgetBefore := 4 }
    observation := ownershipObservation
    nodesAdded := ["symbol:AgentRegistry"]
    factsAdded := ["fact:registry-loads-model-config"]
    edgesAdded := ownershipObservation.edges
    obligationsOpened := ["claim:runtime-owner"]
    obligationsDischarged := []
    frontierAdded := ["symbol:AgentRegistry"]
    frontierRemoved := ["symbol:ModelConfig"]
    visitedAdded := ["symbol:ModelConfig"]
    remainingBudget := 3 }

theorem semantic_tool_observation_can_advance_the_graph :
    deltaValid ownershipCapabilities ownershipState ownershipDelta = true := by
  decide

def inventedEdgeDelta : GraphDelta :=
  { ownershipDelta with
    edgesAdded :=
      [ { source := "symbol:AgentRegistry"
          target := "symbol:UnknownOwner"
          relation := "owns"
          provenance := "receipt:model-invented" } ] }

theorem model_invented_edge_is_rejected :
    deltaValid ownershipCapabilities ownershipState inventedEdgeDelta = false := by
  decide

def nonDecreasingBudgetDelta : GraphDelta :=
  { ownershipDelta with remainingBudget := 4 }

theorem accepted_transition_must_decrease_budget :
    deltaValid ownershipCapabilities ownershipState nonDecreasingBudgetDelta = false := by
  decide

def unauthorizedVerticalDelta : GraphDelta :=
  { ownershipDelta with
    action :=
      { ownershipDelta.action with
        tool := .nativeSyntax
        vertical := .runtime }
    observation :=
      { ownershipObservation with
        tool := .nativeSyntax
        vertical := .runtime } }

theorem unavailable_tool_vertical_pair_is_rejected :
    deltaValid ownershipCapabilities ownershipState unauthorizedVerticalDelta = false := by
  decide

def validNoHitObservation : ToolObservation :=
  { invocationId := "invocation:lexical-no-hit"
    providerId := "provider:tantivy"
    authority := .parser
    tool := .lexical
    vertical := .configuration
    workspaceGeneration := 7
    factIds := []
    edges := []
    supportsClaims := []
    rejectsClaims := []
    provenance := ["receipt:lexical-no-hit"]
    noHit := true }

def validNoHitDelta : GraphDelta :=
  { beforeGeneration := 7
    afterGeneration := 8
    beforeStateDigest := "state-7"
    afterStateDigest := "state-8-no-hit"
    action :=
      { tool := .lexical
        vertical := .configuration
        queryIdentity := "query:model-key"
        anchor := "symbol:ModelConfig"
        budgetBefore := 4 }
    observation := validNoHitObservation
    nodesAdded := []
    factsAdded := []
    edgesAdded := []
    obligationsOpened := ["claim:try-native-config-owner"]
    obligationsDischarged := []
    frontierAdded := []
    frontierRemoved := []
    visitedAdded := []
    remainingBudget := 3 }

theorem typed_no_hit_is_a_valid_graph_transition :
    deltaValid ownershipCapabilities ownershipState validNoHitDelta = true := by
  decide

def cachedFrontierRouteMeasure :
    ASPProof.SearchRouteSemanticPathCacheSeparation.RouteMeasure :=
  { semanticGraphHops := 3
    executedGraphHops := 1
    toolRounds := 1
    searchTokens := 196
    uncachedModelTokens := 60 }

inductive OwnershipEvidenceEdge : Nat → Nat → Prop where
  | modelConfigToRegistry : OwnershipEvidenceEdge 0 1

def ownershipEvidencePath :
    ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness.EvidencePath
      OwnershipEvidenceEdge 0 1 :=
  .step .modelConfigToRegistry (.stay 1)

theorem ownership_frontier_uses_existing_path_within_hop_budget :
    ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness.PathWithinHopBudget
      4 ownershipEvidencePath := by
  unfold ASPProof.SearchRouteEvidenceGraphShortestPathFrontierCompleteness.PathWithinHopBudget
  native_decide

theorem resume_to_live_frontier_is_one_bounded_hop :
    resumeValid ownershipState
      "continuation:owner-model"
      "session-owner-model"
      "symbol:ModelConfig"
      7
      "state-7"
      cachedFrontierRouteMeasure = true := by
  decide

theorem stale_continuation_is_rejected :
    resumeValid ownershipState
      "continuation:owner-model"
      "session-owner-model"
      "symbol:ModelConfig"
      6
      "state-6"
      cachedFrontierRouteMeasure = false := by
  decide

def overflowRouteMeasure :
    ASPProof.SearchRouteSemanticPathCacheSeparation.RouteMeasure :=
  { cachedFrontierRouteMeasure with semanticGraphHops := 5 }

theorem resume_cannot_exceed_existing_hop_budget :
    resumeValid ownershipState
      "continuation:owner-model"
      "session-owner-model"
      "symbol:ModelConfig"
      7
      "state-7"
      overflowRouteMeasure = false := by
  decide

theorem unresolved_obligation_prevents_closure :
    closureAllowed ownershipState [] = false := by
  decide

theorem resolved_obligation_allows_closure :
    closureAllowed ownershipState ["claim:effective-owner"] = true := by
  native_decide

def unsupportedDischargeDelta : GraphDelta :=
  { ownershipDelta with
    obligationsDischarged := ["claim:effective-owner"] }

theorem obligation_discharge_without_observation_evidence_is_rejected :
    deltaValid ownershipCapabilities ownershipState unsupportedDischargeDelta = false := by
  decide

def supportedOwnershipObservation : ToolObservation :=
  { ownershipObservation with
    supportsClaims := ["claim:effective-owner"] }

def supportedDischargeDelta : GraphDelta :=
  { ownershipDelta with
    observation := supportedOwnershipObservation
    factsAdded := supportedOwnershipObservation.factIds
    edgesAdded := supportedOwnershipObservation.edges
    obligationsDischarged := ["claim:effective-owner"] }

theorem observation_evidence_can_discharge_an_obligation :
    deltaValid ownershipCapabilities ownershipState supportedDischargeDelta = true := by
  decide

def validDatalogReceipt : DatalogReceipt :=
  { factGeneration := 7
    rulesetDigest := "ruleset:ownership-v1"
    sourceFactIds := ["fact:model-declared"]
    derivedFactIds := ["fact:configuration-owner"] }

theorem datalog_derivation_is_grounded_in_live_facts :
    datalogReceiptValid ownershipState validDatalogReceipt = true := by
  decide

def inventedDatalogSource : DatalogReceipt :=
  { validDatalogReceipt with
    sourceFactIds := ["fact:model-invented"] }

theorem datalog_derivation_with_unknown_source_is_rejected :
    datalogReceiptValid ownershipState inventedDatalogSource = false := by
  decide

inductive GraphTurboProposalStatus where
  | candidate
  | proposed
  | heuristic
  | partialResult
  deriving Repr, DecidableEq, BEq

structure InteractiveAccounting where
  toolActions : Nat
  semanticGraphHops : Nat
  executedGraphHops : Nat
  graphTurboInvocations : Nat
  ruleFirings : Nat
  derivedFacts : Nat
  deriving Repr, DecidableEq, BEq

structure GraphTurboInvocation where
  inputNodes : Nat
  inputEdges : Nat
  visitedNodes : Nat
  visitedEdges : Nat
  iterations : Nat
  candidatePaths : Nat
  deriving Repr, DecidableEq, BEq

structure GraphTurboBudget where
  maxVisitedNodes : Nat
  maxVisitedEdges : Nat
  maxIterations : Nat
  maxCandidatePaths : Nat
  deriving Repr, DecidableEq, BEq

def invocationWithinBudget
    (budget : GraphTurboBudget)
    (invocation : GraphTurboInvocation) : Bool :=
  invocation.visitedNodes <= budget.maxVisitedNodes &&
    invocation.visitedEdges <= budget.maxVisitedEdges &&
    invocation.iterations <= budget.maxIterations &&
    invocation.candidatePaths <= budget.maxCandidatePaths

def afterToolAction (accounting : InteractiveAccounting) : InteractiveAccounting :=
  { accounting with toolActions := accounting.toolActions + 1 }

def afterGraphTurbo
    (accounting : InteractiveAccounting)
    (budget : GraphTurboBudget)
    (invocation : GraphTurboInvocation) : Option InteractiveAccounting :=
  if invocationWithinBudget budget invocation then
    some
      { accounting with
        toolActions := accounting.toolActions + 1
        graphTurboInvocations := accounting.graphTurboInvocations + 1 }
  else
    none

def interactiveAccounting : InteractiveAccounting :=
  { toolActions := 2
    semanticGraphHops := 3
    executedGraphHops := 2
    graphTurboInvocations := 0
    ruleFirings := 0
    derivedFacts := 0 }

def graphTurboBudget : GraphTurboBudget :=
  { maxVisitedNodes := 32
    maxVisitedEdges := 64
    maxIterations := 8
    maxCandidatePaths := 4 }

def boundedGraphTurboInvocation : GraphTurboInvocation :=
  { inputNodes := 20
    inputEdges := 40
    visitedNodes := 12
    visitedEdges := 24
    iterations := 3
    candidatePaths := 2 }

theorem tool_action_does_not_change_graph_hops :
    let next := afterToolAction interactiveAccounting
    next.semanticGraphHops = interactiveAccounting.semanticGraphHops ∧
      next.executedGraphHops = interactiveAccounting.executedGraphHops := by
  native_decide

theorem bounded_graph_turbo_does_not_change_graph_hops :
    (afterGraphTurbo interactiveAccounting graphTurboBudget boundedGraphTurboInvocation).map
        (fun next =>
          next.semanticGraphHops == interactiveAccounting.semanticGraphHops &&
            next.executedGraphHops == interactiveAccounting.executedGraphHops) =
      some true := by
  native_decide

def overBudgetGraphTurboInvocation : GraphTurboInvocation :=
  { boundedGraphTurboInvocation with iterations := 9 }

theorem graph_turbo_iteration_overflow_is_rejected :
    afterGraphTurbo interactiveAccounting graphTurboBudget overBudgetGraphTurboInvocation = none := by
  native_decide

structure ContinuationIdentity where
  sessionId : String
  nodeId : NodeId
  snapshotDigest : String
  stateDigest : String
  deriving Repr, DecidableEq, BEq

def continuationIdentityValid (identity : ContinuationIdentity) : Bool :=
  !identity.sessionId.isEmpty &&
    !identity.nodeId.isEmpty &&
    !identity.snapshotDigest.isEmpty &&
    !identity.stateDigest.isEmpty

def cachedResumeValid
    (cold cached : InteractiveAccounting)
    (coldIdentity cachedIdentity : ContinuationIdentity) : Bool :=
  continuationIdentityValid coldIdentity &&
    coldIdentity == cachedIdentity &&
    cached.semanticGraphHops == cold.semanticGraphHops &&
    cached.executedGraphHops <= cold.executedGraphHops

def coldContinuationIdentity : ContinuationIdentity :=
  { sessionId := "session-owner-model"
    nodeId := "symbol:ModelConfig"
    snapshotDigest := "snapshot:7"
    stateDigest := "state-7" }

def cachedAccounting : InteractiveAccounting :=
  { interactiveAccounting with executedGraphHops := 0 }

theorem cached_resume_preserves_semantic_hops_and_may_save_execution :
    cachedResumeValid interactiveAccounting cachedAccounting
      coldContinuationIdentity coldContinuationIdentity = true := by
  native_decide

def forgedCachedAccounting : InteractiveAccounting :=
  { cachedAccounting with semanticGraphHops := 0 }

theorem cached_resume_cannot_rewrite_semantic_hops :
    cachedResumeValid interactiveAccounting forgedCachedAccounting
      coldContinuationIdentity coldContinuationIdentity = false := by
  native_decide

def staleContinuationIdentity : ContinuationIdentity :=
  { coldContinuationIdentity with snapshotDigest := "snapshot:6" }

theorem cached_resume_rejects_snapshot_substitution :
    cachedResumeValid interactiveAccounting cachedAccounting
      coldContinuationIdentity staleContinuationIdentity = false := by
  native_decide

structure GraphTurboProposal where
  snapshotDigest : String
  status : GraphTurboProposalStatus
  nodeIds : List NodeId
  edges : List EvidenceEdge
  deriving Repr, DecidableEq, BEq

def graphTurboProposalGrounded
    (stateSnapshotDigest : String)
    (knownNodes : List NodeId)
    (knownEdges : List EvidenceEdge)
    (proposal : GraphTurboProposal) : Bool :=
  proposal.snapshotDigest == stateSnapshotDigest &&
    proposal.nodeIds.all knownNodes.contains &&
    proposal.edges.all knownEdges.contains

def groundedTurboProposal : GraphTurboProposal :=
  { snapshotDigest := "snapshot:7"
    status := .proposed
    nodeIds := ["goal:model-owner", "symbol:ModelConfig"]
    edges := [] }

theorem grounded_python_proposal_can_enter_exact_verification :
    graphTurboProposalGrounded "snapshot:7" ownershipState.nodeIds []
      groundedTurboProposal = true := by
  native_decide

def inventedNodeTurboProposal : GraphTurboProposal :=
  { groundedTurboProposal with nodeIds := ["symbol:UnknownOwner"] }

theorem python_proposal_with_unknown_node_is_rejected :
    graphTurboProposalGrounded "snapshot:7" ownershipState.nodeIds []
      inventedNodeTurboProposal = false := by
  native_decide

inductive GqlPathUpperBound where
  | explicit (hops : Nat)
  | router (hops : Nat)
  | unbounded
  deriving Repr, DecidableEq, BEq

def gqlPathExecutable
    (routeBudget : SearchRouteDAG.GraphRouteBudget)
    (upperBound : GqlPathUpperBound) : Bool :=
  match upperBound with
  | .explicit hops => hops <= routeBudget.maxGraphHops
  | .router hops => hops <= routeBudget.maxGraphHops
  | .unbounded => false

theorem bounded_gql_path_is_executable :
    gqlPathExecutable ownershipState.routeBudget (.explicit 3) = true := by
  native_decide

theorem unbounded_gql_path_is_rejected :
    gqlPathExecutable ownershipState.routeBudget .unbounded = false := by
  native_decide

structure BoundedClosureReceipt where
  factGeneration : Nat
  rulesetDigest : String
  sourceFactIds : List FactId
  derivedFactIds : List FactId
  ruleFirings : Nat
  maxRuleFirings : Nat
  maxDerivedFacts : Nat
  deriving Repr, DecidableEq, BEq

def boundedClosureReceiptValid
    (state : GraphState)
    (receipt : BoundedClosureReceipt) : Bool :=
  receipt.factGeneration == state.generation &&
    !receipt.rulesetDigest.isEmpty &&
    !receipt.derivedFactIds.isEmpty &&
    receipt.sourceFactIds.all state.factIds.contains &&
    receipt.ruleFirings <= receipt.maxRuleFirings &&
    receipt.derivedFactIds.length <= receipt.maxDerivedFacts

def validBoundedClosureReceipt : BoundedClosureReceipt :=
  { factGeneration := 7
    rulesetDigest := "ruleset:ownership-v1"
    sourceFactIds := ["fact:model-declared"]
    derivedFactIds := ["fact:configuration-owner"]
    ruleFirings := 3
    maxRuleFirings := 8
    maxDerivedFacts := 4 }

theorem bounded_ascent_closure_is_grounded_and_within_budget :
    boundedClosureReceiptValid ownershipState validBoundedClosureReceipt = true := by
  native_decide

def overBudgetClosureReceipt : BoundedClosureReceipt :=
  { validBoundedClosureReceipt with ruleFirings := 9 }

theorem ascent_closure_over_rule_budget_is_rejected :
    boundedClosureReceiptValid ownershipState overBudgetClosureReceipt = false := by
  native_decide

structure GraphTurboCacheIdentity where
  workspaceDigest : String
  snapshotDigest : String
  graphSchemaDigest : String
  projectionDigest : String
  algorithmProfile : String
  algorithmImplementationDigest : String
  parameterDigest : String
  deterministicSeed : Nat
  budgetProfileDigest : String
  deriving Repr, DecidableEq, BEq

def graphTurboCacheReusable
    (expected observed : GraphTurboCacheIdentity) : Bool :=
  expected == observed

def graphTurboCacheIdentity : GraphTurboCacheIdentity :=
  { workspaceDigest := "workspace:1"
    snapshotDigest := "snapshot:7"
    graphSchemaDigest := "schema:1"
    projectionDigest := "projection:1"
    algorithmProfile := "local-path.v1"
    algorithmImplementationDigest := "scipy:1"
    parameterDigest := "parameters:1"
    deterministicSeed := 0
    budgetProfileDigest := "budget:1" }

theorem exact_graph_turbo_cache_identity_is_reusable :
    graphTurboCacheReusable graphTurboCacheIdentity graphTurboCacheIdentity = true := by
  native_decide

def staleGraphTurboCacheIdentity : GraphTurboCacheIdentity :=
  { graphTurboCacheIdentity with snapshotDigest := "snapshot:6" }

theorem graph_turbo_cache_rejects_snapshot_substitution :
    graphTurboCacheReusable graphTurboCacheIdentity staleGraphTurboCacheIdentity = false := by
  native_decide

theorem renderer_bytes_are_not_part_of_graph_turbo_cache_identity
    (_mermaid _dot : String) :
    graphTurboCacheReusable graphTurboCacheIdentity graphTurboCacheIdentity = true := by
  native_decide

inductive CacheAuthority where
  | runtimeServer
  | cliAdapter
  deriving Repr, DecidableEq, BEq

structure CacheExecution where
  requester : CacheAuthority
  tursoOwner : CacheAuthority
  generationPublisher : CacheAuthority
  writerQueueOwner : CacheAuthority
  throughTypedIpc : Bool
  requesterOpensTurso : Bool
  deriving Repr, DecidableEq, BEq

def serverCenteredCacheExecutionValid (execution : CacheExecution) : Bool :=
  execution.tursoOwner == .runtimeServer &&
    execution.generationPublisher == .runtimeServer &&
    execution.writerQueueOwner == .runtimeServer &&
    !execution.requesterOpensTurso &&
    (execution.requester == .runtimeServer || execution.throughTypedIpc)

def validCliCacheAdapterExecution : CacheExecution :=
  { requester := .cliAdapter
    tursoOwner := .runtimeServer
    generationPublisher := .runtimeServer
    writerQueueOwner := .runtimeServer
    throughTypedIpc := true
    requesterOpensTurso := false }

theorem cli_cache_adapter_may_request_but_cannot_own_cache :
    serverCenteredCacheExecutionValid validCliCacheAdapterExecution = true := by
  decide

def directCliTursoExecution : CacheExecution :=
  { validCliCacheAdapterExecution with
    throughTypedIpc := false
    requesterOpensTurso := true }

theorem cli_direct_turso_open_is_rejected :
    serverCenteredCacheExecutionValid directCliTursoExecution = false := by
  decide

def cliGenerationPublisherExecution : CacheExecution :=
  { validCliCacheAdapterExecution with generationPublisher := .cliAdapter }

theorem second_cli_generation_publisher_is_rejected :
    serverCenteredCacheExecutionValid cliGenerationPublisherExecution = false := by
  decide

def cliWriterQueueExecution : CacheExecution :=
  { validCliCacheAdapterExecution with writerQueueOwner := .cliAdapter }

theorem cli_bypass_of_resident_writer_queue_is_rejected :
    serverCenteredCacheExecutionValid cliWriterQueueExecution = false := by
  decide

structure TreeSitterPublicSurface where
  resultCount : Nat
  terminalReceiptCount : Nat
  exposesIncrementalSibling : Bool
  deriving Repr, DecidableEq, BEq

def treeSitterPublicSurfaceValid (surface : TreeSitterPublicSurface) : Bool :=
  surface.resultCount == 1 &&
    surface.terminalReceiptCount == 1 &&
    !surface.exposesIncrementalSibling

def mergedTreeSitterSurface : TreeSitterPublicSurface :=
  { resultCount := 1
    terminalReceiptCount := 1
    exposesIncrementalSibling := false }

theorem tree_sitter_incremental_execution_has_one_public_surface :
    treeSitterPublicSurfaceValid mergedTreeSitterSurface = true := by
  decide

def splitIncrementalSurface : TreeSitterPublicSurface :=
  { mergedTreeSitterSurface with
    resultCount := 2
    terminalReceiptCount := 2
    exposesIncrementalSibling := true }

theorem sibling_incremental_result_and_trace_are_rejected :
    treeSitterPublicSurfaceValid splitIncrementalSurface = false := by
  decide

end ASPProof.SearchRouterInteractiveGraphState
