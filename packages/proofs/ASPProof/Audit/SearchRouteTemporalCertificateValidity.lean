import Lean
import ASPProof.SearchRouteTemporalCertificateValidity

namespace ASPProof.Audit.SearchRouteTemporalCertificateValidity

open Lean

private def stringArray (values : Array String) : Json :=
  .arr (values.map Json.str)

private def declaration
    (name theoremFamily description clauseId : String) : Json :=
  .mkObj [
    ("axioms", stringArray #[]),
    ("hasSorryAx", .bool false),
    ("kind", .str "theorem"),
    ("name", .str name),
    ("rfcClauseIds", stringArray #[clauseId]),
    ("theoremFamily", .str theoremFamily),
    ("type", .str description)
  ]

def declarations : Array Json := #[
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.historical_validity_survives_later_revocation"
    "historical-validity-preservation"
    "later revocation does not rewrite validity at the recorded execution time"
    "ASP-RFC-10.05-TCV-HISTORY",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.revocation_rejects_new_issuance"
    "revocation-issuance-rejection"
    "a revoked lifecycle rejects new issuance at or after revocation"
    "ASP-RFC-10.05-TCV-ISSUANCE",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.execution_before_revocation_is_valid"
    "pre-revocation-execution"
    "execution inside the issuance interval is valid at execution"
    "ASP-RFC-10.05-TCV-EXECUTION",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.historical_audit_remains_valid_after_revocation"
    "post-revocation-audit"
    "a valid recorded execution remains auditable after later revocation"
    "ASP-RFC-10.05-TCV-AUDIT",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.archived_snapshot_enables_historical_replay"
    "archived-historical-replay"
    "historical replay additionally requires the archived snapshot"
    "ASP-RFC-10.05-TCV-REPLAY",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.revoked_certificate_cannot_issue_new_decision"
    "revoked-new-decision-rejection"
    "a revoked certificate cannot authorize a newly issued decision"
    "ASP-RFC-10.05-TCV-NEW-DECISION",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.revoked_certificate_cannot_authorize_new_pareto"
    "revoked-pareto-rejection"
    "a revoked certificate cannot authorize a new Pareto comparison"
    "ASP-RFC-10.05-TCV-PARETO",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.execution_after_revocation_is_invalid"
    "late-execution-rejection"
    "execution at or after revocation is invalid"
    "ASP-RFC-10.05-TCV-LATE",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.missing_archive_blocks_historical_replay"
    "missing-archive-rejection"
    "missing archived evidence blocks replay without invalidating historical audit"
    "ASP-RFC-10.05-TCV-ARCHIVE",
  declaration
    "ASPProof.SearchRouteTemporalCertificateValidity.pre_revocation_issuance_is_authorized"
    "pre-revocation-issuance"
    "issuance before revocation remains authorized"
    "ASP-RFC-10.05-TCV-PRE-REVOCATION"
]

def receipt : Json :=
  .mkObj [
    ("axiomDependentDeclarationCount", .num 0),
    ("axiomFreeDeclarationCount", .num 10),
    ("axiomInventory", stringArray #[]),
    ("declarationCount", .num 10),
    ("declarations", .arr declarations),
    ("hasSorryAx", .bool false),
    ("leanVersion", .str "4.32.2"),
    ("module", .str "ASPProof.SearchRouteTemporalCertificateValidity"),
    ("proofPackage", .str "ASPProof"),
    ("schemaId", .str "asp.lean-proof-audit.v1"),
    ("schemaVersion", .str "1"),
    ("sourcePath", .str "packages/proofs/ASPProof/SearchRouteTemporalCertificateValidity.lean")
  ]

end ASPProof.Audit.SearchRouteTemporalCertificateValidity
