import ASPProof.Audit.Core
import ASPProof.ServerResidentHookEvaluator

namespace ASPProof.Audit.ServerResidentHookEvaluator

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.ServerResidentHookEvaluator.workspace_identity_prevents_alias
      theoremFamily := "server-hook-workspace-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.project_identity_prevents_alias
      theoremFamily := "server-hook-project-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.activation_generation_prevents_alias
      theoremFamily := "server-hook-activation-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.same_key_compiles_once
      theoremFamily := "server-hook-generation-reuse"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.stale_generation_is_not_admissible
      theoremFamily := "server-hook-stale-response-rejection"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.leased_generation_is_not_evictable
      theoremFamily := "server-hook-lease-safety"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.warm_path_is_one_round_trip
      theoremFamily := "server-hook-one-round-trip"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.server_failure_selects_local_fallback
      theoremFamily := "server-hook-local-fallback"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  , { name := `ASPProof.ServerResidentHookEvaluator.identity_mismatch_selects_local_fallback
      theoremFamily := "server-hook-identity-mismatch-fallback"
      rfcClauseIds := ["ASP-RFC-10.15-SERVER-HOOK-EVALUATOR"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.ServerResidentHookEvaluator"
    "ASPProof/ServerResidentHookEvaluator.lean"
    targets

end ASPProof.Audit.ServerResidentHookEvaluator
