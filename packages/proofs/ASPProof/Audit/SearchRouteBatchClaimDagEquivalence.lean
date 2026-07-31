import Lean
import ASPProof.SearchRouteBatchClaimDagEquivalence

namespace ASPProof.Audit.SearchRouteBatchClaimDagEquivalence

def theoremDeclaration
    (name theoremFamily type : String)
    (rfcClauseIds : List String)
    (axioms : List String := []) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", Lean.toJson name),
    ("kind", Lean.toJson "theorem"),
    ("theoremFamily", Lean.toJson theoremFamily),
    ("type", Lean.toJson type),
    ("rfcClauseIds", Lean.toJson rfcClauseIds),
    ("axioms", Lean.toJson axioms),
    ("hasSorryAx", Lean.toJson false)
  ]

def declarations : Array Lean.Json := #[
  theoremDeclaration
    "valid_individual_claim_projects_expected_outcome"
    "individual-outcome-projection"
    "Individual execution projects the expected typed claim outcome"
    ["ASP-RFC-10.05-BCDE-SEMANTIC-EQUIVALENCE"]
    ["propext"],
  theoremDeclaration
    "valid_batch_projects_each_expected_outcome"
    "batch-outcome-projection"
    "Batch execution contains each expected typed claim outcome"
    ["ASP-RFC-10.05-BCDE-SEMANTIC-EQUIVALENCE"]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "valid_batch_and_individual_claims_have_equivalent_outcomes"
    "semantic-equivalence"
    "Valid individual and batch transports agree on claim outcomes"
    ["ASP-RFC-10.05-BCDE-SEMANTIC-EQUIVALENCE"]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "valid_batch_has_distinct_replay_executor"
    "independent-replay"
    "Valid batch replay uses a distinct executor"
    ["ASP-RFC-10.05-BCDE-INDEPENDENT-REPLAY"],
  theoremDeclaration
    "valid_batch_has_nonempty_unique_manifest"
    "manifest-integrity"
    "A valid batch has nonempty duplicate-free targets"
    [
      "ASP-RFC-10.05-BCDE-NONEMPTY-MANIFEST",
      "ASP-RFC-10.05-BCDE-UNIQUE-TARGETS"
    ],
  theoremDeclaration
    "valid_batch_binds_canonical_manifest"
    "manifest-integrity"
    "A valid batch binds the complete canonical manifest predicate"
    ["ASP-RFC-10.05-BCDE-CANONICAL-MANIFEST"],
  theoremDeclaration
    "valid_batch_entries_share_execution_key"
    "shared-execution-key"
    "Every valid batch entry shares one execution key"
    ["ASP-RFC-10.05-BCDE-SHARED-EXECUTION-KEY"],
  theoremDeclaration
    "batch_tool_rounds_no_more_than_individual"
    "tool-round-bound"
    "Nonempty batch tool rounds do not exceed individual rounds"
    ["ASP-RFC-10.05-BCDE-COST-BOUND"]
    ["propext"],
  theoremDeclaration
    "batch_tool_rounds_strictly_less_for_multiple_claims"
    "tool-round-bound"
    "Multiple claims have strictly fewer batch tool rounds"
    ["ASP-RFC-10.05-BCDE-COST-BOUND"]
    ["propext"],
  theoremDeclaration
    "batch_graph_nodes_no_more_than_individual"
    "graph-node-bound"
    "Nonempty batch graph nodes do not exceed individual nodes"
    ["ASP-RFC-10.05-BCDE-COST-BOUND"]
    ["propext"],
  theoremDeclaration
    "batch_graph_nodes_strictly_less_for_multiple_claims"
    "graph-node-bound"
    "Multiple claims have strictly fewer batch graph nodes"
    ["ASP-RFC-10.05-BCDE-COST-BOUND"]
    ["propext"],
  theoremDeclaration
    "batch_input_tokens_no_more_than_individual"
    "token-bound"
    "Nonempty batch input tokens do not exceed individual tokens"
    ["ASP-RFC-10.05-BCDE-COST-BOUND"]
    ["propext"],
  theoremDeclaration
    "example_batch_is_valid"
    "manifest-integrity"
    "The canonical example closes batch execution"
    [
      "ASP-RFC-10.05-BCDE-CANONICAL-MANIFEST",
      "ASP-RFC-10.05-BCDE-NONEMPTY-MANIFEST",
      "ASP-RFC-10.05-BCDE-UNIQUE-TARGETS"
    ]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "duplicate_manifest_targets_are_rejected"
    "duplicate-target-rejection"
    "Duplicate manifest targets are rejected"
    [
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT",
      "ASP-RFC-10.05-BCDE-UNIQUE-TARGETS"
    ]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "cross_policy_batch_entry_is_rejected"
    "cross-policy-rejection"
    "A cross-policy batch entry cannot share the execution key"
    [
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT",
      "ASP-RFC-10.05-BCDE-SHARED-EXECUTION-KEY"
    ]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "permuted_decision_vector_is_rejected"
    "ordered-decision-rejection"
    "A permuted accepted decision vector is rejected"
    [
      "ASP-RFC-10.05-BCDE-ORDERED-DECISIONS",
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT"
    ]
    ["Quot.sound", "propext"],
  theoremDeclaration
    "wrong_manifest_invocation_is_rejected"
    "manifest-invocation-rejection"
    "Invocation under another manifest is rejected"
    [
      "ASP-RFC-10.05-BCDE-CANONICAL-MANIFEST",
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT"
    ]
    ["propext"],
  theoremDeclaration
    "same_executor_batch_replay_is_rejected"
    "independent-replay"
    "Same-executor batch replay is rejected"
    [
      "ASP-RFC-10.05-BCDE-INDEPENDENT-REPLAY",
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT"
    ]
    ["propext"],
  theoremDeclaration
    "binary_only_cache_hit_does_not_prove_search_batch_reuse"
    "search-cache-separation"
    "Binary-only equality does not prove search batch cache reuse"
    [
      "ASP-RFC-10.05-BCDE-PREFIX-INSUFFICIENT",
      "ASP-RFC-10.05-BCDE-SEARCH-CACHE-IDENTITY"
    ]
    ["propext"],
  theoremDeclaration
    "model_prefix_cache_hit_does_not_prove_search_evidence_reuse"
    "model-cache-separation"
    "Model prefix cache equality does not prove search evidence reuse"
    ["ASP-RFC-10.05-BCDE-MODEL-CACHE-SEPARATION"]
    ["propext"],
  theoremDeclaration
    "batch_semantic_equivalence_does_not_fabricate_individual_invocation"
    "transport-history-separation"
    "Equivalent outcomes do not imply identical invocation histories"
    [
      "ASP-RFC-10.05-BCDE-NO-FALSE-INDIVIDUAL",
      "ASP-RFC-10.05-BCDE-SEMANTIC-EQUIVALENCE"
    ]
    ["Quot.sound", "propext"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson "ASPProof.SearchRouteBatchClaimDagEquivalence"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteBatchClaimDagEquivalence.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 4),
    ("axiomDependentDeclarationCount", Lean.toJson 17),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["Quot.sound", "propext"]),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson "01.20-searchroute-batch-claim-dag-equivalence"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteBatchClaimDagEquivalence
