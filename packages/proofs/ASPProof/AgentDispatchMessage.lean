-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.AgentDispatchMessage

set_option maxRecDepth 4096

structure Route where
  agentKind : String
  callTarget : String
  role : String
  description : String
deriving DecidableEq, Repr

structure ProviderRoute where
  providerId : String
  languageId : String
  agent : Route
deriving DecidableEq, Repr

def collaborationTool : String :=
  "collaboration.spawn_agent"

def render (route : Route) : String :=
  "Use `" ++ collaborationTool ++
    "` to create the " ++ route.agentKind ++
    " `" ++ route.callTarget ++ "` (" ++ route.role ++
    "; " ++ route.description ++ ")."

def renderProvider (route : ProviderRoute) : String :=
  render route.agent

def explorer : Route where
  agentKind := "Agent"
  callTarget := "@asp_explorer"
  role := "Evidence Explorer"
  description := "for code and evidence search"

def testing : Route where
  agentKind := "Agent"
  callTarget := "@asp_testing"
  role := "Test Runner"
  description := "for build and test jobs"

theorem rendering_is_four_slot_substitution (route : Route) :
    render route =
      "Use `collaboration.spawn_agent` to create the " ++
        route.agentKind ++ " `" ++ route.callTarget ++ "` (" ++ route.role ++
        "; " ++ route.description ++ ")." := by
  rfl

theorem provider_identity_does_not_change_message_grammar
    (providerId languageId : String) (agent : Route) :
    renderProvider { providerId, languageId, agent } = render agent := by
  rfl

theorem explorer_sentence_is_canonical :
    render explorer =
      "Use `collaboration.spawn_agent` to create the Agent `@asp_explorer` (Evidence Explorer; for code and evidence search)." := by
  rfl

theorem testing_sentence_is_canonical :
    render testing =
      "Use `collaboration.spawn_agent` to create the Agent `@asp_testing` (Test Runner; for build and test jobs)." := by
  rfl

end ASPProof.AgentDispatchMessage
