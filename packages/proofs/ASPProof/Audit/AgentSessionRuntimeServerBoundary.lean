-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.Audit.Core
import ASPProof.AgentSessionRuntimeServerBoundary

namespace ASPProof.Audit.AgentSessionRuntimeServerBoundary

open Lean Elab Term
open ASPProof.Audit.Core

def targets : List Target :=
  [ { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.runtime_server_owns_index_write
      theoremFamily := "runtime-server-index-authority"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-RUNTIME"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.runtime_server_owns_generation_publication
      theoremFamily := "runtime-server-generation-authority"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-RUNTIME"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.agent_session_has_no_index_write_authority
      theoremFamily := "session-index-authority-exclusion"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-DISJOINT"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.resident_subagent_has_no_generation_publish_authority
      theoremFamily := "subagent-generation-authority-exclusion"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-DISJOINT"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.runtime_server_has_no_subagent_binding_authority
      theoremFamily := "server-subagent-authority-exclusion"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-DISJOINT"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.agent_session_owns_subagent_binding
      theoremFamily := "session-subagent-binding-authority"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-SESSION"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.host_runtime_owns_native_dispatch
      theoremFamily := "host-native-dispatch-authority"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-HOST"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.query_request_does_not_confer_lease_issue_authority
      theoremFamily := "query-capability-nonownership"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-CAPABILITY"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.live_session_subagent_routes_natively
      theoremFamily := "live-subagent-native-route"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-ROUTE"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.invalid_recommended_next_is_rejected_without_binding
      theoremFamily := "invalid-next-command-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-ROUTE"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.owner_language_mismatch_is_rejected_without_binding
      theoremFamily := "owner-language-mismatch-rejection"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-ROUTE"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.dispatch_requires_host_attestation
      theoremFamily := "host-attestation-dispatch-gate"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-DISPATCH"] }
  , { name :=
        `ASPProof.AgentSessionRuntimeServerBoundary.dispatch_requires_session_binding
      theoremFamily := "session-binding-dispatch-gate"
      rfcClauseIds := ["ASP-RFC-10.05-ASRS-DISPATCH"] }
  ]

def auditJson : TermElabM Json :=
  proofAuditJson
    "ASPProof.AgentSessionRuntimeServerBoundary"
    "ASPProof/AgentSessionRuntimeServerBoundary.lean"
    targets

end ASPProof.Audit.AgentSessionRuntimeServerBoundary
