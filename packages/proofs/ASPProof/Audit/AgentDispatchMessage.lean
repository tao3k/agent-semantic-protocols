-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentDispatchMessage

namespace ASPProof.Audit.AgentDispatchMessage

open ASPProof.AgentDispatchMessage

#check rendering_is_four_slot_substitution
#check provider_identity_does_not_change_message_grammar
#check explorer_sentence_is_canonical
#check testing_sentence_is_canonical

example : explorer.agentKind = "Subagent" := by rfl
example : explorer.callTarget = "@asp_explorer" := by rfl
example : testing.agentKind = "Subagent" := by rfl
example : testing.callTarget = "@asp_testing" := by rfl

end ASPProof.Audit.AgentDispatchMessage
