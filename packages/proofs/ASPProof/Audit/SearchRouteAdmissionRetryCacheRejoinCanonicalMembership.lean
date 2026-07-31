import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership

def targets : List Target :=
  [ Target.mk ``member_order_does_not_change_membership_digest
      "membership-order-invariance" [ "ASP-RFC-10.05-CRCM-CANONICAL" ]
  , Target.mk ``equal_membership_digest_implies_equal_canonical_policy
      "canonical-commitment-injectivity" [ "ASP-RFC-10.05-CRCM-DIGEST" ]
  , Target.mk ``changed_quorum_weight_changes_membership_digest
      "quorum-weight-sensitivity" [ "ASP-RFC-10.05-CRCM-POLICY" ]
  , Target.mk ``changed_failure_domain_requirement_changes_membership_digest
      "failure-domain-sensitivity" [ "ASP-RFC-10.05-CRCM-POLICY" ]
  , Target.mk ``duplicate_node_identity_is_not_well_formed
      "duplicate-node-counterexample" [ "ASP-RFC-10.05-CRCM-UNIQUE" ]
  , Target.mk ``zero_weight_member_is_not_well_formed
      "zero-weight-counterexample" [ "ASP-RFC-10.05-CRCM-WEIGHT" ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinCanonicalMembership.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
