--------------------- MODULE ChangeAwareEvidenceTrajectory ---------------------
EXTENDS Naturals, FiniteSets, Sequences, TLC

CONSTANTS
  BaseEvidence,
  DerivedEvidence,
  Readers,
  MaxGeneration,
  ParentOf,
  ImpactOf

Evidence == BaseEvidence \union DerivedEvidence
Generations == 0..MaxGeneration

ASSUME BaseEvidence \cap DerivedEvidence = {}
ASSUME ParentOf \in [DerivedEvidence -> SUBSET Evidence]
ASSUME ImpactOf \in [BaseEvidence -> SUBSET Evidence]
ASSUME \A base \in BaseEvidence : base \in ImpactOf[base]
ASSUME \A base \in BaseEvidence :
  \A derived \in DerivedEvidence :
    base \in ParentOf[derived] => derived \in ImpactOf[base]

VARIABLES
  activeGeneration,
  candidateGeneration,
  phase,
  validEvidence,
  candidateEvidence,
  pendingRevalidation,
  affectedEvidence,
  revalidatedEvidence,
  publishedEvidence,
  readerGeneration,
  readerEvidence,
  changedBaseEvidence,
  previousRoundEvidence,
  currentRoundEvidence,
  deltaNew,
  deltaRetained,
  deltaDisappeared,
  trajectory

vars ==
  <<activeGeneration, candidateGeneration, phase, validEvidence,
    candidateEvidence, pendingRevalidation, affectedEvidence,
    revalidatedEvidence, publishedEvidence, readerGeneration,
    readerEvidence, changedBaseEvidence, previousRoundEvidence,
    currentRoundEvidence, deltaNew, deltaRetained, deltaDisappeared,
    trajectory>>

Affected(changed) ==
  UNION {ImpactOf[base] : base \in changed}

DerivedClosed(evidence) ==
  \A derived \in (evidence \cap DerivedEvidence) :
    ParentOf[derived] \subseteq evidence

Init ==
  /\ activeGeneration = 0
  /\ candidateGeneration = 0
  /\ phase = "ready"
  /\ validEvidence = Evidence
  /\ candidateEvidence = Evidence
  /\ pendingRevalidation = {}
  /\ affectedEvidence = {}
  /\ revalidatedEvidence = {}
  /\ publishedEvidence =
       [generation \in Generations |->
         IF generation = 0 THEN Evidence ELSE {}]
  /\ readerGeneration = [reader \in Readers |-> 0]
  /\ readerEvidence = [reader \in Readers |-> Evidence]
  /\ changedBaseEvidence = {}
  /\ previousRoundEvidence = Evidence
  /\ currentRoundEvidence = Evidence
  /\ deltaNew = {}
  /\ deltaRetained = Evidence
  /\ deltaDisappeared = {}
  /\ trajectory = <<>>

StartMutation(changed) ==
  /\ phase = "ready"
  /\ activeGeneration < MaxGeneration
  /\ changed \in SUBSET BaseEvidence
  /\ changed # {}
  /\ phase' = "building"
  /\ candidateGeneration' = activeGeneration + 1
  /\ affectedEvidence' = Affected(changed)
  /\ changedBaseEvidence' = changed
  /\ candidateEvidence' = validEvidence \ Affected(changed)
  /\ pendingRevalidation' = Affected(changed)
  /\ revalidatedEvidence' = {}
  /\ trajectory' =
       Append(trajectory,
         [kind |-> "mutation-started",
          fromGeneration |-> activeGeneration,
          toGeneration |-> activeGeneration + 1,
          affected |-> Affected(changed)])
  /\ UNCHANGED
       <<activeGeneration, validEvidence, publishedEvidence,
         readerGeneration, readerEvidence, previousRoundEvidence,
         currentRoundEvidence, deltaNew, deltaRetained, deltaDisappeared>>

RevalidateEvidence(evidence) ==
  /\ phase = "building"
  /\ evidence \in pendingRevalidation
  /\ candidateEvidence' = candidateEvidence \union {evidence}
  /\ pendingRevalidation' = pendingRevalidation \ {evidence}
  /\ revalidatedEvidence' = revalidatedEvidence \union {evidence}
  /\ trajectory' =
       Append(trajectory,
         [kind |-> "revalidated",
          generation |-> candidateGeneration,
          evidence |-> evidence])
  /\ UNCHANGED
       <<activeGeneration, candidateGeneration, phase, validEvidence,
         affectedEvidence, publishedEvidence, readerGeneration,
         readerEvidence, changedBaseEvidence, previousRoundEvidence,
         currentRoundEvidence, deltaNew, deltaRetained, deltaDisappeared>>

RejectEvidence(evidence) ==
  /\ phase = "building"
  /\ evidence \in pendingRevalidation
  /\ pendingRevalidation' = pendingRevalidation \ {evidence}
  /\ trajectory' =
       Append(trajectory,
         [kind |-> "invalidated",
          generation |-> candidateGeneration,
          evidence |-> evidence])
  /\ UNCHANGED
       <<activeGeneration, candidateGeneration, phase, validEvidence,
         candidateEvidence, affectedEvidence, revalidatedEvidence,
         publishedEvidence, readerGeneration, readerEvidence,
         changedBaseEvidence, previousRoundEvidence, currentRoundEvidence,
         deltaNew, deltaRetained, deltaDisappeared>>

Publish ==
  /\ phase = "building"
  /\ pendingRevalidation = {}
  /\ DerivedClosed(candidateEvidence)
  /\ phase' = "ready"
  /\ activeGeneration' = candidateGeneration
  /\ validEvidence' = candidateEvidence
  /\ previousRoundEvidence' = validEvidence
  /\ currentRoundEvidence' = candidateEvidence
  /\ deltaNew' = candidateEvidence \ validEvidence
  /\ deltaRetained' = candidateEvidence \cap validEvidence
  /\ deltaDisappeared' = validEvidence \ candidateEvidence
  /\ publishedEvidence' =
       [publishedEvidence EXCEPT ![candidateGeneration] = candidateEvidence]
  /\ trajectory' =
       Append(trajectory,
         [kind |-> "published",
          generation |-> candidateGeneration,
          evidence |-> candidateEvidence])
  /\ UNCHANGED
       <<candidateGeneration, candidateEvidence, pendingRevalidation,
         affectedEvidence, revalidatedEvidence, readerGeneration,
         readerEvidence, changedBaseEvidence>>

AcquireReader(reader) ==
  /\ reader \in Readers
  /\ readerGeneration' =
       [readerGeneration EXCEPT ![reader] = activeGeneration]
  /\ readerEvidence' =
       [readerEvidence EXCEPT ![reader] = publishedEvidence[activeGeneration]]
  /\ UNCHANGED
       <<activeGeneration, candidateGeneration, phase, validEvidence,
         candidateEvidence, pendingRevalidation, affectedEvidence,
         revalidatedEvidence, publishedEvidence, changedBaseEvidence,
         previousRoundEvidence, currentRoundEvidence, deltaNew,
         deltaRetained, deltaDisappeared, trajectory>>

Next ==
  \/ \E changed \in SUBSET BaseEvidence : StartMutation(changed)
  \/ \E evidence \in Evidence : RevalidateEvidence(evidence)
  \/ \E evidence \in Evidence : RejectEvidence(evidence)
  \/ Publish
  \/ \E reader \in Readers : AcquireReader(reader)

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ activeGeneration \in Generations
  /\ candidateGeneration \in Generations
  /\ phase \in {"ready", "building"}
  /\ validEvidence \subseteq Evidence
  /\ candidateEvidence \subseteq Evidence
  /\ pendingRevalidation \subseteq Evidence
  /\ affectedEvidence \subseteq Evidence
  /\ revalidatedEvidence \subseteq Evidence
  /\ publishedEvidence \in [Generations -> SUBSET Evidence]
  /\ readerGeneration \in [Readers -> Generations]
  /\ readerEvidence \in [Readers -> SUBSET Evidence]
  /\ changedBaseEvidence \subseteq BaseEvidence
  /\ previousRoundEvidence \subseteq Evidence
  /\ currentRoundEvidence \subseteq Evidence
  /\ deltaNew \subseteq Evidence
  /\ deltaRetained \subseteq Evidence
  /\ deltaDisappeared \subseteq Evidence

PublishedDerivedParentsValid ==
  \A generation \in Generations :
    DerivedClosed(publishedEvidence[generation])

NoPublishWithPendingRevalidation ==
  phase = "ready" => pendingRevalidation = {}

ActiveGenerationIsPublished ==
  validEvidence = publishedEvidence[activeGeneration]

ReaderSnapshotConsistent ==
  \A reader \in Readers :
    readerEvidence[reader] = publishedEvidence[readerGeneration[reader]]

AffectedEvidenceNeedsRevalidation ==
  phase = "ready" =>
    (affectedEvidence \ revalidatedEvidence) \cap validEvidence = {}

AffectedEvidenceMatchesDeclaredImpact ==
  affectedEvidence = Affected(changedBaseEvidence)

RoundDeltaExact ==
  /\ deltaNew = currentRoundEvidence \ previousRoundEvidence
  /\ deltaRetained = currentRoundEvidence \cap previousRoundEvidence
  /\ deltaDisappeared = previousRoundEvidence \ currentRoundEvidence

RoundDeltaPartitionsPriorAndCurrent ==
  /\ currentRoundEvidence = deltaNew \union deltaRetained
  /\ previousRoundEvidence = deltaDisappeared \union deltaRetained

=============================================================================
