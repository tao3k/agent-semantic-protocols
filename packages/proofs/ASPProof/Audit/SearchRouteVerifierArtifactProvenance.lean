import Lean
import ASPProof.SearchRouteVerifierArtifactProvenance

namespace ASPProof.Audit.SearchRouteVerifierArtifactProvenance

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String := []) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", Lean.toJson name),
    ("kind", Lean.toJson "theorem"),
    ("theoremFamily", Lean.toJson theoremFamily),
    ("type", Lean.toJson type),
    ("rfcClauseIds", Lean.toJson [rfcClauseId]),
    ("axioms", Lean.toJson axioms),
    ("hasSorryAx", Lean.toJson false)
  ]

def declarations : Array Lean.Json := #[
  theoremDeclaration
    "valid_provenance_binds_all_stages"
    "stage-closure"
    "Valid provenance binds all five artifact stages"
    "ASP-RFC-10.05-VAP-STAGE-CLOSURE",
  theoremDeclaration
    "valid_provenance_binds_source_snapshot"
    "source-snapshot"
    "Valid provenance binds the immutable source snapshot"
    "ASP-RFC-10.05-VAP-SOURCE-SNAPSHOT",
  theoremDeclaration
    "valid_provenance_binds_hermetic_recipe"
    "hermetic-recipe"
    "Valid provenance binds the hermetic build recipe"
    "ASP-RFC-10.05-VAP-HERMETIC-RECIPE",
  theoremDeclaration
    "valid_provenance_binds_build_attestation"
    "build-attestation"
    "Valid provenance binds the signed build statement"
    "ASP-RFC-10.05-VAP-BUILD-ATTESTATION",
  theoremDeclaration
    "valid_provenance_binds_resolved_binary"
    "binary-binding"
    "The attested build output equals the resolved verifier binary"
    "ASP-RFC-10.05-VAP-EXECUTABLE-PROJECTION",
  theoremDeclaration
    "valid_provenance_uses_distinct_builders"
    "independent-rebuild"
    "The rebuild authority differs from the attesting builder"
    "ASP-RFC-10.05-VAP-INDEPENDENT-REBUILD",
  theoremDeclaration
    "valid_provenance_has_fresh_append_only_transparency"
    "transparency"
    "Valid provenance requires fresh append-only transparency"
    "ASP-RFC-10.05-VAP-TRANSPARENCY",
  theoremDeclaration
    "supply_chain_verified_signature_projects_executable_verification"
    "executable-projection"
    "Supply-chain verification preserves executable verification"
    "ASP-RFC-10.05-VAP-EXECUTABLE-PROJECTION",
  theoremDeclaration
    "supply_chain_verified_signature_binds_executed_binary_to_build"
    "binary-binding"
    "Supply-chain verification binds the executed binary to its build"
    "ASP-RFC-10.05-VAP-EXECUTABLE-PROJECTION",
  theoremDeclaration
    "trusted_digest_equality_prevents_semantic_substitution"
    "digest-assumption-boundary"
    "Digest equality excludes substitution only on an injective trusted domain"
    "ASP-RFC-10.05-VAP-DIGEST-ASSUMPTION",
  theoremDeclaration
    "unverified_route_cannot_win_by_lower_cost"
    "feasibility-first"
    "An unverified route cannot win through lower graph or token cost"
    "ASP-RFC-10.05-VAP-FEASIBILITY-FIRST",
  theoremDeclaration
    "example_provenance_is_valid"
    "stage-closure"
    "The complete example closes all provenance stages"
    "ASP-RFC-10.05-VAP-STAGE-CLOSURE"
    ["propext"],
  theoremDeclaration
    "unpublished_build_is_rejected"
    "transparency"
    "An unpublished build statement is rejected"
    "ASP-RFC-10.05-VAP-TRANSPARENCY"
    ["propext"],
  theoremDeclaration
    "stale_checkpoint_is_rejected"
    "transparency"
    "A stale transparency checkpoint is rejected"
    "ASP-RFC-10.05-VAP-TRANSPARENCY"
    ["propext"],
  theoremDeclaration
    "same_builder_rebuild_is_rejected"
    "independent-rebuild"
    "A same-builder rebuild is not independent"
    "ASP-RFC-10.05-VAP-INDEPENDENT-REBUILD"
    ["propext"],
  theoremDeclaration
    "substituted_binary_is_rejected"
    "substitution-rejection"
    "A build output that differs from the resolved binary is rejected"
    "ASP-RFC-10.05-VAP-BUILD-ATTESTATION"
    ["propext"],
  theoremDeclaration
    "nonhermetic_recipe_is_rejected"
    "hermetic-recipe"
    "A non-hermetic build recipe is rejected"
    "ASP-RFC-10.05-VAP-HERMETIC-RECIPE"
    ["propext"],
  theoremDeclaration
    "binary_digest_only_does_not_prove_provenance"
    "prefix-insufficiency"
    "Binary digest equality alone does not prove provenance"
    "ASP-RFC-10.05-VAP-PREFIX-INSUFFICIENT"
    ["propext"],
  theoremDeclaration
    "logged_build_without_independent_rebuild_does_not_prove_provenance"
    "prefix-insufficiency"
    "A logged build without an independent rebuild is incomplete"
    "ASP-RFC-10.05-VAP-PREFIX-INSUFFICIENT"
    ["propext"],
  theoremDeclaration
    "constant_digest_allows_semantic_substitution"
    "digest-assumption-boundary"
    "A constant digest demonstrates the semantic substitution gap"
    "ASP-RFC-10.05-VAP-DIGEST-ASSUMPTION"
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson "ASPProof.SearchRouteVerifierArtifactProvenance"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteVerifierArtifactProvenance.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 12),
    ("axiomDependentDeclarationCount", Lean.toJson 8),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["propext"]),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson "01.18-searchroute-verifier-artifact-provenance"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteVerifierArtifactProvenance
