-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchEvidenceReflection

def main : IO Unit := do
  let checks := ASPProof.SearchEvidenceReflection.checks
  for (name, passed) in checks do
    IO.println s!"{name}: {if passed then "PASS" else "FAIL"}"
  IO.println s!"checks={checks.length} passed={checks.filter (·.2) |>.length}"
  if !(checks.all (·.2)) then throw (IO.userError "reflection counterexample regression")
