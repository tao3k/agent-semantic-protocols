-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.SearchRouteInspectFiniteCommitment

namespace ASPProof.Audit.SearchRouteInspectFiniteCommitment

def targets : List Core.Target :=
  [
    {
      name :=
        `SearchRouteInspectFiniteCommitment.universe_root_binding
      theoremFamily := "root-binding"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-IDENTITY",
          "ASP-RFC-10.05-FCM-CANONICAL",
          "ASP-RFC-10.05-FCM-BINDING"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.universe_and_frontier_roots_are_domain_separated
      theoremFamily := "domain-separation"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-IDENTITY",
          "ASP-RFC-10.05-FCM-CANONICAL",
          "ASP-RFC-10.05-FCM-DOMAIN"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_membership_sound
      theoremFamily := "membership-soundness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-MEMBERSHIP",
          "ASP-RFC-10.05-FCM-SCHEME"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_membership_complete
      theoremFamily := "membership-completeness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-MEMBERSHIP",
          "ASP-RFC-10.05-FCM-SCHEME"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_contained_root_is_valid
      theoremFamily := "root-validity"
      rfcClauseIds := ["ASP-RFC-10.05-FCM-VALID-ROOT"]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_exclusion_sound
      theoremFamily := "exclusion-soundness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-BINDING",
          "ASP-RFC-10.05-FCM-EXCLUSION"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_exclusion_root_is_valid
      theoremFamily := "root-validity"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-VALID-ROOT",
          "ASP-RFC-10.05-FCM-EXCLUSION"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_exclusion_complete
      theoremFamily := "exclusion-completeness"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-VALID-ROOT",
          "ASP-RFC-10.05-FCM-EXCLUSION"
        ]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_opening_materializes_manifest
      theoremFamily := "opening-size-boundary"
      rfcClauseIds := ["ASP-RFC-10.05-FCM-COST-BOUNDARY"]
    },
    {
      name :=
        `SearchRouteInspectFiniteCommitment.finite_verified_membership_and_exclusion_conflict
      theoremFamily := "opening-consistency"
      rfcClauseIds :=
        [
          "ASP-RFC-10.05-FCM-MEMBERSHIP",
          "ASP-RFC-10.05-FCM-EXCLUSION",
          "ASP-RFC-10.05-FCM-SCHEME"
        ]
    }
  ]

def auditJson : Lean.Elab.TermElabM Lean.Json :=
  Core.proofAuditJson
    "ASPProof.SearchRouteInspectFiniteCommitment"
    "packages/proofs/ASPProof/SearchRouteInspectFiniteCommitment.lean"
    targets

end ASPProof.Audit.SearchRouteInspectFiniteCommitment
