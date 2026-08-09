import Std

namespace ASPProof.IncrementalMemorySearchPageStore

abbrev Digest := String
abbrev Owner := String

inductive SectionKind
  | ownerDirectory
  | ownerBytes
  | lexicalIndex
  | selectorIndex
  | graphRelations
  deriving DecidableEq, Repr

structure PageStore where
  present : Digest → Prop

structure Roots where
  sectionRoot : SectionKind → Digest

def Closed (store : PageStore) (reachable : Digest → Prop) : Prop :=
  ∀ digest, reachable digest → store.present digest

structure PreparedGeneration (store : PageStore) where
  epoch : Nat
  roots : Roots
  reachable : Digest → Prop
  rootsReachable : ∀ sectionKind, reachable (roots.sectionRoot sectionKind)
  closed : Closed store reachable

structure RuntimeState where
  store : PageStore
  pointer : Option (Nat × Roots)

def commit
    (state : RuntimeState)
    (prepared : PreparedGeneration state.store) : RuntimeState :=
  { state with pointer := some (prepared.epoch, prepared.roots) }

theorem committed_roots_are_present
    (state : RuntimeState)
    (prepared : PreparedGeneration state.store)
    (sectionKind : SectionKind) :
    (commit state prepared).store.present (prepared.roots.sectionRoot sectionKind) := by
  exact prepared.closed _ (prepared.rootsReachable sectionKind)

structure IncrementalUpdate (before after : PreparedGeneration store) where
  changedOwners : Owner → Prop
  changedPage : Digest → Prop
  unchangedPageReused :
    ∀ digest, before.reachable digest → ¬ changedPage digest → after.reachable digest
  unchangedDigestStable :
    ∀ sectionKind,
      (∀ digest, before.reachable digest → changedPage digest → False) →
      after.roots.sectionRoot sectionKind = before.roots.sectionRoot sectionKind

theorem unchanged_update_preserves_section_root
    {before after : PreparedGeneration store}
    (update : IncrementalUpdate before after)
    (sectionKind : SectionKind)
    (noChangedPage : ∀ digest, before.reachable digest → update.changedPage digest → False) :
    after.roots.sectionRoot sectionKind = before.roots.sectionRoot sectionKind := by
  exact update.unchangedDigestStable sectionKind noChangedPage

structure TombstoneTransition
    (before after : PreparedGeneration store)
    (owner : Owner) where
  ownerLeaf : Digest
  reachableBefore : before.reachable ownerLeaf
  unreachableAfter : ¬ after.reachable ownerLeaf

theorem committed_tombstone_cannot_resolve_old_owner_leaf
    {state : RuntimeState}
    {before after : PreparedGeneration state.store}
    {owner : Owner}
    (tombstone : TombstoneTransition before after owner) :
    ¬ after.reachable tombstone.ownerLeaf := by
  exact tombstone.unreachableAfter

inductive ReadEffect
  | mapRoot
  | mapPage
  | validateDigest
  | returnRecord
  | openTurso
  | connectSocket
  | spawnProvider
  | publishGeneration
  deriving DecidableEq, Repr

def AdmittedReadEffect (effect : ReadEffect) : Prop :=
  effect = .mapRoot ∨ effect = .mapPage ∨ effect = .validateDigest ∨
    effect = .returnRecord

theorem admitted_read_has_no_control_plane_effect
    {effect : ReadEffect}
    (admitted : AdmittedReadEffect effect) :
    effect ≠ .openTurso ∧ effect ≠ .connectSocket ∧
      effect ≠ .spawnProvider ∧ effect ≠ .publishGeneration := by
  rcases admitted with h | h | h | h <;> subst effect <;> decide

end ASPProof.IncrementalMemorySearchPageStore
