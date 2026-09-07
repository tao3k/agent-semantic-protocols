-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairness

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairnessAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-recovery-retry-fairness-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedRecoveryRetryFairnessAudit
