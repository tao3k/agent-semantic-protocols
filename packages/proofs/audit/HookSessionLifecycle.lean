-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.HookSessionLifecycle

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.HookSessionLifecycle.auditJson
  IO.FS.writeFile
    "receipts/hook-session-lifecycle-v1.json"
    json.pretty
