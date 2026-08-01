import ASPProof.RuntimeSupervisorReconciliation

open ASPProof.RuntimeSupervisorReconciliation

#check supervisor_control_is_data_plane_independent
#check stale_transport_rejects_status
#check stale_transport_requests_authenticated_drain
#check matching_runtime_and_transport_is_noop
#check unauthenticated_reconcile_cannot_drain
#check first_reconcile_owns_single_flight
#check concurrent_reconcile_does_not_create_second_owner
#check reconciliation_strictly_progresses
#check bounded_reconciliation_reaches_healthy
#check hook_reconciliation_never_waits

example : reconcileRequestsDrain ⟨true, true, false⟩ = true := by decide
example : statusAdmitted ⟨true, true, false⟩ = false := by decide
example : next (next (next (next .intent))) = .healthy := by decide
