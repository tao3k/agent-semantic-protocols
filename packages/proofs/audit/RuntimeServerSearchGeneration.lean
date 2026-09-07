-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.RuntimeServerSearchGeneration

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.RuntimeServerSearchGeneration.auditJson
  IO.FS.writeFile
    "receipts/runtime-server-search-generation-v1.json"
    json.pretty
