-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteRegistrySnapshotResolution
import Lean

namespace ASPProof.Audit.SearchRouteRegistrySnapshotResolution

open Lean

private def strings (xs : Array String) : Json := Json.arr (xs.map Json.str)
private def decl (name family type : String) (clause : String) : Json :=
  Json.mkObj
    [("name", .str name), ("kind", .str "theorem"), ("type", .str type),
     ("theoremFamily", .str family), ("rfcClauseIds", strings #[clause]),
     ("axioms", strings #["propext"]), ("hasSorryAx", .bool false)]

def declarations : Array Json :=
  #[ decl "ASPProof.SearchRouteRegistrySnapshotResolution.snapshot_digest_is_injective"
       "snapshot-digest-injectivity" "snapshot digest uniquely identifies registry snapshot"
       "ASP-RFC-10.05-RSR-SNAPSHOT"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.resolution_is_unique_within_snapshot"
       "snapshot-resolution-uniqueness" "policy resolution is functional within one snapshot"
       "ASP-RFC-10.05-RSR-UNIQUE"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.comparable_certificates_resolve_same_semantics"
       "snapshot-policy-comparability" "equal snapshot and policy identities resolve equal semantics"
       "ASP-RFC-10.05-RSR-COMPARABLE"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.policy_digest_alone_is_insufficient"
       "policy-only-counterexample" "same policy digest resolves different semantics across snapshots"
       "ASP-RFC-10.05-RSR-POLICY-INSUFFICIENT"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.snapshot_upgrade_rejects_stale_replay"
       "snapshot-upgrade-rejection" "registry upgrade rejects stale replay comparison"
       "ASP-RFC-10.05-RSR-UPGRADE"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.revoked_snapshot_resolves_no_policy"
       "revoked-resolution-rejection" "revoked snapshot resolves no policy"
       "ASP-RFC-10.05-RSR-REVOKED"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.revoked_policy_has_no_resolution_certificate"
       "revoked-certificate-impossibility" "revoked policy cannot produce a resolution certificate"
       "ASP-RFC-10.05-RSR-NO-CERTIFICATE"
   , decl "ASPProof.SearchRouteRegistrySnapshotResolution.certificate_is_replay_comparable_with_itself"
       "resolution-comparability-reflexivity" "resolution certificate is replay comparable with itself"
       "ASP-RFC-10.05-RSR-REFLEXIVE" ]

def receipt : Json := Json.mkObj
  [("schemaId", .str "asp.lean-proof-audit.v1"), ("schemaVersion", .str "1"),
   ("leanVersion", .str "4.32.2"), ("proofPackage", .str "ASPProof"),
   ("module", .str "ASPProof.SearchRouteRegistrySnapshotResolution"),
   ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteRegistrySnapshotResolution.lean"),
   ("declarations", .arr declarations), ("declarationCount", .num 8),
   ("axiomFreeDeclarationCount", .num 0), ("axiomDependentDeclarationCount", .num 8),
   ("axiomInventory", strings #["propext"]), ("hasSorryAx", .bool false)]

end ASPProof.Audit.SearchRouteRegistrySnapshotResolution
