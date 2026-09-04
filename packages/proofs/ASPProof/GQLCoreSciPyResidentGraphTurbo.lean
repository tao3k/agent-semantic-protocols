import ASPProof.SearchRouterInteractiveGraphState
import ASPProof.SearchRouteGraphRouterParetoCostSelection

namespace ASPProof.GQLCoreSciPyResidentGraphTurbo

open ASPProof.SearchRouterInteractiveGraphState

abbrev NodeId := ASPProof.SearchRouterInteractiveGraphState.NodeId
abbrev FactId := ASPProof.SearchRouterInteractiveGraphState.FactId

inductive PythonProposalStatus where
  | candidate
  | proposed
  | heuristic
  | partialResult
  deriving Repr, DecidableEq, BEq

inductive EvidenceAuthority where
  | candidate
  | derived
  | asserted
  | proved
  deriving Repr, DecidableEq, BEq

def pythonProposalAuthority (_status : PythonProposalStatus) : EvidenceAuthority :=
  .candidate

theorem python_proposal_cannot_mint_evidence_authority
    (status : PythonProposalStatus) :
    pythonProposalAuthority status = .candidate := by
  cases status <;> rfl

inductive SearchGenerationConstructionOwner where
  | rustServer
  | pythonWorker
  deriving Repr, DecidableEq, BEq

structure BaseGenerationInputs where
  immutableSnapshot : Bool
  rgAcquisitionComplete : Bool
  nativeSyntaxPlaybookComplete : Bool
  tantivyLexicalComplete : Bool
  residentGraphComplete : Bool
  deriving Repr, DecidableEq, BEq

structure NativeSyntaxPlaybookCoverage where
  owners : Bool
  canonicalSelectors : Bool
  properByteRanges : Bool
  nonEmptyQueryKeys : Bool
  derivedProjections : Bool
  relationsBoundToOwners : Bool
  deriving Repr, DecidableEq, BEq

structure NativeSyntaxSelectorProjection where
  selector : String
  byteStart : Nat
  byteEnd : Nat
  queryKeys : List String
  derivedProjectionDigest : String
  deriving Repr, DecidableEq, BEq

structure NativeSyntaxOwnerProjection where
  ownerPath : String
  contentDigest : String
  selectors : List NativeSyntaxSelectorProjection
  deriving Repr, DecidableEq, BEq

structure NativeSyntaxRelationProjection where
  ownerPath : String
  relationDigest : String
  deriving Repr, DecidableEq, BEq

structure NativeSyntaxUnavailableDiagnostic where
  ownerPath : String
  contentDigest : String
  reasonKind : String
  message : String
  deriving Repr, DecidableEq, BEq

def nativeSyntaxDiagnosticValid
    (diagnostic : NativeSyntaxUnavailableDiagnostic) : Bool :=
  !diagnostic.ownerPath.isEmpty &&
    !diagnostic.contentDigest.isEmpty &&
    diagnostic.reasonKind == "source-syntax-unavailable" &&
    !diagnostic.message.isEmpty

def nativeSyntaxOwnerAccountingComplete
    (selectedOwners : List String)
    (projections : List NativeSyntaxOwnerProjection)
    (diagnostics : List NativeSyntaxUnavailableDiagnostic) : Bool :=
  let projectedOwners : List String := projections.map (·.ownerPath)
  let diagnosedOwners : List String := diagnostics.map (·.ownerPath)
  decide selectedOwners.Nodup &&
    decide projectedOwners.Nodup &&
    decide diagnosedOwners.Nodup &&
    projectedOwners.all (fun owner => !diagnosedOwners.contains owner) &&
    diagnostics.all nativeSyntaxDiagnosticValid &&
    selectedOwners.all (fun owner =>
      projectedOwners.contains owner || diagnosedOwners.contains owner) &&
    projectedOwners.all selectedOwners.contains &&
    diagnosedOwners.all selectedOwners.contains

theorem one_unavailable_owner_can_complete_total_accounting :
    nativeSyntaxOwnerAccountingComplete
      ["src/ready.rs", "src/unavailable.rs"]
      [ { ownerPath := "src/ready.rs"
          contentDigest := "blake3-256:ready"
          selectors := [] } ]
      [ { ownerPath := "src/unavailable.rs"
          contentDigest := "blake3-256:unavailable"
          reasonKind := "source-syntax-unavailable"
          message := "bounded parser diagnostic" } ] = true := by
  decide

theorem unreported_owner_cannot_complete_total_accounting :
    nativeSyntaxOwnerAccountingComplete
      ["src/ready.rs", "src/missing.rs"]
      [ { ownerPath := "src/ready.rs"
          contentDigest := "blake3-256:ready"
          selectors := [] } ]
      [] = false := by
  decide

def independentSearchEvidenceSurvivesNativeDiagnostic
    (rg lexical graph : Bool)
    (_diagnostics : List NativeSyntaxUnavailableDiagnostic) : Bool :=
  rg && lexical && graph

theorem one_native_diagnostic_cannot_revoke_independent_search_evidence :
    independentSearchEvidenceSurvivesNativeDiagnostic true true true
      [ { ownerPath := "src/unavailable.rs"
          contentDigest := "blake3-256:unavailable"
          reasonKind := "source-syntax-unavailable"
          message := "bounded parser diagnostic" } ] = true := by
  rfl

def nativeSyntaxSelectorProjectionValid
    (selector : NativeSyntaxSelectorProjection) : Bool :=
  !selector.selector.isEmpty &&
    decide (selector.byteStart < selector.byteEnd) &&
    !selector.queryKeys.isEmpty &&
    selector.queryKeys.all (fun key => !key.isEmpty) &&
    !selector.derivedProjectionDigest.isEmpty

def nativeSyntaxProjectionValid
    (projections : List NativeSyntaxOwnerProjection)
    (relations : List NativeSyntaxRelationProjection) : Bool :=
  decide (projections.map (·.ownerPath)).Nodup &&
    projections.all (fun projection =>
      !projection.ownerPath.isEmpty &&
        !projection.contentDigest.isEmpty &&
        !projection.selectors.isEmpty &&
        decide (projection.selectors.map (·.selector)).Nodup &&
        projection.selectors.all nativeSyntaxSelectorProjectionValid) &&
    decide relations.Nodup &&
    relations.all (fun relation =>
      !relation.relationDigest.isEmpty &&
        projections.any (fun projection => projection.ownerPath == relation.ownerPath))

def ownerOnlyNativeSyntaxProjection : List NativeSyntaxOwnerProjection :=
  [ { ownerPath := "src/router.rs"
      contentDigest := "blake3-256:content"
      selectors := [] } ]

theorem owner_identity_without_parser_projection_is_rejected :
    nativeSyntaxProjectionValid ownerOnlyNativeSyntaxProjection [] = false := by
  decide

def nativeSyntaxCoverageComplete (coverage : NativeSyntaxPlaybookCoverage) : Bool :=
  coverage.owners &&
    coverage.canonicalSelectors &&
    coverage.properByteRanges &&
    coverage.nonEmptyQueryKeys &&
    coverage.derivedProjections &&
    coverage.relationsBoundToOwners

def ownerOnlyNativeSyntaxCoverage : NativeSyntaxPlaybookCoverage :=
  { owners := true
    canonicalSelectors := false
    properByteRanges := false
    nonEmptyQueryKeys := false
    derivedProjections := false
    relationsBoundToOwners := false }

def completeNativeSyntaxPlaybookCoverage : NativeSyntaxPlaybookCoverage :=
  { owners := true
    canonicalSelectors := true
    properByteRanges := true
    nonEmptyQueryKeys := true
    derivedProjections := true
    relationsBoundToOwners := true }

theorem owner_only_lookup_cannot_complete_native_syntax_playbook :
    nativeSyntaxCoverageComplete ownerOnlyNativeSyntaxCoverage = false := by
  decide

inductive SearchPipelineStage where
  | rgAcquisition
  | nativeSyntaxPlaybook
  | tantivyLexical
  | residentGraph
  deriving Repr, DecidableEq, BEq

def searchPipelineOrderAdmitted (stages : List SearchPipelineStage) : Bool :=
  stages == [.rgAcquisition, .nativeSyntaxPlaybook, .tantivyLexical, .residentGraph]

theorem canonical_search_pipeline_order_is_admitted :
    searchPipelineOrderAdmitted
      [.rgAcquisition, .nativeSyntaxPlaybook, .tantivyLexical, .residentGraph] = true := by
  decide

theorem peer_lane_or_reversed_search_composition_is_rejected :
    searchPipelineOrderAdmitted
      [.rgAcquisition, .tantivyLexical, .nativeSyntaxPlaybook, .residentGraph] = false := by
  decide

/- A ClientFrame terminal is bound to the admitted source snapshot root.
The owner-index Merkle root is a derived attachment identity and cannot be
substituted even when both values are valid BLAKE3 digests. -/
structure SearchGenerationRootIdentity where
  sourceRootDigest : String
  ownerIndexRootDigest : String
  deriving Repr, DecidableEq, BEq

def clientTerminalRootAdmitted
    (identity : SearchGenerationRootIdentity)
    (terminalRootDigest : String) : Bool :=
  terminalRootDigest == identity.sourceRootDigest

def distinctSearchGenerationRoots : SearchGenerationRootIdentity :=
  { sourceRootDigest := "source-root"
    ownerIndexRootDigest := "owner-index-root" }

theorem admitted_source_root_is_a_valid_client_terminal_identity :
    clientTerminalRootAdmitted distinctSearchGenerationRoots
      distinctSearchGenerationRoots.sourceRootDigest = true := by
  rfl

theorem derived_owner_index_root_cannot_replace_source_root_in_client_terminal :
    clientTerminalRootAdmitted distinctSearchGenerationRoots
      distinctSearchGenerationRoots.ownerIndexRootDigest = false := by
  decide

def searchPipelineStageDirectlyToolAddressable (_stage : SearchPipelineStage) : Bool :=
  false

theorem internal_search_stages_are_not_public_tool_calls :
    searchPipelineStageDirectlyToolAddressable .rgAcquisition = false ∧
      searchPipelineStageDirectlyToolAddressable .nativeSyntaxPlaybook = false ∧
      searchPipelineStageDirectlyToolAddressable .tantivyLexical = false ∧
      searchPipelineStageDirectlyToolAddressable .residentGraph = false := by
  decide

def baseGenerationReady
    (owner : SearchGenerationConstructionOwner)
    (inputs : BaseGenerationInputs) : Bool :=
  owner == .rustServer &&
    inputs.immutableSnapshot &&
    inputs.rgAcquisitionComplete &&
    inputs.nativeSyntaxPlaybookComplete &&
    inputs.tantivyLexicalComplete &&
    inputs.residentGraphComplete

def completeBaseGenerationInputs : BaseGenerationInputs :=
  { immutableSnapshot := true
    rgAcquisitionComplete := true
    nativeSyntaxPlaybookComplete :=
      nativeSyntaxCoverageComplete completeNativeSyntaxPlaybookCoverage
    tantivyLexicalComplete := true
    residentGraphComplete := true }

theorem rust_is_the_only_base_generation_construction_owner :
    baseGenerationReady .rustServer completeBaseGenerationInputs = true := by
  decide

theorem python_worker_cannot_publish_a_base_generation :
    baseGenerationReady .pythonWorker completeBaseGenerationInputs = false := by
  decide

def baseGenerationReadyWithPython (_pythonAvailable : Bool) : Bool :=
  baseGenerationReady .rustServer completeBaseGenerationInputs

theorem python_graph_outage_cannot_block_base_generation_commit :
    baseGenerationReadyWithPython false = true := by
  decide

theorem python_graph_availability_does_not_change_base_generation_identity :
    baseGenerationReadyWithPython false = baseGenerationReadyWithPython true := by
  decide

inductive PublicSearchOperation where
  | playbook
  | prime
  | pipe
  | lexical
  | ownerSearch
  deriving Repr, DecidableEq, BEq

def publicSearchOperationAdmitted : PublicSearchOperation -> Bool
  | .playbook => true
  | _ => false

theorem playbook_is_the_only_public_search_operation :
    publicSearchOperationAdmitted .playbook = true := by
  rfl

def admittedPublicSearchOperations : List PublicSearchOperation :=
  [.playbook, .prime, .pipe, .lexical, .ownerSearch]
    |>.filter publicSearchOperationAdmitted

theorem public_search_tool_call_branching_factor_is_one :
    admittedPublicSearchOperations.length = 1 := by
  decide

theorem retired_search_operations_cannot_form_a_second_authority :
    publicSearchOperationAdmitted .prime = false ∧
      publicSearchOperationAdmitted .pipe = false ∧
      publicSearchOperationAdmitted .lexical = false ∧
      publicSearchOperationAdmitted .ownerSearch = false := by
  decide

inductive HookSearchRouteKind where
  | playbook
  | prime
  | owner
  | lexical
  | ingest
  deriving Repr, DecidableEq, BEq

def hookSearchRouteAdmitted : HookSearchRouteKind -> Bool
  | .playbook => true
  | _ => false

theorem hook_cannot_reintroduce_a_retired_search_authority :
    hookSearchRouteAdmitted .prime = false ∧
      hookSearchRouteAdmitted .owner = false ∧
      hookSearchRouteAdmitted .lexical = false ∧
      hookSearchRouteAdmitted .ingest = false := by
  decide

inductive PublicSearchExecutionOwner where
  | searchPackage
  | languageProviderCli
  deriving Repr, DecidableEq, BEq

def publicSearchExecutionOwnerAdmitted : PublicSearchExecutionOwner -> Bool
  | .searchPackage => true
  | .languageProviderCli => false

theorem language_provider_cli_cannot_form_a_parallel_search_workflow :
    publicSearchExecutionOwnerAdmitted .languageProviderCli = false := by
  rfl

inductive SearchConstructionInput where
  | sourceDocument
  | byteCoverageInput
  | graphEntryNode
  deriving Repr, DecidableEq, BEq

def isGraphInternalInput : SearchConstructionInput -> Bool
  | .graphEntryNode => true
  | _ => false

theorem source_inputs_cannot_be_reinterpreted_as_graph_entry_nodes :
    isGraphInternalInput .sourceDocument = false ∧
      isGraphInternalInput .byteCoverageInput = false := by
  decide

inductive RuntimeObservation where
  | endpointReachable
  | transactionIdentityBound
  | hostPermissionDenied
  deriving Repr, DecidableEq, BEq

def runtimeReady : RuntimeObservation -> Bool
  | .transactionIdentityBound => true
  | _ => false

theorem endpoint_reachability_is_not_runtime_readiness :
    runtimeReady .endpointReachable = false := by
  rfl

theorem host_permission_denial_cannot_prove_runtime_termination :
    runtimeReady .hostPermissionDenied = false := by
  rfl

inductive BootstrapObservation where
  | spawnAccepted
  | healthyTransaction
  | failedTerminal
  | cancelledTerminal
  deriving Repr, DecidableEq, BEq

def bootstrapTerminal : BootstrapObservation -> Bool
  | .spawnAccepted => false
  | .healthyTransaction => true
  | .failedTerminal => true
  | .cancelledTerminal => true

theorem spawn_acceptance_is_not_a_search_terminal :
    bootstrapTerminal .spawnAccepted = false := by
  rfl

inductive EndpointIdentityAuthority where
  | runtimePublished
  | clientDerivedStateHomePath
  | filesystemProbe
  deriving Repr, DecidableEq, BEq

def endpointIdentityAdmitted : EndpointIdentityAuthority -> Bool
  | .runtimePublished => true
  | .clientDerivedStateHomePath => false
  | .filesystemProbe => false

theorem client_derived_endpoint_path_cannot_authorize_search :
    endpointIdentityAdmitted .clientDerivedStateHomePath = false := by
  rfl

structure SchemaFamilyMembership where
  registered : Bool
  schemaPresent : Bool
  deriving Repr, DecidableEq, BEq

def schemaFamilyMembershipAdmitted (membership : SchemaFamilyMembership) : Bool :=
  membership.registered && membership.schemaPresent

theorem deleted_schema_cannot_remain_an_admitted_family_member :
    schemaFamilyMembershipAdmitted
      { registered := true, schemaPresent := false } = false := by
  rfl

inductive RuntimeSchemaCatalogAuthority where
  | buildVerifiedEmbeddedBytes
  | mutableCheckoutAtDaemonStartup
  | clientMaterializedCopy
  deriving Repr, DecidableEq, BEq

def runtimeSchemaCatalogAuthorityAdmitted :
    RuntimeSchemaCatalogAuthority -> Bool
  | .buildVerifiedEmbeddedBytes => true
  | .mutableCheckoutAtDaemonStartup => false
  | .clientMaterializedCopy => false

theorem daemon_startup_cannot_admit_mutable_checkout_schema_authority :
    runtimeSchemaCatalogAuthorityAdmitted
      .mutableCheckoutAtDaemonStartup = false := by
  rfl

inductive BuildSchemaResolutionMode where
  | resolveCanonicalBytes
  | materializePackageBundle
  deriving Repr, DecidableEq, BEq

def buildSchemaResolutionAdmitted : BuildSchemaResolutionMode -> Bool
  | .resolveCanonicalBytes => true
  | .materializePackageBundle => false

theorem cargo_build_cannot_materialize_a_downstream_schema_bundle :
    buildSchemaResolutionAdmitted .materializePackageBundle = false := by
  rfl

inductive OptionalCapabilityArtifactAuthority where
  | activeRuntimeBundleMember
  | looseStateHomeDescriptor
  | pathLookup
  deriving Repr, DecidableEq, BEq

def optionalCapabilityArtifactAdmitted :
    OptionalCapabilityArtifactAuthority -> Bool
  | .activeRuntimeBundleMember => true
  | .looseStateHomeDescriptor => false
  | .pathLookup => false

theorem active_bundle_member_is_the_only_optional_capability_artifact_authority :
    optionalCapabilityArtifactAdmitted .activeRuntimeBundleMember = true := by
  rfl

theorem loose_descriptor_cannot_authorize_optional_capability_execution :
    optionalCapabilityArtifactAdmitted .looseStateHomeDescriptor = false := by
  rfl

theorem path_lookup_cannot_authorize_optional_capability_execution :
    optionalCapabilityArtifactAdmitted .pathLookup = false := by
  rfl

structure BaseSearchHotPathCost where
  selectedProviderLookups : Nat
  childProcessStarts : Nat
  fullMerkleRebuilds : Nat
  deriving Repr, DecidableEq, BEq

def baseSearchHotPathCost (_installedLanguageCount : Nat) : BaseSearchHotPathCost :=
  { selectedProviderLookups := 1
    childProcessStarts := 0
    fullMerkleRebuilds := 0 }

theorem unselected_languages_do_not_change_base_search_hot_path_cost
    (leftInstalled rightInstalled : Nat) :
    baseSearchHotPathCost leftInstalled = baseSearchHotPathCost rightInstalled := by
  rfl

theorem base_search_hot_path_never_starts_optional_workers
    (installedLanguageCount : Nat) :
    (baseSearchHotPathCost installedLanguageCount).childProcessStarts = 0 := by
  rfl

inductive GenerationAdmissionScope where
  | targeted (languageId : String)
  | completeWorkspace (languageIds : List String)
  deriving Repr, DecidableEq, BEq

def requiredProviderLanguages : GenerationAdmissionScope -> List String
  | .targeted languageId => [languageId]
  | .completeWorkspace languageIds => languageIds

theorem targeted_query_admission_is_independent_of_unselected_languages
    (selected _unselected : String) :
    requiredProviderLanguages (.targeted selected) = [selected] := by
  rfl

theorem complete_generation_is_the_only_cross_language_fan_in
    (languages : List String) :
    requiredProviderLanguages (.completeWorkspace languages) = languages := by
  rfl

inductive ProjectGenerationIsolationKey where
  | canonicalWorkspaceId
  | workspaceIdPlusAbsoluteRoot
  deriving Repr, DecidableEq, BEq

def projectGenerationIsolationKeyAdmitted : ProjectGenerationIsolationKey -> Bool
  | .canonicalWorkspaceId => true
  | .workspaceIdPlusAbsoluteRoot => false

theorem absolute_root_cannot_create_a_second_workspace_partition :
    projectGenerationIsolationKeyAdmitted .workspaceIdPlusAbsoluteRoot = false := by
  rfl

structure ProviderGenerationMembership where
  languageId : String
  providerId : String
  deriving Repr, DecidableEq, BEq

inductive ProviderEvidenceKind where
  | owner
  | relation
  | projectResolution
  deriving Repr, DecidableEq, BEq

structure ProviderEvidenceMembership where
  authority : ProviderGenerationMembership
  kind : ProviderEvidenceKind
  deriving Repr, DecidableEq, BEq

def replaceProviderEvidence
    (active replacement : List ProviderEvidenceMembership)
    (target : ProviderGenerationMembership) : List ProviderEvidenceMembership :=
  (active.filter fun evidence => decide (evidence.authority ≠ target)) ++
    (replacement.filter fun evidence => decide (evidence.authority = target))

theorem targeted_replacement_preserves_unselected_project_resolution
    (active replacement : List ProviderEvidenceMembership)
    (target unselected : ProviderGenerationMembership)
    (present : ProviderEvidenceMembership.mk unselected .projectResolution ∈ active)
    (different : unselected ≠ target) :
    ProviderEvidenceMembership.mk unselected .projectResolution ∈
      replaceProviderEvidence active replacement target := by
  simp [replaceProviderEvidence, different, present]

def replaceProviderMembership
    (active replacement : List ProviderGenerationMembership)
    (target : ProviderGenerationMembership) : List ProviderGenerationMembership :=
  (active.filter fun member => decide (member ≠ target)) ++
    (replacement.filter fun member => decide (member = target))

theorem targeted_replacement_preserves_unselected_provider_membership
    (active replacement : List ProviderGenerationMembership)
    (target unselected : ProviderGenerationMembership)
    (different : unselected ≠ target)
    (present : unselected ∈ active) :
    unselected ∈ replaceProviderMembership active replacement target := by
  simp [replaceProviderMembership, different, present]

inductive ProviderDemandPublication where
  | selectedOnlyGeneration
  | completeSuccessorFromActiveBase
  deriving Repr, DecidableEq, BEq

def providerDemandPublicationAdmitted : ProviderDemandPublication -> Bool
  | .selectedOnlyGeneration => false
  | .completeSuccessorFromActiveBase => true

theorem selected_provider_generation_cannot_replace_complete_active_generation :
    providerDemandPublicationAdmitted .selectedOnlyGeneration = false := by
  rfl

structure GraphSessionIdentity where
  workspaceDigest : String
  sessionId : String
  nodeId : NodeId
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  projectionDigest : String
  routeDigest : String
  deriving Repr, DecidableEq, BEq

def graphSessionIdentityValid (identity : GraphSessionIdentity) : Bool :=
  !identity.workspaceDigest.isEmpty &&
    !identity.sessionId.isEmpty &&
    !identity.nodeId.isEmpty &&
    !identity.snapshotDigest.isEmpty &&
    !identity.workspaceGenerationRootDigest.isEmpty &&
    !identity.projectionDigest.isEmpty &&
    !identity.routeDigest.isEmpty

def sameGraphSession
    (expected observed : GraphSessionIdentity) : Bool :=
  graphSessionIdentityValid expected && expected == observed

def graphSessionA : GraphSessionIdentity :=
  { workspaceDigest := "workspace:1"
    sessionId := "session:a"
    nodeId := "node:owner"
    snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    projectionDigest := "projection:owner-v1"
    routeDigest := "route:owner-v1" }

def graphSessionB : GraphSessionIdentity :=
  { graphSessionA with sessionId := "session:b" }

theorem exact_graph_session_identity_is_admitted :
    sameGraphSession graphSessionA graphSessionA = true := by
  decide

theorem cross_session_substitution_is_rejected :
    sameGraphSession graphSessionA graphSessionB = false := by
  decide

structure DecodedHopAccounting where
  semanticGraphHops : Nat
  executedGraphHops : Nat
  deriving Repr, DecidableEq, BEq

def validateDecodedHopAccounting
    (decoded : DecodedHopAccounting) : Option DecodedHopAccounting :=
  if decoded.executedGraphHops <= decoded.semanticGraphHops then
    some decoded
  else
    none

theorem valid_deserialized_hop_accounting_is_admitted :
    validateDecodedHopAccounting
      { semanticGraphHops := 3, executedGraphHops := 1 } =
      some { semanticGraphHops := 3, executedGraphHops := 1 } := by
  decide

theorem executed_hops_above_semantic_hops_are_rejected :
    validateDecodedHopAccounting
      { semanticGraphHops := 1, executedGraphHops := 2 } = none := by
  decide

structure InteractiveCostAccounting where
  semanticGraphHops : Nat
  executedGraphHops : Nat
  toolActions : Nat
  llmRounds : Nat
  inputTokens : Nat
  outputTokens : Nat
  searchExecutions : Nat
  modelPrefixRecomputations : Nat
  ruleFirings : Nat
  derivedFacts : Nat
  deriving Repr, DecidableEq, BEq

def accountingWellFormed (accounting : InteractiveCostAccounting) : Bool :=
  accounting.executedGraphHops <= accounting.semanticGraphHops

def cacheResumeAdmitted
    (expectedIdentity observedIdentity : GraphSessionIdentity)
    (cold cached : InteractiveCostAccounting) : Bool :=
  sameGraphSession expectedIdentity observedIdentity &&
    accountingWellFormed cold &&
    accountingWellFormed cached &&
    cached.semanticGraphHops == cold.semanticGraphHops &&
    cached.executedGraphHops <= cold.executedGraphHops &&
    cached.toolActions <= cold.toolActions &&
    cached.llmRounds <= cold.llmRounds &&
    cached.searchExecutions <= cold.searchExecutions &&
    cached.modelPrefixRecomputations <= cold.modelPrefixRecomputations

def coldAccounting : InteractiveCostAccounting :=
  { semanticGraphHops := 3
    executedGraphHops := 3
    toolActions := 4
    llmRounds := 3
    inputTokens := 1200
    outputTokens := 240
    searchExecutions := 2
    modelPrefixRecomputations := 2
    ruleFirings := 8
    derivedFacts := 5 }

def cachedAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    executedGraphHops := 1
    toolActions := 2
    llmRounds := 1
    searchExecutions := 0
    modelPrefixRecomputations := 0 }

theorem cache_resume_preserves_semantic_route_and_may_reduce_realization :
    cacheResumeAdmitted graphSessionA graphSessionA coldAccounting cachedAccounting = true := by
  decide

def forgedSemanticAccounting : InteractiveCostAccounting :=
  { cachedAccounting with semanticGraphHops := 1 }

theorem cache_resume_cannot_rewrite_semantic_hops :
    cacheResumeAdmitted graphSessionA graphSessionA
      coldAccounting forgedSemanticAccounting = false := by
  decide

def searchCacheOnlyAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    searchExecutions := 0
    modelPrefixRecomputations := 1 }

def modelCacheOnlyAccounting : InteractiveCostAccounting :=
  { coldAccounting with
    searchExecutions := 1
    modelPrefixRecomputations := 0 }

def combinedCacheMisses (accounting : InteractiveCostAccounting) : Nat :=
  accounting.searchExecutions + accounting.modelPrefixRecomputations

theorem equal_combined_cache_work_can_hide_orthogonal_cache_lanes :
    combinedCacheMisses searchCacheOnlyAccounting =
      combinedCacheMisses modelCacheOnlyAccounting := by
  decide

theorem search_cache_reuse_does_not_imply_model_prefix_reuse :
    searchCacheOnlyAccounting.searchExecutions = 0 ∧
      searchCacheOnlyAccounting.modelPrefixRecomputations = 1 := by
  decide

theorem model_prefix_reuse_does_not_imply_search_cache_reuse :
    modelCacheOnlyAccounting.modelPrefixRecomputations = 0 ∧
      modelCacheOnlyAccounting.searchExecutions = 1 := by
  decide

structure AscentClosureBudget where
  maxInputFacts : Nat
  maxRuleFirings : Nat
  maxDerivedFacts : Nat
  maxGraphHops : Nat
  deriving Repr, DecidableEq, BEq

structure AscentClosureReceipt where
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  rulesetDigest : String
  sourceFactIds : List FactId
  derivedFactIds : List FactId
  ruleFirings : Nat
  derivedFacts : Nat
  maxDerivedGraphHops : Nat
  canonicalFactOrderDigest : String
  deriving Repr, DecidableEq, BEq

def ascentClosureReceiptAdmitted
    (identity : GraphSessionIdentity)
    (admittedFacts trustedRulesets : List String)
    (budget : AscentClosureBudget)
    (receipt : AscentClosureReceipt) : Bool :=
  graphSessionIdentityValid identity &&
    receipt.snapshotDigest == identity.snapshotDigest &&
    receipt.workspaceGenerationRootDigest ==
      identity.workspaceGenerationRootDigest &&
    trustedRulesets.contains receipt.rulesetDigest &&
    receipt.sourceFactIds.all admittedFacts.contains &&
    receipt.sourceFactIds.length <= budget.maxInputFacts &&
    receipt.ruleFirings <= budget.maxRuleFirings &&
    receipt.derivedFacts == receipt.derivedFactIds.length &&
    receipt.derivedFacts <= budget.maxDerivedFacts &&
    receipt.maxDerivedGraphHops <= budget.maxGraphHops &&
    !receipt.canonicalFactOrderDigest.isEmpty

def ascentBudget : AscentClosureBudget :=
  { maxInputFacts := 8
    maxRuleFirings := 16
    maxDerivedFacts := 8
    maxGraphHops := 4 }

def admittedAscentReceipt : AscentClosureReceipt :=
  { snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    rulesetDigest := "ruleset:owner-v1"
    sourceFactIds := ["fact:model-declared"]
    derivedFactIds := ["fact:configuration-owner"]
    ruleFirings := 3
    derivedFacts := 1
    maxDerivedGraphHops := 2
    canonicalFactOrderDigest := "facts:canonical:1" }

theorem bounded_ascent_receipt_is_snapshot_scoped_and_grounded :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget admittedAscentReceipt = true := by
  decide

structure NamedAscentRule where
  ruleId : String
  premiseFactIds : List FactId
  conclusionFactId : FactId
  deriving Repr, DecidableEq, BEq

structure AscentDerivationStep where
  ruleId : String
  premiseFactIds : List FactId
  conclusionFactId : FactId
  deriving Repr, DecidableEq, BEq

def ruleClosedOverFacts (facts : List FactId) (rule : NamedAscentRule) : Bool :=
  !rule.premiseFactIds.all facts.contains || facts.contains rule.conclusionFactId

def receiptIsFixedPoint
    (rules : List NamedAscentRule)
    (receipt : AscentClosureReceipt) : Bool :=
  let facts := receipt.sourceFactIds ++ receipt.derivedFactIds
  rules.all (ruleClosedOverFacts facts)

def derivationStepGrounded
    (rules : List NamedAscentRule)
    (facts : List FactId)
    (step : AscentDerivationStep) : Bool :=
  rules.any fun rule =>
    rule.ruleId == step.ruleId &&
      rule.premiseFactIds == step.premiseFactIds &&
      rule.conclusionFactId == step.conclusionFactId &&
      step.premiseFactIds.all facts.contains &&
      !facts.contains step.conclusionFactId

def replayDerivationTrace
    (rules : List NamedAscentRule) :
    List FactId → List AscentDerivationStep → Option (List FactId)
  | facts, [] => some facts
  | facts, step :: rest =>
      if derivationStepGrounded rules facts step then
        replayDerivationTrace rules (facts ++ [step.conclusionFactId]) rest
      else
        none

def derivationTraceGrounded
    (rules : List NamedAscentRule)
    (receipt : AscentClosureReceipt)
    (trace : List AscentDerivationStep) : Bool :=
  match replayDerivationTrace rules receipt.sourceFactIds trace with
  | none => false
  | some replayedFacts =>
      trace.length == receipt.ruleFirings &&
        replayedFacts == receipt.sourceFactIds ++ receipt.derivedFactIds

def ascentClosureVerifiedAdmitted
    (identity : GraphSessionIdentity)
    (admittedFacts trustedRulesets : List String)
    (budget : AscentClosureBudget)
    (expectedCanonicalFactOrderDigest : String)
    (rules : List NamedAscentRule)
    (trace : List AscentDerivationStep)
    (receipt : AscentClosureReceipt) : Bool :=
  ascentClosureReceiptAdmitted identity admittedFacts trustedRulesets budget receipt &&
    receipt.canonicalFactOrderDigest == expectedCanonicalFactOrderDigest &&
    derivationTraceGrounded rules receipt trace &&
    receiptIsFixedPoint rules receipt

def ownerRule : NamedAscentRule :=
  { ruleId := "rule:model-to-configuration-owner"
    premiseFactIds := ["fact:model-declared"]
    conclusionFactId := "fact:configuration-owner" }

def ownerDerivationStep : AscentDerivationStep :=
  { ruleId := ownerRule.ruleId
    premiseFactIds := ownerRule.premiseFactIds
    conclusionFactId := ownerRule.conclusionFactId }

def traceGroundedAscentReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with ruleFirings := 1 }

theorem recomputed_ascent_closure_is_grounded_complete_and_canonical :
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [ownerDerivationStep]
      traceGroundedAscentReceipt = true := by
  decide

def incompleteAscentReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with
    derivedFactIds := []
    ruleFirings := 0
    derivedFacts := 0 }

theorem budget_only_admission_can_accept_a_non_fixed_point_receipt :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget incompleteAscentReceipt = true ∧
    receiptIsFixedPoint [ownerRule] incompleteAscentReceipt = false := by
  decide

theorem verified_admission_rejects_a_non_fixed_point_receipt :
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [] incompleteAscentReceipt = false := by
  decide

def forgedCanonicalDigestReceipt : AscentClosureReceipt :=
  { traceGroundedAscentReceipt with
    canonicalFactOrderDigest := "facts:caller-invented" }

theorem nonempty_digest_check_does_not_establish_canonical_order :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      forgedCanonicalDigestReceipt = true ∧
    ascentClosureVerifiedAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"] ascentBudget
      "facts:canonical:1" [ownerRule] [ownerDerivationStep]
      forgedCanonicalDigestReceipt = false := by
  decide

theorem scalar_rule_firing_count_does_not_ground_a_derivation :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget admittedAscentReceipt = true ∧
    derivationTraceGrounded [ownerRule] admittedAscentReceipt
      [ownerDerivationStep] = false := by
  decide

def untrustedRulesetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with rulesetDigest := "ruleset:model-injected" }

theorem model_injected_ascent_ruleset_is_rejected :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget untrustedRulesetReceipt = false := by
  decide

def overRuleBudgetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with ruleFirings := 17 }

theorem ascent_rule_firing_budget_is_independent_and_fail_closed :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget overRuleBudgetReceipt = false := by
  decide

def overDerivedFactBudgetReceipt : AscentClosureReceipt :=
  { admittedAscentReceipt with
    derivedFactIds :=
      ["fact:1", "fact:2", "fact:3", "fact:4", "fact:5",
       "fact:6", "fact:7", "fact:8", "fact:9"]
    derivedFacts := 9 }

theorem ascent_derived_fact_budget_is_independent_and_fail_closed :
    ascentClosureReceiptAdmitted graphSessionA
      ["fact:model-declared"] ["ruleset:owner-v1"]
      ascentBudget overDerivedFactBudgetReceipt = false := by
  decide

structure VerifiedGraphDeltaReceipt where
  beforeStateDigest : String
  afterStateDigest : String
  snapshotDigest : String
  workspaceGenerationRootDigest : String
  closureReceiptDigest : String
  exactVerificationDigest : String
  derivedFactIds : List FactId
  deriving Repr, DecidableEq, BEq

def graphDeltaAdmissionValid
    (identity : GraphSessionIdentity)
    (closure : AscentClosureReceipt)
    (delta : VerifiedGraphDeltaReceipt) : Bool :=
  delta.beforeStateDigest == identity.routeDigest &&
    delta.afterStateDigest != delta.beforeStateDigest &&
    delta.snapshotDigest == identity.snapshotDigest &&
    delta.workspaceGenerationRootDigest ==
      identity.workspaceGenerationRootDigest &&
    !delta.closureReceiptDigest.isEmpty &&
    !delta.exactVerificationDigest.isEmpty &&
    delta.derivedFactIds == closure.derivedFactIds

def admittedGraphDelta : VerifiedGraphDeltaReceipt :=
  { beforeStateDigest := "route:owner-v1"
    afterStateDigest := "route:owner-v2"
    snapshotDigest := "snapshot:7"
    workspaceGenerationRootDigest := "generation:7"
    closureReceiptDigest := "closure:1"
    exactVerificationDigest := "verification:1"
    derivedFactIds := ["fact:configuration-owner"] }

theorem verified_graph_delta_can_admit_bounded_closure_output :
    graphDeltaAdmissionValid graphSessionA
      admittedAscentReceipt admittedGraphDelta = true := by
  decide

def closureReceiptAloneAuthorizesMutation
    (_receipt : AscentClosureReceipt) : Bool := false

theorem ascent_receipt_alone_cannot_mutate_evidence_graph :
    closureReceiptAloneAuthorizesMutation admittedAscentReceipt = false := by
  rfl

def pythonProposalAloneAuthorizesMutation
    (_status : PythonProposalStatus) : Bool := false

theorem python_proposal_alone_cannot_mutate_evidence_graph
    (status : PythonProposalStatus) :
    pythonProposalAloneAuthorizesMutation status = false := by
  rfl

inductive GqlPathBound where
  | explicit (hops : Nat)
  | routerSupplied (hops : Nat)
  | unbounded
  deriving Repr, DecidableEq, BEq

def gqlPathAdmitted (maxGraphHops : Nat) (bound : GqlPathBound) : Bool :=
  match bound with
  | .explicit hops => hops <= maxGraphHops
  | .routerSupplied hops => hops <= maxGraphHops
  | .unbounded => false

theorem router_bounded_gql_path_is_admitted :
    gqlPathAdmitted 4 (.routerSupplied 3) = true := by
  decide

theorem unbounded_gql_path_is_fail_closed :
    gqlPathAdmitted 4 .unbounded = false := by
  rfl

abbrev RouteCost :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.RouteCost

abbrev RouteCaps :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.RouteCost

def routeFeasible (cost caps : RouteCost) : Prop :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.Feasible cost caps

def routeNoWorse (left right : RouteCost) : Prop :=
  ASPProof.SearchRouteGraphRouterParetoCostSelection.NoWorse left right

theorem pareto_improvement_preserves_hard_budget_feasibility
    (better worse caps : RouteCost)
    (noWorse : routeNoWorse better worse)
    (feasible : routeFeasible worse caps) :
    routeFeasible better caps := by
  exact
    ASPProof.SearchRouteGraphRouterParetoCostSelection.no_worse_route_preserves_feasibility
      better worse caps noWorse feasible

theorem shortest_hop_order_requires_prior_token_and_round_feasibility
    (tokenCap : Nat) :
    let unsafeRoute : RouteCost :=
      { graphHops := 0
        interactionRounds := 0
        projectedTokens := tokenCap + 1
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    let alternativeRoute : RouteCost :=
      { graphHops := 1
        interactionRounds := 0
        projectedTokens := 0
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    let caps : RouteCost :=
      { graphHops := 1
        interactionRounds := 0
        projectedTokens := tokenCap
        verificationOps := 0
        searchExecutions := 0
        modelPrefixRecomputations := 0 }
    ASPProof.SearchRouteGraphRouterParetoCostSelection.HopFirstLexBetter
        unsafeRoute alternativeRoute ∧
      ¬ routeFeasible unsafeRoute caps := by
  simpa [routeFeasible, Nat.succ_eq_add_one] using
    ASPProof.SearchRouteGraphRouterParetoCostSelection.hop_first_lex_order_requires_prior_feasibility
      tokenCap

structure ServerProcessIdentity where
  protocolDigest : String
  implementationDigest : String
  generation : Nat
  deriving Repr, DecidableEq, BEq

def processCanServe
    (process : ServerProcessIdentity)
    (session : GraphSessionIdentity) : Bool :=
  !process.protocolDigest.isEmpty &&
    !process.implementationDigest.isEmpty &&
    graphSessionIdentityValid session

def residentProcess : ServerProcessIdentity :=
  { protocolDigest := "resident-protocol:1"
    implementationDigest := "scipy-runtime:1"
    generation := 3 }

theorem one_process_can_serve_distinct_isolated_graph_sessions :
    processCanServe residentProcess graphSessionA = true ∧
      processCanServe residentProcess graphSessionB = true ∧
      sameGraphSession graphSessionA graphSessionB = false := by
  decide

structure ProjectionCacheIdentity where
  workspaceDigest : String
  snapshotDigest : String
  graphSchemaDigest : String
  projectionDigest : String
  algorithmImplementationDigest : String
  parameterDigest : String
  deterministicSeed : Nat
  budgetProfileDigest : String
  deriving Repr, DecidableEq, BEq

structure ModelPrefixCacheIdentity where
  modelDigest : String
  promptAndToolDigest : String
  prefixDigest : String
  deriving Repr, DecidableEq, BEq

def projectionCacheReusable
    (expected observed : ProjectionCacheIdentity) : Bool :=
  expected == observed

def modelPrefixCacheReusable
    (expected observed : ModelPrefixCacheIdentity) : Bool :=
  expected == observed

def projectionCacheIdentity : ProjectionCacheIdentity :=
  { workspaceDigest := "workspace:1"
    snapshotDigest := "snapshot:7"
    graphSchemaDigest := "schema:1"
    projectionDigest := "projection:1"
    algorithmImplementationDigest := "scipy:1"
    parameterDigest := "parameters:1"
    deterministicSeed := 0
    budgetProfileDigest := "budget:1" }

def modelPrefixCacheIdentity : ModelPrefixCacheIdentity :=
  { modelDigest := "model:1"
    promptAndToolDigest := "prompt-tools:1"
    prefixDigest := "prefix:1" }

theorem search_projection_cache_identity_is_exact :
    projectionCacheReusable projectionCacheIdentity projectionCacheIdentity = true := by
  decide

theorem model_prefix_cache_identity_is_exact :
    modelPrefixCacheReusable modelPrefixCacheIdentity modelPrefixCacheIdentity = true := by
  decide

def changedSnapshotProjectionCacheIdentity : ProjectionCacheIdentity :=
  { projectionCacheIdentity with snapshotDigest := "snapshot:6" }

theorem stale_snapshot_invalidates_search_projection_cache :
    projectionCacheReusable projectionCacheIdentity
      changedSnapshotProjectionCacheIdentity = false := by
  decide

theorem renderer_bytes_are_outside_search_and_model_cache_identity
    (_mermaid _dot : String) :
    projectionCacheReusable projectionCacheIdentity projectionCacheIdentity = true ∧
      modelPrefixCacheReusable modelPrefixCacheIdentity modelPrefixCacheIdentity = true := by
  decide

end ASPProof.GQLCoreSciPyResidentGraphTurbo
