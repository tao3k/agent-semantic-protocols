-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ResidentWorkspaceRetirement

namespace ASPProof.Audit.ResidentWorkspaceRetirement

open ASPProof.ResidentWorkspaceRetirement

#check missing_workspace_without_activity_is_eligible
#check idle_workspace_without_activity_is_eligible
#check live_lease_prevents_retirement
#check in_flight_request_prevents_retirement
#check retirement_implies_quiescence

end ASPProof.Audit.ResidentWorkspaceRetirement
