-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteCanonicalPayloadDigestReplay

open Lean Elab Command Term
open ASPProof.Audit.Core

def targets : List Target := [
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.canonical_encoding_is_injective
    theoremFamily := "canonical-shortcut-encoding-injectivity"
    rfcClauseIds := ["CPD-CANONICAL"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.full_field_replay_needs_no_digest_assumption
    theoremFamily := "full-field-replay-assumption-free-identity"
    rfcClauseIds := ["CPD-FULL-FIELD"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.schema_version_is_inside_canonical_preimage
    theoremFamily := "schema-version-inside-canonical-preimage"
    rfcClauseIds := ["CPD-SCHEMA"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.hash_algorithm_is_inside_canonical_preimage
    theoremFamily := "hash-algorithm-inside-canonical-preimage"
    rfcClauseIds := ["CPD-HASH"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.admitted_digest_equality_implies_payload_equality
    theoremFamily := "collision-free-admitted-digest-replay-identity"
    rfcClauseIds := ["CPD-DIGEST", "CPD-ASSUMPTION"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.constant_digest_collides
    theoremFamily := "constant-digest-collision-counterexample"
    rfcClauseIds := ["CPD-COUNTEREXAMPLE"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.collision_payloads_are_distinct
    theoremFamily := "collision-counterexample-payload-distinctness"
    rfcClauseIds := ["CPD-COUNTEREXAMPLE"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.digest_equality_alone_does_not_imply_payload_equality
    theoremFamily := "digest-equality-alone-insufficient"
    rfcClauseIds := ["CPD-COUNTEREXAMPLE", "CPD-ASSUMPTION"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.digest_compressed_replay_is_constant_shape
    theoremFamily := "digest-compressed-replay-constant-shape"
    rfcClauseIds := ["CPD-RECEIPT"]
  },
  {
    name := ``ASPProof.SearchRouteCanonicalPayloadDigestReplay.digest_compressed_projection_is_smaller
    theoremFamily := "digest-compressed-projection-smaller-than-full-field"
    rfcClauseIds := ["CPD-RECEIPT"]
  }
]

elab "#writeCanonicalPayloadDigestReplayAudit" : command => do
  let json ← liftTermElabM do
    proofAuditJson
      "ASPProof.SearchRouteCanonicalPayloadDigestReplay"
      "ASPProof/SearchRouteCanonicalPayloadDigestReplay.lean"
      targets
  liftIO <| IO.FS.writeFile
    "receipts/searchroute-canonical-payload-digest-replay-audit-v1.json"
    json.pretty

#writeCanonicalPayloadDigestReplayAudit
