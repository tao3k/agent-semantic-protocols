import ASPProof.HookCommandBehavior

namespace ASPProof.Audit.HookCommandBehavior

open ASPProof.HookCommandBehavior

def batKey : BehaviorKey :=
  { executableIdentity := "blake3:bat"
    argumentShape := ["-s", "{registered-source}"]
    profileExtension := "rs" }

def changedBatKey : BehaviorKey :=
  { executableIdentity := "blake3:bat-new"
    argumentShape := ["-s", "{registered-source}"]
    profileExtension := "rs" }

example :
    reusableFor
      { key := batKey, access := .read, origin := .processMemory }
      batKey = true := by
  exact exact_behavior_key_reuses_verified_read batKey .processMemory

example :
    reusableFor
      { key := batKey, access := .read, origin := .stateHomeCatalog }
      changedBatKey = false := by
  exact changed_behavior_key_cannot_reuse_read batKey changedBatKey (by decide) .stateHomeCatalog

example : publishObservation .missing batKey .unknown = .missing := by
  exact unknown_observation_cannot_publish .missing batKey

example : publishObservation .missing batKey .read = .verifiedRead batKey := by
  exact read_observation_publishes_positive_fact .missing batKey

example : observePermissionDifferential "exit:0" "exit:1" = .read := by
  exact distinct_permission_terminals_prove_read "exit:0" "exit:1" (by decide)

example : decideShard (.observing 7 batKey) batKey = .wait := by
  exact same_key_observer_forces_follower_wait 7 batKey

example : decideShard (.verifiedRead batKey) batKey = .consume := by
  exact committed_same_key_is_consumed_without_probe batKey

#check retained_writer_cannot_reach_input_terminal
#check released_writer_reaches_input_terminal
#check unknown_inner_candidate_does_not_mask_wrapped_read
#check non_read_candidates_cannot_create_reader_authority
#check equal_permission_terminals_remain_unknown
#check distinct_permission_terminals_prove_read
#check same_key_observer_forces_follower_wait
#check committed_same_key_is_consumed_without_probe

end ASPProof.Audit.HookCommandBehavior
