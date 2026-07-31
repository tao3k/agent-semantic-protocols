import ASPProof.Audit.Receipt
import ASPProof.ActivationAdmission

namespace ASPProof.Audit.ActivationAdmission

open ASPProof.Audit.Core
open ASPProof.ActivationAdmission

def targets : List Target :=
  [ Target.mk
      ``reuse_iff_complete_identity
      "activation-admission-completeness"
      [ "ASP-RFC-10.05-ALAE-ADMISSION" ]
  , Target.mk
      ``any_identity_drift_requires_rebuild
      "activation-identity-drift"
      [ "ASP-RFC-10.05-ALAE-DRIFT" ]
  , Target.mk
      ``failed_rebuild_cannot_serve_stale_activation
      "activation-no-stale-fallback"
      [ "ASP-RFC-10.05-ALAE-NO-STALE-FALLBACK" ]
  , Target.mk
      ``missing_activation_requires_rebuild
      "activation-missing-self-repair"
      [ "ASP-RFC-10.05-ALAE-DRIFT" ]
  , Target.mk
      ``reconciled_provider_closure_matches_activation
      "activation-provider-closure-exactness"
      [ "ASP-RFC-10.05-ALAE-PROVIDER-CLOSURE" ]
  , Target.mk
      ``retired_provider_cannot_survive_reconciliation
      "activation-retired-provider-elimination"
      [ "ASP-RFC-10.05-ALAE-PROVIDER-CLOSURE" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.ActivationAdmission"
    "ASPProof/ActivationAdmission.lean"
    targets

end ASPProof.Audit.ActivationAdmission
