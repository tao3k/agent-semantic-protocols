-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ProjectScopeBootstrap

namespace ASPProof.Audit.ProjectScopeBootstrap

open ASPProof.ProjectScopeBootstrap

#check scope_is_independent_of_candidate_source_paths
#check registered_default_scope_bootstraps_without_source_candidates
#check explicit_scope_preserves_registered_language_contract
#check bootstrap_transition_decreases_rank

end ASPProof.Audit.ProjectScopeBootstrap
