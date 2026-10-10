-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinPolicySnapshotAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-policy-snapshot-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinPolicySnapshot.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinPolicySnapshotAudit
