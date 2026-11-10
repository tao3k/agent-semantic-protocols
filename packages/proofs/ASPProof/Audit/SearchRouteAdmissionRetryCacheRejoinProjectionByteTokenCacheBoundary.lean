-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

namespace ASPProof.Audit

def writeReceipt
    (path : System.FilePath)
    (auditJson : Lean.Elab.TermElabM Lean.Json) :
    Lean.Elab.Command.CommandElabM Unit := do
  let audit ← Lean.Elab.Command.liftTermElabM auditJson
  IO.FS.writeFile path (audit.pretty ++ "\n")
  Lean.logInfo m!"wrote {path}"

end ASPProof.Audit

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

def targets : List Target := [
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.zero_candidate_projection_has_only_fixed_bytes
    "zero-candidate-fixed-projection"
    ["PBTC-BYTES", "PBTC-EMPTY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.projection_bytes_are_bounded_by_candidate_capacity
    "encoded-projection-byte-bound"
    ["PBTC-BYTES", "PBTC-CAPACITY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.projected_tokens_are_bounded_by_encoded_bytes
    "tokenizer-contract-bound"
    ["PBTC-TOKENS", "PBTC-ENCODED"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.recovery_projection_bytes_are_attempt_and_capacity_bounded
    "retry-projection-byte-bound"
    ["PBTC-RETRY", "PBTC-CAPACITY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.escaped_json_size_exceeds_raw_size
    "json-escape-expansion"
    ["PBTC-JSON", "PBTC-COUNTEREXAMPLE"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.raw_cap_without_encoder_contract_does_not_bound_encoded_bytes
    "raw-cap-insufficient"
    ["PBTC-JSON", "PBTC-ENCODER-CONTRACT"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.unchanged_projection_identity_is_compatible
    "projection-identity-reflexive"
    ["PBTC-PROJECTION", "PBTC-IDENTITY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.renderer_change_invalidates_projection_identity
    "renderer-version-invalidates-projection"
    ["PBTC-PROJECTION", "PBTC-RENDERER"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.unchanged_search_result_cache_key_is_compatible
    "search-cache-key-reflexive"
    ["PBTC-SEARCH-CACHE", "PBTC-IDENTITY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.same_rendered_prefix_does_not_validate_changed_search_snapshot
    "render-prefix-not-search-validity"
    ["PBTC-SEARCH-CACHE", "PBTC-SNAPSHOT"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.unchanged_model_prefix_cache_key_is_compatible
    "model-prefix-key-reflexive"
    ["PBTC-MODEL-CACHE", "PBTC-IDENTITY"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.same_rendered_prefix_does_not_validate_changed_model
    "render-prefix-not-model-validity"
    ["PBTC-MODEL-CACHE", "PBTC-MODEL"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.search_result_cache_validity_does_not_imply_model_prefix_cache_validity
    "search-cache-not-model-cache"
    ["PBTC-SEARCH-CACHE", "PBTC-MODEL-CACHE"],
  Target.mk
    ``ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.model_prefix_cache_validity_does_not_imply_search_result_cache_validity
    "model-cache-not-search-cache"
    ["PBTC-MODEL-CACHE", "PBTC-SEARCH-CACHE"]
]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundary

elab "writeSearchRouteAdmissionRetryCacheRejoinProjectionByteTokenCacheBoundaryAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-projection-byte-token-cache-boundary-audit-v1.json"
    auditJson
