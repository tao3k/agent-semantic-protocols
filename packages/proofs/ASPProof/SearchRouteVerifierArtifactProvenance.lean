import ASPProof.SearchRouteExecutableVerifierConformance

namespace ASPProof.SearchRouteVerifierArtifactProvenance

open ASPProof.SearchRouteExecutableVerifierConformance
open ASPProof.SearchRouteCryptographicTransferReceiptBinding

/-!
The executable-verifier closure proves that a resolved binary was invoked and
independently replayed. It does not prove that the resolved binary was produced
from the intended source snapshot and build recipe. This module closes that
artifact-provenance gap.
-/

structure VerifierArtifactProvenanceContext where
  sourceRepositoryDigest : Nat
  sourceRevisionDigest : Nat
  sourceSnapshotDigest : Nat
  buildRecipeDigest : Nat
  toolchainDigest : Nat
  dependencyLockDigest : Nat
  buildEnvironmentDigest : Nat
  builderAuthorityDigest : Nat
  rebuildAuthorityDigest : Nat
  provenanceStatementDigest : Nat
  transparencyLogDigest : Nat
  transparencyCheckpointDigest : Nat
  transparencyRootDigest : Nat
  minimumTreeSize : Nat
  deriving DecidableEq, Repr

structure SourceSnapshotReceipt where
  repositoryDigest : Nat
  revisionDigest : Nat
  snapshotDigest : Nat
  immutable : Bool
  deriving DecidableEq, Repr

structure BuildRecipeReceipt where
  recipeDigest : Nat
  toolchainDigest : Nat
  dependencyLockDigest : Nat
  environmentDigest : Nat
  hermetic : Bool
  deriving DecidableEq, Repr

structure ArtifactBuildReceipt where
  sourceSnapshotDigest : Nat
  buildRecipeDigest : Nat
  verifierIdDigest : Nat
  algorithmDigest : Nat
  verifierBinaryDigest : Nat
  builderAuthorityDigest : Nat
  provenanceStatementDigest : Nat
  buildSucceeded : Bool
  attestationSignatureVerified : Bool
  deriving DecidableEq, Repr

structure TransparencyInclusionReceipt where
  statementDigest : Nat
  logDigest : Nat
  checkpointDigest : Nat
  rootDigest : Nat
  leafIndex : Nat
  treeSize : Nat
  included : Bool
  checkpointSignatureVerified : Bool
  appendOnlyConsistent : Bool
  fresh : Bool
  deriving DecidableEq, Repr

structure IndependentRebuildReceipt where
  sourceSnapshotDigest : Nat
  buildRecipeDigest : Nat
  verifierBinaryDigest : Nat
  builderAuthorityDigest : Nat
  rebuildSucceeded : Bool
  outputCompared : Bool
  deriving DecidableEq, Repr

structure VerifierArtifactProvenanceReceipt where
  source : SourceSnapshotReceipt
  recipe : BuildRecipeReceipt
  build : ArtifactBuildReceipt
  transparency : TransparencyInclusionReceipt
  rebuild : IndependentRebuildReceipt
  deriving DecidableEq, Repr

def SourceSnapshotConformant
    (context : VerifierArtifactProvenanceContext)
    (receipt : SourceSnapshotReceipt) : Prop :=
  receipt.repositoryDigest = context.sourceRepositoryDigest ∧
  receipt.revisionDigest = context.sourceRevisionDigest ∧
  receipt.snapshotDigest = context.sourceSnapshotDigest ∧
  receipt.immutable = true

def BuildRecipeConformant
    (context : VerifierArtifactProvenanceContext)
    (receipt : BuildRecipeReceipt) : Prop :=
  receipt.recipeDigest = context.buildRecipeDigest ∧
  receipt.toolchainDigest = context.toolchainDigest ∧
  receipt.dependencyLockDigest = context.dependencyLockDigest ∧
  receipt.environmentDigest = context.buildEnvironmentDigest ∧
  receipt.hermetic = true

def BuildAttestationConformant
    (context : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (source : SourceSnapshotReceipt)
    (recipe : BuildRecipeReceipt)
    (receipt : ArtifactBuildReceipt) : Prop :=
  receipt.sourceSnapshotDigest = source.snapshotDigest ∧
  receipt.buildRecipeDigest = recipe.recipeDigest ∧
  receipt.verifierIdDigest = resolution.verifierIdDigest ∧
  receipt.algorithmDigest = resolution.algorithmDigest ∧
  receipt.verifierBinaryDigest = resolution.verifierBinaryDigest ∧
  receipt.builderAuthorityDigest = context.builderAuthorityDigest ∧
  receipt.provenanceStatementDigest = context.provenanceStatementDigest ∧
  receipt.buildSucceeded = true ∧
  receipt.attestationSignatureVerified = true

def TransparencyInclusionConformant
    (context : VerifierArtifactProvenanceContext)
    (build : ArtifactBuildReceipt)
    (receipt : TransparencyInclusionReceipt) : Prop :=
  receipt.statementDigest = build.provenanceStatementDigest ∧
  receipt.logDigest = context.transparencyLogDigest ∧
  receipt.checkpointDigest = context.transparencyCheckpointDigest ∧
  receipt.rootDigest = context.transparencyRootDigest ∧
  receipt.leafIndex < receipt.treeSize ∧
  context.minimumTreeSize ≤ receipt.treeSize ∧
  receipt.included = true ∧
  receipt.checkpointSignatureVerified = true ∧
  receipt.appendOnlyConsistent = true ∧
  receipt.fresh = true

def IndependentRebuildConformant
    (context : VerifierArtifactProvenanceContext)
    (build : ArtifactBuildReceipt)
    (receipt : IndependentRebuildReceipt) : Prop :=
  receipt.sourceSnapshotDigest = build.sourceSnapshotDigest ∧
  receipt.buildRecipeDigest = build.buildRecipeDigest ∧
  receipt.verifierBinaryDigest = build.verifierBinaryDigest ∧
  receipt.builderAuthorityDigest = context.rebuildAuthorityDigest ∧
  receipt.builderAuthorityDigest ≠ build.builderAuthorityDigest ∧
  receipt.rebuildSucceeded = true ∧
  receipt.outputCompared = true

def ValidVerifierArtifactProvenance
    (context : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (receipt : VerifierArtifactProvenanceReceipt) : Prop :=
  SourceSnapshotConformant context receipt.source ∧
  BuildRecipeConformant context receipt.recipe ∧
  BuildAttestationConformant context resolution receipt.source receipt.recipe receipt.build ∧
  TransparencyInclusionConformant context receipt.build receipt.transparency ∧
  IndependentRebuildConformant context receipt.build receipt.rebuild

def BinaryDigestOnlyEvidence
    (resolution : VerifierResolutionReceipt)
    (receipt : VerifierArtifactProvenanceReceipt) : Prop :=
  receipt.build.verifierBinaryDigest = resolution.verifierBinaryDigest

def AttestedBuildOnlyEvidence
    (context : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (receipt : VerifierArtifactProvenanceReceipt) : Prop :=
  SourceSnapshotConformant context receipt.source ∧
  BuildRecipeConformant context receipt.recipe ∧
  BuildAttestationConformant context resolution receipt.source receipt.recipe receipt.build

def LoggedBuildOnlyEvidence
    (context : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (receipt : VerifierArtifactProvenanceReceipt) : Prop :=
  AttestedBuildOnlyEvidence context resolution receipt ∧
  TransparencyInclusionConformant context receipt.build receipt.transparency

def SupplyChainVerifiedExecutableSignature
    (executableContext : ExecutableVerificationContext)
    (provenanceContext : VerifierArtifactProvenanceContext)
    (signature : SignatureEnvelope)
    (executableReceipt : ExecutableSignatureVerificationReceipt)
    (provenanceReceipt : VerifierArtifactProvenanceReceipt) : Prop :=
  ValidExecutableSignatureVerification executableContext signature executableReceipt ∧
  ValidVerifierArtifactProvenance
    provenanceContext executableReceipt.verifierResolution provenanceReceipt

structure ProvenanceGuardedRouteCandidate where
  graphHops : Nat
  inputTokens : Nat
  searchRounds : Nat
  interactionRounds : Nat
  searchCacheMisses : Nat
  modelCacheMissTokens : Nat
  provenance : VerifierArtifactProvenanceReceipt
  deriving DecidableEq, Repr

def RouteCostNoWorse
    (left right : ProvenanceGuardedRouteCandidate) : Prop :=
  left.graphHops ≤ right.graphHops ∧
  left.inputTokens ≤ right.inputTokens ∧
  left.searchRounds ≤ right.searchRounds ∧
  left.interactionRounds ≤ right.interactionRounds ∧
  left.searchCacheMisses ≤ right.searchCacheMisses ∧
  left.modelCacheMissTokens ≤ right.modelCacheMissTokens

def ProvenanceAdmittedRouteNoWorse
    (context : VerifierArtifactProvenanceContext)
    (resolution : VerifierResolutionReceipt)
    (left right : ProvenanceGuardedRouteCandidate) : Prop :=
  ValidVerifierArtifactProvenance context resolution left.provenance ∧
  RouteCostNoWorse left right

structure ArtifactSemantics where
  sourceSnapshotDigest : Nat
  buildRecipeDigest : Nat
  behaviorDigest : Nat
  deriving DecidableEq, Repr

def DigestInjectiveOn
    (digest : ArtifactSemantics → Nat)
    (trusted : ArtifactSemantics → Prop) : Prop :=
  ∀ {left right},
    trusted left →
    trusted right →
    digest left = digest right →
    left = right

def constantArtifactDigest (_ : ArtifactSemantics) : Nat := 0

theorem valid_provenance_binds_all_stages
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    SourceSnapshotConformant context receipt.source ∧
    BuildRecipeConformant context receipt.recipe ∧
    BuildAttestationConformant context resolution receipt.source receipt.recipe receipt.build ∧
    TransparencyInclusionConformant context receipt.build receipt.transparency ∧
    IndependentRebuildConformant context receipt.build receipt.rebuild :=
  valid

theorem valid_provenance_binds_source_snapshot
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    SourceSnapshotConformant context receipt.source :=
  valid.1

theorem valid_provenance_binds_hermetic_recipe
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    BuildRecipeConformant context receipt.recipe :=
  valid.2.1

theorem valid_provenance_binds_build_attestation
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    BuildAttestationConformant
      context resolution receipt.source receipt.recipe receipt.build :=
  valid.2.2.1

theorem valid_provenance_binds_resolved_binary
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    receipt.build.verifierBinaryDigest = resolution.verifierBinaryDigest :=
  valid.2.2.1.2.2.2.2.1

theorem valid_provenance_uses_distinct_builders
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    receipt.rebuild.builderAuthorityDigest ≠ receipt.build.builderAuthorityDigest :=
  by
    rcases valid with ⟨_, _, _, _, rebuild⟩
    exact rebuild.2.2.2.2.1

theorem valid_provenance_has_fresh_append_only_transparency
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {receipt : VerifierArtifactProvenanceReceipt}
    (valid : ValidVerifierArtifactProvenance context resolution receipt) :
    receipt.transparency.appendOnlyConsistent = true ∧
    receipt.transparency.fresh = true :=
  by
    rcases valid with ⟨_, _, _, transparency, _⟩
    rcases transparency with ⟨_, _, _, _, _, _, _, _, appendOnly, fresh⟩
    exact ⟨appendOnly, fresh⟩

theorem supply_chain_verified_signature_projects_executable_verification
    {executableContext : ExecutableVerificationContext}
    {provenanceContext : VerifierArtifactProvenanceContext}
    {signature : SignatureEnvelope}
    {executableReceipt : ExecutableSignatureVerificationReceipt}
    {provenanceReceipt : VerifierArtifactProvenanceReceipt}
    (valid : SupplyChainVerifiedExecutableSignature
      executableContext provenanceContext signature executableReceipt provenanceReceipt) :
    ValidExecutableSignatureVerification executableContext signature executableReceipt :=
  valid.1

theorem supply_chain_verified_signature_binds_executed_binary_to_build
    {executableContext : ExecutableVerificationContext}
    {provenanceContext : VerifierArtifactProvenanceContext}
    {signature : SignatureEnvelope}
    {executableReceipt : ExecutableSignatureVerificationReceipt}
    {provenanceReceipt : VerifierArtifactProvenanceReceipt}
    (valid : SupplyChainVerifiedExecutableSignature
      executableContext provenanceContext signature executableReceipt provenanceReceipt) :
    provenanceReceipt.build.verifierBinaryDigest =
      executableReceipt.verifierResolution.verifierBinaryDigest :=
  valid_provenance_binds_resolved_binary valid.2

theorem trusted_digest_equality_prevents_semantic_substitution
    {digest : ArtifactSemantics → Nat}
    {trusted : ArtifactSemantics → Prop}
    {left right : ArtifactSemantics}
    (faithful : DigestInjectiveOn digest trusted)
    (leftTrusted : trusted left)
    (rightTrusted : trusted right)
    (sameDigest : digest left = digest right) :
    left = right :=
  faithful leftTrusted rightTrusted sameDigest

theorem unverified_route_cannot_win_by_lower_cost
    {context : VerifierArtifactProvenanceContext}
    {resolution : VerifierResolutionReceipt}
    {left right : ProvenanceGuardedRouteCandidate}
    (invalid : ¬ ValidVerifierArtifactProvenance
      context resolution left.provenance) :
    ¬ ProvenanceAdmittedRouteNoWorse context resolution left right := by
  intro admitted
  exact invalid admitted.1

def exampleProvenanceContext : VerifierArtifactProvenanceContext where
  sourceRepositoryDigest := 101
  sourceRevisionDigest := 102
  sourceSnapshotDigest := 103
  buildRecipeDigest := 201
  toolchainDigest := 202
  dependencyLockDigest := 203
  buildEnvironmentDigest := 204
  builderAuthorityDigest := 301
  rebuildAuthorityDigest := 302
  provenanceStatementDigest := 401
  transparencyLogDigest := 501
  transparencyCheckpointDigest := 502
  transparencyRootDigest := 503
  minimumTreeSize := 10

def exampleResolution : VerifierResolutionReceipt where
  verifierIdDigest := 601
  verifierBinaryDigest := 602
  algorithmDigest := 603
  resolved := true

def exampleSource : SourceSnapshotReceipt where
  repositoryDigest := 101
  revisionDigest := 102
  snapshotDigest := 103
  immutable := true

def exampleRecipe : BuildRecipeReceipt where
  recipeDigest := 201
  toolchainDigest := 202
  dependencyLockDigest := 203
  environmentDigest := 204
  hermetic := true

def exampleBuild : ArtifactBuildReceipt where
  sourceSnapshotDigest := 103
  buildRecipeDigest := 201
  verifierIdDigest := 601
  algorithmDigest := 603
  verifierBinaryDigest := 602
  builderAuthorityDigest := 301
  provenanceStatementDigest := 401
  buildSucceeded := true
  attestationSignatureVerified := true

def exampleTransparency : TransparencyInclusionReceipt where
  statementDigest := 401
  logDigest := 501
  checkpointDigest := 502
  rootDigest := 503
  leafIndex := 4
  treeSize := 12
  included := true
  checkpointSignatureVerified := true
  appendOnlyConsistent := true
  fresh := true

def exampleRebuild : IndependentRebuildReceipt where
  sourceSnapshotDigest := 103
  buildRecipeDigest := 201
  verifierBinaryDigest := 602
  builderAuthorityDigest := 302
  rebuildSucceeded := true
  outputCompared := true

def exampleProvenanceReceipt : VerifierArtifactProvenanceReceipt where
  source := exampleSource
  recipe := exampleRecipe
  build := exampleBuild
  transparency := exampleTransparency
  rebuild := exampleRebuild

theorem example_provenance_is_valid :
    ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution exampleProvenanceReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    SourceSnapshotConformant,
    BuildRecipeConformant,
    BuildAttestationConformant,
    TransparencyInclusionConformant,
    IndependentRebuildConformant,
    exampleProvenanceContext,
    exampleResolution,
    exampleProvenanceReceipt,
    exampleSource,
    exampleRecipe,
    exampleBuild,
    exampleTransparency,
    exampleRebuild
  ]

def unpublishedProvenanceReceipt : VerifierArtifactProvenanceReceipt :=
  { exampleProvenanceReceipt with
    transparency := { exampleTransparency with included := false } }

theorem unpublished_build_is_rejected :
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution unpublishedProvenanceReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    TransparencyInclusionConformant,
    unpublishedProvenanceReceipt,
    exampleProvenanceReceipt,
    exampleTransparency
  ]

def staleCheckpointProvenanceReceipt : VerifierArtifactProvenanceReceipt :=
  { exampleProvenanceReceipt with
    transparency := { exampleTransparency with fresh := false } }

theorem stale_checkpoint_is_rejected :
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution staleCheckpointProvenanceReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    TransparencyInclusionConformant,
    staleCheckpointProvenanceReceipt,
    exampleProvenanceReceipt,
    exampleTransparency
  ]

def sameBuilderRebuildReceipt : VerifierArtifactProvenanceReceipt :=
  { exampleProvenanceReceipt with
    rebuild := { exampleRebuild with builderAuthorityDigest := 301 } }

theorem same_builder_rebuild_is_rejected :
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution sameBuilderRebuildReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    IndependentRebuildConformant,
    sameBuilderRebuildReceipt,
    exampleProvenanceReceipt,
    exampleRebuild,
    exampleBuild
  ]

def substitutedBinaryReceipt : VerifierArtifactProvenanceReceipt :=
  { exampleProvenanceReceipt with
    build := { exampleBuild with verifierBinaryDigest := 999 } }

theorem substituted_binary_is_rejected :
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution substitutedBinaryReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    BuildAttestationConformant,
    substitutedBinaryReceipt,
    exampleProvenanceReceipt,
    exampleBuild,
    exampleResolution
  ]

def nonhermeticRecipeReceipt : VerifierArtifactProvenanceReceipt :=
  { exampleProvenanceReceipt with
    recipe := { exampleRecipe with hermetic := false } }

theorem nonhermetic_recipe_is_rejected :
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution nonhermeticRecipeReceipt := by
  simp [
    ValidVerifierArtifactProvenance,
    BuildRecipeConformant,
    nonhermeticRecipeReceipt,
    exampleProvenanceReceipt,
    exampleRecipe
  ]

theorem binary_digest_only_does_not_prove_provenance :
    BinaryDigestOnlyEvidence exampleResolution unpublishedProvenanceReceipt ∧
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution unpublishedProvenanceReceipt := by
  constructor
  · simp [
      BinaryDigestOnlyEvidence,
      unpublishedProvenanceReceipt,
      exampleProvenanceReceipt,
      exampleBuild,
      exampleResolution
    ]
  · exact unpublished_build_is_rejected

theorem logged_build_without_independent_rebuild_does_not_prove_provenance :
    LoggedBuildOnlyEvidence exampleProvenanceContext exampleResolution sameBuilderRebuildReceipt ∧
    ¬ ValidVerifierArtifactProvenance
      exampleProvenanceContext exampleResolution sameBuilderRebuildReceipt := by
  constructor
  · simpa [
      LoggedBuildOnlyEvidence,
      AttestedBuildOnlyEvidence,
      sameBuilderRebuildReceipt
    ] using
      And.intro
        (And.intro
          example_provenance_is_valid.1
          (And.intro
            example_provenance_is_valid.2.1
            example_provenance_is_valid.2.2.1))
        example_provenance_is_valid.2.2.2.1
  · exact same_builder_rebuild_is_rejected

def collidingArtifactLeft : ArtifactSemantics where
  sourceSnapshotDigest := 1
  buildRecipeDigest := 2
  behaviorDigest := 3

def collidingArtifactRight : ArtifactSemantics where
  sourceSnapshotDigest := 1
  buildRecipeDigest := 2
  behaviorDigest := 4

theorem constant_digest_allows_semantic_substitution :
    collidingArtifactLeft ≠ collidingArtifactRight ∧
    constantArtifactDigest collidingArtifactLeft =
      constantArtifactDigest collidingArtifactRight := by
  decide

end ASPProof.SearchRouteVerifierArtifactProvenance
