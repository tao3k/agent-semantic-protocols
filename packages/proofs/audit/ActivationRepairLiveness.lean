import ASPProof.Audit.ActivationRepairLiveness

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.ActivationRepairLiveness.auditJson
  IO.FS.writeFile
    "receipts/activation-repair-liveness-v1.json"
    json.pretty
