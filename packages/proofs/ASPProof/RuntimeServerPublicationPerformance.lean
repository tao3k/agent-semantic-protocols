namespace ASPProof.RuntimeServerPublicationPerformance

structure PublicationObservation where
  serviceMicros : Nat
  schedulerMicros : Nat
  observedMicros : Nat
  observed_refines : observedMicros = serviceMicros + schedulerMicros

def serviceGate (observation : PublicationObservation) : Prop :=
  observation.serviceMicros < 1000

def observedGate (observation : PublicationObservation) : Prop :=
  observation.observedMicros < 1000

theorem observed_failure_does_not_imply_service_failure
    (observation : PublicationObservation)
    (service_fast : serviceGate observation)
    (scheduler_slow : 1000 ≤ observation.schedulerMicros) :
    serviceGate observation ∧ ¬ observedGate observation := by
  constructor
  · exact service_fast
  · unfold observedGate
    rw [observation.observed_refines]
    omega

theorem bounded_service_and_scheduler_imply_observed_bound
    (observation : PublicationObservation)
    (service_bound : observation.serviceMicros ≤ 499)
    (scheduler_bound : observation.schedulerMicros ≤ 500) :
    observedGate observation := by
  unfold observedGate
  rw [observation.observed_refines]
  omega

end ASPProof.RuntimeServerPublicationPerformance
