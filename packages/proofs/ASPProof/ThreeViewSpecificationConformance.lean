-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ThreeViewSpecificationConformance

abbrev ClauseId := Nat
abbrev ArtifactId := Nat
abbrev ArtifactDigest := Nat
abbrev Revision := Nat
abbrev StateId := Nat

inductive EdgeSemantic where
  | transition (identity : ArtifactId)
  | guard (identity : ArtifactId)
  | capability (identity : ArtifactId)
  | prohibitedEffect (identity : ArtifactId)
  deriving DecidableEq, Repr

structure FlowNodeProjection where
  nodeId : ArtifactId
  stateDomain : Option StateId
  deriving DecidableEq, Repr

structure FlowEdgeProjection where
  edgeId : ArtifactId
  sourceState : Option StateId
  targetState : Option StateId
  semantic : Option EdgeSemantic
  deriving DecidableEq, Repr

inductive StatefulNode : FlowNodeProjection → Prop where
  | withState (nodeId : ArtifactId) (stateDomain : StateId) :
      StatefulNode {
        nodeId := nodeId
        stateDomain := some stateDomain
      }

inductive StatefulEdge : FlowEdgeProjection → Prop where
  | withStates
      (edgeId : ArtifactId)
      (sourceState targetState : StateId)
      (semantic : EdgeSemantic) :
      StatefulEdge {
        edgeId := edgeId
        sourceState := some sourceState
        targetState := some targetState
        semantic := some semantic
      }

structure ProjectionEntry where
  clause : ClauseId
  mermaid : Option ArtifactId
  typst : Option ArtifactId
  lean : Option ArtifactId
  deriving DecidableEq, Repr

inductive CompleteProjection : ProjectionEntry → Prop where
  | complete
      (clause : ClauseId)
      (mermaid typst lean : ArtifactId) :
      CompleteProjection {
        clause := clause
        mermaid := some mermaid
        typst := some typst
        lean := some lean
      }

structure ThreeViewBundle where
  clauseDigest : ArtifactDigest
  mermaidDigest : ArtifactDigest
  stateDigest : ArtifactDigest
  typstDigest : ArtifactDigest
  leanDigest : ArtifactDigest
  revision : Revision
  deriving DecidableEq, Repr

structure ThreeViewReceipt where
  clauseDigest : ArtifactDigest
  mermaidDigest : ArtifactDigest
  stateDigest : ArtifactDigest
  typstDigest : ArtifactDigest
  leanDigest : ArtifactDigest
  revision : Revision
  deriving DecidableEq, Repr

def receiptOf (bundle : ThreeViewBundle) : ThreeViewReceipt :=
  {
    clauseDigest := bundle.clauseDigest
    mermaidDigest := bundle.mermaidDigest
    stateDigest := bundle.stateDigest
    typstDigest := bundle.typstDigest
    leanDigest := bundle.leanDigest
    revision := bundle.revision
  }

inductive ReceiptValid : ThreeViewReceipt → ThreeViewBundle → Prop where
  | exact (bundle : ThreeViewBundle) :
      ReceiptValid (receiptOf bundle) bundle

structure ImplementationReceipt where
  clauseDigest : ArtifactDigest
  stateDigest : ArtifactDigest
  leanDigest : ArtifactDigest
  revision : Revision
  passed : Bool
  deriving DecidableEq, Repr

def implementationReceiptOf
    (bundle : ThreeViewBundle)
    (passed : Bool) : ImplementationReceipt :=
  {
    clauseDigest := bundle.clauseDigest
    stateDigest := bundle.stateDigest
    leanDigest := bundle.leanDigest
    revision := bundle.revision
    passed := passed
  }

inductive ImplementationConforms :
    ThreeViewBundle → ImplementationReceipt → Prop where
  | verified (bundle : ThreeViewBundle) :
      ImplementationConforms bundle (implementationReceiptOf bundle true)

theorem stateful_node_constructible
    (nodeId : ArtifactId)
    (stateDomain : StateId) :
    StatefulNode {
      nodeId := nodeId
      stateDomain := some stateDomain
    } := by
  exact StatefulNode.withState nodeId stateDomain

theorem missing_node_state_not_stateful
    (nodeId : ArtifactId) :
    ¬ StatefulNode {
      nodeId := nodeId
      stateDomain := none
    } := by
  intro stateful
  cases stateful

theorem stateful_edge_constructible
    (edgeId : ArtifactId)
    (sourceState targetState : StateId)
    (semantic : EdgeSemantic) :
    StatefulEdge {
      edgeId := edgeId
      sourceState := some sourceState
      targetState := some targetState
      semantic := some semantic
    } := by
  exact StatefulEdge.withStates edgeId sourceState targetState semantic

theorem missing_edge_source_state_rejected
    (edgeId : ArtifactId)
    (targetState : StateId)
    (semantic : EdgeSemantic) :
    ¬ StatefulEdge {
      edgeId := edgeId
      sourceState := none
      targetState := some targetState
      semantic := some semantic
    } := by
  intro stateful
  cases stateful

theorem missing_edge_target_state_rejected
    (edgeId : ArtifactId)
    (sourceState : StateId)
    (semantic : EdgeSemantic) :
    ¬ StatefulEdge {
      edgeId := edgeId
      sourceState := some sourceState
      targetState := none
      semantic := some semantic
    } := by
  intro stateful
  cases stateful

theorem missing_edge_semantics_rejected
    (edgeId : ArtifactId)
    (sourceState targetState : StateId) :
    ¬ StatefulEdge {
      edgeId := edgeId
      sourceState := some sourceState
      targetState := some targetState
      semantic := none
    } := by
  intro stateful
  cases stateful

theorem complete_projection_constructible
    (clause : ClauseId)
    (mermaid typst lean : ArtifactId) :
    CompleteProjection {
      clause := clause
      mermaid := some mermaid
      typst := some typst
      lean := some lean
    } := by
  exact CompleteProjection.complete clause mermaid typst lean

theorem missing_mermaid_not_complete
    (clause : ClauseId)
    (typst lean : ArtifactId) :
    ¬ CompleteProjection {
      clause := clause
      mermaid := none
      typst := some typst
      lean := some lean
    } := by
  intro coverage
  cases coverage

theorem missing_typst_not_complete
    (clause : ClauseId)
    (mermaid lean : ArtifactId) :
    ¬ CompleteProjection {
      clause := clause
      mermaid := some mermaid
      typst := none
      lean := some lean
    } := by
  intro coverage
  cases coverage

theorem missing_lean_not_complete
    (clause : ClauseId)
    (mermaid typst : ArtifactId) :
    ¬ CompleteProjection {
      clause := clause
      mermaid := some mermaid
      typst := some typst
      lean := none
    } := by
  intro coverage
  cases coverage

theorem exact_receipt_valid
    (bundle : ThreeViewBundle) :
    ReceiptValid (receiptOf bundle) bundle := by
  exact ReceiptValid.exact bundle

theorem mermaid_digest_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.mermaidDigest ≠ bundle.mermaidDigest) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem state_digest_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.stateDigest ≠ bundle.stateDigest) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem typst_digest_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.typstDigest ≠ bundle.typstDigest) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem lean_digest_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.leanDigest ≠ bundle.leanDigest) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem clause_digest_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.clauseDigest ≠ bundle.clauseDigest) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem revision_change_invalidates
    (receipt : ThreeViewReceipt)
    (bundle : ThreeViewBundle)
    (changed : receipt.revision ≠ bundle.revision) :
    ¬ ReceiptValid receipt bundle := by
  intro valid
  cases valid
  exact changed rfl

theorem verified_implementation_conforms
    (bundle : ThreeViewBundle) :
    ImplementationConforms
      bundle
      (implementationReceiptOf bundle true) := by
  exact ImplementationConforms.verified bundle

theorem failed_implementation_not_conformant
    (bundle : ThreeViewBundle) :
    ¬ ImplementationConforms
      bundle
      (implementationReceiptOf bundle false) := by
  intro conformance
  cases conformance

theorem implementation_digest_mismatch_rejected
    (bundle : ThreeViewBundle)
    (receipt : ImplementationReceipt)
    (mismatch : receipt.leanDigest ≠ bundle.leanDigest) :
    ¬ ImplementationConforms bundle receipt := by
  intro conformance
  cases conformance
  exact mismatch rfl

theorem implementation_state_digest_mismatch_rejected
    (bundle : ThreeViewBundle)
    (receipt : ImplementationReceipt)
    (mismatch : receipt.stateDigest ≠ bundle.stateDigest) :
    ¬ ImplementationConforms bundle receipt := by
  intro conformance
  cases conformance
  exact mismatch rfl

theorem complete_views_do_not_conform_failed_implementation
    (clause mermaid typst lean : ArtifactId)
    (bundle : ThreeViewBundle) :
    CompleteProjection {
      clause := clause
      mermaid := some mermaid
      typst := some typst
      lean := some lean
    } ∧
    ¬ ImplementationConforms
      bundle
      (implementationReceiptOf bundle false) := by
  exact ⟨
    CompleteProjection.complete clause mermaid typst lean,
    failed_implementation_not_conformant bundle
  ⟩

end ASPProof.ThreeViewSpecificationConformance
