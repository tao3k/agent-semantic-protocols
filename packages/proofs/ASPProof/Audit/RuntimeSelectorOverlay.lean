-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.RuntimeSelectorOverlay

namespace ASPProof.Audit.RuntimeSelectorOverlay

open ASPProof.Audit.Core
open ASPProof.RuntimeSelectorOverlay

def targets : List Target :=
  [ Target.mk
      ``publish_iff_complete_admission
      "runtime-selector-overlay-admission"
      [ "ASP-RFC-10.05-ESGA-OVERLAY-ADMISSION" ]
  , Target.mk
      ``invalid_overlay_cannot_serve
      "runtime-selector-overlay-fail-closed"
      [ "ASP-RFC-10.05-ESGA-NO-SERVE-BEFORE-PUBLISH" ]
  , Target.mk
      ``stale_owner_cannot_serve
      "runtime-selector-overlay-owner-freshness"
      [ "ASP-RFC-10.05-ESGA-OWNER-FRESHNESS" ]
  , Target.mk
      ``selector_only_publication_preserves_source_epoch
      "runtime-selector-overlay-source-epoch-preservation"
      [ "ASP-RFC-10.05-ESGA-SELECTOR-ONLY-EPOCH-PRESERVATION" ]
  , Target.mk
      ``owner_publication_invalidates_selector_overlays
      "runtime-selector-overlay-owner-invalidation"
      [ "ASP-RFC-10.05-ESGA-OWNER-INVALIDATES-SELECTORS" ]
  , Target.mk
      ``prior_lease_remains_snapshot_isolated
      "runtime-selector-overlay-prior-lease-isolation"
      [ "ASP-RFC-10.05-ESGA-LEASE-SNAPSHOT-ISOLATION" ]
  , Target.mk
      ``current_generation_advances_after_owner_publication
      "runtime-selector-overlay-owner-epoch-advance"
      [ "ASP-RFC-10.05-ESGA-OWNER-EPOCH-ADVANCE" ]
  , Target.mk
      ``resident_ready_admission_preserves_generation_and_overlays
      "runtime-resident-ready-memory-first"
      [ "ASP-RFC-10.05-ESGA-RESIDENT-READY-MEMORY-FIRST" ]
  , Target.mk
      ``pre_tool_requests_generation_admission
      "runtime-generation-lifecycle-admission"
      [ "ASP-RFC-10.05-ESGA-LIFECYCLE-ADMISSION-OWNER" ]
  , Target.mk
      ``exact_query_cannot_request_generation_admission
      "runtime-exact-query-no-admission"
      [ "ASP-RFC-10.05-ESGA-LIFECYCLE-ADMISSION-OWNER" ]
  , Target.mk
      ``different_project_roots_have_distinct_admission_scopes
      "runtime-admission-project-root-isolation"
      [ "ASP-RFC-10.05-ESGA-PROJECT-ROOT-ISOLATION" ]
  , Target.mk
      ``committed_materialization_can_publish_without_cross_connection_readback
      "runtime-commit-publication-binding"
      [ "ASP-RFC-10.05-ESGA-COMMIT-PUBLICATION-BINDING" ]
  , Target.mk
      ``identity_drift_cannot_publish_committed_materialization
      "runtime-commit-publication-drift-rejection"
      [ "ASP-RFC-10.05-ESGA-COMMIT-PUBLICATION-BINDING" ]
  , Target.mk
      ``projection_kind_separates_overlay_keys
      "runtime-selector-overlay-projection-binding"
      [ "ASP-RFC-10.05-ESGA-PROJECTION-BINDING" ]
  , Target.mk
      ``serve_requires_successful_publication
      "runtime-selector-overlay-publication-before-serve"
      [ "ASP-RFC-10.05-ESGA-NO-SERVE-BEFORE-PUBLISH" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.RuntimeSelectorOverlay"
    "ASPProof/RuntimeSelectorOverlay.lean"
    targets

end ASPProof.Audit.RuntimeSelectorOverlay
