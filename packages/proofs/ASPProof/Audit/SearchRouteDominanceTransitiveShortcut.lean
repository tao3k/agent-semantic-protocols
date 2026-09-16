-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteDominanceTransitiveShortcut

open ASPProof.SearchRouteDominanceTransitiveShortcut

#print axioms bool_and_true_of_parts
#print axioms bool_or_true_of_left
#print axioms bool_or_true_of_right
#print axioms le_implies_natLe_true
#print axioms noWorse_true_to_components
#print axioms components_to_noWorse_true
#print axioms strictlyImproves_true_to_dimension
#print axioms dimension_to_strictlyImproves_true
#print axioms costNoWorse_is_transitive
#print axioms strictImprovement_followed_by_noWorse
#print axioms noWorse_is_transitive
#print axioms strict_dominance_is_transitive
#print axioms chain_endpoint_eq_or_strictly_dominates
#print axioms nonempty_chain_endpoint_strictly_dominates_start
#print axioms buildTransitiveShortcut
#print axioms shortcut_preserves_removal_soundness
#print axioms shortcut_receipt_is_length_independent
#print axioms shortcut_receipt_beats_nonempty_explicit_chain
