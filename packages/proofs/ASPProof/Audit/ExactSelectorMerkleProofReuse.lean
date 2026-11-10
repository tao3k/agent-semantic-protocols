-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ExactSelectorMerkleProofReuse

namespace ASPProof.Audit.ExactSelectorMerkleProofReuse

open ASPProof.ExactSelectorMerkleProofReuse

theorem selector_count_is_not_a_tree_rebuild_multiplier
    (ownerCount selectorCount : Nat) :
    (cachedOwnerProofWork ownerCount selectorCount).workspaceTreeBuilds ≤ 1 := by
  exact cached_owner_proofs_build_the_workspace_tree_at_most_once
    ownerCount selectorCount

theorem production_plan_preserves_owner_and_selector_cardinality
    (ownerCount selectorCount : Nat) :
    proofReuseClosed (cachedOwnerProofWork ownerCount selectorCount) := by
  exact cached_owner_proof_plan_is_closed ownerCount selectorCount

end ASPProof.Audit.ExactSelectorMerkleProofReuse

#print axioms ASPProof.Audit.ExactSelectorMerkleProofReuse.selector_count_is_not_a_tree_rebuild_multiplier
#print axioms ASPProof.Audit.ExactSelectorMerkleProofReuse.production_plan_preserves_owner_and_selector_cardinality
