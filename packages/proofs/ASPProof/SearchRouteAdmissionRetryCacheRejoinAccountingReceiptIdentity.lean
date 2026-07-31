import ASPProof.SearchRouteAdmissionRetryCacheRejoinConcreteIntersection

namespace ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity

open ASPProof.SearchRouteAdmissionRetryCacheRejoinCanonicalMembership
open ASPProof.SearchRouteAdmissionRetryCacheRejoinQuorumRealization

structure AccountingReceiptPayload
    (MembershipDigest CertificatePair ProjectionVersion : Type) where
  membershipDigest : MembershipDigest
  certificatePair : CertificatePair
  projectionVersion : ProjectionVersion

structure HonestOverlapVerdictPayload
    (AccountingReceiptId FaultBoundSnapshotId : Type) where
  accountingReceiptId : AccountingReceiptId
  faultBoundSnapshotId : FaultBoundSnapshotId

structure AccountingReceiptIdentityScheme
    (MembershipDigest : Type) where
  CertificateId : Type
  CertificatePair : Type
  ProjectionVersion : Type
  FaultBoundSnapshotId : Type
  AccountingReceiptId : Type
  HonestOverlapVerdictId : Type
  canonicalPair : CertificateId → CertificateId → CertificatePair
  canonicalPairSwap :
    ∀ left right,
      canonicalPair left right = canonicalPair right left
  canonicalPairComplete :
    ∀ left right otherLeft otherRight,
      canonicalPair left right =
        canonicalPair otherLeft otherRight →
      (left = otherLeft ∧ right = otherRight) ∨
      (left = otherRight ∧ right = otherLeft)
  commitAccounting :
    AccountingReceiptPayload
      MembershipDigest CertificatePair ProjectionVersion →
      AccountingReceiptId
  accountingCollisionFree : Function.Injective commitAccounting
  commitHonestOverlapVerdict :
    HonestOverlapVerdictPayload
      AccountingReceiptId FaultBoundSnapshotId →
      HonestOverlapVerdictId
  verdictCollisionFree : Function.Injective commitHonestOverlapVerdict

def SameUnorderedCertificatePair
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (left right otherLeft otherRight : scheme.CertificateId) : Prop :=
  (left = otherLeft ∧ right = otherRight) ∨
  (left = otherRight ∧ right = otherLeft)

def accountingReceiptPayload
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion) :
    AccountingReceiptPayload
      MembershipDigest scheme.CertificatePair scheme.ProjectionVersion where
  membershipDigest := membershipDigest
  certificatePair := scheme.canonicalPair left right
  projectionVersion := projectionVersion

def accountingReceiptIdentity
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion) :
    scheme.AccountingReceiptId :=
  scheme.commitAccounting
    (accountingReceiptPayload
      scheme membershipDigest left right projectionVersion)

def honestOverlapVerdictIdentity
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion)
    (faultBoundSnapshotId : scheme.FaultBoundSnapshotId) :
    scheme.HonestOverlapVerdictId :=
  scheme.commitHonestOverlapVerdict {
    accountingReceiptId :=
      accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion
    faultBoundSnapshotId := faultBoundSnapshotId
  }

theorem swapping_certificates_preserves_accounting_receipt_identity
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion) :
    accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion =
      accountingReceiptIdentity
        scheme membershipDigest right left projectionVersion := by
  unfold accountingReceiptIdentity accountingReceiptPayload
  rw [scheme.canonicalPairSwap]

theorem equal_accounting_receipt_implies_same_semantic_owners
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest otherMembershipDigest : MembershipDigest)
    (left right otherLeft otherRight : scheme.CertificateId)
    (projectionVersion otherProjectionVersion : scheme.ProjectionVersion)
    (sameReceipt :
      accountingReceiptIdentity
          scheme membershipDigest left right projectionVersion =
        accountingReceiptIdentity
          scheme otherMembershipDigest otherLeft otherRight
            otherProjectionVersion) :
    membershipDigest = otherMembershipDigest ∧
    SameUnorderedCertificatePair
      scheme left right otherLeft otherRight ∧
    projectionVersion = otherProjectionVersion := by
  have samePayload := scheme.accountingCollisionFree sameReceipt
  have sameMembership :=
    congrArg
      (AccountingReceiptPayload.membershipDigest
        (CertificatePair := scheme.CertificatePair)
        (ProjectionVersion := scheme.ProjectionVersion))
      samePayload
  have samePair :=
    congrArg
      (AccountingReceiptPayload.certificatePair
        (MembershipDigest := MembershipDigest)
        (ProjectionVersion := scheme.ProjectionVersion))
      samePayload
  have sameProjection :=
    congrArg
      (AccountingReceiptPayload.projectionVersion
        (MembershipDigest := MembershipDigest)
        (CertificatePair := scheme.CertificatePair))
      samePayload
  exact ⟨
    sameMembership,
    scheme.canonicalPairComplete
      left right otherLeft otherRight samePair,
    sameProjection
  ⟩

theorem changed_membership_prevents_accounting_receipt_reuse
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest otherMembershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion)
    (changed : membershipDigest ≠ otherMembershipDigest) :
    accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion ≠
      accountingReceiptIdentity
        scheme otherMembershipDigest left right projectionVersion := by
  intro sameReceipt
  exact changed
    (equal_accounting_receipt_implies_same_semantic_owners
      scheme
      membershipDigest otherMembershipDigest
      left right left right
      projectionVersion projectionVersion
      sameReceipt).1

theorem changed_unordered_pair_prevents_accounting_receipt_reuse
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right otherLeft otherRight : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion)
    (changed :
      ¬ SameUnorderedCertificatePair
        scheme left right otherLeft otherRight) :
    accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion ≠
      accountingReceiptIdentity
        scheme membershipDigest otherLeft otherRight projectionVersion := by
  intro sameReceipt
  exact changed
    (equal_accounting_receipt_implies_same_semantic_owners
      scheme
      membershipDigest membershipDigest
      left right otherLeft otherRight
      projectionVersion projectionVersion
      sameReceipt).2.1

theorem changed_projection_prevents_accounting_receipt_reuse
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion otherProjectionVersion : scheme.ProjectionVersion)
    (changed : projectionVersion ≠ otherProjectionVersion) :
    accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion ≠
      accountingReceiptIdentity
        scheme membershipDigest left right otherProjectionVersion := by
  intro sameReceipt
  exact changed
    (equal_accounting_receipt_implies_same_semantic_owners
      scheme
      membershipDigest membershipDigest
      left right left right
      projectionVersion otherProjectionVersion
      sameReceipt).2.2

theorem fault_bound_snapshot_change_preserves_accounting_but_changes_verdict
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest : MembershipDigest)
    (left right : scheme.CertificateId)
    (projectionVersion : scheme.ProjectionVersion)
    (faultBoundSnapshotId otherFaultBoundSnapshotId :
      scheme.FaultBoundSnapshotId)
    (changed :
      faultBoundSnapshotId ≠ otherFaultBoundSnapshotId) :
    accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion =
      accountingReceiptIdentity
        scheme membershipDigest left right projectionVersion ∧
    honestOverlapVerdictIdentity
        scheme membershipDigest left right projectionVersion
          faultBoundSnapshotId ≠
      honestOverlapVerdictIdentity
        scheme membershipDigest left right projectionVersion
          otherFaultBoundSnapshotId := by
  constructor
  · rfl
  · intro sameVerdict
    have samePayload := scheme.verdictCollisionFree sameVerdict
    exact changed
      (congrArg
        (HonestOverlapVerdictPayload.faultBoundSnapshotId
          (AccountingReceiptId := scheme.AccountingReceiptId)
          (FaultBoundSnapshotId := scheme.FaultBoundSnapshotId))
        samePayload)

theorem equal_verdict_implies_same_accounting_owners_and_fault_bound
    {MembershipDigest : Type}
    (scheme : AccountingReceiptIdentityScheme MembershipDigest)
    (membershipDigest otherMembershipDigest : MembershipDigest)
    (left right otherLeft otherRight : scheme.CertificateId)
    (projectionVersion otherProjectionVersion : scheme.ProjectionVersion)
    (faultBoundSnapshotId otherFaultBoundSnapshotId :
      scheme.FaultBoundSnapshotId)
    (sameVerdict :
      honestOverlapVerdictIdentity
          scheme membershipDigest left right projectionVersion
            faultBoundSnapshotId =
        honestOverlapVerdictIdentity
          scheme otherMembershipDigest otherLeft otherRight
            otherProjectionVersion otherFaultBoundSnapshotId) :
    membershipDigest = otherMembershipDigest ∧
    SameUnorderedCertificatePair
      scheme left right otherLeft otherRight ∧
    projectionVersion = otherProjectionVersion ∧
    faultBoundSnapshotId = otherFaultBoundSnapshotId := by
  have sameVerdictPayload := scheme.verdictCollisionFree sameVerdict
  have sameAccountingReceipt :=
    congrArg
      (HonestOverlapVerdictPayload.accountingReceiptId
        (AccountingReceiptId := scheme.AccountingReceiptId)
        (FaultBoundSnapshotId := scheme.FaultBoundSnapshotId))
      sameVerdictPayload
  have sameFaultBoundSnapshot :=
    congrArg
      (HonestOverlapVerdictPayload.faultBoundSnapshotId
        (AccountingReceiptId := scheme.AccountingReceiptId)
        (FaultBoundSnapshotId := scheme.FaultBoundSnapshotId))
      sameVerdictPayload
  have sameAccountingOwners :=
    equal_accounting_receipt_implies_same_semantic_owners
      scheme
      membershipDigest otherMembershipDigest
      left right otherLeft otherRight
      projectionVersion otherProjectionVersion
      sameAccountingReceipt
  exact ⟨
    sameAccountingOwners.1,
    sameAccountingOwners.2.1,
    sameAccountingOwners.2.2,
    sameFaultBoundSnapshot
  ⟩

end ASPProof.SearchRouteAdmissionRetryCacheRejoinAccountingReceiptIdentity
