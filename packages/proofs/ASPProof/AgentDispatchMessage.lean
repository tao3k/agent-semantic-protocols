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

def choicePlaneCommand : String :=
  "asp session --agents choice-plane"

def render (route : Route) : String :=
  "Please use `" ++ choicePlaneCommand ++
    "` to create or resume the " ++ route.agentKind ++
    " `" ++ route.callTarget ++ "` (" ++ route.role ++
    "; " ++ route.description ++ ")."

def renderProvider (route : ProviderRoute) : String :=
  render route.agent

def explorer : Route where
  agentKind := "Subagent"
  callTarget := "@asp_explorer"
  role := "Evidence Explorer"
  description := "for code and evidence search"

def testing : Route where
  agentKind := "Subagent"
  callTarget := "@asp_testing"
  role := "Test Runner"
  description := "for build and test jobs"

theorem rendering_is_four_slot_substitution (route : Route) :
    render route =
      "Please use `asp session --agents choice-plane` to create or resume the " ++
        route.agentKind ++ " `" ++ route.callTarget ++ "` (" ++ route.role ++
        "; " ++ route.description ++ ")." := by
  rfl

theorem provider_identity_does_not_change_message_grammar
    (providerId languageId : String) (agent : Route) :
    renderProvider { providerId, languageId, agent } = render agent := by
  rfl

theorem explorer_sentence_is_canonical :
    render explorer =
      "Please use `asp session --agents choice-plane` to create or resume the Subagent `@asp_explorer` (Evidence Explorer; for code and evidence search)." := by
  rfl

theorem testing_sentence_is_canonical :
    render testing =
      "Please use `asp session --agents choice-plane` to create or resume the Subagent `@asp_testing` (Test Runner; for build and test jobs)." := by
  rfl

end ASPProof.AgentDispatchMessage
