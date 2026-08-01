import ASPProof.Audit.HookBootstrapRepair

open Lean Elab Command
open ASPProof.HookBootstrapRepair
open ASPProof.Audit.HookBootstrapRepair

#check publish_pair_is_coherent
#check public_hook_surface_is_stable
#check drift_cannot_parse_full_config
#check existing_repair_owner_is_preserved
#check repair_replays_at_most_once
#check enforcement_failure_is_denied
#check observational_failure_is_allowed
#check warm_path_has_no_second_process
#check server_warm_generation_is_not_per_hook

#print axioms publish_pair_is_coherent
#print axioms public_hook_surface_is_stable
#print axioms drift_cannot_parse_full_config
#print axioms existing_repair_owner_is_preserved
#print axioms repair_replays_at_most_once
#print axioms enforcement_failure_is_denied
#print axioms observational_failure_is_allowed
#print axioms warm_path_has_no_second_process
#print axioms server_warm_generation_is_not_per_hook

run_elab do
  let json ← ASPProof.Audit.HookBootstrapRepair.auditJson
  IO.FS.writeFile
    "receipts/hook-bootstrap-repair-audit-v1.json"
    json.pretty
