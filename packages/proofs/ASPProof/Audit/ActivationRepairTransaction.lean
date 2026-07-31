import ASPProof.Audit.Core
import ASPProof.ActivationRepairTransaction

namespace ASPProof.Audit.ActivationRepairTransaction

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.ActivationRepairTransaction.base_repair_is_committable
      theoremFamily := "repair-admission"
      rfcClauseIds := ["ASP-RFC-10.05-ART-INDEPENDENCE",
        "ASP-RFC-10.05-ART-IDENTITY", "ASP-RFC-10.05-ART-LEASE"] }
  , { name := `ASPProof.ActivationRepairTransaction.expired_capability_cannot_commit
      theoremFamily := "expired-capability-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-LEASE"] }
  , { name := `ASPProof.ActivationRepairTransaction.revoked_capability_cannot_commit
      theoremFamily := "revoked-capability-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-LEASE"] }
  , { name := `ASPProof.ActivationRepairTransaction.cross_identity_capability_cannot_commit
      theoremFamily := "cross-identity-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-IDENTITY"] }
  , { name := `ASPProof.ActivationRepairTransaction.stale_proposal_cannot_commit
      theoremFamily := "stale-revision-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-CAS"] }
  , { name := `ASPProof.ActivationRepairTransaction.generation_regression_cannot_commit
      theoremFamily := "generation-regression-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-GENERATION"] }
  , { name := `ASPProof.ActivationRepairTransaction.repair_without_rollback_cannot_commit
      theoremFamily := "missing-rollback-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-ROLLBACK"] }
  , { name := `ASPProof.ActivationRepairTransaction.ambiguous_dispatch_requires_quarantine
      theoremFamily := "dispatch-quarantine-counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-ART-DISPATCH"] }
  , { name := `ASPProof.ActivationRepairTransaction.committed_repair_advances_generation
      theoremFamily := "repair-generation-advance"
      rfcClauseIds := ["ASP-RFC-10.05-ART-GENERATION"] }
  , { name := `ASPProof.ActivationRepairTransaction.committed_repair_increments_authority_revision
      theoremFamily := "authority-revision-linearization"
      rfcClauseIds := ["ASP-RFC-10.05-ART-CAS"] }
  , { name := `ASPProof.ActivationRepairTransaction.committed_repair_preserves_canonical_identity
      theoremFamily := "canonical-identity-preservation"
      rfcClauseIds := ["ASP-RFC-10.05-ART-IDENTITY"] }
  , { name := `ASPProof.ActivationRepairTransaction.committed_repair_requires_fresh_readiness
      theoremFamily := "readiness-invalidation"
      rfcClauseIds := ["ASP-RFC-10.05-ART-PUBLICATION"] }
  , { name := `ASPProof.ActivationRepairTransaction.committed_head_rejects_same_proposal
      theoremFamily := "linearized-one-winner"
      rfcClauseIds := ["ASP-RFC-10.05-ART-CAS"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.ActivationRepairTransaction"
    "ASPProof/ActivationRepairTransaction.lean"
    targets

end ASPProof.Audit.ActivationRepairTransaction
