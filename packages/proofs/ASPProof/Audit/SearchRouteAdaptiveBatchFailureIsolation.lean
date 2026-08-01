import Lean
import ASPProof.SearchRouteAdaptiveBatchFailureIsolation

namespace ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation

private def canonicalStrings (values : List String) : List String :=
  (values.mergeSort (fun left right => decide (left < right))).eraseDups

private def declaration
    (name theoremFamily type : String)
    (rfcClauseIds axioms : List String) : Lean.Json :=
  Lean.Json.mkObj [
    ("name", Lean.toJson name),
    ("kind", Lean.toJson "theorem"),
    ("theoremFamily", Lean.toJson theoremFamily),
    ("type", Lean.toJson type),
    ("rfcClauseIds", Lean.toJson (canonicalStrings rfcClauseIds)),
    ("axioms", Lean.toJson (canonicalStrings axioms)),
    ("hasSorryAx", Lean.toJson false)
  ]

private def free (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses []

private def prop (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses ["propext"]

private def quot (name family type : String) (clauses : List String) : Lean.Json :=
  declaration name family type clauses ["Quot.sound", "propext"]

def declarations : Array Lean.Json := #[
  prop "source_claim_is_successful_or_unresolved" "source-triage"
    "Source claim enters successful or unresolved state"
    ["ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  prop "successful_claim_is_not_unresolved" "success-isolation"
    "Successful claim cannot also be unresolved"
    ["ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  quot "annotation_claim_is_eligible_or_fail_closed" "reason-triage"
    "Annotated failure enters eligible or fail-closed state"
    ["ASP-RFC-10.05-ABFI-ANNOTATED-UNRESOLVED",
     "ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  quot "unresolved_claim_is_eligible_or_fail_closed" "reason-triage"
    "Matching annotations classify every unresolved claim"
    ["ASP-RFC-10.05-ABFI-ANNOTATED-UNRESOLVED",
     "ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  free "semantic_rejection_is_not_fallback_eligible" "fallback-eligibility"
    "Semantic rejection is fail-closed"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  free "policy_mismatch_is_not_fallback_eligible" "fallback-eligibility"
    "Policy mismatch is fail-closed"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  free "replay_mismatch_is_not_fallback_eligible" "fallback-eligibility"
    "Replay mismatch is fail-closed"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  free "agent_preference_is_not_fallback_eligible" "fallback-eligibility"
    "Agent preference cannot authorize fallback"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  quot "valid_fallback_contains_only_eligible_annotations" "fallback-soundness"
    "Fallback claims require eligible annotations"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  free "valid_fallback_covers_every_eligible_claim" "fallback-completeness"
    "Valid fallback covers every eligible claim"
    ["ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  quot "every_source_claim_is_success_fallback_or_fail_closed" "final-triage"
    "Every source claim has an observable terminal class"
    ["ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  free "adaptive_tool_rounds_le_individual_iff" "round-nonregression"
    "Round non-regression iff batch plus fallback count fits"
    ["ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION"],
  prop "adaptive_tool_rounds_save_one_claim_iff" "round-savings-margin"
    "One claim of count slack saves one claim of rounds"
    ["ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION"],
  prop "no_failure_partition_saves_rounds" "partition-round-savings"
    "No-failure partition with count slack saves rounds"
    ["ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION", "ASP-RFC-10.05-ABFI-SHARED-EXECUTION"],
  free "batch_count_bound_alone_does_not_prove_round_savings" "cost-counterexample"
    "Batch count alone does not prove post-fallback savings"
    ["ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION"],
  prop "singleton_partition_without_fallback_has_no_round_savings" "singleton-nonsavings"
    "Singleton batches equal individual round cost"
    ["ASP-RFC-10.05-ABFI-NO-SINGLETON-CLAIM"],
  prop "singleton_partition_with_any_fallback_regresses" "singleton-fallback-regression"
    "Any singleton fallback regresses rounds"
    ["ASP-RFC-10.05-ABFI-NO-SINGLETON-CLAIM", "ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION"],
  prop "full_fallback_cannot_satisfy_round_nonregression" "full-fallback-rejection"
    "Full fallback violates non-regression"
    ["ASP-RFC-10.05-ABFI-NO-FULL-FALLBACK"],
  prop "single_batch_plus_full_fallback_always_costs_more" "full-fallback-rejection"
    "One batch plus full fallback costs more"
    ["ASP-RFC-10.05-ABFI-NO-FULL-FALLBACK"],
  quot "adaptive_input_tokens_le_individual_iff" "token-nonregression"
    "Token bound exposes repeated fallback context"
    ["ASP-RFC-10.05-ABFI-TOKEN-NONREGRESSION"],
  free "cost_admission_implies_round_and_token_bounds" "cost-admission"
    "Cost admission projects round and token bounds"
    ["ASP-RFC-10.05-ABFI-ROUND-NONREGRESSION", "ASP-RFC-10.05-ABFI-TOKEN-NONREGRESSION"],
  prop "admitted_execution_cannot_fallback_every_source_claim" "admitted-full-fallback-rejection"
    "Admitted execution cannot fallback every source claim"
    ["ASP-RFC-10.05-ABFI-NO-FULL-FALLBACK"],
  prop "plan_only_cache_hit_does_not_prove_fallback_reuse" "fallback-cache-separation"
    "Source-only equality does not prove fallback reuse"
    ["ASP-RFC-10.05-ABFI-CACHE-IDENTITY"],
  quot "example_partition_is_valid" "partition-integrity"
    "Canonical example is an exact compatible partition"
    ["ASP-RFC-10.05-ABFI-COMPATIBLE-KEY", "ASP-RFC-10.05-ABFI-EXACT-PARTITION"],
  prop "example_partition_shares_execution" "shared-execution"
    "Canonical example has fewer batches than claims"
    ["ASP-RFC-10.05-ABFI-SHARED-EXECUTION"],
  quot "cross_key_batch_is_rejected" "compatibility-rejection"
    "Cross-key claim batch is rejected"
    ["ASP-RFC-10.05-ABFI-COMPATIBLE-KEY"],
  prop "example_attempts_match_partition" "attempt-plan-binding"
    "Example attempts preserve planned batch order"
    ["ASP-RFC-10.05-ABFI-EXACT-PARTITION"],
  free "example_success_isolated_from_failed_batch" "failure-isolation-example"
    "Failed batch leaves successful claims intact"
    ["ASP-RFC-10.05-ABFI-OBSERVABLE-TRIAGE"],
  quot "example_fallback_is_exact" "eligible-fallback-example"
    "Technical failure satisfies exact budgeted fallback"
    ["ASP-RFC-10.05-ABFI-BUDGET", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"],
  quot "semantic_rejection_fallback_is_rejected" "semantic-fallback-rejection"
    "Semantic rejection cannot enter fallback receipt"
    ["ASP-RFC-10.05-ABFI-FAIL-CLOSED", "ASP-RFC-10.05-ABFI-FALLBACK-ELIGIBILITY"]
]

def manifest : Lean.Json :=
  Lean.Json.mkObj [
    ("schemaId", Lean.toJson "asp.lean-proof-audit.v1"),
    ("schemaVersion", Lean.toJson "1"),
    ("leanVersion", Lean.toJson "4.32.2"),
    ("proofPackage", Lean.toJson "ASPProof"),
    ("module", Lean.toJson "ASPProof.SearchRouteAdaptiveBatchFailureIsolation"),
    ("sourcePath", Lean.toJson
      "packages/proofs/ASPProof/SearchRouteAdaptiveBatchFailureIsolation.lean"),
    ("declarationCount", Lean.toJson declarations.size),
    ("axiomFreeDeclarationCount", Lean.toJson 9),
    ("axiomDependentDeclarationCount", Lean.toJson 21),
    ("declarations", Lean.toJson declarations),
    ("axiomInventory", Lean.toJson ["Quot.sound", "propext"]),
    ("hasClassicalChoice", Lean.toJson false),
    ("hasNativeDecideAxiom", Lean.toJson false),
    ("hasSorryAx", Lean.toJson false),
    ("rfc", Lean.toJson "01.24-searchroute-adaptive-batch-failure-isolation"),
    ("status", Lean.toJson "kernel-compiled")
  ]

end ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation
