import ASPProof.Audit.Receipt
import ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization

open ASPProof.Audit.Core
open ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization

def targets : List Target :=
  [ Target.mk
      ``committable_proposal_has_exact_revision_and_allowed_mutation
      "combined-commit-gate"
      [ "ASP-RFC-10.05-CRLA-GATE" ]
  , Target.mk
      ``committed_mutation_advances_revision_once
      "single-revision-advance"
      [ "ASP-RFC-10.05-CRLA-CAS" ]
  , Target.mk
      ``stale_revision_proposal_cannot_commit
      "stale-proposal-counterexample"
      [ "ASP-RFC-10.05-CRLA-REVISION"
      , "ASP-RFC-10.05-CRLA-STALE"
      ]
  , Target.mk
      ``same_snapshot_second_proposal_loses_revision_race
      "same-snapshot-one-winner"
      [ "ASP-RFC-10.05-CRLA-ONE-WINNER"
      , "ASP-RFC-10.05-CRLA-NONIMPLICATION"
      ]
  , Target.mk
      ``concurrent_activation_and_epoch_advance_have_one_revision_winner
      "cross-mutation-one-winner"
      [ "ASP-RFC-10.05-CRLA-ONE-WINNER" ]
  , Target.mk
      ``activation_commit_authorizes_bound_graph_edge
      "winner-edge-binding"
      [ "ASP-RFC-10.05-CRLA-EDGE" ]
  , Target.mk
      ``conflicting_claim_cannot_share_winner_edge
      "conflicting-edge-counterexample"
      [ "ASP-RFC-10.05-CRLA-EDGE"
      , "ASP-RFC-10.05-CRLA-NONIMPLICATION"
      ]
  , Target.mk
      ``stale_revision_edge_is_not_authorized
      "stale-edge-counterexample"
      [ "ASP-RFC-10.05-CRLA-EDGE"
      , "ASP-RFC-10.05-CRLA-STALE"
      ]
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheRejoinLinearization"
    "ASPProof/SearchRouteAdmissionRetryCacheRejoinLinearization.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheRejoinLinearization
