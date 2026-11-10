-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCost

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCostAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-candidate-admission-cost-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedCandidateAdmissionCostAudit
