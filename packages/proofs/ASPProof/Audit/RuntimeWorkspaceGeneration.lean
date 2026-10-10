-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.RuntimeWorkspaceGeneration

namespace ASPProof.Audit.RuntimeWorkspaceGeneration

open ASPProof.RuntimeWorkspaceGeneration

theorem cold_control_identity_is_not_generation_readiness
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) :
    (admitColdWorkspaceIdentity key entries).generationReady = false := by
  exact cold_identity_admission_does_not_construct_generation key entries

theorem cold_control_identity_opens_no_workspace_database
    (key : ProjectWorkspaceKey)
    (entries : List ProjectWorkspaceKey) :
    (admitColdWorkspaceIdentity key entries).databaseOpened = false := by
  exact cold_identity_admission_does_not_open_workspace_database key entries

theorem generation_identity_is_a_function
    (inputs : GenerationInputs) :
    deriveGenerationId inputs = deriveGenerationId inputs := by
  rfl

theorem v1_baseline_has_no_partial_overlay_lineage :
    OverlayLineageComplete none none := by
  exact baseline_overlay_lineage_is_complete

theorem v1_successor_commits_base_and_dirty_set_together
    (baseRoot dirtyPaths : Digest) :
    OverlayLineageComplete (some baseRoot) (some dirtyPaths) := by
  exact successor_overlay_lineage_is_complete baseRoot dirtyPaths

theorem v1_half_overlay_cannot_be_published (baseRoot : Digest) :
    ¬ OverlayLineageComplete (some baseRoot) none := by
  exact base_without_dirty_paths_is_not_a_generation baseRoot

theorem resident_read_gate_requires_zero_db_open
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.databaseOpens = 0 := by
  exact admitted.2.2.1

theorem resident_read_gate_requires_zero_control_roundtrip
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.controlSocketRoundtrips = 0 := by
  exact admitted.2.2.2.2

theorem resident_read_gate_cannot_spawn_generation_provider
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.providerSpawns = 0 := by
  exact admitted.2.2.2.1

theorem resident_read_gate_requires_zero_filesystem_read
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.filesystemReads = 0 := by
  exact admitted.2.1

theorem resident_read_gate_requires_memory_hit
    (receipt : ResidentReadReceipt)
    (admitted : ResidentOnlyRead receipt) :
    receipt.residentHit = true := by
  exact admitted.1

end ASPProof.Audit.RuntimeWorkspaceGeneration
