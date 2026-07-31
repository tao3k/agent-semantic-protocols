import Lean
import ASPProof.SearchRouteProofCarryingSelectiveDisclosure

namespace ASPProof.Audit.SearchRouteProofCarryingSelectiveDisclosure

open Lean

private def stringArray (values : Array String) : Json :=
  .arr (values.map Json.str)

private def declaration
    (name theoremFamily description : String)
    (clauseIds axioms : Array String) : Json :=
  .mkObj [
    ("axioms", stringArray axioms),
    ("hasSorryAx", .bool false),
    ("kind", .str "theorem"),
    ("name", .str name),
    ("rfcClauseIds", stringArray clauseIds),
    ("theoremFamily", .str theoremFamily),
    ("type", .str description)
  ]

def declarations : Array Json := #[
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.valid_disclosure_binds_identity"
    "disclosure-identity-binding"
    "valid disclosure binds summary policy and query identities"
    #["ASP-RFC-10.05-PCSD-IDENTITY"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.valid_disclosure_covers_required_edges"
    "required-edge-coverage"
    "valid disclosure contains the required ordered edge sublist"
    #["ASP-RFC-10.05-PCSD-COVERAGE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.all_below_member"
    "bounded-edge-membership"
    "membership in an all-below list implies the edge is in range"
    #["ASP-RFC-10.05-PCSD-COVERAGE"] #["propext"],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.valid_disclosure_rejects_out_of_range_edges"
    "out-of-range-edge-rejection"
    "every disclosed edge is below the committed summary edge count"
    #["ASP-RFC-10.05-PCSD-COVERAGE", "ASP-RFC-10.05-PCSD-ROUTER"] #["propext"],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.exact_required_edges_are_token_minimal"
    "exact-required-token-minimality"
    "exact canonical required-edge disclosure is token-minimal"
    #["ASP-RFC-10.05-PCSD-MINIMAL", "ASP-RFC-10.05-PCSD-REQUEST"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.authorized_decision_requires_verified_chain"
    "authorization-requires-chain-proof"
    "decision authorization requires the expanded chain proof"
    #["ASP-RFC-10.05-PCSD-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.authorized_decision_requires_valid_disclosure"
    "authorization-requires-disclosure-proof"
    "decision authorization requires a valid disclosure receipt"
    #["ASP-RFC-10.05-PCSD-AUTHORIZATION"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.search_cache_identity_does_not_determine_proof_identity"
    "search-proof-cache-independence"
    "equal search cache identity does not determine proof cache identity"
    #["ASP-RFC-10.05-PCSD-PROOF-CACHE", "ASP-RFC-10.05-PCSD-SEARCH-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.proof_cache_identity_does_not_determine_model_prefix_identity"
    "proof-model-cache-independence"
    "equal proof cache identity does not determine model prefix identity"
    #["ASP-RFC-10.05-PCSD-MODEL-CACHE", "ASP-RFC-10.05-PCSD-PROOF-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.model_prefix_identity_does_not_determine_search_identity"
    "model-search-cache-independence"
    "equal model prefix identity does not determine search cache identity"
    #["ASP-RFC-10.05-PCSD-MODEL-CACHE", "ASP-RFC-10.05-PCSD-SEARCH-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.search_cache_hit_reduces_only_provider_calls"
    "search-cache-cost-isolation"
    "search-only cache hit removes provider calls and preserves other coordinates"
    #["ASP-RFC-10.05-PCSD-COST", "ASP-RFC-10.05-PCSD-SEARCH-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.proof_cache_hit_reduces_only_proof_checks"
    "proof-cache-cost-isolation"
    "proof-only cache hit removes proof checks and preserves other coordinates"
    #["ASP-RFC-10.05-PCSD-COST", "ASP-RFC-10.05-PCSD-PROOF-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.model_prefix_hit_reduces_only_uncached_model_tokens"
    "model-cache-cost-isolation"
    "model-only cache hit removes uncached model tokens and preserves other coordinates"
    #["ASP-RFC-10.05-PCSD-COST", "ASP-RFC-10.05-PCSD-MODEL-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteProofCarryingSelectiveDisclosure.all_cache_misses_preserve_baseline"
    "cache-miss-cost-conservation"
    "all cache misses preserve the complete baseline work vector"
    #["ASP-RFC-10.05-PCSD-COST"] #[]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 2),
    ("axiomFreeDeclarationCount", .num 12),
    ("axiomInventory", stringArray #["propext"]),
    ("declarationCount", .num 14),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteProofCarryingSelectiveDisclosure"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteProofCarryingSelectiveDisclosure.lean")
  ]

end ASPProof.Audit.SearchRouteProofCarryingSelectiveDisclosure
