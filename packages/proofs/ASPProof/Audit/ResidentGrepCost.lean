-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.ResidentGrepCost

namespace ASPProof.Audit.ResidentGrepCost

open ASPProof.ResidentGrepCost

#check exact_hits_subset_candidates
#check more_mandatory_grams_never_widen
#check empty_candidates_imply_no_exact_hits
#check not_materialized_cannot_publish_absence
#check complete_fused_scope_empty_implies_absence
#check externally_pure_has_no_binary_or_filesystem_work
#check prefix_decode_bounded_by_full_decode
#check fixed_cost_blocks_target_factor
#check limit_before_intersection_loses_hit
#check v1_directory_field_accounting
#check mapped_generation_has_zero_workspace_heap_retention
#check admitted_tokio_lane_preserves_reactor

end ASPProof.Audit.ResidentGrepCost
