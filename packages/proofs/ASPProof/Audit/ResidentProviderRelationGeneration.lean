-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ResidentProviderRelationGeneration

namespace ASPProof.Audit.ResidentProviderRelationGeneration

open ASPProof.ResidentProviderRelationGeneration

theorem empty_graph_cannot_mask_incomplete_publication
    (resident : ResidentRelationGeneration)
    (workspaceIdentity generationDigest : String)
    (incomplete : resident.complete = false) :
    admitRelationGeneration resident workspaceIdentity generationDigest ≠
      RelationGenerationState.ready := by
  rw [incomplete_relation_generation_fails_closed resident workspaceIdentity generationDigest incomplete]
  decide

theorem installed_search_is_resident_only :
    installedRelationSearchEffects = [.residentMemoryRead] := by
  rfl

end ASPProof.Audit.ResidentProviderRelationGeneration
