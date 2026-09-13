-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence

open Lean Elab Command

elab "#writeSearchRouteAdmissionRetryCacheRejoinJointFenceAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-retry-cache-rejoin-joint-fence-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinJointFence.auditJson

#writeSearchRouteAdmissionRetryCacheRejoinJointFenceAudit
