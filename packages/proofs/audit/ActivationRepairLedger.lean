import ASPProof.Audit.ActivationRepairLedger

open Lean Elab Command

run_elab do
  let json ← ASPProof.Audit.ActivationRepairLedger.auditJson
  IO.FS.writeFile
    "receipts/activation-repair-ledger-v1.json"
    json.pretty
