import ASPProof.Audit.Core
import ASPProof.SearchRouteInspectCommittedUniverse

namespace ASPProof.Audit.SearchRouteInspectCommittedUniverse

def targets : List Core.Target :=
  [
    {
      name :=
        `SearchRouteInspectCommittedUniverse.verified_membership_is_contained
      theoremFamily := "opening-soundness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-SCHEME",
          "ASP-RFC-10.05-SCU-OPENING"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.verified_exclusion_is_absent
      theoremFamily := "opening-soundness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-SCHEME",
          "ASP-RFC-10.05-SCU-OPENING"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.verified_membership_root_is_valid
      theoremFamily := "root-validity"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-SCHEME",
          "ASP-RFC-10.05-SCU-OPENING"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.verified_exclusion_root_is_valid
      theoremFamily := "root-validity"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-SCHEME",
          "ASP-RFC-10.05-SCU-OPENING"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.membership_and_exclusion_cannot_both_verify
      theoremFamily := "opening-consistency"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-OPENING"]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_ledger_snapshot_is_bound
      theoremFamily := "snapshot-binding"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-SNAPSHOT"]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_ledger_visible_is_optimal
      theoremFamily := "visible-optimality"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-GLOBAL"]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_frontier_pruning_is_safe
      theoremFamily := "frontier-pruning"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-FRONTIER",
          "ASP-RFC-10.05-SCU-CLOSURE"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_ledger_selected_has_provenance
      theoremFamily := "trace-provenance"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-GLOBAL"]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_ledger_selected_realizes
      theoremFamily := "trace-realization"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-GLOBAL"]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.committed_trace_ledger_selects_global_driver
      theoremFamily := "committed-global-optimality"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-SCU-SCHEME",
          "ASP-RFC-10.05-SCU-SNAPSHOT",
          "ASP-RFC-10.05-SCU-FRONTIER",
          "ASP-RFC-10.05-SCU-COVERAGE",
          "ASP-RFC-10.05-SCU-CLOSURE",
          "ASP-RFC-10.05-SCU-GLOBAL"
        ]
    },
    {
      name :=
        `SearchRouteInspectCommittedUniverse.digest_only_universe_is_not_binding
      theoremFamily := "counterexample"
      rfcClauseIds := ["ASP-RFC-10.05-SCU-DIGEST-ONLY"]
    }
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  Core.proofAuditJson
    "ASPProof.SearchRouteInspectCommittedUniverse"
    "packages/proofs/ASPProof/SearchRouteInspectCommittedUniverse.lean"
    targets

end ASPProof.Audit.SearchRouteInspectCommittedUniverse
