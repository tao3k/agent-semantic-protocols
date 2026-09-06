import ASPProof.ProjectTopologyIdentityRefinement

namespace ASPProof.Audit.ProjectTopologyIdentityRefinement

open ASPProof.ProjectTopologyIdentityRefinement

/- Same serialized-looking number in different identity domains cannot be
   substituted at the type boundary.  The executable counterexamples below
   cover the remaining semantic shortcuts. -/

/--
error: Tactic `decide` proved that the proposition
  contractAdmitted bindingA [] candidateA = true
is false
-/
#guard_msgs in
example : contractAdmitted bindingA [] candidateA = true := by decide

/--
error: Tactic `decide` proved that the proposition
  terminalAdmitted { state := TerminalState.admitted, reasonKind := some 7 } = true
is false
-/
#guard_msgs in
example : terminalAdmitted ⟨.admitted, some 7⟩ = true := by decide

#print axioms exact_contract_candidate_is_admitted
#print axioms compilation_receipt_must_be_independently_admitted
#print axioms admitted_terminal_cannot_carry_a_failure_reason
#print axioms self_index_exclusion_is_required
#print axioms source_identity_drift_rejects_the_complete_contract
#print axioms compiled_abi_drift_rejects_the_compilation_receipt
#print axioms activation_generation_is_not_a_product_identity_field

end ASPProof.Audit.ProjectTopologyIdentityRefinement
