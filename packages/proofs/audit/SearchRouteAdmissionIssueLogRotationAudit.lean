-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Receipt
import ASPProof.Audit.SearchRouteAdmissionIssueLogRotation

open Lean Elab Command

elab "writeSearchRouteAdmissionIssueLogRotationAudit" : command => do
  ASPProof.Audit.writeReceipt
    "receipts/searchroute-admission-issue-log-rotation-audit-v1.json"
    ASPProof.Audit.SearchRouteAdmissionIssueLogRotation.auditJson

writeSearchRouteAdmissionIssueLogRotationAudit
