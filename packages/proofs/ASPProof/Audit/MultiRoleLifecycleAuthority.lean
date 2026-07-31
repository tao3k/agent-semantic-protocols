import ASPProof.Audit.Core
import ASPProof.MultiRoleLifecycleAuthority

namespace ASPProof.Audit.MultiRoleLifecycleAuthority

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.MultiRoleLifecycleAuthority.generic_role_claim_is_admissible
      theoremFamily := "generic-role-claim"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-GENERIC",
        "ASP-RFC-10.05-MRLA-KEY"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.per_role_quota_blocks_claim
      theoremFamily := "per-role-quota-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-ROLE-QUOTA"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.total_quota_blocks_claim
      theoremFamily := "total-quota-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TOTAL-QUOTA"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.legacy_claim_gate_ignores_exhausted_quota
      theoremFamily := "missing-quota-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-ROLE-QUOTA"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.distinct_roles_produce_distinct_instance_keys
      theoremFamily := "role-key-isolation"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-KEY"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.claim_commit_increments_generation_revision_and_counts
      theoremFamily := "claim-count-linearization"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-CLAIM"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.dispatch_is_admissible_before_termination
      theoremFamily := "pre-termination-dispatch"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TERMINATION"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.termination_is_admissible_at_same_snapshot
      theoremFamily := "termination-race-admission"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TERMINATION"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.termination_linearization_invalidates_old_dispatch
      theoremFamily := "termination-invalidates-dispatch"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TERMINATION"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.completed_termination_releases_quota
      theoremFamily := "termination-quota-release"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-RELEASE"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.replacement_cannot_reuse_old_message_target
      theoremFamily := "target-reuse-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TARGET"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.replacement_with_fresh_target_is_admissible
      theoremFamily := "fresh-target-replacement"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-TARGET"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.spawned_frontier_path_is_safe
      theoremFamily := "spawned-frontier-safety"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-FRONTIER"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.spawned_frontier_path_is_graph_shortest
      theoremFamily := "spawned-frontier-shortest-path"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-FRONTIER",
        "ASP-RFC-10.05-MRLA-COST"] }
  , { name := `ASPProof.MultiRoleLifecycleAuthority.spawned_frontier_reduces_rounds_and_tokens
      theoremFamily := "frontier-cost-reduction"
      rfcClauseIds := ["ASP-RFC-10.05-MRLA-COST"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.MultiRoleLifecycleAuthority"
    "ASPProof/MultiRoleLifecycleAuthority.lean"
    targets

end ASPProof.Audit.MultiRoleLifecycleAuthority
