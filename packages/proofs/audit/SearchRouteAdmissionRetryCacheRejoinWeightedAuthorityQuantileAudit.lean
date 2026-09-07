-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantile

elab "writeSearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantileAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-weighted-authority-quantile-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinWeightedAuthorityQuantileAudit
