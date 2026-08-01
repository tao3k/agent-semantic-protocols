import ASPProof.Audit.RuntimeResidentLifecycle

open ASPProof.RuntimeResidentLifecycle

example (host : HostCapabilities) (h : host.retireChild = false) :
    decideRepair false host = .blockResidentRoute :=
  missing_retirement_blocks host h

example {before after : RepairPhase} (h : Advances before after) :
    progressRank after < progressRank before :=
  admitted_transition_decreases_rank h

example (decision : RepairDecision) :
    blocksUnrelatedTools decision = false :=
  repair_never_blocks_unrelated_tools decision
