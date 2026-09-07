-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinFaultBoundAuthority

elab "writeSearchRouteAdmissionRetryCacheRejoinFaultBoundAuthorityAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-fault-bound-authority-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinFaultBoundAuthorityAudit
