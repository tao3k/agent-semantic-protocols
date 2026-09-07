-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregation

elab "writeSearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregationAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-multi-authority-fault-aggregation-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinMultiAuthorityFaultAggregationAudit
