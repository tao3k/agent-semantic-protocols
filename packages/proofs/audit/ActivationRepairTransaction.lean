import ASPProof.Audit.ActivationRepairTransaction

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.ActivationRepairTransaction.auditJson
  IO.FS.writeFile
    "receipts/activation-repair-transaction-v1.json"
    json.pretty
