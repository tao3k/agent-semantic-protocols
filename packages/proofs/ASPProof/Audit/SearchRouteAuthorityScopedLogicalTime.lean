-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteAuthorityScopedLogicalTime

namespace ASPProof.Audit.SearchRouteAuthorityScopedLogicalTime

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
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.before_or_at_requires_same_clock"
    "before-or-at-clock-binding"
    "non-strict logical order requires an equal authority and epoch domain"
    #["ASP-RFC-10.05-ASLT-DOMAIN", "ASP-RFC-10.05-ASLT-ORDER"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.strict_before_requires_same_clock"
    "strict-before-clock-binding"
    "strict logical order requires an equal authority and epoch domain"
    #["ASP-RFC-10.05-ASLT-ORDER"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.raw_order_does_not_establish_comparability"
    "raw-order-counterexample"
    "raw sequence order across authorities does not establish comparability"
    #["ASP-RFC-10.05-ASLT-RAW"] #["propext"],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.epoch_rollover_blocks_cross_epoch_order"
    "epoch-rollover-rejection"
    "raw sequence order across epochs does not establish strict logical order"
    #["ASP-RFC-10.05-ASLT-EPOCH"] #["propext"],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.valid_execution_is_authority_scoped"
    "execution-domain-coherence"
    "valid issuance execution and revocation share one clock domain"
    #["ASP-RFC-10.05-ASLT-EXECUTION"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.historical_audit_is_observation_scoped"
    "audit-observation-coherence"
    "historical audit observation shares the execution clock domain"
    #["ASP-RFC-10.05-ASLT-AUDIT"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.revocation_precedes_observation_rejects_new_execution"
    "post-revocation-execution-rejection"
    "an observation after revocation cannot be ordered before that revocation"
    #["ASP-RFC-10.05-ASLT-REVOCATION"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.coherent_lifecycle_is_valid"
    "coherent-lifecycle-construction"
    "same-domain ordered sequences construct execution validity"
    #["ASP-RFC-10.05-ASLT-CONSTRUCTION"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.authority_mismatch_rejects_execution"
    "authority-mismatch-rejection"
    "issuance and execution domain mismatch rejects execution validity"
    #["ASP-RFC-10.05-ASLT-AUTHORITY-MISMATCH"] #[],
  declaration
    "ASPProof.SearchRouteAuthorityScopedLogicalTime.observation_epoch_mismatch_rejects_audit"
    "observation-mismatch-rejection"
    "execution and observation domain mismatch rejects historical audit"
    #["ASP-RFC-10.05-ASLT-OBSERVATION-MISMATCH"] #[]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 2),
    ("axiomFreeDeclarationCount", .num 8),
    ("axiomInventory", stringArray #["propext"]),
    ("declarationCount", .num 10),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteAuthorityScopedLogicalTime"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteAuthorityScopedLogicalTime.lean")
  ]

end ASPProof.Audit.SearchRouteAuthorityScopedLogicalTime
