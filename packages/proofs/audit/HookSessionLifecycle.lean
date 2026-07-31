import ASPProof.Audit.HookSessionLifecycle

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.HookSessionLifecycle.auditJson
  IO.FS.writeFile
    "receipts/hook-session-lifecycle-v1.json"
    json.pretty
