-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity

def targets : List Target :=
  [ Target.mk
      ``vote_from_other_configuration_cannot_validate_certificate
      "cross-configuration-vote-counterexample"
      [ "ASP-RFC-10.05-CRCI-VOTE"
      , "ASP-RFC-10.05-CRCI-REPLAY"
      ]
  , Target.mk
      ``valid_same_configuration_certificates_bind_same_receipt
      "configuration-scoped-slot-uniqueness"
      [ "ASP-RFC-10.05-CRCI-IDENTITY"
      , "ASP-RFC-10.05-CRCI-UNIQUENESS"
      ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinConfigurationIdentity
