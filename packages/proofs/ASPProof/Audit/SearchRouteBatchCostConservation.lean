-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteBatchCostConservation
import Lean

namespace ASPProof.Audit.SearchRouteBatchCostConservation

open Lean

private def strings (values : Array String) : Json :=
  Json.arr (values.map Json.str)

private def declaration
    (name theoremFamily declarationType : String)
    (rfcClauseIds axioms : Array String) : Json :=
  Json.mkObj
    [ ("name", Json.str name)
    , ("kind", Json.str "theorem")
    , ("type", Json.str declarationType)
    , ("theoremFamily", Json.str theoremFamily)
    , ("rfcClauseIds", strings rfcClauseIds)
    , ("axioms", strings axioms)
    , ("hasSorryAx", Json.bool false)
    ]

def declarations : Array Json :=
  #[ declaration
       "ASPProof.SearchRouteBatchCostConservation.batch_measure_decreases_by_discovered_count"
       "batch-measure-credit"
       "a K-route discovery decreases weighted closure measure by at least K"
       #["ASP-RFC-10.05-BCC-BATCH-DECREASE"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.certified_batch_transition_decreases"
       "strict-batch-decrease"
       "every positive certified route batch strictly decreases closure measure"
       #["ASP-RFC-10.05-BCC-STRICT"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.certified_batch_run_round_bound"
       "batch-round-bound"
       "certified batched interaction rounds are bounded by initial closure measure"
       #["ASP-RFC-10.05-BCC-ROUND-BOUND"]
       #["propext"]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.valid_batch_conserves_tool_work"
       "tool-work-conservation"
       "valid batch accounting preserves the semantic tool-work lower bound"
       #["ASP-RFC-10.05-BCC-TOOLS"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.valid_batch_conserves_branch_tokens"
       "token-work-conservation"
       "valid accounting includes the per-route branch-token lower bound in total tokens"
       #["ASP-RFC-10.05-BCC-TOKENS"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.valid_nonempty_batch_consumes_round"
       "nonempty-round-consumption"
       "every non-empty valid batch consumes at least one interaction round"
       #["ASP-RFC-10.05-BCC-NONEMPTY"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.parallel_batch_accounting_is_valid"
       "parallel-accounting-witness"
       "the concrete four-route parallel batch satisfies accounting invariants"
       #["ASP-RFC-10.05-BCC-VALID"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.same_round_count_does_not_determine_work"
       "round-work-separation"
       "equal LLM round counts do not determine tool work or total tokens"
       #["ASP-RFC-10.05-BCC-ROUND-NOT-WORK"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.round_only_cost_underreports_parallel_batch"
       "round-only-underreport"
       "round-only accounting underreports tool and token work in a parallel batch"
       #["ASP-RFC-10.05-BCC-UNDERREPORT"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.zero_round_nonempty_batch_is_invalid"
       "zero-round-rejection"
       "a non-empty batch with zero interaction rounds violates valid accounting"
       #["ASP-RFC-10.05-BCC-ZERO-ROUND"]
       #[]
   , declaration
       "ASPProof.SearchRouteBatchCostConservation.four_route_batch_decreases_measure_by_four"
       "four-route-batch-witness"
       "the concrete four-route batch decreases weighted measure by at least four"
       #["ASP-RFC-10.05-BCC-FOUR"]
       #["propext"]
   ]

def receipt : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.SearchRouteBatchCostConservation")
    , ("sourcePath",
        Json.str "packages/proofs/ASPProof/SearchRouteBatchCostConservation.lean")
    , ("declarations", Json.arr declarations)
    , ("declarationCount", Json.num 11)
    , ("axiomFreeDeclarationCount", Json.num 7)
    , ("axiomDependentDeclarationCount", Json.num 4)
    , ("axiomInventory", strings #["propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.Audit.SearchRouteBatchCostConservation
