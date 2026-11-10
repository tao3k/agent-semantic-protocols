-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

open ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLease

elab "writeSearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLeaseAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-bounded-publication-recovery-lease-audit-v1.json"
    auditJson

writeSearchRouteAdmissionRetryCacheRejoinBoundedPublicationRecoveryLeaseAudit
