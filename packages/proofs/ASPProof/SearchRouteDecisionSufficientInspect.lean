-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteDecisionSufficientInspect

structure InspectSurface
    (World Projection Action : Type) where
  tokenCost : Nat
  project : World → Projection
  decode : Projection → Action

def DecisionCorrect
    {World Projection Action : Type}
    (surface : InspectSurface World Projection Action)
    (requiredAction : World → Action) : Prop :=
  ∀ world,
    surface.decode (surface.project world) = requiredAction world

inductive ExampleWorld where
  | selectorReady
  | selectorMissing
  deriving DecidableEq, Repr

inductive NextAction where
  | execute
  | resolveSelector
  deriving DecidableEq, Repr

def requiredAction : ExampleWorld → NextAction
  | .selectorReady => .execute
  | .selectorMissing => .resolveSelector

inductive TinyProjection where
  | providerReady
  deriving DecidableEq, Repr

inductive StatusProjection where
  | ready
  | missing
  deriving DecidableEq, Repr

def tinyProject (_world : ExampleWorld) : TinyProjection :=
  .providerReady

def statusProject : ExampleWorld → StatusProjection
  | .selectorReady => .ready
  | .selectorMissing => .missing

def statusDecode : StatusProjection → NextAction
  | .ready => .execute
  | .missing => .resolveSelector

theorem no_tiny_decoder_is_decision_correct :
    ¬ ∃ decode : TinyProjection → NextAction,
      ∀ world, decode (tinyProject world) = requiredAction world := by
  intro alleged
  obtain ⟨decode, correct⟩ := alleged
  have ready := correct .selectorReady
  have missing := correct .selectorMissing
  have impossible : NextAction.execute = NextAction.resolveSelector :=
    Eq.trans ready.symm missing
  cases impossible

def statusSurface :
    InspectSurface ExampleWorld StatusProjection NextAction :=
  {
    tokenCost := 2
    project := statusProject
    decode := statusDecode
  }

theorem status_projection_is_decision_correct :
    DecisionCorrect statusSurface requiredAction := by
  intro world
  cases world <;> rfl

def fullStateSurface :
    InspectSurface ExampleWorld ExampleWorld NextAction :=
  {
    tokenCost := 10
    project := fun world => world
    decode := requiredAction
  }

theorem full_state_projection_is_decision_correct :
    DecisionCorrect fullStateSurface requiredAction := by
  intro world
  rfl

theorem status_projection_is_cheaper_than_full_state :
    statusSurface.tokenCost < fullStateSurface.tokenCost := by
  decide

def misleadingTinySurface :
    InspectSurface ExampleWorld TinyProjection NextAction :=
  {
    tokenCost := 1
    project := tinyProject
    decode := fun _ => .execute
  }

theorem misleading_tiny_surface_is_cheaper :
    misleadingTinySurface.tokenCost < statusSurface.tokenCost := by
  decide

theorem misleading_tiny_surface_is_not_decision_correct :
    ¬ DecisionCorrect misleadingTinySurface requiredAction := by
  intro correct
  have missing := correct .selectorMissing
  cases missing

theorem compactness_does_not_imply_decision_sufficiency :
    misleadingTinySurface.tokenCost < statusSurface.tokenCost ∧
      ¬ DecisionCorrect misleadingTinySurface requiredAction ∧
      DecisionCorrect statusSurface requiredAction :=
  ⟨
    misleading_tiny_surface_is_cheaper,
    misleading_tiny_surface_is_not_decision_correct,
    status_projection_is_decision_correct
  ⟩

end ASPProof.SearchRouteDecisionSufficientInspect
