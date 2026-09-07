-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.SearchRouteCertifiedRegistryEpochTransition

namespace ASPProof.Audit.SearchRouteCertifiedRegistryEpochTransition

open Lean

private def stringArray (values : Array String) : Json :=
  .arr (values.map Json.str)

private def declaration
    (name theoremFamily description : String)
    (clauseIds : Array String) : Json :=
  .mkObj [
    ("axioms", stringArray #[]),
    ("hasSorryAx", .bool false),
    ("kind", .str "theorem"),
    ("name", .str name),
    ("rfcClauseIds", stringArray clauseIds),
    ("theoremFamily", .str theoremFamily),
    ("type", .str description)
  ]

def declarations : Array Json := #[
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.valid_transition_is_registry_authorized"
    "registry-successor-authorization"
    "valid transition uses the registry-owned authorized successor relation"
    #["ASP-RFC-10.05-CRET-CERTIFICATE"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.valid_transition_is_old_authority_signed"
    "old-authority-signing"
    "certificate authority equals the old domain authority"
    #["ASP-RFC-10.05-CRET-AUTHORITY"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.valid_transition_changes_snapshot"
    "snapshot-change"
    "valid transition changes the registry snapshot digest"
    #["ASP-RFC-10.05-CRET-SNAPSHOT"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.valid_transition_changes_domain"
    "domain-change"
    "valid transition changes the complete clock domain"
    #["ASP-RFC-10.05-CRET-SNAPSHOT"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.transition_does_not_create_sequence_comparability"
    "cross-domain-comparability-rejection"
    "successor causality does not create direct sequence comparability"
    #["ASP-RFC-10.05-CRET-NO-CROSS-ORDER"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.covered_old_execution_preserves_audit"
    "covered-old-audit-preservation"
    "covered old-domain execution preserves historical audit"
    #["ASP-RFC-10.05-CRET-AUDIT", "ASP-RFC-10.05-CRET-CUTOFF"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.late_old_execution_is_not_covered"
    "late-old-execution-rejection"
    "old-domain execution after the cutoff is not covered"
    #["ASP-RFC-10.05-CRET-CUTOFF"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.missing_archive_blocks_replay"
    "missing-archive-replay-rejection"
    "missing old snapshot archive blocks historical replay"
    #["ASP-RFC-10.05-CRET-REPLAY"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.archived_snapshot_enables_replay"
    "archived-snapshot-replay"
    "preserved audit plus archived snapshot enables replay"
    #["ASP-RFC-10.05-CRET-REPLAY"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.old_domain_rejected_for_new_decision"
    "old-domain-new-decision-rejection"
    "old-domain receipt cannot authorize a new decision"
    #["ASP-RFC-10.05-CRET-NEW-DECISION"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.successor_domain_authorizes_new_decision"
    "successor-domain-new-decision"
    "valid transition and successor-domain receipt authorize a new decision"
    #["ASP-RFC-10.05-CRET-NEW-DECISION"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.invalid_transition_authorizes_no_new_decision"
    "invalid-transition-new-decision-rejection"
    "invalid transition authorizes no new decision"
    #["ASP-RFC-10.05-CRET-NEW-DECISION"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.functional_registry_rejects_successor_fork"
    "functional-successor-fork-rejection"
    "functional registry collapses two valid same-origin successors"
    #["ASP-RFC-10.05-CRET-UNIQUE-SUCCESSOR"],
  declaration
    "ASPProof.SearchRouteCertifiedRegistryEpochTransition.valid_transition_is_exactly_one_epoch"
    "single-epoch-transition"
    "one transition certificate advances exactly one logical epoch"
    #["ASP-RFC-10.05-CRET-NEXT-EPOCH"]
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 0),
    ("axiomFreeDeclarationCount", .num 14),
    ("axiomInventory", stringArray #[]),
    ("declarationCount", .num 14),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteCertifiedRegistryEpochTransition"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteCertifiedRegistryEpochTransition.lean")
  ]

end ASPProof.Audit.SearchRouteCertifiedRegistryEpochTransition
