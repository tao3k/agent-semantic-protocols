-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteEndToEndReplayStateMachine

open ASPProof.SearchRouteEndToEndReplayStateMachine

#print axioms canonicalPayloadOfRaw
#print axioms AcceptedReplay.payloadEqual
#print axioms transition_increments_stage_rank
#print axioms add_one_then_length
#print axioms path_stage_rank_accounting
#print axioms raw_to_accepted_path_has_exact_length
#print axioms raw_to_fallback_path_has_exact_length
#print axioms canonicalAcceptedPath
#print axioms no_direct_raw_to_accepted_transition
#print axioms no_direct_admitted_to_accepted_transition
#print axioms acceptedReplayAdmissionWitness
#print axioms accepted_replay_contains_negotiated_identity
#print axioms acceptedReplayGateClosure
#print axioms fallback_is_not_accepted
#print axioms componentwise_cost_reduction_is_strict_improvement
#print axioms illustrative_digest_shortcut_strictly_improves_core
#print axioms accepted_identity_is_independent_of_cache_credits
#print axioms cache_credits_do_not_change_core_cost
