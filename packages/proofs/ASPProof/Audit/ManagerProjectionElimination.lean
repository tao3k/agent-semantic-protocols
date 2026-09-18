-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ManagerProjectionElimination

namespace ASPProof.Audit.ManagerProjectionElimination

open ASPProof.ManagerProjectionElimination

theorem route_only_admission_is_host_attestation_complete :
    admits
      { agentType := "asp_explorer", hostAgentName := "asp_explorer", roles := ["explore"] }
      { agentType := "asp_explorer", canonicalPath := "/root/asp_explorer",
        childId := "child-2", bindingFresh := true } = true := by
  decide

theorem no_projected_profile_is_needed :
    admits
      { agentType := "asp_testing", hostAgentName := "asp_testing", roles := ["testing"] }
      { agentType := "asp_testing", canonicalPath := "/root/asp_testing",
        childId := "child-3", bindingFresh := true } = true := by
  decide

end ASPProof.Audit.ManagerProjectionElimination
