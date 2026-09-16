-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.RuntimeServerIncrementalIndex

namespace ASPProof.Audit.RuntimeServerIncrementalIndex

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name :=
        `ASPProof.RuntimeServerIncrementalIndex.stale_delta_is_rejected
      theoremFamily := "stale-delta-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-DELTA"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.offline_writer_rejects_delta
      theoremFamily := "offline-writer-delta-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-DELTA"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.distinct_provider_deltas_can_coalesce
      theoremFamily := "cross-provider-coalescing"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COALESCE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.different_workspace_deltas_cannot_coalesce
      theoremFamily := "cross-workspace-coalescing-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COALESCE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.different_base_generation_deltas_cannot_coalesce
      theoremFamily := "cross-generation-coalescing-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COALESCE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.different_workspace_snapshot_deltas_cannot_coalesce
      theoremFamily := "cross-snapshot-coalescing-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COALESCE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.same_provider_different_snapshot_cannot_coalesce
      theoremFamily := "same-provider-snapshot-isolation"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COALESCE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.incomplete_batch_cannot_publish
      theoremFamily := "required-partition-completeness"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-COMPLETE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.repair_does_not_change_active_generation
      theoremFamily := "repair-read-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-REPAIR"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.model_prefix_hit_does_not_validate_search_cache
      theoremFamily := "model-cache-nonimplication"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-CACHE"] }
  , { name :=
        `ASPProof.RuntimeServerIncrementalIndex.valid_search_cache_does_not_imply_model_prefix_hit
      theoremFamily := "search-cache-nonimplication"
      rfcClauseIds := ["ASP-RFC-10.05-SRII-CACHE"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.RuntimeServerIncrementalIndex"
    "ASPProof/RuntimeServerIncrementalIndex.lean"
    targets

end ASPProof.Audit.RuntimeServerIncrementalIndex
