-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchEvidenceReflection

open ASPProof.SearchEvidenceReflection

/- Expected-failure checks require the precise false-proposition diagnostic.
   Import failures or unrelated parser errors must fail this audit. -/

/--
error: Tactic `decide` proved that the proposition
  admitExact before afterBodyEdit = true
is false
-/
#guard_msgs in
example : admitExact before afterBodyEdit = true := by decide

/--
error: Tactic `decide` proved that the proposition
  (uniqueWitnesses repeatedObservation).length = 2
is false
-/
#guard_msgs in
example : (uniqueWitnesses repeatedObservation).length = 2 := by decide

/--
error: Tactic `decide` proved that the proposition
  packetCost separateAtoms ≤ packetCost sharedAtom
is false
-/
#guard_msgs in
example : packetCost separateAtoms ≤ packetCost sharedAtom := by decide

/--
error: Tactic `decide` proved that the proposition
  canReplay keyAfter [keyBefore] [keyBefore] = true
is false
-/
#guard_msgs in
example : canReplay keyAfter [keyBefore] [keyBefore] = true := by decide

/--
error: Tactic `decide` proved that the proposition
  direct reducedGraph Node.refresh Node.storeLoad = true
is false
-/
#guard_msgs in
example : direct reducedGraph .refresh .storeLoad = true := by decide

/--
error: Tactic `decide` proved that the proposition
  unsafeDelimitedRead [RawLine.sourceText, RawLine.orgEndDelimiter, RawLine.sourceText] =
    [RawLine.sourceText, RawLine.orgEndDelimiter, RawLine.sourceText]
is false
-/
#guard_msgs in
example : unsafeDelimitedRead [.sourceText, .orgEndDelimiter, .sourceText] =
    [.sourceText, .orgEndDelimiter, .sourceText] := by decide

/- Trust audit: no custom axioms or sorryAx are permitted. -/
#print axioms topology_equality_does_not_imply_exact_binding
#print axioms resolver_change_is_not_licensed_by_same_content
#print axioms topology_fact_reuse_is_not_body_reuse
#print axioms exact_admission_sound
#print axioms distinct_seeds_share_one_owner
#print axioms owner_selection_independent_of_seed
#print axioms requested_fact_is_preserved
#print axioms omitted_fact_has_indistinguishable_worlds
#print axioms skeleton_cannot_decide_implementation_call
#print axioms projection_preserves_declared_question
#print axioms projection_preserves_validity
#print axioms filtered_positive_with_unfiltered_coverage_is_false_absence
#print axioms honest_partial_directory_is_unknown
#print axioms two_collectors_can_be_one_witness
#print axioms same_fact_with_two_occurrences_retains_two_witnesses
#print axioms witness_membership_preserved
#print axioms deletion_minimal_need_not_minimize_cost
#print axioms shortest_packet_need_not_be_sufficient
#print axioms previously_sent_does_not_imply_replayability
#print axioms retrievable_reference_survives_context_eviction
#print axioms old_reference_does_not_authorize_new_content
#print axioms historical_replay_and_current_admission_are_distinct
#print axioms forged_reference_payload_fails_closed
#print axioms unresolved_history_does_not_become_a_fact
#print axioms reachability_does_not_preserve_direct_calls
#print axioms unescaped_delimiter_loses_native_content
#print axioms direct_member_query_is_legal
#print axioms different_target_cannot_be_silently_expanded
#print axioms all_reflection_checks_pass
#print axioms declared_projection_is_decision_correct
#print axioms raw_and_active_structure_have_distinct_dependencies
#print axioms same_type_name_does_not_identify_an_impl
#print axioms no_decoder_can_recover_an_omitted_fact
#print axioms ruled_out_requires_sound_complete_coverage
#print axioms certified_negative_is_not_merely_missing
#print axioms same_occurrence_with_different_binding_is_not_deduplicated
