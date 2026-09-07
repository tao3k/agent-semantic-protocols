-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatistic

elab "writeSearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatisticAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-general-authority-order-statistic-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinGeneralAuthorityOrderStatisticAudit
