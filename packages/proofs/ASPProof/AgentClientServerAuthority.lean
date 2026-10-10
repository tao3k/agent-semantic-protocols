-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Std

namespace ASPProof.AgentClientServerAuthority

structure AuthorityKey where
  workspaceIdentity : String
  generationDigest : String
  rootDigest : String
  providerCatalogDigest : String
  languageId : String
  providerId : String
  projectionKind : String
  requestDigest : String
  overlayIdentity : Option String
deriving DecidableEq

inductive SearchProducerAxis where
  | language
  | document
deriving DecidableEq

structure SearchSchemaProfile where
  producerId : String
  searchProducerAxes : List SearchProducerAxis
deriving DecidableEq

structure RuntimeSearchProviderRegistration where
  producerId : String
  providerId : String
  searchEnabled : Bool
deriving DecidableEq

def SearchProducerAdmitted
    (profiles : List SearchSchemaProfile)
    (registrations : List RuntimeSearchProviderRegistration)
    (axis : SearchProducerAxis)
    (authority : AuthorityKey) : Prop :=
  (∃ profile ∈ profiles,
      profile.producerId = authority.languageId ∧
      axis ∈ profile.searchProducerAxes) ∧
  (∃ registration ∈ registrations,
      registration.producerId = authority.languageId ∧
      registration.providerId = authority.providerId ∧
      registration.searchEnabled = true)

theorem provider_registration_without_schema_profile_is_not_admitted
    (registrations : List RuntimeSearchProviderRegistration)
    (axis : SearchProducerAxis)
    (authority : AuthorityKey) :
    ¬ SearchProducerAdmitted [] registrations axis authority := by
  simp [SearchProducerAdmitted]

theorem schema_profile_on_wrong_axis_is_not_admitted
    (profile : SearchSchemaProfile)
    (registrations : List RuntimeSearchProviderRegistration)
    (axis : SearchProducerAxis)
    (authority : AuthorityKey)
    (sameProducer : profile.producerId = authority.languageId)
    (wrongAxis : axis ∉ profile.searchProducerAxes) :
    ¬ SearchProducerAdmitted [profile] registrations axis authority := by
  simp [SearchProducerAdmitted, sameProducer, wrongAxis]

theorem schema_profile_without_runtime_provider_is_not_admitted
    (profiles : List SearchSchemaProfile)
    (axis : SearchProducerAxis)
    (authority : AuthorityKey) :
    ¬ SearchProducerAdmitted profiles [] axis authority := by
  simp [SearchProducerAdmitted]

theorem schema_profile_and_runtime_provider_admit_exact_axis
    (axis : SearchProducerAxis)
    (authority : AuthorityKey) :
    SearchProducerAdmitted
      [{ producerId := authority.languageId, searchProducerAxes := [axis] }]
      [{ producerId := authority.languageId,
         providerId := authority.providerId,
         searchEnabled := true }]
      axis
      authority := by
  simp [SearchProducerAdmitted]

structure CanonicalSchemaRoot where
  name : String
  digest : String
  deriving DecidableEq

structure PackageSchemaProjection where
  name : String
  sourceDigest : String
  deriving DecidableEq

def PackageSchemaProjection.isCurrent
    (root : CanonicalSchemaRoot)
    (projection : PackageSchemaProjection) : Prop :=
  projection.name = root.name ∧ projection.sourceDigest = root.digest

theorem stale_package_schema_projection_is_not_current
    (root : CanonicalSchemaRoot)
    (projection : PackageSchemaProjection)
    (digestDrift : projection.sourceDigest ≠ root.digest) :
    ¬ projection.isCurrent root := by
  intro current
  exact digestDrift current.2

theorem current_package_schema_projection_has_root_digest
    (root : CanonicalSchemaRoot)
    (projection : PackageSchemaProjection)
    (current : projection.isCurrent root) :
    projection.sourceDigest = root.digest :=
  current.2

structure WorkspaceProviderClosure where
  requiredLanguages : List String
  admittedLanguages : List String

def WorkspaceProviderClosure.ready (closure : WorkspaceProviderClosure) : Prop :=
  ∀ languageId, languageId ∈ closure.requiredLanguages →
    languageId ∈ closure.admittedLanguages

theorem missing_unrequired_provider_does_not_block_workspace
    (closure : WorkspaceProviderClosure)
    (languageId : String)
    (ready : closure.ready)
    (unrequired : languageId ∉ closure.requiredLanguages) :
    closure.ready ∧ languageId ∉ closure.requiredLanguages := by
  exact ⟨ready, unrequired⟩

theorem missing_required_provider_blocks_workspace
    (closure : WorkspaceProviderClosure)
    (languageId : String)
    (required : languageId ∈ closure.requiredLanguages)
    (missing : languageId ∉ closure.admittedLanguages) :
    ¬ closure.ready := by
  intro ready
  exact missing (ready languageId required)

def RuntimeProviderArtifactProjection.ready
    (canonicalProviders storedArtifacts requiredLanguages : List String) : Prop :=
  ∀ languageId, languageId ∈ requiredLanguages →
    languageId ∈ canonicalProviders ∧ languageId ∈ storedArtifacts

theorem retired_artifact_cannot_block_unrelated_workspace
    (canonicalProviders storedArtifacts requiredLanguages : List String)
    (retiredLanguage : String)
    (ready : RuntimeProviderArtifactProjection.ready
      canonicalProviders storedArtifacts requiredLanguages)
    (retired : retiredLanguage ∉ canonicalProviders) :
    RuntimeProviderArtifactProjection.ready
      canonicalProviders storedArtifacts requiredLanguages ∧
      retiredLanguage ∉ requiredLanguages := by
  constructor
  · exact ready
  · intro required
    exact retired (ready retiredLanguage required).1

structure ProviderSchemaReference where
  schemaId : String
  schemaVersion : String

structure ProviderRuntimeOperation where
  operation : String
  requestSchema : ProviderSchemaReference
  responseSchema : ProviderSchemaReference

def ProviderRuntimeOperation.valid (operation : ProviderRuntimeOperation) : Prop :=
  operation.operation ≠ "" ∧
  operation.requestSchema.schemaId ≠ "" ∧
  operation.requestSchema.schemaVersion = "1" ∧
  operation.responseSchema.schemaId ≠ "" ∧
  operation.responseSchema.schemaVersion = "1"

theorem provider_runtime_operation_requires_structured_schema_references
    (operation : ProviderRuntimeOperation)
    (valid : operation.valid) :
    operation.requestSchema.schemaVersion = "1" ∧
    operation.responseSchema.schemaVersion = "1" := by
  exact ⟨valid.2.2.1, valid.2.2.2.2⟩

inductive TerminalStatus where
  | completed
  | cancelled
  | failed
deriving DecidableEq

structure OperationState where
  operationId : String
  authority : AuthorityKey
  terminal : Option TerminalStatus
deriving DecidableEq

inductive OperationEvent where
  | complete
  | cancel
  | fail
deriving DecidableEq

def terminalStatus : OperationEvent → TerminalStatus
  | .complete => .completed
  | .cancel => .cancelled
  | .fail => .failed

def applyEvent
    (targetOperationId : String)
    (event : OperationEvent)
    (operation : OperationState) : OperationState :=
  if operation.operationId != targetOperationId then
    operation
  else
    match operation.terminal with
    | some _ => operation
    | none => { operation with terminal := some (terminalStatus event) }

theorem unrelatedOperationIsIsolated
    (operation : OperationState)
    (event : OperationEvent)
    (targetOperationId : String)
    (different : operation.operationId ≠ targetOperationId) :
    applyEvent targetOperationId event operation = operation := by
  simp [applyEvent, different]

theorem terminalReceiptIsNeverReplaced
    (operation : OperationState)
    (event : OperationEvent)
    (status : TerminalStatus)
    (terminal : operation.terminal = some status) :
    (applyEvent operation.operationId event operation).terminal = some status := by
  simp [applyEvent, terminal]

theorem cancellationPublishesTerminalReceipt
    (operation : OperationState)
    (openOperation : operation.terminal = none) :
    (applyEvent operation.operationId .cancel operation).terminal =
      some .cancelled := by
  simp [applyEvent, openOperation, terminalStatus]

structure CacheEntry where
  authority : AuthorityKey
  resultDigest : String

def CacheHit (requested : AuthorityKey) (entry : CacheEntry) : Prop :=
  requested = entry.authority

theorem cacheHitImpliesCompleteAuthorityEquality
    (requested : AuthorityKey)
    (entry : CacheEntry)
    (hit : CacheHit requested entry) :
    requested = entry.authority :=
  hit

inductive PublicationState where
  | building
  | ready
  | failed
deriving DecidableEq

def readable : PublicationState → Bool
  | .ready => true
  | .building | .failed => false

theorem partialPublicationIsNeverReadable :
    readable .building = false :=
  rfl

theorem failedPublicationIsNeverReadable :
    readable .failed = false :=
  rfl

structure CapabilityContract where
  providerCatalogDigest : String
  operations : List String
deriving DecidableEq

structure ActorState where
  ready : Bool
  contract : CapabilityContract
deriving DecidableEq

def dynamicallyRegister
    (state : ActorState)
    (replacement : CapabilityContract) : ActorState :=
  if state.ready then state else { state with contract := replacement }

theorem readyCapabilityContractIsImmutable
    (state : ActorState)
    (replacement : CapabilityContract)
    (ready : state.ready = true) :
    dynamicallyRegister state replacement = state := by
  simp [dynamicallyRegister, ready]

inductive ClientAction where
  | initialize
  | request (method : String)
  | cancel (requestId : String)
  | shutdown
  | exit
deriving DecidableEq

def clientLaunchesProvider : ClientAction → Bool
  | _ => false

theorem clientActionNeverLaunchesProvider
    (action : ClientAction) :
    clientLaunchesProvider action = false := by
  cases action <;> rfl

structure ClientCatalog where
  generationDigest : String
  workspaceGenerationDigest : String
  methods : List String
deriving DecidableEq

def clientMethodAdmitted
    (catalog : ClientCatalog)
    (catalogGeneration workspaceGeneration method : String) : Bool :=
  catalogGeneration == catalog.generationDigest &&
    workspaceGeneration == catalog.workspaceGenerationDigest &&
    catalog.methods.contains method

theorem methodOutsideCatalogFailsClosed
    (catalog : ClientCatalog)
    (catalogGeneration workspaceGeneration method : String)
    (missing : method ∉ catalog.methods) :
    clientMethodAdmitted catalog catalogGeneration workspaceGeneration method = false := by
  simp [clientMethodAdmitted, missing]

theorem staleClientCatalogGenerationFailsClosed
    (catalog : ClientCatalog)
    (catalogGeneration workspaceGeneration method : String)
    (stale : catalogGeneration ≠ catalog.generationDigest) :
    clientMethodAdmitted catalog catalogGeneration workspaceGeneration method = false := by
  simp [clientMethodAdmitted, stale]

theorem clientCatalogAdmissionCannotLaunchProvider
    (catalog : ClientCatalog)
    (catalogGeneration workspaceGeneration method : String) :
    clientMethodAdmitted catalog catalogGeneration workspaceGeneration method = true →
      clientLaunchesProvider (.request method) = false := by
  intro _
  rfl

inductive ProviderHttpOperationClass where
  | lifecycleProbe
  | generationFrame
deriving DecidableEq

def adapterMayOwnWallClockDeadline : ProviderHttpOperationClass → Bool
  | .lifecycleProbe => true
  | .generationFrame => false

theorem lifecycleProbeMayUseAdapterDeadline :
    adapterMayOwnWallClockDeadline .lifecycleProbe = true :=
  rfl

theorem generationFrameCannotUseAdapterDeadline :
    adapterMayOwnWallClockDeadline .generationFrame = false :=
  rfl

theorem adapterDeadlineCannotRejectGenerationFrame
    (ownsDeadline : adapterMayOwnWallClockDeadline .generationFrame = true) :
    False := by
  simp [adapterMayOwnWallClockDeadline] at ownsDeadline

end ASPProof.AgentClientServerAuthority
