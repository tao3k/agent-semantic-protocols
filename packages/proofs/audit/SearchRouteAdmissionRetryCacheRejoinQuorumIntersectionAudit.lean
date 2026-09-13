-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinQuorumIntersection

elab "writeSearchRouteAdmissionRetryCacheRejoinQuorumIntersectionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-quorum-intersection-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinQuorumIntersectionAudit
