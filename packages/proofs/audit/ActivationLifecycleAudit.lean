-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.ActivationLifecycleAudit

open Lean Elab Command

elab "#writeActivationLifecycleAudit" : command =>
  ASPProof.Audit.writeReceipt
    "receipts/activation-lifecycle-audit-v1.json"
    ASPProof.Audit.ActivationLifecycleAudit.auditJson

#writeActivationLifecycleAudit
