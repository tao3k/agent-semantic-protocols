-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.ResidentProviderRelationGeneration

inductive RelationGenerationState where
  | ready
  | publicationRequired
  deriving DecidableEq, Repr

structure ProviderRelationEndpoint where
  kind : String
  id : String
  deriving DecidableEq, Repr

structure ProviderRelation where
  source : ProviderRelationEndpoint
  kind : String
  target : ProviderRelationEndpoint
  deriving DecidableEq, Repr

structure ResidentRelationGeneration where
  workspaceIdentity : String
  generationDigest : String
  complete : Bool
  relations : List ProviderRelation
  deriving DecidableEq, Repr

inductive InstalledSearchEffect where
  | residentMemoryRead
  | providerSpawn
  | packageManagerSpawn
  | providerStdoutDecode
  deriving DecidableEq, Repr

def admitRelationGeneration
    (resident : ResidentRelationGeneration)
    (workspaceIdentity generationDigest : String) : RelationGenerationState :=
  if resident.complete = true
      ∧ resident.workspaceIdentity = workspaceIdentity
      ∧ resident.generationDigest = generationDigest then
    .ready
  else
    .publicationRequired

def installedRelationSearchEffects : List InstalledSearchEffect :=
  [.residentMemoryRead]

theorem incomplete_relation_generation_fails_closed
    (resident : ResidentRelationGeneration)
    (workspaceIdentity generationDigest : String)
    (incomplete : resident.complete = false) :
    admitRelationGeneration resident workspaceIdentity generationDigest =
      .publicationRequired := by
  simp [admitRelationGeneration, incomplete]

theorem cross_workspace_relation_generation_is_rejected
    (resident : ResidentRelationGeneration)
    (workspaceIdentity generationDigest : String)
    (differentWorkspace : resident.workspaceIdentity ≠ workspaceIdentity) :
    admitRelationGeneration resident workspaceIdentity generationDigest =
      .publicationRequired := by
  simp [admitRelationGeneration, differentWorkspace]

theorem cross_generation_relation_read_is_rejected
    (resident : ResidentRelationGeneration)
    (workspaceIdentity generationDigest : String)
    (differentGeneration : resident.generationDigest ≠ generationDigest) :
    admitRelationGeneration resident workspaceIdentity generationDigest =
      .publicationRequired := by
  simp [admitRelationGeneration, differentGeneration]

theorem installed_relation_search_has_no_process_effects :
    InstalledSearchEffect.providerSpawn ∉ installedRelationSearchEffects
      ∧ InstalledSearchEffect.packageManagerSpawn ∉ installedRelationSearchEffects
      ∧ InstalledSearchEffect.providerStdoutDecode ∉ installedRelationSearchEffects := by
  decide

end ASPProof.ResidentProviderRelationGeneration
