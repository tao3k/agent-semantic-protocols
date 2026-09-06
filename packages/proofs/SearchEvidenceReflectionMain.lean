import ASPProof.SearchEvidenceReflection

def main : IO Unit := do
  let checks := ASPProof.SearchEvidenceReflection.checks
  for (name, passed) in checks do
    IO.println s!"{name}: {if passed then "PASS" else "FAIL"}"
  IO.println s!"checks={checks.length} passed={checks.filter (·.2) |>.length}"
  if !(checks.all (·.2)) then throw (IO.userError "reflection counterexample regression")
