-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryPublication

namespace ASPProof.Audit.SearchRouteAdmissionRetryPublication

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.visible_retry_has_durable_valid_certificate
      "publication validity"
      [ "ASP-RFC-10.05-RPCS-CERTIFICATE"
      , "ASP-RFC-10.05-RPCS-PUBLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.durable_acknowledgment_requires_visible_publication
      "acknowledgment ordering"
      ["ASP-RFC-10.05-RPCS-ACKNOWLEDGMENT"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.crash_before_certificate_is_unpublished_and_unrecoverable
      "pre-certificate crash"
      ["ASP-RFC-10.05-RPCS-RECOVERY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.durable_certificate_before_publication_is_recoverable_but_invisible
      "certificate-only crash"
      [ "ASP-RFC-10.05-RPCS-CERTIFICATE"
      , "ASP-RFC-10.05-RPCS-RECOVERY"
      , "ASP-RFC-10.05-RPCS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.lost_acknowledgment_does_not_revoke_durable_publication
      "lost-acknowledgment crash"
      [ "ASP-RFC-10.05-RPCS-ACKNOWLEDGMENT"
      , "ASP-RFC-10.05-RPCS-RECOVERY"
      , "ASP-RFC-10.05-RPCS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.publication_without_certificate_is_not_well_formed
      "publication-order counterexample"
      [ "ASP-RFC-10.05-RPCS-PUBLICATION"
      , "ASP-RFC-10.05-RPCS-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryPublication.acknowledgment_without_publication_is_not_well_formed
      "acknowledgment-order counterexample"
      [ "ASP-RFC-10.05-RPCS-ACKNOWLEDGMENT"
      , "ASP-RFC-10.05-RPCS-NONIMPLICATION" ] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryPublication"
    "ASPProof/SearchRouteAdmissionRetryPublication.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryPublication
