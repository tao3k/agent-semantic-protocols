-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.RuntimeGenerationIncrementalAdmission

namespace ASPProof.Audit.RuntimeGenerationIncrementalAdmission

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name :=
        `ASPProof.RuntimeGenerationIncrementalAdmission.replacement_partition_is_disjoint
      theoremFamily := "replacement-owner-partition-disjointness"
      rfcClauseIds := ["ASP-RFC-10.05-WORKSPACE-DELTA-PARTITION"] }
  , { name :=
        `ASPProof.RuntimeGenerationIncrementalAdmission.affected_absent_owner_is_tombstoned
      theoremFamily := "affected-absent-owner-tombstone"
      rfcClauseIds := ["ASP-RFC-10.05-WORKSPACE-DELTA-PARTITION"] }
  , { name :=
        `ASPProof.RuntimeGenerationIncrementalAdmission.affected_present_owner_is_rebuilt
      theoremFamily := "affected-present-owner-upsert"
      rfcClauseIds := ["ASP-RFC-10.05-WORKSPACE-DELTA-PARTITION"] }
  , { name :=
        `ASPProof.RuntimeGenerationIncrementalAdmission.parser_auxiliary_blob_has_admitted_hash
      theoremFamily := "parser-auxiliary-blob-hash-coverage"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-BLOB-HASH-MEMBERSHIP"] }
  , { name :=
        `ASPProof.RuntimeGenerationIncrementalAdmission.non_scope_hash_requires_source_blob
      theoremFamily := "non-scope-hash-blob-necessity"
      rfcClauseIds := ["ASP-RFC-10.05-SRSG-BLOB-HASH-MEMBERSHIP"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.RuntimeGenerationIncrementalAdmission"
    "ASPProof/RuntimeGenerationIncrementalAdmission.lean"
    targets

end ASPProof.Audit.RuntimeGenerationIncrementalAdmission
