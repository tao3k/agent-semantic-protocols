-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.ActivationAdmission

open Lean Elab Command

elab "#writeActivationAdmissionAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/activation-admission-audit-v1.json"
    ASPProof.Audit.ActivationAdmission.auditJson

#writeActivationAdmissionAudit
