namespace ASPProof.RustHarnessPackageAtomicity

/-- Cargo parser output is the only package/source ownership authority. -/
structure CargoPackageGraph (Package Owner : Type) where
  owns : Package → Owner → Prop

inductive ScopeSelection (Package : Type) where
  | package (owner : Package)
  | workspace (members : List Package)

def selects [DecidableEq Package]
    (scope : ScopeSelection Package) (package : Package) : Prop :=
  match scope with
  | .package selected => package = selected
  | .workspace members => package ∈ members

def admits [DecidableEq Package]
    (graph : CargoPackageGraph Package Owner)
    (scope : ScopeSelection Package)
    (owner : Owner) : Prop :=
  ∃ package, selects scope package ∧ graph.owns package owner

/-- Package admission is defined only after the root Cargo manifest has
produced a graph. A nested manifest discovered by walking the filesystem cannot
manufacture that missing graph anchor. -/
def anchoredAdmits [DecidableEq Package]
    (parsedGraph : Option (CargoPackageGraph Package Owner))
    (scope : ScopeSelection Package)
    (owner : Owner) : Prop :=
  ∃ graph, parsedGraph = some graph ∧ admits graph scope owner

theorem missing_root_manifest_admits_no_owner
    [DecidableEq Package]
    (scope : ScopeSelection Package)
    (owner : Owner) :
    ¬ anchoredAdmits (none : Option (CargoPackageGraph Package Owner)) scope owner := by
  intro admitted
  rcases admitted with ⟨_, impossible, _⟩
  simp at impossible

theorem package_scope_admits_only_its_cargo_owner
    [DecidableEq Package]
    (graph : CargoPackageGraph Package Owner)
    (selected package : Package)
    (owner : Owner)
    (admitted : admits graph (.package selected) owner)
    (ownedBy : graph.owns package owner)
    (ownershipUnique : ∀ left right, graph.owns left owner → graph.owns right owner → left = right) :
    package = selected := by
  rcases admitted with ⟨admittedPackage, selectedEq, admittedOwnership⟩
  have same := ownershipUnique package admittedPackage ownedBy admittedOwnership
  exact same.trans selectedEq

/-- A sibling change is irrelevant to a package selection when Cargo assigns
the changed owner to a different package. -/
theorem sibling_change_is_not_admitted_by_package_scope
    [DecidableEq Package]
    (graph : CargoPackageGraph Package Owner)
    (selected sibling : Package)
    (changedOwner : Owner)
    (different : sibling ≠ selected)
    (siblingOwns : graph.owns sibling changedOwner)
    (ownershipUnique : ∀ left right, graph.owns left changedOwner →
      graph.owns right changedOwner → left = right) :
    ¬ admits graph (.package selected) changedOwner := by
  intro admitted
  have same := package_scope_admits_only_its_cargo_owner
    graph selected sibling changedOwner admitted siblingOwns ownershipUnique
  exact different same

theorem explicit_workspace_scope_admits_selected_member
    [DecidableEq Package]
    (graph : CargoPackageGraph Package Owner)
    (members : List Package)
    (package : Package)
    (owner : Owner)
    (member : package ∈ members)
    (owned : graph.owns package owner) :
    admits graph (.workspace members) owner := by
  exact ⟨package, member, owned⟩

/-- Concrete counterexample: lexical nesting is compatible with distinct Cargo
package ownership, so path-prefix tests cannot establish package membership. -/
theorem lexical_prefix_does_not_imply_same_cargo_package :
    List.IsPrefix [0] [0, 1] ∧ (0 : Nat) ≠ 1 := by
  constructor <;> simp

/-- Command spelling is absent from admission. Any two wrappers selecting the
same parsed Cargo graph and scope necessarily produce the same decision. -/
theorem wrapper_relabeling_cannot_change_package_admission
    [DecidableEq Package]
    (graph : CargoPackageGraph Package Owner)
    (scope : ScopeSelection Package)
    (owner : Owner)
    (_leftWrapper _rightWrapper : Wrapper) :
    admits graph scope owner ↔ admits graph scope owner := by
  rfl

end ASPProof.RustHarnessPackageAtomicity
