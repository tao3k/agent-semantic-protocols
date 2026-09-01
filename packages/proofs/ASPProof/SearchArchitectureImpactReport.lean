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

theorem proofReportNeverInventsAxioms (inventory : FactInventory) :
    (buildProofReport inventory).axioms = [] := by rfl

end ASPProof.SearchArchitectureImpactReport
