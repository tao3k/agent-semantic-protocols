-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

namespace ASPProof.SearchRouteTriadRelationshipContract

abbrev ClauseId := Nat
abbrev Digest := Nat

structure ClauseCommitment where
  clauseId : ClauseId
  sourceDigest : Digest
  dependencySetDigest : Digest
  deriving DecidableEq, Repr

structure AuditRfcBinding where
  clauseId : ClauseId
  sourceDigest : Digest
  dependencySetDigest : Digest
  deriving DecidableEq, Repr

def IdentifierOnlyBound
    (commitment : ClauseCommitment)
    (binding : AuditRfcBinding) : Prop :=
  commitment.clauseId = binding.clauseId

def ExactRfcBound
    (commitment : ClauseCommitment)
    (binding : AuditRfcBinding) : Prop :=
  commitment.clauseId = binding.clauseId ∧
    commitment.sourceDigest = binding.sourceDigest ∧
    commitment.dependencySetDigest = binding.dependencySetDigest

theorem exact_binding_has_identifier_binding
    {commitment : ClauseCommitment}
    {binding : AuditRfcBinding}
    (exact : ExactRfcBound commitment binding) :
    IdentifierOnlyBound commitment binding :=
  exact.1

theorem changed_clause_digest_invalidates
    {commitment : ClauseCommitment}
    {binding : AuditRfcBinding}
    (changed : commitment.sourceDigest ≠ binding.sourceDigest) :
    ¬ ExactRfcBound commitment binding := by
  intro exact
  exact changed exact.2.1

theorem changed_dependency_set_invalidates
    {commitment : ClauseCommitment}
    {binding : AuditRfcBinding}
    (changed : commitment.dependencySetDigest ≠ binding.dependencySetDigest) :
    ¬ ExactRfcBound commitment binding := by
  intro exact
  exact changed exact.2.2

def currentClause : ClauseCommitment where
  clauseId := 64
  sourceDigest := 101
  dependencySetDigest := 202

def staleSameIdBinding : AuditRfcBinding where
  clauseId := 64
  sourceDigest := 100
  dependencySetDigest := 202

theorem identifier_only_binding_accepts_changed_semantics :
    IdentifierOnlyBound currentClause staleSameIdBinding := by
  rfl

theorem exact_binding_rejects_changed_semantics :
    ¬ ExactRfcBound currentClause staleSameIdBinding :=
  changed_clause_digest_invalidates (by decide)

structure AuditCoverage where
  primaryFamily : Bool
  additionalFamily : Bool
  primaryClause : Bool
  additionalClause : Bool
  deriving DecidableEq, Repr

def DeclaredCoverageIsSubset
    (declared observed : AuditCoverage) : Prop :=
  (declared.primaryFamily = true → observed.primaryFamily = true) ∧
    (declared.additionalFamily = true → observed.additionalFamily = true) ∧
    (declared.primaryClause = true → observed.primaryClause = true) ∧
    (declared.additionalClause = true → observed.additionalClause = true)

def ExactCoverage
    (declared observed : AuditCoverage) : Prop :=
  declared.primaryFamily = observed.primaryFamily ∧
    declared.additionalFamily = observed.additionalFamily ∧
    declared.primaryClause = observed.primaryClause ∧
    declared.additionalClause = observed.additionalClause

def underdeclaredCoverage : AuditCoverage where
  primaryFamily := true
  additionalFamily := false
  primaryClause := true
  additionalClause := false

def observedCoverage : AuditCoverage where
  primaryFamily := true
  additionalFamily := true
  primaryClause := true
  additionalClause := true

theorem subset_check_accepts_underdeclared_dependencies :
    DeclaredCoverageIsSubset underdeclaredCoverage observedCoverage := by
  unfold DeclaredCoverageIsSubset underdeclaredCoverage observedCoverage
  decide

theorem exact_coverage_rejects_underdeclared_dependencies :
    ¬ ExactCoverage underdeclaredCoverage observedCoverage := by
  unfold ExactCoverage underdeclaredCoverage observedCoverage
  decide

structure GateEvidence where
  orgContractPassed : Bool
  declaredCoverage : AuditCoverage
  observedCoverage : AuditCoverage
  declaredCommitment : ClauseCommitment
  observedBinding : AuditRfcBinding
  deriving DecidableEq, Repr

def GateAdmitted (evidence : GateEvidence) : Prop :=
  evidence.orgContractPassed = true ∧
    ExactCoverage evidence.declaredCoverage evidence.observedCoverage ∧
    ExactRfcBound evidence.declaredCommitment evidence.observedBinding

theorem admitted_gate_has_org_contract
    {evidence : GateEvidence}
    (admitted : GateAdmitted evidence) :
    evidence.orgContractPassed = true :=
  admitted.1

theorem underdeclared_coverage_blocks_gate
    {evidence : GateEvidence}
    (changed : ¬ ExactCoverage evidence.declaredCoverage
      evidence.observedCoverage) :
    ¬ GateAdmitted evidence := by
  intro admitted
  exact changed admitted.2.1

theorem stale_clause_blocks_gate
    {evidence : GateEvidence}
    (changed : evidence.declaredCommitment.sourceDigest ≠
      evidence.observedBinding.sourceDigest) :
    ¬ GateAdmitted evidence := by
  intro admitted
  exact changed admitted.2.2.2.1

def currentBinding : AuditRfcBinding where
  clauseId := currentClause.clauseId
  sourceDigest := currentClause.sourceDigest
  dependencySetDigest := currentClause.dependencySetDigest

def digestOnlyEvidence : GateEvidence where
  orgContractPassed := false
  declaredCoverage := observedCoverage
  observedCoverage := observedCoverage
  declaredCommitment := currentClause
  observedBinding := currentBinding

theorem exact_digests_without_org_contract_are_not_admitted :
    ¬ GateAdmitted digestOnlyEvidence := by
  intro admitted
  exact Bool.noConfusion admitted.1

def contractedExactEvidence : GateEvidence where
  orgContractPassed := true
  declaredCoverage := observedCoverage
  observedCoverage := observedCoverage
  declaredCommitment := currentClause
  observedBinding := currentBinding

theorem contract_and_exact_evidence_admit :
    GateAdmitted contractedExactEvidence := by
  unfold GateAdmitted contractedExactEvidence ExactCoverage ExactRfcBound
  exact ⟨rfl, ⟨⟨rfl, rfl, rfl, rfl⟩, ⟨rfl, rfl, rfl⟩⟩⟩

theorem unrelated_clause_change_preserves_local_binding
    {commitment : ClauseCommitment}
    {binding : AuditRfcBinding}
    (exact : ExactRfcBound commitment binding)
    (_unrelatedOld _unrelatedNew : ClauseCommitment) :
    ExactRfcBound commitment binding := by
  exact exact

end ASPProof.SearchRouteTriadRelationshipContract
