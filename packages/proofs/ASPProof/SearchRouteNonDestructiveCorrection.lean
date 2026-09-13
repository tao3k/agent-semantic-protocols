-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteMonotonePartialResolution

namespace ASPProof.SearchRouteNonDestructiveCorrection

open ASPProof.SearchRoutePerDimensionEffectResolution
open ASPProof.SearchRouteMonotonePartialResolution

inductive ResourceDimension where
  | tokens
  | money
  | providerQuota
  deriving DecidableEq, Repr

inductive FinalOutcome where
  | consumed
  | noEffect
  deriving DecidableEq, Repr

def FinalOutcome.asDimensionOutcome : FinalOutcome → DimensionOutcome
  | .consumed => .consumed
  | .noEffect => .noEffect

structure FinalReceipt where
  digest : Nat
  requestDigest : Nat
  effectNonce : Nat
  dimension : ResourceDimension
  outcome : FinalOutcome
  revision : Nat
  deriving DecidableEq, Repr

structure CorrectionAuthority where
  digest : Nat
  dimension : ResourceDimension
  deriving DecidableEq, Repr

structure CorrectionReceipt where
  originalDigest : Nat
  requestDigest : Nat
  effectNonce : Nat
  dimension : ResourceDimension
  before : FinalOutcome
  after : FinalOutcome
  authorityDigest : Nat
  contradictionDigest : Nat
  revision : Nat
  deriving DecidableEq, Repr

def CorrectionBound
    (original : FinalReceipt)
    (authority : CorrectionAuthority)
    (correction : CorrectionReceipt) : Prop :=
  correction.originalDigest = original.digest ∧
  correction.requestDigest = original.requestDigest ∧
  correction.effectNonce = original.effectNonce ∧
  correction.dimension = original.dimension ∧
  correction.before = original.outcome ∧
  authority.dimension = original.dimension ∧
  correction.authorityDigest = authority.digest ∧
  original.revision < correction.revision

structure CorrectedView where
  original : FinalReceipt
  authority : CorrectionAuthority
  correction : CorrectionReceipt
  bound : CorrectionBound original authority correction

def applyCorrection
    (original : FinalReceipt)
    (authority : CorrectionAuthority)
    (correction : CorrectionReceipt)
    (bound : CorrectionBound original authority correction) :
    CorrectedView :=
  {
    original := original
    authority := authority
    correction := correction
    bound := bound
  }

def CorrectedView.effectiveOutcome
    (view : CorrectedView) : FinalOutcome :=
  view.correction.after

theorem apply_correction_preserves_original
    (original : FinalReceipt)
    (authority : CorrectionAuthority)
    (correction : CorrectionReceipt)
    (bound : CorrectionBound original authority correction) :
    (applyCorrection original authority correction bound).original =
      original := by
  rfl

theorem corrected_view_carries_authorization
    (view : CorrectedView) :
    CorrectionBound view.original view.authority view.correction :=
  view.bound

theorem corrected_view_advances_revision
    (view : CorrectedView) :
    view.original.revision < view.correction.revision :=
  view.bound.2.2.2.2.2.2.2

theorem ordinary_consumed_to_no_effect_remains_invalid :
    ¬ OutcomeRefines .consumed .noEffect := by
  unfold OutcomeRefines
  decide

def originalMoneyReceipt : FinalReceipt :=
  {
    digest := 151
    requestDigest := 111
    effectNonce := 121
    dimension := .money
    outcome := .consumed
    revision := 2
  }

def moneyCorrectionAuthority : CorrectionAuthority :=
  {
    digest := 161
    dimension := .money
  }

def moneyCorrection : CorrectionReceipt :=
  {
    originalDigest := 151
    requestDigest := 111
    effectNonce := 121
    dimension := .money
    before := .consumed
    after := .noEffect
    authorityDigest := 161
    contradictionDigest := 171
    revision := 3
  }

theorem example_correction_is_bound :
    CorrectionBound
      originalMoneyReceipt
      moneyCorrectionAuthority
      moneyCorrection := by
  unfold CorrectionBound originalMoneyReceipt moneyCorrectionAuthority
    moneyCorrection
  decide

def correctedMoneyView : CorrectedView :=
  applyCorrection
    originalMoneyReceipt
    moneyCorrectionAuthority
    moneyCorrection
    example_correction_is_bound

theorem example_historical_outcome_remains_consumed :
    correctedMoneyView.original.outcome = .consumed := by
  decide

theorem example_effective_outcome_becomes_no_effect :
    correctedMoneyView.effectiveOutcome = .noEffect := by
  decide

def wrongAuthority : CorrectionAuthority :=
  {
    digest := 999
    dimension := .money
  }

theorem mismatched_authority_rejects_correction :
    ¬ CorrectionBound
      originalMoneyReceipt
      wrongAuthority
      moneyCorrection := by
  unfold CorrectionBound originalMoneyReceipt wrongAuthority moneyCorrection
  decide

def wrongRequestCorrection : CorrectionReceipt :=
  { moneyCorrection with requestDigest := 999 }

theorem mismatched_request_rejects_correction :
    ¬ CorrectionBound
      originalMoneyReceipt
      moneyCorrectionAuthority
      wrongRequestCorrection := by
  unfold CorrectionBound originalMoneyReceipt moneyCorrectionAuthority
    wrongRequestCorrection moneyCorrection
  decide

def wrongDimensionCorrection : CorrectionReceipt :=
  { moneyCorrection with dimension := .tokens }

theorem mismatched_dimension_rejects_correction :
    ¬ CorrectionBound
      originalMoneyReceipt
      moneyCorrectionAuthority
      wrongDimensionCorrection := by
  unfold CorrectionBound originalMoneyReceipt moneyCorrectionAuthority
    wrongDimensionCorrection moneyCorrection
  decide

end ASPProof.SearchRouteNonDestructiveCorrection
