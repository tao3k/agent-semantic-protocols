-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation

def main (args : List String) : IO Unit := do
  let payload :=
    ASPProof.Audit.SearchRouteAdaptiveBatchFailureIsolation.manifest.compress ++ "\n"
  match args with
  | [] => IO.print payload
  | ["--output", path] => IO.FS.writeFile path payload
  | _ => throw <| IO.userError "usage: SearchRouteAdaptiveBatchFailureIsolationAudit [--output PATH]"
