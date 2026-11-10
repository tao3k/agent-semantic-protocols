-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.PolyglotSearchSemanticRefinement

namespace ASPProof.Audit

def writePolyglotSearchSemanticRefinementReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.PolyglotSearchSemanticRefinement

open ASPProof.Audit.Core
open ASPProof.PolyglotSearchSemanticRefinement

def targets : List Target := [
  Target.mk ``one_hop_evaluation_matches_reference_relation "gql-one-hop-semantics"
    ["ASP-RFC-PSSR-GQL", "ASP-RFC-PSSR-SOUNDNESS"],
  Target.mk ``one_hop_evaluation_preserves_parallel_edge_witnesses "gql-bag-multiplicity"
    ["ASP-RFC-PSSR-GQL", "ASP-RFC-PSSR-MULTIPLICITY"],
  Target.mk ``typed_equality_has_no_case_coercion "typed-equality-no-coercion"
    ["ASP-RFC-PSSR-WHERE", "ASP-RFC-PSSR-TYPES"],
  Target.mk ``alias_shadowing_is_rejected_before_evaluation "alias-scope"
    ["ASP-RFC-PSSR-RETURN", "ASP-RFC-PSSR-SCOPE"],
  Target.mk ``noncanonical_graph_order_is_rejected "canonical-graph-order"
    ["ASP-RFC-PSSR-ORDER", "ASP-RFC-PSSR-DETERMINISM"],
  Target.mk ``conjunctive_join_preserves_shared_candidate_binding "logic-join-binding"
    ["ASP-RFC-PSSR-LOGIC", "ASP-RFC-PSSR-BINDING"],
  Target.mk ``conjunctive_join_preserves_bag_multiplicity "logic-bag-multiplicity"
    ["ASP-RFC-PSSR-LOGIC", "ASP-RFC-PSSR-MULTIPLICITY"],
  Target.mk ``exact_source_ast_binding_is_admitted "source-ast-binding-positive"
    ["ASP-RFC-PSSR-AST", "ASP-RFC-PSSR-DIGEST"],
  Target.mk ``tampered_ast_digest_is_rejected "ast-digest-tamper"
    ["ASP-RFC-PSSR-AST", "ASP-RFC-PSSR-DIGEST"],
  Target.mk ``valid_semantic_trace_is_admitted "semantic-trace-positive"
    ["ASP-RFC-PSSR-TRACE", "ASP-RFC-PSSR-REFINEMENT"],
  Target.mk ``semantic_evaluation_is_read_only "semantic-trace-read-only"
    ["ASP-RFC-PSSR-TRACE", "ASP-RFC-PSSR-READ-ONLY"],
  Target.mk ``tampered_trace_digest_is_rejected "semantic-trace-tamper"
    ["ASP-RFC-PSSR-TRACE", "ASP-RFC-PSSR-DIGEST"],
  Target.mk ``evaluator_is_deterministic "evaluator-determinism"
    ["ASP-RFC-PSSR-EVALUATOR", "ASP-RFC-PSSR-DETERMINISM"],
  Target.mk ``evaluator_cannot_mutate_graph "evaluator-nonmutation"
    ["ASP-RFC-PSSR-EVALUATOR", "ASP-RFC-PSSR-READ-ONLY"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.PolyglotSearchSemanticRefinement"
    "ASPProof/PolyglotSearchSemanticRefinement.lean"
    targets

end ASPProof.Audit.PolyglotSearchSemanticRefinement

open ASPProof.Audit.PolyglotSearchSemanticRefinement

elab "writePolyglotSearchSemanticRefinementAudit" : command =>
  ASPProof.Audit.writePolyglotSearchSemanticRefinementReceipt
    "receipts/polyglot-search-semantic-refinement-audit-v1.json"
    auditJson
