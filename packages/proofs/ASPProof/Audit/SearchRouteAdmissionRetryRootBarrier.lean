-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryRootBarrier

namespace ASPProof.Audit.SearchRouteAdmissionRetryRootBarrier

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryRootBarrier.concurrent_retry_barrier_enters_complete_live_roots
      "complete-root bridge"
      [ "ASP-RFC-10.05-CRRB-ROOT-UNION"
      , "ASP-RFC-10.05-CRRB-BARRIER-CAPTURE" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryRootBarrier.certified_concurrent_retry_survives_safe_issue_log_rotation
      "safe-rotation bridge"
      [ "ASP-RFC-10.05-CRRB-ISSUE-BEFORE-CUT"
      , "ASP-RFC-10.05-CRRB-ROTATION-BRIDGE" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryRootBarrier.remembered_retry_is_live_but_snapshot_only_roots_drop_it
      "snapshot-only counterexample"
      [ "ASP-RFC-10.05-CRRB-ROOT-UNION"
      , "ASP-RFC-10.05-CRRB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryRootBarrier.rotation_over_snapshot_only_roots_is_unsafe_for_remembered_retry
      "incomplete-rotation counterexample"
      [ "ASP-RFC-10.05-CRRB-ROTATION-BRIDGE"
      , "ASP-RFC-10.05-CRRB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryRootBarrier.root_membership_without_pre_rotation_issue_does_not_create_origin
      "origin counterexample"
      [ "ASP-RFC-10.05-CRRB-ISSUE-BEFORE-CUT"
      , "ASP-RFC-10.05-CRRB-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryRootBarrier"
    "ASPProof/SearchRouteAdmissionRetryRootBarrier.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryRootBarrier
