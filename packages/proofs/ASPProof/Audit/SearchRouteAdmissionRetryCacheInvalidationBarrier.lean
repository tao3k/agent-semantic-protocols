import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier

namespace ASPProof.Audit.SearchRouteAdmissionRetryCacheInvalidationBarrier

open ASPProof.Audit.Core

def targets : List Target :=
  [ Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.safe_barrier_serving_replica_has_crossed_target_generation
      "serving-replica barrier"
      ["ASP-RFC-10.05-DCIB-ACKNOWLEDGMENT"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.safe_barrier_rejects_older_entry_at_serving_replica
      "stale-entry rejection"
      ["ASP-RFC-10.05-DCIB-STALE-ENTRY"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.emitted_invalidation_event_does_not_establish_safe_barrier
      "event-emission counterexample"
      ["ASP-RFC-10.05-DCIB-NONIMPLICATION"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.one_replica_quorum_does_not_protect_unfenced_late_replica
      "quorum-only counterexample"
      [ "ASP-RFC-10.05-DCIB-QUORUM"
      , "ASP-RFC-10.05-DCIB-NONIMPLICATION" ],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.quorum_with_unacknowledged_replica_fenced_is_safe
      "quorum fencing"
      ["ASP-RFC-10.05-DCIB-QUORUM"],
    Target.mk
      ``ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier.unfenced_late_replica_can_treat_stale_entry_as_locally_current
      "late-replica counterexample"
      ["ASP-RFC-10.05-DCIB-NONIMPLICATION"] ]

def auditJson :=
  proofAuditJson
    "ASPProof.SearchRouteAdmissionRetryCacheInvalidationBarrier"
    "ASPProof/SearchRouteAdmissionRetryCacheInvalidationBarrier.lean"
    targets

end ASPProof.Audit.SearchRouteAdmissionRetryCacheInvalidationBarrier
