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
  native_decide

def inventedEdgeDelta : GraphDelta :=
  { ownershipDelta with
    edgesAdded :=
      [ { source := "symbol:AgentRegistry"
          target := "symbol:UnknownOwner"
          relation := "owns"
          provenance := "receipt:model-invented" } ] }

theorem model_invented_edge_is_rejected :
    deltaValid ownershipCapabilities ownershipState inventedEdgeDelta = false := by
  native_decide

def nonDecreasingBudgetDelta : GraphDelta :=
  { ownershipDelta with remainingBudget := 4 }

theorem accepted_transition_must_decrease_budget :
    deltaValid ownershipCapabilities ownershipState nonDecreasingBudgetDelta = false := by
  native_decide

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
  native_decide

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
  native_decide

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
  native_decide

theorem stale_continuation_is_rejected :
    resumeValid ownershipState
      "continuation:owner-model"
      "session-owner-model"
      "symbol:ModelConfig"
      6
      "state-6"
      cachedFrontierRouteMeasure = false := by
  native_decide

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
  native_decide

theorem unresolved_obligation_prevents_closure :
    closureAllowed ownershipState [] = false := by
  native_decide

theorem resolved_obligation_allows_closure :
    closureAllowed ownershipState ["claim:effective-owner"] = true := by
  native_decide

def unsupportedDischargeDelta : GraphDelta :=
  { ownershipDelta with
    obligationsDischarged := ["claim:effective-owner"] }

theorem obligation_discharge_without_observation_evidence_is_rejected :
    deltaValid ownershipCapabilities ownershipState unsupportedDischargeDelta = false := by
  native_decide

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
  native_decide

def validDatalogReceipt : DatalogReceipt :=
  { factGeneration := 7
    rulesetDigest := "ruleset:ownership-v1"
    sourceFactIds := ["fact:model-declared"]
    derivedFactIds := ["fact:configuration-owner"] }

theorem datalog_derivation_is_grounded_in_live_facts :
    datalogReceiptValid ownershipState validDatalogReceipt = true := by
  native_decide

def inventedDatalogSource : DatalogReceipt :=
  { validDatalogReceipt with
    sourceFactIds := ["fact:model-invented"] }

theorem datalog_derivation_with_unknown_source_is_rejected :
    datalogReceiptValid ownershipState inventedDatalogSource = false := by
  native_decide

end ASPProof.SearchRouterInteractiveGraphState
