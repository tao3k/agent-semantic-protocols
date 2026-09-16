-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedian

elab "writeSearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedianAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-resilient-authority-median-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinResilientAuthorityMedianAudit
