-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionChoiceState

namespace ASPProof.Audit.AgentSessionChoiceState

open ASPProof.AgentSessionChoiceState

#check unavailable_host_terminates_missing_namespace
#check available_host_allows_first_registration
#check registered_namespace_is_generation_parametric
#check workspace_and_provider_generations_do_not_gate_choice

end ASPProof.Audit.AgentSessionChoiceState
