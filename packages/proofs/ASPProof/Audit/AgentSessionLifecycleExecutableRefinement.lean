-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.AgentSessionLifecycleExecutableRefinement

namespace ASPProof.Audit.AgentSessionLifecycleExecutableRefinement

open ASPProof.AgentSessionLifecycleProduct
open ASPProof.AgentSessionLifecycleExecutableRefinement

example
    (state : LifecycleProduct)
    (lateReceipt : state.binding.generation ≠ state.session.generation) :
    ¬ executableFollowupAdmitted state := by
  exact stale_binding_generation_rejects_followup lateReceipt

example
    (state : LifecycleProduct)
    (admitted : executableFollowupAdmitted state) :
    followupTaskAdmitted state = true := by
  exact executable_followup_refines_product admitted

example
    (state : LifecycleProduct)
    (absent : state.binding.pathObservation = .absent) :
    ¬ executableFollowupAdmitted state := by
  exact absent_path_rejects_executable_followup absent

example : SessionPhase.unobserved ≠ SessionPhase.declared := by
  decide

example : ServerHealth.unobserved ≠ ServerHealth.unavailable := by
  decide

end ASPProof.Audit.AgentSessionLifecycleExecutableRefinement
