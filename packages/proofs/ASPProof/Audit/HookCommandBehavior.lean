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
      { key := batKey, access := .readOnly, origin := .processMemory }
      batKey = true := by
  exact exact_behavior_key_reuses_verified_read batKey .processMemory

example :
    reusableFor
      { key := batKey, access := .readOnly, origin := .stateHomeCatalog }
      changedBatKey = false := by
  exact changed_behavior_key_cannot_reuse_read batKey changedBatKey (by decide) .stateHomeCatalog

example : publishObservation .missing batKey .unknown = .missing := by
  exact unknown_observation_cannot_publish .missing batKey

example : publishObservation .missing batKey .notReadOnly = .missing := by
  exact write_capable_observation_cannot_publish .missing batKey

example : publishObservation .missing batKey .readOnly = .verifiedRead batKey := by
  exact read_only_observation_publishes_positive_fact .missing batKey

#check retained_writer_cannot_reach_input_terminal
#check released_writer_reaches_input_terminal

end ASPProof.Audit.HookCommandBehavior
