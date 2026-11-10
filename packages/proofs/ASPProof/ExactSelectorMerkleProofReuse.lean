-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ExactSelectorMerkleProofReuse

/-- Generation work is separated from selector attachment so the model cannot
silently charge a complete workspace-tree rebuild to every selector. -/
structure ProjectionWork where
  ownerCount : Nat
  selectorCount : Nat
  workspaceTreeBuilds : Nat
  ownerProofDerivations : Nat
  selectorProofAttachments : Nat
  deriving DecidableEq, Repr

def cachedOwnerProofWork (ownerCount selectorCount : Nat) : ProjectionWork :=
  {
    ownerCount
    selectorCount
    workspaceTreeBuilds := if ownerCount = 0 then 0 else 1
    ownerProofDerivations := ownerCount
    selectorProofAttachments := selectorCount
  }

def proofReuseClosed (work : ProjectionWork) : Prop :=
  work.workspaceTreeBuilds ≤ 1 ∧
    work.ownerProofDerivations = work.ownerCount ∧
    work.selectorProofAttachments = work.selectorCount

theorem cached_owner_proofs_build_the_workspace_tree_at_most_once
    (ownerCount selectorCount : Nat) :
    (cachedOwnerProofWork ownerCount selectorCount).workspaceTreeBuilds ≤ 1 := by
  cases ownerCount with
  | zero => exact Nat.zero_le 1
  | succ _ => exact Nat.le_refl 1

theorem selector_growth_cannot_increase_workspace_tree_builds
    (ownerCount leftSelectors rightSelectors : Nat) :
    (cachedOwnerProofWork ownerCount leftSelectors).workspaceTreeBuilds =
      (cachedOwnerProofWork ownerCount rightSelectors).workspaceTreeBuilds := by
  rfl

theorem cached_owner_proof_plan_is_closed
    (ownerCount selectorCount : Nat) :
    proofReuseClosed (cachedOwnerProofWork ownerCount selectorCount) := by
  refine ⟨cached_owner_proofs_build_the_workspace_tree_at_most_once ownerCount selectorCount, ?_⟩
  exact ⟨rfl, rfl⟩

end ASPProof.ExactSelectorMerkleProofReuse
