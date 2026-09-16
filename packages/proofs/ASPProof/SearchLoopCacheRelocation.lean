-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchLoopCacheIdentity

namespace SearchLoopCacheRelocation

open SearchLoopCacheIdentity

def relocateSelector
    (key : SemanticKey)
    (selector : Digest) :
    SemanticKey :=
  { key with selector := selector }

theorem selector_drift_invalidates_without_relocation
    (key : SemanticKey)
    (newSelector : Digest)
    (drift : key.selector ≠ newSelector) :
    decideSemanticReuse key (relocateSelector key newSelector) =
      CacheDecision.invalidate := by
  apply semantic_key_drift_invalidates
  intro equal
  have equalSelector := congrArg SemanticKey.selector equal
  simp [relocateSelector] at equalSelector
  exact drift equalSelector

theorem explicit_relocation_rekeys_to_reuse
    (cached current : SemanticKey)
    (relocationProof :
      relocateSelector cached current.selector = current) :
    decideSemanticReuse
        (relocateSelector cached current.selector)
        current =
      CacheDecision.reuse := by
  rw [relocationProof]
  exact stable_semantic_key_reuses current

end SearchLoopCacheRelocation
