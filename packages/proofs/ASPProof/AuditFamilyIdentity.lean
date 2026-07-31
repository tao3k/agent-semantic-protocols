import Std

namespace ASPProof.AuditFamilyIdentity

/-- One exported theorem obligation in a Lean audit receipt. -/
structure TheoremRow where
  theoremId : String
  familyId : String
  obligationId : String
  deriving DecidableEq, Repr

/-- The exported theorem identity is globally unique within one receipt. -/
def UniqueTheoremIds (rows : List TheoremRow) : Prop :=
  (rows.map TheoremRow.theoremId).Nodup

/--
The pair is unique within one receipt. A family may intentionally contain more
than one independently named obligation.
-/
def UniqueFamilyObligations (rows : List TheoremRow) : Prop :=
  (rows.map fun row => (row.familyId, row.obligationId)).Nodup

/-- This predicate captures the rejected, over-strong legacy consumer rule. -/
def UniqueFamilyIds (rows : List TheoremRow) : Prop :=
  (rows.map TheoremRow.familyId).Nodup

def firstMergeRow : TheoremRow :=
  { theoremId := "merge_commutative"
    familyId := "merge-algebra"
    obligationId := "commutative" }

def secondMergeRow : TheoremRow :=
  { theoremId := "merge_associative"
    familyId := "merge-algebra"
    obligationId := "associative" }

def repeatedFamilyExample : List TheoremRow :=
  [firstMergeRow, secondMergeRow]

/--
Countermodel: a valid receipt may contain two distinct theorem obligations in
one theorem family. Therefore family uniqueness cannot be the receipt identity
invariant.
-/
theorem repeated_family_preserves_receipt_identity :
    UniqueTheoremIds repeatedFamilyExample ∧
      UniqueFamilyObligations repeatedFamilyExample ∧
      ¬ UniqueFamilyIds repeatedFamilyExample := by
  simp [UniqueTheoremIds, UniqueFamilyObligations, UniqueFamilyIds,
    repeatedFamilyExample, firstMergeRow, secondMergeRow]

/--
The receipt may group multiple theorems by family without weakening the unique
identity of either the theorem or the family-scoped obligation.
-/
theorem family_is_grouping_not_identity :
    firstMergeRow.familyId = secondMergeRow.familyId ∧
      firstMergeRow.theoremId ≠ secondMergeRow.theoremId ∧
      firstMergeRow.obligationId ≠ secondMergeRow.obligationId := by
  simp [firstMergeRow, secondMergeRow]

end ASPProof.AuditFamilyIdentity
