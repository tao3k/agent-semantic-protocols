namespace ASPProof.HookRegistrationDeadlock

inductive Decision where
  | receiptRequired
  | allow
  | denyBinding
  deriving DecidableEq, Repr

/-- A root session owns a tree; every subagent is a distinct child node. -/
structure ChildSubject where
  rootSessionId : Nat
  parentSessionId : Nat
  childSessionId : Nat
  deriving DecidableEq, Repr

structure Registration where
  subject : ChildSubject
  canonicalAgent : Nat
  generation : Nat
  validated : Bool
  deriving DecidableEq, Repr

structure OldRouteKey where
  rootSessionId : Nat
  canonicalAgent : Nat
  deriving DecidableEq, Repr

def oldRouteKey (registration : Registration) : OldRouteKey :=
  { rootSessionId := registration.subject.rootSessionId
    canonicalAgent := registration.canonicalAgent }

def OldRouteRegistry := OldRouteKey → Option Registration

def emptyOldRouteRegistry : OldRouteRegistry := fun _ => none

/-- The old route-slot registry incorrectly treats a configured route as one physical child slot. -/
def oldRouteClaim
    (registry : OldRouteRegistry)
    (registration : Registration) : OldRouteRegistry :=
  fun key => if key = oldRouteKey registration then some registration else registry key

structure OldState where
  registrationValidated : Bool
  decision : Decision
  choicePlaneDenied : Bool
  deriving DecidableEq, Repr

def oldInitial : OldState :=
  { registrationValidated := false
    decision := .receiptRequired
    choicePlaneDenied := true }

/-- The faulty transition materializes registration but preserves the stale shard. -/
def oldMaterialize (state : OldState) : OldState :=
  { state with registrationValidated := true }

def oldProgressEnabled (state : OldState) : Bool :=
  state.decision != .receiptRequired || !state.choicePlaneDenied

def oldDeadlockWitness : OldState := oldMaterialize oldInitial

theorem old_registered_stale_receipt_required_deadlock_is_reachable :
    oldDeadlockWitness =
      { registrationValidated := true
        decision := .receiptRequired
        choicePlaneDenied := true } := by
  rfl

theorem old_registered_stale_receipt_required_has_no_progress :
    oldProgressEnabled oldDeadlockWitness = false := by
  rfl

theorem old_unique_root_route_overwrites_a_distinct_child
    (left right : Registration)
    (hSameRoot : left.subject.rootSessionId = right.subject.rootSessionId)
    (hSameAgent : left.canonicalAgent = right.canonicalAgent)
    (hDistinctChild : left.subject.childSessionId ≠ right.subject.childSessionId) :
    let published :=
      oldRouteClaim (oldRouteClaim emptyOldRouteRegistry left) right
    published (oldRouteKey left) = some right ∧ left ≠ right := by
  have hSameKey : oldRouteKey left = oldRouteKey right := by
    cases left
    cases right
    simp_all [oldRouteKey]
  have hDistinctRegistration : left ≠ right := by
    intro hEqual
    apply hDistinctChild
    exact congrArg (fun registration => registration.subject.childSessionId) hEqual
  simp [oldRouteClaim, hSameKey, hDistinctRegistration]

def Registry := ChildSubject → Option Registration

def emptyRegistry : Registry := fun _ => none

/-- One durable registration fact is owned by one child identity. -/
def claim (registry : Registry) (registration : Registration) : Registry :=
  fun subject =>
    if subject = registration.subject then some registration else registry subject

/-- Registration is followed by the same capability classifier used normally. -/
def reclassify
    (expectedSubject : ChildSubject)
    (targetCanonicalAgent : Nat)
    (registration : Registration) : Decision :=
  if registration.validated &&
      registration.subject == expectedSubject &&
      registration.canonicalAgent == targetCanonicalAgent then
    .allow
  else
    .denyBinding

theorem validated_registration_cannot_leave_stale_receipt_required
    (expectedSubject : ChildSubject)
    (targetCanonicalAgent : Nat)
    (registration : Registration)
    (_hValidated : registration.validated = true) :
    reclassify expectedSubject targetCanonicalAgent registration ≠ .receiptRequired := by
  unfold reclassify
  split <;> exact Decision.noConfusion

theorem legal_canonical_binding_progresses_to_allow
    (registration : Registration)
    (hValidated : registration.validated = true) :
    reclassify registration.subject registration.canonicalAgent registration = .allow := by
  simp [reclassify, hValidated]

theorem wrong_child_binding_is_explicit_deny
    (expectedSubject : ChildSubject)
    (registration : Registration)
    (hWrongSubject : registration.subject ≠ expectedSubject) :
    reclassify expectedSubject registration.canonicalAgent registration = .denyBinding := by
  simp [reclassify, hWrongSubject]

theorem duplicate_claim_is_idempotent
    (registry : Registry)
    (registration : Registration) :
    claim (claim registry registration) registration = claim registry registration := by
  funext subject
  unfold claim
  by_cases hSubject : subject = registration.subject <;> simp [hSubject]

theorem distinct_child_claims_commute
    (registry : Registry)
    (left right : Registration)
    (hDistinct : left.subject ≠ right.subject) :
    claim (claim registry left) right = claim (claim registry right) left := by
  funext subject
  simp only [claim]
  by_cases hLeft : subject = left.subject
  · subst subject
    simp [hDistinct]
  · by_cases hRight : subject = right.subject
    · subst subject
      simp [hLeft]
    · simp [hLeft, hRight]

theorem one_root_can_retain_two_independent_child_sessions
    (registry : Registry)
    (left right : Registration)
    (_hSameRoot : left.subject.rootSessionId = right.subject.rootSessionId)
    (hDistinctChild : left.subject.childSessionId ≠ right.subject.childSessionId) :
    let published := claim (claim registry left) right
    published left.subject = some left ∧
      published right.subject = some right := by
  have hDistinctSubject : left.subject ≠ right.subject := by
    intro hEqual
    apply hDistinctChild
    exact congrArg ChildSubject.childSessionId hEqual
  simp [claim, hDistinctSubject]

theorem one_root_can_retain_two_children_with_the_same_canonical_agent
    (registry : Registry)
    (left right : Registration)
    (hSameRoot : left.subject.rootSessionId = right.subject.rootSessionId)
    (_hSameAgent : left.canonicalAgent = right.canonicalAgent)
    (hDistinctChild : left.subject.childSessionId ≠ right.subject.childSessionId) :
    let published := claim (claim registry left) right
    published left.subject = some left ∧
      published right.subject = some right := by
  exact one_root_can_retain_two_independent_child_sessions
    registry left right hSameRoot hDistinctChild

theorem distinct_child_claims_preserve_generation_and_subject_uniqueness
    (registry : Registry)
    (left right : Registration)
    (hLeftPositive : 0 < left.generation)
    (hRightPositive : 0 < right.generation)
    (hDistinct : left.subject ≠ right.subject) :
    let published := claim (claim registry left) right
    published left.subject = some left ∧
      published right.subject = some right ∧
      0 < left.generation ∧
      0 < right.generation := by
  simp [claim, hDistinct, hLeftPositive, hRightPositive]

structure ResidentAuthority where
  artifactDigest : Nat
  publicationNonce : Nat
  appliedArtifactDigest : Nat
  appliedPublicationNonce : Nat
  launcherArtifactDigest : Nat
  launcherPublicationNonce : Nat
  endpointArtifactDigest : Nat
  endpointPublicationNonce : Nat
  launcherAbsolute : Bool
  spawnArgvPresent : Bool
  endpointsSameTransaction : Bool
  drainValid : Bool
  deriving DecidableEq

structure RuntimeStatusAuthority where
  healthy : Bool
  runtimeDigestMatches : Bool
  residentTransaction : Option ResidentAuthority
  deriving DecidableEq

def legacyRuntimeStatusAccepts (status : RuntimeStatusAuthority) : Bool :=
  status.healthy && status.runtimeDigestMatches

def residentAuthorityValid (authority : ResidentAuthority) : Bool :=
  authority.artifactDigest > 0 &&
    authority.publicationNonce > 0 &&
    authority.artifactDigest == authority.appliedArtifactDigest &&
    authority.publicationNonce == authority.appliedPublicationNonce &&
    authority.artifactDigest == authority.launcherArtifactDigest &&
    authority.publicationNonce == authority.launcherPublicationNonce &&
    authority.artifactDigest == authority.endpointArtifactDigest &&
    authority.publicationNonce == authority.endpointPublicationNonce &&
    authority.launcherAbsolute &&
    authority.spawnArgvPresent &&
    authority.endpointsSameTransaction &&
    authority.drainValid

def currentV1RuntimeStatusAccepts (status : RuntimeStatusAuthority) : Bool :=
  status.healthy && status.runtimeDigestMatches &&
    match status.residentTransaction with
    | some authority => residentAuthorityValid authority
    | none => false

theorem old_healthy_matching_digest_without_transaction_is_false_positive :
    let status : RuntimeStatusAuthority := {
      healthy := true
      runtimeDigestMatches := true
      residentTransaction := none
    }
    legacyRuntimeStatusAccepts status = true ∧
      currentV1RuntimeStatusAccepts status = false := by
  decide

theorem old_healthy_matching_digest_cross_publication_is_false_positive :
    let status : RuntimeStatusAuthority := {
      healthy := true
      runtimeDigestMatches := true
      residentTransaction := some {
        artifactDigest := 2
        publicationNonce := 2
        appliedArtifactDigest := 2
        appliedPublicationNonce := 2
        launcherArtifactDigest := 2
        launcherPublicationNonce := 1
        endpointArtifactDigest := 2
        endpointPublicationNonce := 2
        launcherAbsolute := true
        spawnArgvPresent := true
        endpointsSameTransaction := true
        drainValid := true
      }
    }
    legacyRuntimeStatusAccepts status = true ∧
      currentV1RuntimeStatusAccepts status = false := by
  decide

theorem current_v1_healthy_requires_resident_transaction
    (status : RuntimeStatusAuthority)
    (hCurrent : currentV1RuntimeStatusAccepts status = true) :
    ∃ authority, status.residentTransaction = some authority ∧
      residentAuthorityValid authority = true := by
  cases hResident : status.residentTransaction with
  | none =>
      simp [currentV1RuntimeStatusAccepts, hResident] at hCurrent
  | some authority =>
      refine ⟨authority, rfl, ?_⟩
      have hAll :
          (status.healthy = true ∧ status.runtimeDigestMatches = true) ∧
            residentAuthorityValid authority = true := by
        simpa [currentV1RuntimeStatusAccepts, hResident, Bool.and_eq_true] using hCurrent
      exact hAll.2

end ASPProof.HookRegistrationDeadlock
