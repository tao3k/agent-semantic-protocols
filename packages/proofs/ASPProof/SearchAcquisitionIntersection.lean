-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

/-!
Executable V1 countermodel for Scheme acquisition lowering.

Every rg and Tantivy leaf under the admitted acquisition `intersect` is a
necessary predicate. Grouping same-axis leaves with union changes both the
result and the amount of downstream exact work.
-/

namespace ASPProof.SearchAcquisitionIntersection

abbrev Owner := Nat
abbrev OwnerScope := List Owner

def inEveryScope (owner : Owner) (scopes : List OwnerScope) : Bool :=
  scopes.all fun scope => scope.contains owner

def inAnyScope (owner : Owner) (scopes : List OwnerScope) : Bool :=
  scopes.any fun scope => scope.contains owner

def intendedIntersect
    (owner : Owner) (rgScopes tantivyScopes : List OwnerScope) : Bool :=
  inEveryScope owner rgScopes && inEveryScope owner tantivyScopes

def flattenedAxisUnion
    (owner : Owner) (rgScopes tantivyScopes : List OwnerScope) : Bool :=
  inAnyScope owner rgScopes && inAnyScope owner tantivyScopes

def admittedOwners
    (candidates : OwnerScope)
    (predicate : Owner → Bool) : OwnerScope :=
  candidates.filter predicate

def rgScopes : List OwnerScope := [[1, 2], [2, 3]]
def tantivyScopes : List OwnerScope := [[1, 2], [2, 3]]

theorem same_axis_union_changes_v1_scheme_meaning :
    admittedOwners [1, 2, 3, 4, 5]
      (fun owner => intendedIntersect owner rgScopes tantivyScopes) = [2] ∧
    admittedOwners [1, 2, 3, 4, 5]
      (fun owner => flattenedAxisUnion owner rgScopes tantivyScopes) = [1, 2, 3] := by
  decide

theorem flattened_plan_amplifies_downstream_owner_work :
    (admittedOwners [1, 2, 3, 4, 5]
      (fun owner => intendedIntersect owner rgScopes tantivyScopes)).length = 1 ∧
    (admittedOwners [1, 2, 3, 4, 5]
      (fun owner => flattenedAxisUnion owner rgScopes tantivyScopes)).length = 3 := by
  decide

end ASPProof.SearchAcquisitionIntersection
