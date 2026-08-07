import Std

namespace ASPProof.ASPWorkspaceGenerationRelationOverlay

structure Relation where
  owner : String
  fromKind : String
  fromId : String
  relationKind : String
  toKind : String
  toId : String
  deriving DecidableEq, Repr

def overlayRelations
    (changed : String → Prop)
    [DecidablePred changed]
    (base replacement : List Relation) : List Relation :=
  base.filter (fun relation => ¬ changed relation.owner) ++ replacement

theorem unchanged_relation_is_preserved
    (changed : String → Prop)
    [DecidablePred changed]
    (base replacement : List Relation)
    (relation : Relation)
    (present : relation ∈ base)
    (unchanged : ¬ changed relation.owner) :
    relation ∈ overlayRelations changed base replacement := by
  simp [overlayRelations, present, unchanged]

theorem changed_relation_requires_replacement_evidence
    (changed : String → Prop)
    [DecidablePred changed]
    (base replacement : List Relation)
    (relation : Relation)
    (visible : relation ∈ overlayRelations changed base replacement)
    (isChanged : changed relation.owner) :
    relation ∈ replacement := by
  simp [overlayRelations, isChanged] at visible
  exact visible

theorem unchanged_generation_is_identity
    (base : List Relation) :
    overlayRelations (fun _ => False) base [] = base := by
  simp [overlayRelations]

end ASPProof.ASPWorkspaceGenerationRelationOverlay
