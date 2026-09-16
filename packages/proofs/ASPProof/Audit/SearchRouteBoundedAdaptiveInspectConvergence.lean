-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteBoundedAdaptiveInspectConvergence

namespace ASPProof.Audit.SearchRouteBoundedAdaptiveInspectConvergence

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
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_step_preserves_request"
    "inspect-step-request-identity"
    "inspect step preserves the disclosure request identity"
    #["ASP-RFC-10.05-BAIC-STATE", "ASP-RFC-10.05-BAIC-STEP"] #[],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_step_strictly_decreases_missing_potential"
    "strict-missing-potential-decrease"
    "every inspect step strictly decreases missing-edge potential"
    #["ASP-RFC-10.05-BAIC-PROGRESS"] #["propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_step_conserves_token_plus_missing_units"
    "step-token-potential-conservation"
    "one step conserves disclosed token units plus remaining edges"
    #["ASP-RFC-10.05-BAIC-TOKEN"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_step_preserves_partition"
    "step-partition-preservation"
    "one step preserves disclosed-plus-remaining partition"
    #["ASP-RFC-10.05-BAIC-PARTITION"] #["propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_step_is_not_reflexive"
    "no-op-inspect-rejection"
    "no inspect state can validly step to itself"
    #["ASP-RFC-10.05-BAIC-PROGRESS", "ASP-RFC-10.05-BAIC-ROUTER"] #["propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.batched_step_reduces_potential_by_at_least_two"
    "batch-potential-reduction"
    "batch of at least two edges reduces missing potential by at least two"
    #["ASP-RFC-10.05-BAIC-BATCH"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_run_rounds_are_bounded_by_initial_missing_edges"
    "run-round-upper-bound"
    "run step count is bounded by initial missing-edge count"
    #["ASP-RFC-10.05-BAIC-BOUND"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_run_conserves_token_plus_missing_units"
    "run-token-potential-conservation"
    "complete run conserves disclosed token units plus remaining edges"
    #["ASP-RFC-10.05-BAIC-TOKEN"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_run_preserves_partition"
    "run-partition-preservation"
    "complete run preserves the required-edge partition"
    #["ASP-RFC-10.05-BAIC-PARTITION"] #["propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.inspect_run_accounts_for_every_llm_round"
    "exact-llm-round-accounting"
    "final LLM rounds equal initial rounds plus run steps"
    #["ASP-RFC-10.05-BAIC-ROUND"] #["Quot.sound", "propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.complete_partition_discloses_exact_required_edges"
    "complete-partition-exact-disclosure"
    "complete partition exposes exactly the requested edges"
    #["ASP-RFC-10.05-BAIC-READY"] #["propext"],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.decision_ready_requires_verified_chain"
    "readiness-requires-chain-proof"
    "decision readiness requires a verified trust chain"
    #["ASP-RFC-10.05-BAIC-READY"] #[],
  declaration
    "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence.decision_ready_discloses_exact_required_edges"
    "readiness-exact-disclosure"
    "ready state discloses exactly all required edges"
    #["ASP-RFC-10.05-BAIC-READY", "ASP-RFC-10.05-BAIC-ROUTER"] #["propext"]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 11),
    ("axiomFreeDeclarationCount", .num 2),
    ("axiomInventory", stringArray #["Quot.sound", "propext"]),
    ("declarationCount", .num 13),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteBoundedAdaptiveInspectConvergence"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteBoundedAdaptiveInspectConvergence.lean")
  ]

end ASPProof.Audit.SearchRouteBoundedAdaptiveInspectConvergence
