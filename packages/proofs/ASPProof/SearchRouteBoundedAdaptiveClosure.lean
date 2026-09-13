-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

namespace ASPProof.SearchRouteBoundedAdaptiveClosure

structure ClosureState where
  undiscoveredRoutes : Nat
  activeInspectPotential : Nat
  deriving DecidableEq

def closureMeasure (maxInspectPerRoute : Nat) (state : ClosureState) : Nat :=
  state.undiscoveredRoutes * (maxInspectPerRoute + 1) +
  state.activeInspectPotential

def Closed (state : ClosureState) : Prop :=
  state.undiscoveredRoutes = 0 ∧ state.activeInspectPotential = 0

inductive CertifiedTransition
    (maxInspectPerRoute : Nat) : ClosureState → ClosureState → Prop where
  | inspect {before after : ClosureState}
      (sameCatalog :
        after.undiscoveredRoutes = before.undiscoveredRoutes)
      (strictInspectProgress :
        after.activeInspectPotential < before.activeInspectPotential) :
      CertifiedTransition maxInspectPerRoute before after
  | discover {before after : ClosureState}
      (discoversOne :
        before.undiscoveredRoutes = after.undiscoveredRoutes + 1)
      (boundedNewInspect :
        after.activeInspectPotential ≤
          before.activeInspectPotential + maxInspectPerRoute) :
      CertifiedTransition maxInspectPerRoute before after

theorem certified_transition_decreases
    {maxInspectPerRoute : Nat}
    {before after : ClosureState}
    (transition :
      CertifiedTransition maxInspectPerRoute before after) :
    closureMeasure maxInspectPerRoute after <
      closureMeasure maxInspectPerRoute before := by
  cases transition with
  | inspect sameCatalog strictInspectProgress =>
      calc
        closureMeasure maxInspectPerRoute after =
            before.undiscoveredRoutes * (maxInspectPerRoute + 1) +
              after.activeInspectPotential := by
                simp [closureMeasure, sameCatalog]
        _ < before.undiscoveredRoutes * (maxInspectPerRoute + 1) +
              before.activeInspectPotential :=
                Nat.add_lt_add_left strictInspectProgress _
        _ = closureMeasure maxInspectPerRoute before := rfl
  | discover discoversOne boundedNewInspect =>
      calc
        closureMeasure maxInspectPerRoute after =
            after.undiscoveredRoutes * (maxInspectPerRoute + 1) +
              after.activeInspectPotential := rfl
        _ ≤ after.undiscoveredRoutes * (maxInspectPerRoute + 1) +
              (before.activeInspectPotential + maxInspectPerRoute) :=
                Nat.add_le_add_left boundedNewInspect _
        _ < (after.undiscoveredRoutes + 1) *
              (maxInspectPerRoute + 1) +
              before.activeInspectPotential := by
                have rearrange :
                    after.undiscoveredRoutes * (maxInspectPerRoute + 1) +
                        (before.activeInspectPotential + maxInspectPerRoute) + 1 =
                      (after.undiscoveredRoutes + 1) *
                          (maxInspectPerRoute + 1) +
                        before.activeInspectPotential := by
                  simp only
                    [ Nat.add_mul
                    , Nat.one_mul
                    , Nat.add_assoc
                    , Nat.add_comm
                    , Nat.add_left_comm
                    ]
                rw [← rearrange]
                exact Nat.lt_succ_self _
        _ = closureMeasure maxInspectPerRoute before := by
              simp [closureMeasure, discoversOne]

inductive CertifiedRun
    (maxInspectPerRoute : Nat) :
    ClosureState → ClosureState → Nat → Prop where
  | zero {state : ClosureState} :
      CertifiedRun maxInspectPerRoute state state 0
  | next {start middle finish : ClosureState} {rounds : Nat} :
      CertifiedTransition maxInspectPerRoute start middle →
      CertifiedRun maxInspectPerRoute middle finish rounds →
      CertifiedRun maxInspectPerRoute start finish (rounds + 1)

theorem certified_run_round_bound
    {maxInspectPerRoute rounds : Nat}
    {start finish : ClosureState}
    (run : CertifiedRun maxInspectPerRoute start finish rounds) :
    rounds ≤ closureMeasure maxInspectPerRoute start := by
  induction run with
  | zero =>
      exact Nat.zero_le _
  | next transition _ inductionHypothesis =>
      exact Nat.le_trans
        (Nat.succ_le_succ inductionHypothesis)
        (certified_transition_decreases transition)

theorem zero_measure_iff_closed
    (maxInspectPerRoute : Nat)
    (state : ClosureState) :
    closureMeasure maxInspectPerRoute state = 0 ↔ Closed state := by
  constructor
  · intro measureZero
    have componentsZero := Nat.add_eq_zero_iff.mp measureZero
    constructor
    · rcases Nat.mul_eq_zero.mp componentsZero.1 with
        routesZero | factorZero
      · exact routesZero
      · exact False.elim (Nat.succ_ne_zero _ factorZero)
    · exact componentsZero.2
  · intro closed
    simp [closureMeasure, closed.1, closed.2]

theorem no_certified_self_loop
    (maxInspectPerRoute : Nat)
    (state : ClosureState) :
    ¬ CertifiedTransition maxInspectPerRoute state state := by
  intro transition
  exact Nat.lt_irrefl _
    (certified_transition_decreases transition)

def beforeDiscovery : ClosureState where
  undiscoveredRoutes := 1
  activeInspectPotential := 0

def afterDiscovery : ClosureState where
  undiscoveredRoutes := 0
  activeInspectPotential := 3

theorem bounded_discovery_is_certified :
    CertifiedTransition 3 beforeDiscovery afterDiscovery :=
  CertifiedTransition.discover rfl (by decide)

theorem weighted_measure_decreases_on_discovery :
    closureMeasure 3 afterDiscovery < closureMeasure 3 beforeDiscovery :=
  certified_transition_decreases bounded_discovery_is_certified

def inspectOnlyMeasure (state : ClosureState) : Nat :=
  state.activeInspectPotential

theorem inspect_only_measure_increases_on_discovery :
    inspectOnlyMeasure beforeDiscovery <
      inspectOnlyMeasure afterDiscovery := by
  decide

theorem zero_inspect_potential_does_not_imply_closed :
    inspectOnlyMeasure beforeDiscovery = 0 ∧
    ¬ Closed beforeDiscovery := by
  constructor
  · rfl
  · simp [Closed, beforeDiscovery]

def UncertifiedTransition (before after : ClosureState) : Prop :=
  before = after

theorem uncertified_self_loop_exists :
    UncertifiedTransition beforeDiscovery beforeDiscovery :=
  rfl

end ASPProof.SearchRouteBoundedAdaptiveClosure
