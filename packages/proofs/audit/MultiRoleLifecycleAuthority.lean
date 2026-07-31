import ASPProof.Audit.MultiRoleLifecycleAuthority

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.MultiRoleLifecycleAuthority.auditJson
  IO.FS.writeFile
    "receipts/multi-role-lifecycle-authority-v1.json"
    json.pretty
