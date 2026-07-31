import ASPProof.Audit.Core
import ASPProof.SearchRouteAdmissionHistory

namespace ASPProof.Audit.SearchRouteAdmissionHistory

open ASPProof.Audit.Core

def targets : List Target := [
  Target.mk
    `SearchRouteAdmissionHistory.atomic_record_commit_persists_acknowledgement
    "durable-admission-record"
    ["ASP-RFC-10.05-AHR-RECORD"],
  Target.mk
    `SearchRouteAdmissionHistory.first_linearized_commit_rejects_second_same_run
    "single-winner-linearization"
    ["ASP-RFC-10.05-AHR-LINEARIZATION"],
  Target.mk
    `SearchRouteAdmissionHistory.lost_acknowledgement_is_recoverable_without_recommit
    "lost-ack-recovery"
    ["ASP-RFC-10.05-AHR-ACK-RECOVERY"],
  Target.mk
    `SearchRouteAdmissionHistory.conflicting_retry_returns_existing_record
    "conflicting-retry"
    ["ASP-RFC-10.05-AHR-CONFLICT"],
  Target.mk
    `SearchRouteAdmissionHistory.boolean_commit_marker_loses_acknowledgement_payload
    "boolean-marker-counterexample"
    ["ASP-RFC-10.05-AHR-NONIMPLICATION", "ASP-RFC-10.05-AHR-RECORD"]
]

end ASPProof.Audit.SearchRouteAdmissionHistory
