-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProofCarryingSelectiveDisclosure

namespace ASPProof.SearchRouteFourLayerGraphReasoning

abbrev Digest := Nat
abbrev NodeId := Nat
abbrev EdgeId := Nat

inductive Relation where
  | calls
  | dispatches
  | normalizes
  | exactQuery
  | clientBackend
deriving DecidableEq, Repr

inductive ProjectionSyntax where
  | d2
  | edgeList
  | dot
  | mermaid
  | typedJson
deriving DecidableEq, Repr

structure Edge where
  edgeId : EdgeId
  source : NodeId
  target : NodeId
  relation : Relation
deriving DecidableEq, Repr

structure CanonicalGraph where
  graphDigest : Digest
  edges : List Edge
deriving DecidableEq, Repr

structure QueryPlan where
  queryDigest : Digest
  source : NodeId
  target : NodeId
  allowedRelations : List Relation
  excludedEdges : List EdgeId
deriving DecidableEq, Repr

structure QueryExecutionReceipt where
  graphDigest : Digest
  queryDigest : Digest
  executionDigest : Digest
  selectedPath : List Edge
  rejectedEdges : List EdgeId
  executable : Bool
deriving DecidableEq, Repr

structure ReasoningPacket where
  graphDigest : Digest
  queryDigest : Digest
  executionDigest : Digest
  packetDigest : Digest
  projectionSyntax : ProjectionSyntax
  selectedPath : List Edge
  disclosedEdges : List Edge
  maxDisclosedEdges : Nat
deriving DecidableEq, Repr

structure LeanReceipt where
  graphDigest : Digest
  queryDigest : Digest
  executionDigest : Digest
  packetDigest : Digest
  accepted : Bool
deriving DecidableEq, Repr

def continuousFromTo : NodeId → NodeId → List Edge → Bool
  | source, target, [] => decide (source = target)
  | source, target, edge :: rest =>
      decide (edge.source = source) &&
        continuousFromTo edge.target target rest

def allEdgesInGraph (graph : CanonicalGraph) (path : List Edge) : Bool :=
  path.all fun edge => decide (edge ∈ graph.edges)

def allRelationsAllowed (plan : QueryPlan) (path : List Edge) : Bool :=
  path.all fun edge => decide (edge.relation ∈ plan.allowedRelations)

def excludesForbiddenEdges (plan : QueryPlan) (path : List Edge) : Bool :=
  path.all fun edge => decide (edge.edgeId ∉ plan.excludedEdges)

def coversRejectedEdges
    (plan : QueryPlan)
    (receipt : QueryExecutionReceipt) : Bool :=
  plan.excludedEdges.all fun edgeId =>
    decide (edgeId ∈ receipt.rejectedEdges)

def executionValidBool
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (receipt : QueryExecutionReceipt) : Bool :=
  decide (receipt.graphDigest = graph.graphDigest) &&
    decide (receipt.queryDigest = plan.queryDigest) &&
    receipt.executable &&
    decide (receipt.selectedPath ≠ []) &&
    continuousFromTo plan.source plan.target receipt.selectedPath &&
    allEdgesInGraph graph receipt.selectedPath &&
    allRelationsAllowed plan receipt.selectedPath &&
    excludesForbiddenEdges plan receipt.selectedPath &&
    coversRejectedEdges plan receipt

def packetValidBool
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket) : Bool :=
  decide (packet.graphDigest = graph.graphDigest) &&
    decide (packet.queryDigest = plan.queryDigest) &&
    decide (packet.executionDigest = execution.executionDigest) &&
    decide (packet.selectedPath = execution.selectedPath) &&
    packet.selectedPath.all (fun edge => decide (edge ∈ packet.disclosedEdges)) &&
    allEdgesInGraph graph packet.disclosedEdges &&
    decide (packet.disclosedEdges.length ≤ packet.maxDisclosedEdges)

def proofValidBool
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket)
    (receipt : LeanReceipt) : Bool :=
  decide (receipt.graphDigest = graph.graphDigest) &&
    decide (receipt.queryDigest = plan.queryDigest) &&
    decide (receipt.executionDigest = execution.executionDigest) &&
    decide (receipt.packetDigest = packet.packetDigest) &&
    receipt.accepted

def ExecutionValid
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (receipt : QueryExecutionReceipt) : Prop :=
  executionValidBool graph plan receipt = true

def PacketValid
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket) : Prop :=
  packetValidBool graph plan execution packet = true

def ProofValid
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket)
    (receipt : LeanReceipt) : Prop :=
  proofValidBool graph plan execution packet receipt = true

def FourLayerValid
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket)
    (receipt : LeanReceipt) : Prop :=
  ExecutionValid graph plan execution ∧
    PacketValid graph plan execution packet ∧
    ProofValid graph plan execution packet receipt

instance executionValidDecidable
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (receipt : QueryExecutionReceipt) :
    Decidable (ExecutionValid graph plan receipt) := by
  unfold ExecutionValid
  infer_instance

instance packetValidDecidable
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket) :
    Decidable (PacketValid graph plan execution packet) := by
  unfold PacketValid
  infer_instance

instance proofValidDecidable
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket)
    (receipt : LeanReceipt) :
    Decidable (ProofValid graph plan execution packet receipt) := by
  unfold ProofValid
  infer_instance

instance fourLayerValidDecidable
    (graph : CanonicalGraph)
    (plan : QueryPlan)
    (execution : QueryExecutionReceipt)
    (packet : ReasoningPacket)
    (receipt : LeanReceipt) :
    Decidable (FourLayerValid graph plan execution packet receipt) := by
  unfold FourLayerValid
  infer_instance

theorem four_layer_valid_requires_executable_query
    {graph : CanonicalGraph}
    {plan : QueryPlan}
    {execution : QueryExecutionReceipt}
    {packet : ReasoningPacket}
    {receipt : LeanReceipt}
    (valid : FourLayerValid graph plan execution packet receipt) :
    ExecutionValid graph plan execution :=
  valid.1

theorem four_layer_valid_requires_bounded_packet
    {graph : CanonicalGraph}
    {plan : QueryPlan}
    {execution : QueryExecutionReceipt}
    {packet : ReasoningPacket}
    {receipt : LeanReceipt}
    (valid : FourLayerValid graph plan execution packet receipt) :
    PacketValid graph plan execution packet :=
  valid.2.1

theorem four_layer_valid_requires_bound_proof
    {graph : CanonicalGraph}
    {plan : QueryPlan}
    {execution : QueryExecutionReceipt}
    {packet : ReasoningPacket}
    {receipt : LeanReceipt}
    (valid : FourLayerValid graph plan execution packet receipt) :
    ProofValid graph plan execution packet receipt :=
  valid.2.2

def dispatchEdge : Edge :=
  { edgeId := 1, source := 0, target := 1,
    relation := .dispatches }

def callEdge : Edge :=
  { edgeId := 2, source := 1, target := 2,
    relation := .calls }

def normalizeEdge : Edge :=
  { edgeId := 3, source := 2, target := 3,
    relation := .normalizes }

def exactQueryDecoy : Edge :=
  { edgeId := 4, source := 0, target := 9,
    relation := .exactQuery }

def decoyReturn : Edge :=
  { edgeId := 5, source := 9, target := 3,
    relation := .calls }

def d3Graph : CanonicalGraph :=
  {
    graphDigest := 101
    edges := [dispatchEdge, callEdge, normalizeEdge,
      exactQueryDecoy, decoyReturn]
  }

def ownerQuery : QueryPlan :=
  {
    queryDigest := 202
    source := 0
    target := 3
    allowedRelations := [.dispatches, .calls, .normalizes]
    excludedEdges := [4, 5]
  }

def goodExecution : QueryExecutionReceipt :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 303
    selectedPath := [dispatchEdge, callEdge, normalizeEdge]
    rejectedEdges := [4, 5]
    executable := true
  }

def goodD2Packet : ReasoningPacket :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 303
    packetDigest := 404
    projectionSyntax := .d2
    selectedPath := [dispatchEdge, callEdge, normalizeEdge]
    disclosedEdges := [dispatchEdge, callEdge, normalizeEdge]
    maxDisclosedEdges := 3
  }

def goodLeanReceipt : LeanReceipt :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 303
    packetDigest := 404
    accepted := true
  }

theorem d3_query_conditioned_d2_packet_is_four_layer_valid :
    FourLayerValid d3Graph ownerQuery goodExecution
      goodD2Packet goodLeanReceipt := by
  decide

def forbiddenBranchExecution : QueryExecutionReceipt :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 305
    selectedPath := [exactQueryDecoy, decoyReturn]
    rejectedEdges := []
    executable := true
  }

theorem excluded_exact_query_branch_is_rejected :
    ¬ ExecutionValid d3Graph ownerQuery forbiddenBranchExecution := by
  decide

def missingRejectionEvidenceExecution : QueryExecutionReceipt :=
  { goodExecution with rejectedEdges := [] }

theorem missing_rejection_evidence_is_rejected :
    ¬ ExecutionValid d3Graph ownerQuery missingRejectionEvidenceExecution := by
  decide

def inventedEdge : Edge :=
  { edgeId := 99, source := 1, target := 3,
    relation := .normalizes }

def inventedPathPacket : ReasoningPacket :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 303
    packetDigest := 405
    projectionSyntax := .typedJson
    selectedPath := [dispatchEdge, inventedEdge]
    disclosedEdges := [dispatchEdge, inventedEdge]
    maxDisclosedEdges := 3
  }

theorem model_invented_edge_is_rejected :
    ¬ PacketValid d3Graph ownerQuery goodExecution inventedPathPacket := by
  decide

def fullGraphDumpPacket : ReasoningPacket :=
  {
    graphDigest := 101
    queryDigest := 202
    executionDigest := 303
    packetDigest := 406
    projectionSyntax := .d2
    selectedPath := [dispatchEdge, callEdge, normalizeEdge]
    disclosedEdges := d3Graph.edges
    maxDisclosedEdges := 3
  }

theorem full_graph_dump_over_budget_is_rejected :
    ¬ PacketValid d3Graph ownerQuery goodExecution fullGraphDumpPacket := by
  decide

def staleGraphExecution : QueryExecutionReceipt :=
  { goodExecution with graphDigest := 100 }

theorem stale_graph_execution_is_rejected :
    ¬ ExecutionValid d3Graph ownerQuery staleGraphExecution := by
  decide

def goodEdgeListPacket : ReasoningPacket :=
  { goodD2Packet with projectionSyntax := .edgeList }

theorem syntax_choice_does_not_change_packet_admission :
    PacketValid d3Graph ownerQuery goodExecution goodD2Packet ∧
      PacketValid d3Graph ownerQuery goodExecution goodEdgeListPacket := by
  decide

end ASPProof.SearchRouteFourLayerGraphReasoning
