-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.RuntimeServerSearchGeneration

namespace ASPProof.Audit.RuntimeServerSearchGeneration

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name :=
        `ASPProof.RuntimeServerSearchGeneration.cli_has_no_generation_write_authority
      theoremFamily := "cli-write-authority-exclusion"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-AUTHORITY"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.staged_generation_is_not_queryable
      theoremFamily := "staged-generation-invisibility"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-STAGING"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.stale_revision_publish_is_rejected
      theoremFamily := "stale-revision-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-CAS"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.offline_writer_cannot_publish
      theoremFamily := "offline-writer-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-READ-WRITE"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.non_monotone_publish_is_rejected
      theoremFamily := "non-monotone-publication-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-CAS"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.successful_publish_is_monotone
      theoremFamily := "monotone-publication"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-CAS"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.successful_publish_clears_staging
      theoremFamily := "publication-clears-staging"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-CAS",
        "ASP-RFC-10.05-SRSG-STAGING"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.query_lease_is_generation_consistent
      theoremFamily := "lease-generation-consistency"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-LEASE"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.one_ready_provider_preserves_searchloop
      theoremFamily := "provider-partition-availability"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-AVAILABILITY"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.writer_failure_preserves_active_query
      theoremFamily := "writer-failure-read-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-READ-WRITE",
        "ASP-RFC-10.05-SRSG-LEASE"] }
  , { name :=
        `ASPProof.RuntimeServerSearchGeneration.server_warm_cost_dominates_cli_rebuild
      theoremFamily := "warm-path-cost-dominance"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-COST"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.RuntimeServerSearchGeneration"
    "ASPProof/RuntimeServerSearchGeneration.lean"
    targets

end ASPProof.Audit.RuntimeServerSearchGeneration
