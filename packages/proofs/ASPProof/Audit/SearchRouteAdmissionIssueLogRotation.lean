-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionIssueLogRotation

namespace ASPProof.Audit.SearchRouteAdmissionIssueLogRotation

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.live_reference_resolution_preserved
      "live-reference preservation"
      ["ASP-RFC-10.05-AILR-LIVE-COVERAGE"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.accepted_after_rotation_was_authoritatively_issued_before
      "no-fabrication preservation"
      [ "ASP-RFC-10.05-AILR-NO-FABRICATION"
      , "ASP-RFC-10.05-AILR-ACCEPTANCE" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.revoked_token_cannot_be_reaccepted_after_rotation
      "revocation preservation"
      [ "ASP-RFC-10.05-AILR-REVOCATION"
      , "ASP-RFC-10.05-AILR-ACCEPTANCE" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.publication_only_compaction_can_drop_retry_reference
      "live-root counterexample"
      [ "ASP-RFC-10.05-AILR-LIVE-ROOTS"
      , "ASP-RFC-10.05-AILR-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.live_coverage_without_no_fabrication_admits_forged_issue_record
      "no-fabrication counterexample"
      [ "ASP-RFC-10.05-AILR-NO-FABRICATION"
      , "ASP-RFC-10.05-AILR-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionIssueLogRotation.rotation_that_forgets_revocation_reaccepts_token
      "revocation counterexample"
      [ "ASP-RFC-10.05-AILR-REVOCATION"
      , "ASP-RFC-10.05-AILR-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionIssueLogRotation"
    "ASPProof/SearchRouteAdmissionIssueLogRotation.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionIssueLogRotation
