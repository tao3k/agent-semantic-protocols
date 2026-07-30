import ASPProof.SearchLoopTrace

namespace SearchLoopMerge

open SearchLoopClauseFirst

inductive Outcome where
  | pending
  | discharged
  | contradiction
  deriving DecidableEq, Repr

def joinOutcome : Outcome → Outcome → Outcome
  | Outcome.contradiction, _ => Outcome.contradiction
  | _, Outcome.contradiction => Outcome.contradiction
  | Outcome.discharged, _ => Outcome.discharged
  | _, Outcome.discharged => Outcome.discharged
  | Outcome.pending, Outcome.pending => Outcome.pending

theorem joinOutcome_commutative (left right : Outcome) :
    joinOutcome left right = joinOutcome right left := by
  cases left <;> cases right <;> rfl

theorem joinOutcome_associative (first second third : Outcome) :
    joinOutcome (joinOutcome first second) third =
      joinOutcome first (joinOutcome second third) := by
  cases first <;> cases second <;> cases third <;> rfl

theorem joinOutcome_idempotent (outcome : Outcome) :
    joinOutcome outcome outcome = outcome := by
  cases outcome <;> rfl

structure AdmittedResult (domain : ValidationDomain) where
  clause : ClauseId
  outcome : Outcome
  deriving DecidableEq, Repr

abbrev NormalizedLedger (_domain : ValidationDomain) :=
  ClauseId → Outcome

def mergeAdmitted
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain)
    (result : AdmittedResult domain) :
    NormalizedLedger domain :=
  fun clause =>
    if clause = result.clause then
      joinOutcome (ledger clause) result.outcome
    else
      ledger clause

theorem admitted_merges_commute
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain)
    (left right : AdmittedResult domain) :
    mergeAdmitted (mergeAdmitted ledger left) right =
      mergeAdmitted (mergeAdmitted ledger right) left := by
  cases left with
  | mk leftClause leftOutcome =>
      cases right with
      | mk rightClause rightOutcome =>
          by_cases sameClause : leftClause = rightClause
          · subst rightClause
            funext clause
            by_cases hit : clause = leftClause
            · simp only [mergeAdmitted, hit, if_true]
              rw [joinOutcome_associative, joinOutcome_associative]
              rw [joinOutcome_commutative leftOutcome rightOutcome]
            · simp [mergeAdmitted, hit]
          · funext clause
            by_cases leftHit : clause = leftClause
            · simp [mergeAdmitted, leftHit, sameClause]
            · by_cases rightHit : clause = rightClause
              · simp [mergeAdmitted, rightHit, Ne.symm sameClause]
              · simp [mergeAdmitted, leftHit, rightHit]

theorem admitted_merge_is_idempotent
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain)
    (result : AdmittedResult domain) :
    mergeAdmitted (mergeAdmitted ledger result) result =
      mergeAdmitted ledger result := by
  funext clause
  by_cases hit : clause = result.clause
  · simp only [mergeAdmitted, hit, if_true]
    rw [joinOutcome_associative, joinOutcome_idempotent]
  · simp [mergeAdmitted, hit]

def mergeMany
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain) :
    List (AdmittedResult domain) → NormalizedLedger domain
  | [] => ledger
  | result :: rest => mergeMany (mergeAdmitted ledger result) rest

theorem merge_after_many
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain)
    (result : AdmittedResult domain)
    (results : List (AdmittedResult domain)) :
    mergeAdmitted (mergeMany ledger results) result =
      mergeMany (mergeAdmitted ledger result) results := by
  induction results generalizing ledger with
  | nil => rfl
  | cons head tail inductionHypothesis =>
      simp only [mergeMany]
      rw [inductionHypothesis]
      rw [admitted_merges_commute ledger head result]

theorem mergeMany_permutation_invariant
    {domain : ValidationDomain}
    (ledger : NormalizedLedger domain)
    {left right : List (AdmittedResult domain)}
    (permutation : left.Perm right) :
    mergeMany ledger left = mergeMany ledger right := by
  induction permutation generalizing ledger with
  | nil => rfl
  | cons head permutation inductionHypothesis =>
      simp only [mergeMany]
      exact inductionHypothesis (mergeAdmitted ledger head)
  | swap first second tail =>
      simp only [mergeMany]
      exact congrArg
        (fun nextLedger => mergeMany nextLedger tail)
        (admitted_merges_commute ledger second first)
  | trans firstPermutation secondPermutation firstHypothesis secondHypothesis =>
      exact
        (firstHypothesis ledger).trans
          (secondHypothesis ledger)

structure ClauseResult where
  clause : ClauseId
  domain : ValidationDomain
  outcome : Outcome
  deriving DecidableEq, Repr

abbrev ResultStore := ClauseId → Option ClauseResult

def emptyStore : ResultStore :=
  fun _ => none

def writeResult (store : ResultStore) (result : ClauseResult) : ResultStore :=
  fun clause =>
    if clause = result.clause then
      some result
    else
      store clause

theorem independent_writes_commute
    (store : ResultStore)
    (left right : ClauseResult)
    (distinct : left.clause ≠ right.clause) :
    writeResult (writeResult store left) right =
      writeResult (writeResult store right) left := by
  funext clause
  by_cases leftClause : clause = left.clause
  · subst clause
    simp [writeResult, distinct]
  · by_cases rightClause : clause = right.clause
    · subst clause
      simp [writeResult, leftClause]
    · simp [writeResult, leftClause, rightClause]

inductive MergeConflict where
  | clauseIdentity
  | validationDomain
  deriving DecidableEq, Repr

inductive PairMerge where
  | merged (result : ClauseResult)
  | conflict (reason : MergeConflict)
  deriving DecidableEq, Repr

def mergePair (left right : ClauseResult) : PairMerge :=
  if left.clause = right.clause then
    if left.domain = right.domain then
      PairMerge.merged
        { clause := left.clause
          domain := left.domain
          outcome := joinOutcome left.outcome right.outcome }
    else
      PairMerge.conflict MergeConflict.validationDomain
  else
    PairMerge.conflict MergeConflict.clauseIdentity

theorem mergePair_commutative (left right : ClauseResult) :
    mergePair left right = mergePair right left := by
  cases left with
  | mk leftClause leftDomain leftOutcome =>
      cases right with
      | mk rightClause rightDomain rightOutcome =>
          by_cases sameClause : leftClause = rightClause
          · subst rightClause
            by_cases sameDomain : leftDomain = rightDomain
            · subst rightDomain
              simp [mergePair, joinOutcome_commutative]
            · simp [mergePair, sameDomain, Ne.symm sameDomain]
          · simp [mergePair, sameClause, Ne.symm sameClause]

theorem mergePair_idempotent (result : ClauseResult) :
    mergePair result result = PairMerge.merged result := by
  simp [mergePair, joinOutcome_idempotent]

theorem domain_mismatch_is_typed_conflict
    (left right : ClauseResult)
    (sameClause : left.clause = right.clause)
    (domainMismatch : left.domain ≠ right.domain) :
    mergePair left right =
      PairMerge.conflict MergeConflict.validationDomain := by
  simp [mergePair, sameClause, domainMismatch]

theorem clause_mismatch_is_typed_conflict
    (left right : ClauseResult)
    (clauseMismatch : left.clause ≠ right.clause) :
    mergePair left right =
      PairMerge.conflict MergeConflict.clauseIdentity := by
  simp [mergePair, clauseMismatch]

def pendingResult : ClauseResult :=
  { clause := 1
    domain := exampleDomain
    outcome := Outcome.pending }

def contradictoryResult : ClauseResult :=
  { clause := 1
    domain := exampleDomain
    outcome := Outcome.contradiction }

theorem last_write_wins_is_order_dependent :
    writeResult (writeResult emptyStore pendingResult) contradictoryResult ≠
      writeResult (writeResult emptyStore contradictoryResult) pendingResult := by
  intro equalStores
  have equalAtClause := congrFun equalStores 1
  simp [writeResult, pendingResult, contradictoryResult] at equalAtClause

theorem conflicting_outcomes_join_to_contradiction :
    mergePair pendingResult contradictoryResult =
      PairMerge.merged contradictoryResult := by
  rfl

end SearchLoopMerge
