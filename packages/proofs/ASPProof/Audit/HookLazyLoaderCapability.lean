-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.HookLazyLoaderCapability

namespace ASPProof.Audit.HookLazyLoaderCapability

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name := `ASPProof.HookLazyLoaderCapability.json_read_with_jq_activates
      theoremFamily := "hook-lazy-loader-capability-activation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.json_read_with_jq_routes_to_structured_read
      theoremFamily := "hook-lazy-loader-structured-route"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.json_read_without_jq_is_unavailable
      theoremFamily := "hook-lazy-loader-capability-unavailable"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.unavailable_jq_cannot_forge_a_route
      theoremFamily := "hook-lazy-loader-route-nonforgery"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.non_read_json_does_not_activate
      theoremFamily := "hook-lazy-loader-effect-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.non_json_read_does_not_activate
      theoremFamily := "hook-lazy-loader-document-isolation"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.jq_route_implies_all_activation_preconditions
      theoremFamily := "hook-lazy-loader-route-soundness"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.jq_route_is_bounded
      theoremFamily := "hook-lazy-loader-bounded-projection"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.raw_json_read_remains_fail_closed
      theoremFamily := "hook-lazy-loader-raw-read-fail-closed"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.missing_jq_does_not_globally_deadlock_the_hook
      theoremFamily := "hook-lazy-loader-global-deadlock-exclusion"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.config_compilation_preserves_json_jq_activation
      theoremFamily := "hook-lazy-loader-config-compiler-refinement"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.incompatible_candidate_cannot_publish
      theoremFamily := "hook-lazy-loader-binary-config-publication-admission"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  , { name := `ASPProof.HookLazyLoaderCapability.compatible_candidate_may_publish
      theoremFamily := "hook-lazy-loader-binary-config-publication-admission"
      rfcClauseIds := ["ASP-RFC-10.15-HOOK-LAZY-LOADER-CAPABILITY"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.HookLazyLoaderCapability"
    "ASPProof/HookLazyLoaderCapability.lean"
    targets

end ASPProof.Audit.HookLazyLoaderCapability
