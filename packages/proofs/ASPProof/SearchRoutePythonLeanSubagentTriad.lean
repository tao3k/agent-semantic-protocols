-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

namespace ASPProof.SearchRoutePythonLeanSubagentTriad

structure TriadEvidence where
  selectorBound : Bool
  pythonExecuted : Bool
  leanAxiomFree : Bool
  goldenAgreement : Bool
  deriving DecidableEq, Repr

def TriadAdmitted (evidence : TriadEvidence) : Prop :=
  evidence.selectorBound = true ∧
    evidence.pythonExecuted = true ∧
    evidence.leanAxiomFree = true ∧
    evidence.goldenAgreement = true

theorem complete_evidence_is_admitted
    {evidence : TriadEvidence}
    (selectorBound : evidence.selectorBound = true)
    (pythonExecuted : evidence.pythonExecuted = true)
    (leanAxiomFree : evidence.leanAxiomFree = true)
    (goldenAgreement : evidence.goldenAgreement = true) :
    TriadAdmitted evidence :=
  ⟨selectorBound, pythonExecuted, leanAxiomFree, goldenAgreement⟩

theorem missing_lean_receipt_blocks_admission
    {evidence : TriadEvidence}
    (missingLean : evidence.leanAxiomFree = false) :
    ¬ TriadAdmitted evidence := by
  intro admitted
  have contradiction : false = true :=
    missingLean.symm.trans admitted.2.2.1
  exact Bool.noConfusion contradiction

theorem missing_golden_agreement_blocks_admission
    {evidence : TriadEvidence}
    (missingAgreement : evidence.goldenAgreement = false) :
    ¬ TriadAdmitted evidence := by
  intro admitted
  have contradiction : false = true :=
    missingAgreement.symm.trans admitted.2.2.2
  exact Bool.noConfusion contradiction

theorem parent_confirmation_does_not_close_nested_receipt
    {nested parent : TriadEvidence}
    (nestedMissingLean : nested.leanAxiomFree = false)
    (_parentAdmitted : TriadAdmitted parent) :
    ¬ TriadAdmitted nested :=
  missing_lean_receipt_blocks_admission nestedMissingLean

def observedNestedReceipt : TriadEvidence where
  selectorBound := true
  pythonExecuted := true
  leanAxiomFree := false
  goldenAgreement := false

def observedParentReceipt : TriadEvidence where
  selectorBound := true
  pythonExecuted := true
  leanAxiomFree := true
  goldenAgreement := true

theorem observed_nested_receipt_is_not_admitted :
    ¬ TriadAdmitted observedNestedReceipt :=
  missing_lean_receipt_blocks_admission rfl

theorem observed_parent_receipt_is_admitted :
    TriadAdmitted observedParentReceipt :=
  complete_evidence_is_admitted rfl rfl rfl rfl

end ASPProof.SearchRoutePythonLeanSubagentTriad
