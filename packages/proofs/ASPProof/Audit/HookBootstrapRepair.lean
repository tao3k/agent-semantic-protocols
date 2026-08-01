import ASPProof.Audit.Core
import ASPProof.HookBootstrapRepair

namespace ASPProof.Audit.HookBootstrapRepair

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.HookBootstrapRepair.publish_pair_is_coherent
      theoremFamily := "hook-bootstrap-coherent-publication"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.public_hook_surface_is_stable
      theoremFamily := "hook-bootstrap-stable-public-surface"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.drift_cannot_parse_full_config
      theoremFamily := "hook-bootstrap-parser-order"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.existing_repair_owner_is_preserved
      theoremFamily := "hook-bootstrap-single-writer"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.repair_replays_at_most_once
      theoremFamily := "hook-bootstrap-bounded-replay"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.enforcement_failure_is_denied
      theoremFamily := "hook-bootstrap-enforcement-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.observational_failure_is_allowed
      theoremFamily := "hook-bootstrap-lifecycle-nondeadlock"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.warm_path_has_no_second_process
      theoremFamily := "hook-bootstrap-one-process-warm-path"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  , { name := `ASPProof.HookBootstrapRepair.server_warm_generation_is_not_per_hook
      theoremFamily := "hook-bootstrap-server-generation-sharing"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-BOOTSTRAP-REPAIR"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookBootstrapRepair"
    "ASPProof/HookBootstrapRepair.lean"
    targets

end ASPProof.Audit.HookBootstrapRepair
