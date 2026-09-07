-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

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
#print axioms exact_parser_owned_manifest_is_admitted
#print axioms duplicate_manifest_declarations_are_rejected
#print axioms host_derived_binding_cannot_replace_the_manifest
#print axioms runtime_generated_binding_cannot_replace_the_manifest
#print axioms parser_ownership_is_required_for_manifest_admission
#print axioms exact_contract_is_bound_to_the_exact_manifest
#print axioms another_valid_workspace_cannot_bypass_the_manifest
#print axioms compilation_receipt_must_be_independently_admitted
#print axioms admitted_terminal_cannot_carry_a_failure_reason
#print axioms self_index_exclusion_is_required
#print axioms source_identity_drift_rejects_the_complete_contract
#print axioms workspace_root_boundary_drift_rejects_the_complete_contract
#print axioms compiled_abi_drift_rejects_the_compilation_receipt
#print axioms compilation_receipt_id_collision_does_not_authorize_changed_content
#print axioms distinct_host_local_worktrees_can_bind_the_same_admitted_source
#print axioms local_file_locator_cannot_claim_cross_machine_portability
#print axioms activation_generation_is_not_a_product_identity_field

end ASPProof.Audit.ProjectTopologyIdentityRefinement
