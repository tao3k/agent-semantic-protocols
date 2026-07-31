import Lean
import ASPProof.SearchRouteCertifiedTransitionChainCompression

namespace ASPProof.Audit.SearchRouteCertifiedTransitionChainCompression

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
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.valid_chain_append"
    "valid-chain-composition"
    "valid chains sharing a middle domain compose by append"
    #["ASP-RFC-10.05-CTCC-CHAIN", "ASP-RFC-10.05-CTCC-COMPOSE"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.valid_chain_covers_every_transition"
    "complete-transition-coverage"
    "valid chain proves every underlying transition certificate"
    #["ASP-RFC-10.05-CTCC-COVERAGE"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.valid_chain_edge_count_matches_epoch_span"
    "epoch-span-edge-count"
    "start epoch plus chain length equals end epoch"
    #["ASP-RFC-10.05-CTCC-NO-SKIP"] #["propext"],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.distinct_epoch_span_requires_nonempty_chain"
    "distinct-epoch-nonempty-chain"
    "different epochs cannot be connected by an empty chain"
    #["ASP-RFC-10.05-CTCC-NO-SKIP"] #["propext"],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.missing_head_archive_blocks_full_replay"
    "missing-archive-chain-replay-rejection"
    "one missing old snapshot archive blocks full chain replay"
    #["ASP-RFC-10.05-CTCC-REPLAY"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.archived_valid_chain_is_fully_replayable"
    "fully-archived-chain-replay"
    "valid chain with all old snapshots archived is fully replayable"
    #["ASP-RFC-10.05-CTCC-REPLAY"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.certified_summary_preserves_every_trust_edge"
    "summary-preserves-trust-coverage"
    "certified summary preserves validation of every trust edge"
    #["ASP-RFC-10.05-CTCC-COVERAGE", "ASP-RFC-10.05-CTCC-SUMMARY"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.certified_summary_reports_exact_trust_edge_count"
    "summary-exact-edge-count"
    "summary edge count equals expanded trust chain length"
    #["ASP-RFC-10.05-CTCC-SUMMARY"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.nonempty_chain_summary_has_one_presentation_hop"
    "single-hop-summary-presentation"
    "non-empty expanded chain may render as one presentation hop"
    #["ASP-RFC-10.05-CTCC-PRESENTATION"] #["propext"],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.cache_identity_binds_digest_domains_and_edge_count"
    "cache-identity-completeness"
    "cache reuse binds digest start end and edge count"
    #["ASP-RFC-10.05-CTCC-CACHE"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.injective_digest_makes_summary_chain_unique"
    "injective-digest-chain-identity"
    "injective chain digest makes one summary identify one transition list"
    #["ASP-RFC-10.05-CTCC-DIGEST"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.digest_collision_blocks_unique_chain_identity"
    "digest-collision-identity-rejection"
    "admitted digest collision refutes unique chain identity"
    #["ASP-RFC-10.05-CTCC-DIGEST"] #[],
  declaration
    "ASPProof.SearchRouteCertifiedTransitionChainCompression.certified_summary_cannot_skip_epoch_edges"
    "router-summary-no-skip"
    "certified summary edge count still equals the complete epoch span"
    #["ASP-RFC-10.05-CTCC-NO-SKIP", "ASP-RFC-10.05-CTCC-ROUTER"] #["propext"]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 4),
    ("axiomFreeDeclarationCount", .num 9),
    ("axiomInventory", stringArray #["propext"]),
    ("declarationCount", .num 13),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteCertifiedTransitionChainCompression"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteCertifiedTransitionChainCompression.lean")
  ]

end ASPProof.Audit.SearchRouteCertifiedTransitionChainCompression
