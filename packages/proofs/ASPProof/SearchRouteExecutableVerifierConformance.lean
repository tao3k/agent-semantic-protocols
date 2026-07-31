import ASPProof.SearchRouteCryptographicTransferReceiptBinding

namespace ASPProof.SearchRouteExecutableVerifierConformance

open ASPProof.SearchRouteGraphRouterParetoCostSelection
open ASPProof.SearchRouteParetoFrontierVectorCompleteness
open ASPProof.SearchRouteVectorCoverageIdentityFence
open ASPProof.SearchRouteCertifiedRegistryEpochTransition
open ASPProof.SearchRouteCoverageTransitionChainIntegrity
open ASPProof.SearchRouteAuthorityRotationChainContinuity
open ASPProof.SearchRouteCryptographicTransferReceiptBinding

structure ExecutableVerificationContext where
  serializerDigest : Nat
  statementSchemaDigest : Nat
  keyRegistrySnapshotDigest : Nat
  verifierIdDigest : Nat
  verifierBinaryDigest : Nat
  decoderSchemaDigest : Nat
deriving DecidableEq, Repr

structure CanonicalStatementReceipt where
  role : SignatureRole
  authorityDigest : Nat
  keyEpoch : Nat
  algorithmDigest : Nat
  replayDomainDigest : Nat
  payloadDigest : Nat
  signatureDigest : Nat
  serializerDigest : Nat
  statementSchemaDigest : Nat
  statementBytesDigest : Nat
  canonical : Bool
deriving DecidableEq, Repr

structure KeyResolutionReceipt where
  registrySnapshotDigest : Nat
  authorityDigest : Nat
  keyEpoch : Nat
  algorithmDigest : Nat
  publicKeyDigest : Nat
  resolved : Bool
  revoked : Bool
deriving DecidableEq, Repr

structure VerifierResolutionReceipt where
  verifierIdDigest : Nat
  verifierBinaryDigest : Nat
  algorithmDigest : Nat
  resolved : Bool
deriving DecidableEq, Repr

structure VerifierInvocationReceipt where
  verifierBinaryDigest : Nat
  statementBytesDigest : Nat
  publicKeyDigest : Nat
  signatureDigest : Nat
  executorDigest : Nat
  exitCode : Nat
  responseBytesDigest : Nat
deriving DecidableEq, Repr

structure DecodedVerificationReceipt where
  responseBytesDigest : Nat
  decoderSchemaDigest : Nat
  decoded : Bool
  accepted : Bool
deriving DecidableEq, Repr

structure IndependentReplayReceipt where
  verifierBinaryDigest : Nat
  statementBytesDigest : Nat
  publicKeyDigest : Nat
  signatureDigest : Nat
  executorDigest : Nat
  accepted : Bool
  replayed : Bool
deriving DecidableEq, Repr

structure ExecutableSignatureVerificationReceipt where
  statement : CanonicalStatementReceipt
  keyResolution : KeyResolutionReceipt
  verifierResolution : VerifierResolutionReceipt
  invocation : VerifierInvocationReceipt
  decoded : DecodedVerificationReceipt
  replay : IndependentReplayReceipt
deriving DecidableEq, Repr

def StatementConformant
    (context : ExecutableVerificationContext)
    (signature : SignatureEnvelope)
    (statement : CanonicalStatementReceipt) : Prop :=
  statement.role = signature.role ∧
    statement.authorityDigest = signature.authorityDigest ∧
    statement.keyEpoch = signature.keyEpoch ∧
    statement.algorithmDigest = signature.algorithmDigest ∧
    statement.replayDomainDigest = signature.replayDomainDigest ∧
    statement.payloadDigest = signature.payloadDigest ∧
    statement.signatureDigest = signature.signatureDigest ∧
    statement.serializerDigest = context.serializerDigest ∧
    statement.statementSchemaDigest =
      context.statementSchemaDigest ∧
    statement.canonical = true

def KeyResolutionConformant
    (context : ExecutableVerificationContext)
    (signature : SignatureEnvelope)
    (resolution : KeyResolutionReceipt) : Prop :=
  resolution.registrySnapshotDigest =
      context.keyRegistrySnapshotDigest ∧
    resolution.authorityDigest = signature.authorityDigest ∧
    resolution.keyEpoch = signature.keyEpoch ∧
    resolution.algorithmDigest = signature.algorithmDigest ∧
    resolution.resolved = true ∧
    resolution.revoked = false

def VerifierResolutionConformant
    (context : ExecutableVerificationContext)
    (signature : SignatureEnvelope)
    (resolution : VerifierResolutionReceipt) : Prop :=
  resolution.verifierIdDigest = context.verifierIdDigest ∧
    resolution.verifierBinaryDigest =
      context.verifierBinaryDigest ∧
    resolution.algorithmDigest = signature.algorithmDigest ∧
    resolution.resolved = true

def InvocationConformant
    (statement : CanonicalStatementReceipt)
    (keyResolution : KeyResolutionReceipt)
    (verifierResolution : VerifierResolutionReceipt)
    (invocation : VerifierInvocationReceipt) : Prop :=
  invocation.verifierBinaryDigest =
      verifierResolution.verifierBinaryDigest ∧
    invocation.statementBytesDigest =
      statement.statementBytesDigest ∧
    invocation.publicKeyDigest =
      keyResolution.publicKeyDigest ∧
    invocation.signatureDigest = statement.signatureDigest ∧
    invocation.exitCode = 0

def DecodeConformant
    (context : ExecutableVerificationContext)
    (invocation : VerifierInvocationReceipt)
    (decoded : DecodedVerificationReceipt) : Prop :=
  decoded.responseBytesDigest = invocation.responseBytesDigest ∧
    decoded.decoderSchemaDigest = context.decoderSchemaDigest ∧
    decoded.decoded = true ∧
    decoded.accepted = true

def IndependentReplayConformant
    (invocation : VerifierInvocationReceipt)
    (decoded : DecodedVerificationReceipt)
    (replay : IndependentReplayReceipt) : Prop :=
  replay.verifierBinaryDigest = invocation.verifierBinaryDigest ∧
    replay.statementBytesDigest = invocation.statementBytesDigest ∧
    replay.publicKeyDigest = invocation.publicKeyDigest ∧
    replay.signatureDigest = invocation.signatureDigest ∧
    replay.executorDigest ≠ invocation.executorDigest ∧
    replay.accepted = decoded.accepted ∧
    replay.replayed = true

def ValidExecutableSignatureVerification
    (context : ExecutableVerificationContext)
    (signature : SignatureEnvelope)
    (receipt : ExecutableSignatureVerificationReceipt) : Prop :=
  StatementConformant context signature receipt.statement ∧
    KeyResolutionConformant
      context signature receipt.keyResolution ∧
    VerifierResolutionConformant
      context signature receipt.verifierResolution ∧
    InvocationConformant
      receipt.statement
      receipt.keyResolution
      receipt.verifierResolution
      receipt.invocation ∧
    DecodeConformant context receipt.invocation receipt.decoded ∧
    IndependentReplayConformant
      receipt.invocation receipt.decoded receipt.replay

def DigestOnlyEvidence
    (signature : SignatureEnvelope)
    (receipt : ExecutableSignatureVerificationReceipt) : Prop :=
  receipt.statement.payloadDigest = signature.payloadDigest

def ResolvedOnlyEvidence
    (receipt : ExecutableSignatureVerificationReceipt) : Prop :=
  receipt.verifierResolution.resolved = true

def InvokedOnlyEvidence
    (receipt : ExecutableSignatureVerificationReceipt) : Prop :=
  receipt.invocation.exitCode = 0

def DecodedOnlyEvidence
    (receipt : ExecutableSignatureVerificationReceipt) : Prop :=
  receipt.decoded.decoded = true ∧ receipt.decoded.accepted = true

structure ExecutableTransferVerificationReceipt where
  oldVerification : ExecutableSignatureVerificationReceipt
  newVerification : ExecutableSignatureVerificationReceipt
deriving DecidableEq, Repr

def ValidExecutableTransferVerification
    (oldContext newContext : ExecutableVerificationContext)
    (transfer : CryptographicTransferReceipt)
    (execution : ExecutableTransferVerificationReceipt) : Prop :=
  ValidExecutableSignatureVerification
      oldContext transfer.oldSignature execution.oldVerification ∧
    ValidExecutableSignatureVerification
      newContext transfer.newSignature execution.newVerification

structure ExecutableCryptographicAuthorityRotationEdge where
  cryptographic : CryptographicAuthorityRotationEdge
  execution : ExecutableTransferVerificationReceipt
deriving DecidableEq, Repr

def ValidExecutableCryptographicAuthorityRotationEdge
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (transferContext : TransferVerificationContext)
    (oldExecutionContext newExecutionContext :
      ExecutableVerificationContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (edge : ExecutableCryptographicAuthorityRotationEdge) : Prop :=
  ValidCryptographicAuthorityRotationEdge
      authorizedSuccessor
      digestPayload
      verify
      transferContext
      boundHeader
      lineage
      edge.cryptographic ∧
    ValidExecutableTransferVerification
      oldExecutionContext
      newExecutionContext
      edge.cryptographic.receipt
      edge.execution

inductive ValidExecutableCryptographicAuthorityRotationChain
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (transferContext : TransferVerificationContext)
    (oldExecutionContext newExecutionContext :
      ExecutableVerificationContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage) :
    ClockDomain →
      ClockDomain →
      List ExecutableCryptographicAuthorityRotationEdge →
      Prop
  | nil (domain : ClockDomain) :
      ValidExecutableCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify transferContext
        oldExecutionContext newExecutionContext
        boundHeader lineage domain domain []
  | cons
      {fromDomain nextDomain finalDomain : ClockDomain}
      {edge : ExecutableCryptographicAuthorityRotationEdge}
      {rest : List ExecutableCryptographicAuthorityRotationEdge}
      (fromMatches :
        edge.cryptographic.rotation.registry.fromDomain = fromDomain)
      (toMatches :
        edge.cryptographic.rotation.registry.toDomain = nextDomain)
      (valid :
        ValidExecutableCryptographicAuthorityRotationEdge
          authorizedSuccessor digestPayload verify transferContext
          oldExecutionContext newExecutionContext
          boundHeader lineage edge)
      (tail :
        ValidExecutableCryptographicAuthorityRotationChain
          authorizedSuccessor digestPayload verify transferContext
          oldExecutionContext newExecutionContext
          boundHeader lineage nextDomain finalDomain rest) :
      ValidExecutableCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify transferContext
        oldExecutionContext newExecutionContext
        boundHeader lineage fromDomain finalDomain (edge :: rest)

def cryptographicRotationEdges :
    List ExecutableCryptographicAuthorityRotationEdge →
      List CryptographicAuthorityRotationEdge :=
  List.map ExecutableCryptographicAuthorityRotationEdge.cryptographic

def ExecutableCryptographicChainAdmitted
    (authorizedSuccessor : ClockDomain → ClockDomain → Prop)
    (digestPayload : TransferPayload → Nat)
    (verify : SignatureEnvelope → Prop)
    (transferContext : TransferVerificationContext)
    (oldExecutionContext newExecutionContext :
      ExecutableVerificationContext)
    (coverageContext : CoverageContext)
    (boundHeader : AuthorityBoundCoverageHeader)
    (lineage : CoverageSemanticLineage)
    (endDomain : ClockDomain)
    (edges : List ExecutableCryptographicAuthorityRotationEdge) : Prop :=
  CryptographicAuthorityRotationChainAdmitted
      authorizedSuccessor
      digestPayload
      verify
      transferContext
      coverageContext
      boundHeader
      lineage
      endDomain
      (cryptographicRotationEdges edges) ∧
    ValidExecutableCryptographicAuthorityRotationChain
      authorizedSuccessor
      digestPayload
      verify
      transferContext
      oldExecutionContext
      newExecutionContext
      boundHeader
      lineage
      boundHeader.issuedDomain
      endDomain
      edges

theorem valid_executable_verification_binds_all_stages
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (valid :
      ValidExecutableSignatureVerification
        context signature receipt) :
    StatementConformant context signature receipt.statement ∧
      KeyResolutionConformant
        context signature receipt.keyResolution ∧
      VerifierResolutionConformant
        context signature receipt.verifierResolution ∧
      InvocationConformant
        receipt.statement
        receipt.keyResolution
        receipt.verifierResolution
        receipt.invocation ∧
      DecodeConformant context receipt.invocation receipt.decoded ∧
      IndependentReplayConformant
        receipt.invocation receipt.decoded receipt.replay :=
  valid

theorem valid_executable_verification_has_distinct_replay_executor
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (valid :
      ValidExecutableSignatureVerification
        context signature receipt) :
    receipt.replay.executorDigest ≠
      receipt.invocation.executorDigest :=
  valid.2.2.2.2.2.2.2.2.2.1

theorem noncanonical_statement_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (noncanonical : receipt.statement.canonical = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have canonical : receipt.statement.canonical = true :=
    valid.1.2.2.2.2.2.2.2.2.2
  have impossible : false = true := noncanonical.symm.trans canonical
  cases impossible

theorem unresolved_key_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (unresolved : receipt.keyResolution.resolved = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have resolved : receipt.keyResolution.resolved = true :=
    valid.2.1.2.2.2.2.1
  have impossible : false = true := unresolved.symm.trans resolved
  cases impossible

theorem wrong_registry_snapshot_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (mismatch :
      receipt.keyResolution.registrySnapshotDigest ≠
        context.keyRegistrySnapshotDigest) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  exact mismatch valid.2.1.1

theorem unresolved_verifier_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (unresolved : receipt.verifierResolution.resolved = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have resolved : receipt.verifierResolution.resolved = true :=
    valid.2.2.1.2.2.2
  have impossible : false = true := unresolved.symm.trans resolved
  cases impossible

theorem failed_invocation_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (failed : receipt.invocation.exitCode ≠ 0) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  exact failed valid.2.2.2.1.2.2.2.2

theorem undecoded_response_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (undecoded : receipt.decoded.decoded = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have decoded : receipt.decoded.decoded = true :=
    valid.2.2.2.2.1.2.2.1
  have impossible : false = true := undecoded.symm.trans decoded
  cases impossible

theorem rejected_decision_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (rejected : receipt.decoded.accepted = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have accepted : receipt.decoded.accepted = true :=
    valid.2.2.2.2.1.2.2.2
  have impossible : false = true := rejected.symm.trans accepted
  cases impossible

theorem missing_replay_rejects_executable_verification
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (missing : receipt.replay.replayed = false) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  have replayValid :
      IndependentReplayConformant
        receipt.invocation receipt.decoded receipt.replay :=
    valid.2.2.2.2.2
  have replayed : receipt.replay.replayed = true :=
    replayValid.2.2.2.2.2.2
  have impossible : false = true := missing.symm.trans replayed
  cases impossible

theorem same_executor_replay_rejects_independence
    {context : ExecutableVerificationContext}
    {signature : SignatureEnvelope}
    {receipt : ExecutableSignatureVerificationReceipt}
    (same :
      receipt.replay.executorDigest =
        receipt.invocation.executorDigest) :
    ¬ ValidExecutableSignatureVerification
      context signature receipt := by
  intro valid
  exact
    (valid_executable_verification_has_distinct_replay_executor valid)
      same

theorem valid_executable_transfer_verifies_both_signatures
    {oldContext newContext : ExecutableVerificationContext}
    {transfer : CryptographicTransferReceipt}
    {execution : ExecutableTransferVerificationReceipt}
    (valid :
      ValidExecutableTransferVerification
        oldContext newContext transfer execution) :
    ValidExecutableSignatureVerification
        oldContext transfer.oldSignature execution.oldVerification ∧
      ValidExecutableSignatureVerification
        newContext transfer.newSignature execution.newVerification :=
  valid

theorem valid_executable_edge_is_valid_cryptographic_edge
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {transferContext : TransferVerificationContext}
    {oldExecutionContext newExecutionContext :
      ExecutableVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {edge : ExecutableCryptographicAuthorityRotationEdge}
    (valid :
      ValidExecutableCryptographicAuthorityRotationEdge
        authorizedSuccessor digestPayload verify transferContext
        oldExecutionContext newExecutionContext
        boundHeader lineage edge) :
    ValidCryptographicAuthorityRotationEdge
      authorizedSuccessor digestPayload verify transferContext
      boundHeader lineage edge.cryptographic :=
  valid.1

theorem valid_executable_chain_projects_cryptographic_chain
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {transferContext : TransferVerificationContext}
    {oldExecutionContext newExecutionContext :
      ExecutableVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List ExecutableCryptographicAuthorityRotationEdge}
    (chain :
      ValidExecutableCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify transferContext
        oldExecutionContext newExecutionContext
        boundHeader lineage startDomain endDomain edges) :
    ValidCryptographicAuthorityRotationChain
      authorizedSuccessor digestPayload verify transferContext
      boundHeader lineage startDomain endDomain
      (cryptographicRotationEdges edges) := by
  induction chain with
  | nil domain =>
      exact ValidCryptographicAuthorityRotationChain.nil domain
  | cons fromMatches toMatches valid _ inductionHypothesis =>
      exact ValidCryptographicAuthorityRotationChain.cons
        fromMatches
        toMatches
        valid.1
        inductionHypothesis

theorem valid_executable_chain_exact_epoch_span
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    {transferContext : TransferVerificationContext}
    {oldExecutionContext newExecutionContext :
      ExecutableVerificationContext}
    {boundHeader : AuthorityBoundCoverageHeader}
    {lineage : CoverageSemanticLineage}
    {startDomain endDomain : ClockDomain}
    {edges : List ExecutableCryptographicAuthorityRotationEdge}
    (chain :
      ValidExecutableCryptographicAuthorityRotationChain
        authorizedSuccessor digestPayload verify transferContext
        oldExecutionContext newExecutionContext
        boundHeader lineage startDomain endDomain edges) :
    startDomain.epoch + edges.length = endDomain.epoch := by
  have span :=
    valid_cryptographic_chain_exact_epoch_span
      (valid_executable_chain_projects_cryptographic_chain chain)
  simpa [cryptographicRotationEdges] using span

theorem executable_chain_admitted_vector_coverage_lifts_global_pareto
    {Candidate : Type}
    (cost : Candidate → RouteCost)
    (frontier globalCatalog : Candidate → Prop)
    (frontierSubset :
      ∀ candidate,
        frontier candidate → globalCatalog candidate)
    {authorizedSuccessor : ClockDomain → ClockDomain → Prop}
    {digestPayload : TransferPayload → Nat}
    {verify : SignatureEnvelope → Prop}
    (transferContext : TransferVerificationContext)
    (oldExecutionContext newExecutionContext :
      ExecutableVerificationContext)
    (coverageContext : CoverageContext)
    (certificate : CoverageCertificate cost frontier globalCatalog)
    (issuedDomain endDomain : ClockDomain)
    (lineage : CoverageSemanticLineage)
    (edges : List ExecutableCryptographicAuthorityRotationEdge)
    (admitted :
      ExecutableCryptographicChainAdmitted
        authorizedSuccessor
        digestPayload
        verify
        transferContext
        oldExecutionContext
        newExecutionContext
        coverageContext
        {
          header := certificate.header
          issuedDomain := issuedDomain
        }
        lineage
        endDomain
        edges)
    (chosen : Candidate)
    (frontierMinimal : ParetoMinimal cost frontier chosen) :
    ParetoMinimal cost globalCatalog chosen ∧
      certificate.header.semanticIdentity =
        coverageContext.semanticIdentity := by
  exact
    ⟨ vector_complete_frontier_lifts_pareto_minimality
        cost
        frontier
        globalCatalog
        frontierSubset
        certificate.vectorCoverage
        chosen
        frontierMinimal
    , admitted.1.1.1
    ⟩

def oldExecutableContext : ExecutableVerificationContext :=
  {
    serializerDigest := 301
    statementSchemaDigest := 302
    keyRegistrySnapshotDigest := 303
    verifierIdDigest := 304
    verifierBinaryDigest := 305
    decoderSchemaDigest := 306
  }

def newExecutableContext : ExecutableVerificationContext :=
  { oldExecutableContext with verifierBinaryDigest := 405 }

def oldCanonicalStatement : CanonicalStatementReceipt :=
  {
    role := .oldApprover
    authorityDigest := 77
    keyEpoch := 3
    algorithmDigest := 101
    replayDomainDigest := 900
    payloadDigest := 501
    signatureDigest := 7001
    serializerDigest := 301
    statementSchemaDigest := 302
    statementBytesDigest := 801
    canonical := true
  }

def newCanonicalStatement : CanonicalStatementReceipt :=
  {
    role := .newAcceptor
    authorityDigest := 88
    keyEpoch := 4
    algorithmDigest := 202
    replayDomainDigest := 900
    payloadDigest := 501
    signatureDigest := 7002
    serializerDigest := 301
    statementSchemaDigest := 302
    statementBytesDigest := 802
    canonical := true
  }

def oldKeyResolution : KeyResolutionReceipt :=
  {
    registrySnapshotDigest := 303
    authorityDigest := 77
    keyEpoch := 3
    algorithmDigest := 101
    publicKeyDigest := 901
    resolved := true
    revoked := false
  }

def newKeyResolution : KeyResolutionReceipt :=
  {
    registrySnapshotDigest := 303
    authorityDigest := 88
    keyEpoch := 4
    algorithmDigest := 202
    publicKeyDigest := 902
    resolved := true
    revoked := false
  }

def oldVerifierResolution : VerifierResolutionReceipt :=
  {
    verifierIdDigest := 304
    verifierBinaryDigest := 305
    algorithmDigest := 101
    resolved := true
  }

def newVerifierResolution : VerifierResolutionReceipt :=
  {
    verifierIdDigest := 304
    verifierBinaryDigest := 405
    algorithmDigest := 202
    resolved := true
  }

def oldInvocation : VerifierInvocationReceipt :=
  {
    verifierBinaryDigest := 305
    statementBytesDigest := 801
    publicKeyDigest := 901
    signatureDigest := 7001
    executorDigest := 1001
    exitCode := 0
    responseBytesDigest := 1101
  }

def newInvocation : VerifierInvocationReceipt :=
  {
    verifierBinaryDigest := 405
    statementBytesDigest := 802
    publicKeyDigest := 902
    signatureDigest := 7002
    executorDigest := 1001
    exitCode := 0
    responseBytesDigest := 1102
  }

def oldDecoded : DecodedVerificationReceipt :=
  {
    responseBytesDigest := 1101
    decoderSchemaDigest := 306
    decoded := true
    accepted := true
  }

def newDecoded : DecodedVerificationReceipt :=
  {
    responseBytesDigest := 1102
    decoderSchemaDigest := 306
    decoded := true
    accepted := true
  }

def oldReplay : IndependentReplayReceipt :=
  {
    verifierBinaryDigest := 305
    statementBytesDigest := 801
    publicKeyDigest := 901
    signatureDigest := 7001
    executorDigest := 1002
    accepted := true
    replayed := true
  }

def newReplay : IndependentReplayReceipt :=
  {
    verifierBinaryDigest := 405
    statementBytesDigest := 802
    publicKeyDigest := 902
    signatureDigest := 7002
    executorDigest := 1002
    accepted := true
    replayed := true
  }

def oldExecutableReceipt : ExecutableSignatureVerificationReceipt :=
  {
    statement := oldCanonicalStatement
    keyResolution := oldKeyResolution
    verifierResolution := oldVerifierResolution
    invocation := oldInvocation
    decoded := oldDecoded
    replay := oldReplay
  }

def newExecutableReceipt : ExecutableSignatureVerificationReceipt :=
  {
    statement := newCanonicalStatement
    keyResolution := newKeyResolution
    verifierResolution := newVerifierResolution
    invocation := newInvocation
    decoded := newDecoded
    replay := newReplay
  }

def exampleExecutableTransfer : ExecutableTransferVerificationReceipt :=
  {
    oldVerification := oldExecutableReceipt
    newVerification := newExecutableReceipt
  }

theorem example_old_executable_verification_is_valid :
    ValidExecutableSignatureVerification
      oldExecutableContext
      oldExampleSignature
      oldExecutableReceipt := by
  simp
    [ ValidExecutableSignatureVerification
    , StatementConformant
    , KeyResolutionConformant
    , VerifierResolutionConformant
    , InvocationConformant
    , DecodeConformant
    , IndependentReplayConformant
    , oldExecutableContext
    , oldExampleSignature
    , oldExecutableReceipt
    , oldCanonicalStatement
    , oldKeyResolution
    , oldVerifierResolution
    , oldInvocation
    , oldDecoded
    , oldReplay
    ]

theorem example_new_executable_verification_is_valid :
    ValidExecutableSignatureVerification
      newExecutableContext
      newExampleSignature
      newExecutableReceipt := by
  simp
    [ ValidExecutableSignatureVerification
    , StatementConformant
    , KeyResolutionConformant
    , VerifierResolutionConformant
    , InvocationConformant
    , DecodeConformant
    , IndependentReplayConformant
    , newExecutableContext
    , oldExecutableContext
    , newExampleSignature
    , newExecutableReceipt
    , newCanonicalStatement
    , newKeyResolution
    , newVerifierResolution
    , newInvocation
    , newDecoded
    , newReplay
    ]

theorem example_executable_transfer_verifies_both_signatures :
    ValidExecutableTransferVerification
      oldExecutableContext
      newExecutableContext
      exampleCryptographicReceipt
      exampleExecutableTransfer :=
  ⟨ example_old_executable_verification_is_valid
  , example_new_executable_verification_is_valid
  ⟩

def unresolvedOldVerifierReceipt : ExecutableSignatureVerificationReceipt :=
  { oldExecutableReceipt with
      verifierResolution :=
        { oldVerifierResolution with resolved := false } }

def sameExecutorReplayReceipt : ExecutableSignatureVerificationReceipt :=
  { oldExecutableReceipt with
      replay := { oldReplay with executorDigest := 1001 } }

def wrongRegistrySnapshotReceipt :
    ExecutableSignatureVerificationReceipt :=
  { oldExecutableReceipt with
      keyResolution :=
        { oldKeyResolution with registrySnapshotDigest := 999 } }

def undecodedOldReceipt : ExecutableSignatureVerificationReceipt :=
  { oldExecutableReceipt with
      decoded := { oldDecoded with decoded := false } }

theorem digest_and_resolved_prefix_do_not_prove_execution :
    DigestOnlyEvidence oldExampleSignature unresolvedOldVerifierReceipt ∧
      ¬ ValidExecutableSignatureVerification
        oldExecutableContext
        oldExampleSignature
        unresolvedOldVerifierReceipt := by
  constructor
  · rfl
  · apply unresolved_verifier_rejects_executable_verification
    rfl

theorem same_executor_replay_is_not_independent :
    InvokedOnlyEvidence sameExecutorReplayReceipt ∧
      DecodedOnlyEvidence sameExecutorReplayReceipt ∧
      ¬ ValidExecutableSignatureVerification
        oldExecutableContext
        oldExampleSignature
        sameExecutorReplayReceipt := by
  exact
    ⟨ rfl
    , ⟨rfl, rfl⟩
    , same_executor_replay_rejects_independence rfl
    ⟩

theorem wrong_registry_snapshot_example_is_rejected :
    ¬ ValidExecutableSignatureVerification
      oldExecutableContext
      oldExampleSignature
      wrongRegistrySnapshotReceipt := by
  apply wrong_registry_snapshot_rejects_executable_verification
  decide

theorem undecoded_example_response_is_rejected :
    ¬ ValidExecutableSignatureVerification
      oldExecutableContext
      oldExampleSignature
      undecodedOldReceipt := by
  apply undecoded_response_rejects_executable_verification
  rfl

def exampleExecutableCryptographicEdge :
    ExecutableCryptographicAuthorityRotationEdge :=
  {
    cryptographic := exampleCryptographicRotationEdge
    execution := exampleExecutableTransfer
  }

theorem example_executable_cryptographic_edge_is_valid :
    ValidExecutableCryptographicAuthorityRotationEdge
      authorityRotationSuccessor
      constantPayloadDigest
      exampleSignatureVerify
      exampleVerificationContext
      oldExecutableContext
      newExecutableContext
      authorityBoundCurrentHeader
      stableSemanticLineage
      exampleExecutableCryptographicEdge :=
  ⟨ example_cryptographic_rotation_edge_is_valid
  , example_executable_transfer_verifies_both_signatures
  ⟩

end ASPProof.SearchRouteExecutableVerifierConformance
