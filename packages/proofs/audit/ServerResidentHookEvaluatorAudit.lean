import ASPProof.Audit.ServerResidentHookEvaluator

open Lean Elab Command
open ASPProof.ServerResidentHookEvaluator
open ASPProof.Audit.ServerResidentHookEvaluator

#check workspace_identity_prevents_alias
#check project_identity_prevents_alias
#check activation_generation_prevents_alias
#check same_key_compiles_once
#check stale_generation_is_not_admissible
#check leased_generation_is_not_evictable
#check warm_path_is_one_round_trip
#check server_failure_selects_local_fallback
#check identity_mismatch_selects_local_fallback

#print axioms workspace_identity_prevents_alias
#print axioms project_identity_prevents_alias
#print axioms activation_generation_prevents_alias
#print axioms same_key_compiles_once
#print axioms stale_generation_is_not_admissible
#print axioms leased_generation_is_not_evictable
#print axioms warm_path_is_one_round_trip
#print axioms server_failure_selects_local_fallback
#print axioms identity_mismatch_selects_local_fallback

run_elab do
  let json ← ASPProof.Audit.ServerResidentHookEvaluator.auditJson
  IO.FS.writeFile
    "receipts/server-resident-hook-evaluator-audit-v1.json"
    json.pretty
