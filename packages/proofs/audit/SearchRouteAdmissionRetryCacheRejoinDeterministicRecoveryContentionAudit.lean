-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContention

elab "writeSearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContentionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-deterministic-recovery-contention-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinDeterministicRecoveryContentionAudit
