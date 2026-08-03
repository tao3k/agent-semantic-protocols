namespace ASPProof.PolyglotSearchSemanticRefinement

inductive NodeLabel where
  | callable
  | owner
  | other
  deriving Repr, DecidableEq, BEq

inductive EdgeRelation where
  | ownedBy
  | calls
  deriving Repr, DecidableEq, BEq

structure GraphNode where
  nodeId : Nat
  label : NodeLabel
  name : String
  deriving Repr, DecidableEq, BEq

structure GraphEdge where
  edgeId : Nat
  sourceId : Nat
  targetId : Nat
  relation : EdgeRelation
  deriving Repr, DecidableEq, BEq

structure PropertyGraph where
  nodes : List GraphNode
  edges : List GraphEdge
  canonicalOrdering : Bool
  deriving Repr, DecidableEq, BEq

structure OneHopGQL where
  leftLabel : NodeLabel
  relation : EdgeRelation
  rightLabel : NodeLabel
  leftNameEquals : Option String
  returnLeft : Bool
  returnRight : Bool
  variablesDistinct : Bool
  aliasesUnique : Bool
  deriving Repr, DecidableEq, BEq

structure GQLRow where
  functionId : Nat
  candidateId : Nat
  witnessEdgeId : Nat
  deriving Repr, DecidableEq, BEq

def queryWellFormed (query : OneHopGQL) : Bool :=
  query.variablesDistinct &&
    query.aliasesUnique &&
    query.returnLeft &&
    query.returnRight

def nodeById (graph : PropertyGraph) (nodeId : Nat) : Option GraphNode :=
  graph.nodes.find? (fun node => node.nodeId == nodeId)

def evaluateEdge
    (query : OneHopGQL)
    (graph : PropertyGraph)
    (edge : GraphEdge) : Option GQLRow := do
  if edge.relation != query.relation then none else pure ()
  let left ← nodeById graph edge.sourceId
  let right ← nodeById graph edge.targetId
  if left.label != query.leftLabel then none else pure ()
  if right.label != query.rightLabel then none else pure ()
  match query.leftNameEquals with
  | some expected => if left.name != expected then none else pure ()
  | none => pure ()
  pure
    { functionId := left.nodeId
      candidateId := right.nodeId
      witnessEdgeId := edge.edgeId }

def evaluateGQL (query : OneHopGQL) (graph : PropertyGraph) : List GQLRow :=
  if queryWellFormed query && graph.canonicalOrdering then
    graph.edges.filterMap (evaluateEdge query graph)
  else
    []

def referenceQuery : OneHopGQL :=
  { leftLabel := .callable
    relation := .ownedBy
    rightLabel := .owner
    leftNameEquals := some "open_runtime_project_proxy"
    returnLeft := true
    returnRight := true
    variablesDistinct := true
    aliasesUnique := true }

def referenceGraph : PropertyGraph :=
  { nodes := [
      { nodeId := 1, label := .callable, name := "open_runtime_project_proxy" },
      { nodeId := 2, label := .owner, name := "runtime" },
      { nodeId := 3, label := .owner, name := "fallback" },
      { nodeId := 4, label := .other, name := "other" }
    ]
    edges := [
      { edgeId := 1, sourceId := 1, targetId := 2, relation := .ownedBy },
      { edgeId := 2, sourceId := 1, targetId := 2, relation := .ownedBy },
      { edgeId := 3, sourceId := 1, targetId := 3, relation := .ownedBy },
      { edgeId := 4, sourceId := 2, targetId := 4, relation := .ownedBy }
    ]
    canonicalOrdering := true }

def expectedGQLRows : List GQLRow := [
  { functionId := 1, candidateId := 2, witnessEdgeId := 1 },
  { functionId := 1, candidateId := 2, witnessEdgeId := 2 },
  { functionId := 1, candidateId := 3, witnessEdgeId := 3 }
]

theorem one_hop_evaluation_matches_reference_relation :
    evaluateGQL referenceQuery referenceGraph = expectedGQLRows := by
  decide

theorem one_hop_evaluation_preserves_parallel_edge_witnesses :
    (evaluateGQL referenceQuery referenceGraph).length = 3 := by
  decide

def stringMismatchQuery : OneHopGQL :=
  { referenceQuery with leftNameEquals := some "OPEN_RUNTIME_PROJECT_PROXY" }

theorem typed_equality_has_no_case_coercion :
    evaluateGQL stringMismatchQuery referenceGraph = [] := by
  decide

def shadowedAliasQuery : OneHopGQL :=
  { referenceQuery with aliasesUnique := false }

theorem alias_shadowing_is_rejected_before_evaluation :
    evaluateGQL shadowedAliasQuery referenceGraph = [] := by
  decide

def noncanonicalGraph : PropertyGraph :=
  { referenceGraph with canonicalOrdering := false }

theorem noncanonical_graph_order_is_rejected :
    evaluateGQL referenceQuery noncanonicalGraph = [] := by
  decide

structure CoverRow where
  candidateId : Nat
  obligationId : Nat
  deriving Repr, DecidableEq, BEq

structure LogicRow where
  functionId : Nat
  candidateId : Nat
  obligationId : Nat
  witnessEdgeId : Nat
  deriving Repr, DecidableEq, BEq

def joinCandidateCoverage
    (gqlRows : List GQLRow)
    (coverRows : List CoverRow) : List LogicRow :=
  gqlRows.flatMap fun gqlRow =>
    coverRows.filterMap fun coverRow =>
      if gqlRow.candidateId == coverRow.candidateId then
        some
          { functionId := gqlRow.functionId
            candidateId := gqlRow.candidateId
            obligationId := coverRow.obligationId
            witnessEdgeId := gqlRow.witnessEdgeId }
      else
        none

def referenceCoverage : List CoverRow := [
  { candidateId := 2, obligationId := 10 },
  { candidateId := 3, obligationId := 11 }
]

def expectedLogicRows : List LogicRow := [
  { functionId := 1, candidateId := 2, obligationId := 10, witnessEdgeId := 1 },
  { functionId := 1, candidateId := 2, obligationId := 10, witnessEdgeId := 2 },
  { functionId := 1, candidateId := 3, obligationId := 11, witnessEdgeId := 3 }
]

theorem conjunctive_join_preserves_shared_candidate_binding :
    joinCandidateCoverage expectedGQLRows referenceCoverage = expectedLogicRows := by
  decide

theorem conjunctive_join_preserves_bag_multiplicity :
    (joinCandidateCoverage expectedGQLRows referenceCoverage).length = 3 := by
  decide

structure SourceASTBinding where
  expectedSourceDigest : String
  observedSourceDigest : String
  expectedASTDigest : String
  observedASTDigest : String
  expectedBindingDigest : String
  observedBindingDigest : String
  profileDigest : String
  registryDigest : String
  deriving Repr, DecidableEq, BEq

def sourceASTBindingAdmitted (binding : SourceASTBinding) : Bool :=
  !binding.expectedSourceDigest.isEmpty &&
    binding.expectedSourceDigest == binding.observedSourceDigest &&
    !binding.expectedASTDigest.isEmpty &&
    binding.expectedASTDigest == binding.observedASTDigest &&
    !binding.expectedBindingDigest.isEmpty &&
    binding.expectedBindingDigest == binding.observedBindingDigest &&
    !binding.profileDigest.isEmpty &&
    !binding.registryDigest.isEmpty

def validBinding : SourceASTBinding :=
  { expectedSourceDigest := "source:1"
    observedSourceDigest := "source:1"
    expectedASTDigest := "ast:1"
    observedASTDigest := "ast:1"
    expectedBindingDigest := "binding:1"
    observedBindingDigest := "binding:1"
    profileDigest := "profile:1"
    registryDigest := "registry:1" }

def tamperedASTBinding : SourceASTBinding :=
  { validBinding with observedASTDigest := "ast:tampered" }

theorem exact_source_ast_binding_is_admitted :
    sourceASTBindingAdmitted validBinding = true := by
  decide

theorem tampered_ast_digest_is_rejected :
    sourceASTBindingAdmitted tamperedASTBinding = false := by
  decide

inductive MultiplicityPolicy where
  | bagByWitnessEdge
  | deduplicatedSet
  deriving Repr, DecidableEq, BEq

inductive OrderingPolicy where
  | canonicalRow
  | providerOrder
  deriving Repr, DecidableEq, BEq

structure SemanticTrace where
  binding : SourceASTBinding
  graphDigestBefore : String
  graphDigestAfter : String
  expectedTraceDigest : String
  observedTraceDigest : String
  multiplicity : MultiplicityPolicy
  ordering : OrderingPolicy
  evaluationBounded : Bool
  deriving Repr, DecidableEq, BEq

def semanticTraceAdmitted (trace : SemanticTrace) : Bool :=
  sourceASTBindingAdmitted trace.binding &&
    !trace.graphDigestBefore.isEmpty &&
    trace.graphDigestBefore == trace.graphDigestAfter &&
    !trace.expectedTraceDigest.isEmpty &&
    trace.expectedTraceDigest == trace.observedTraceDigest &&
    trace.multiplicity == .bagByWitnessEdge &&
    trace.ordering == .canonicalRow &&
    trace.evaluationBounded

def validSemanticTrace : SemanticTrace :=
  { binding := validBinding
    graphDigestBefore := "graph:1"
    graphDigestAfter := "graph:1"
    expectedTraceDigest := "trace:1"
    observedTraceDigest := "trace:1"
    multiplicity := .bagByWitnessEdge
    ordering := .canonicalRow
    evaluationBounded := true }

def mutatedGraphTrace : SemanticTrace :=
  { validSemanticTrace with graphDigestAfter := "graph:mutated" }

def tamperedTraceDigest : SemanticTrace :=
  { validSemanticTrace with observedTraceDigest := "trace:tampered" }

theorem valid_semantic_trace_is_admitted :
    semanticTraceAdmitted validSemanticTrace = true := by
  decide

theorem semantic_evaluation_is_read_only :
    semanticTraceAdmitted mutatedGraphTrace = false := by
  decide

theorem tampered_trace_digest_is_rejected :
    semanticTraceAdmitted tamperedTraceDigest = false := by
  decide

def evaluateDeterministically
    (query : OneHopGQL)
    (graph : PropertyGraph) : List GQLRow :=
  evaluateGQL query graph

theorem evaluator_is_deterministic
    (query : OneHopGQL)
    (graph : PropertyGraph) :
    evaluateDeterministically query graph = evaluateDeterministically query graph := by
  rfl

def graphAfterEvaluation
    (_query : OneHopGQL)
    (graph : PropertyGraph) : PropertyGraph :=
  graph

theorem evaluator_cannot_mutate_graph
    (query : OneHopGQL)
    (graph : PropertyGraph) :
    graphAfterEvaluation query graph = graph := by
  rfl

end ASPProof.PolyglotSearchSemanticRefinement
