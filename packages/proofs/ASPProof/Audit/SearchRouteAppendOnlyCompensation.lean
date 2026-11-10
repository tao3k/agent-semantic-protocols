-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteAppendOnlyCompensation
import Lean

open Lean

namespace ASPProof.Audit.SearchRouteAppendOnlyCompensation

def theoremDeclaration
    (name theoremFamily type rfcClauseId : String)
    (axioms : List String) : Json :=
  Json.mkObj
    [ ("name", toJson name)
    , ("kind", toJson "theorem")
    , ("theoremFamily", toJson theoremFamily)
    , ("type", toJson type)
    , ("rfcClauseIds", toJson [rfcClauseId])
    , ("axioms", toJson axioms)
    , ("hasSorryAx", toJson false)
    ]

def declarations : Array Json :=
  #[ theoremDeclaration
       "adjustment_preserves_balance"
       "ledger-conservation"
       "Balanced ledger -> CanAdjust ledger amount kind -> Balanced (applyAdjustment ledger amount kind)"
       "ASP-RFC-10.05-ALC-BALANCE"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "gross_consumed_is_append_only"
       "historical-monotonicity"
       "ledger.grossConsumed <= (applyAdjustment ledger amount kind).grossConsumed"
       "ASP-RFC-10.05-ALC-GROSS-HISTORY"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "credits_are_append_only"
       "historical-monotonicity"
       "ledger.credits <= (applyAdjustment ledger amount kind).credits"
       "ASP-RFC-10.05-ALC-CREDIT-HISTORY"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "allowed_credit_remains_bounded_by_history"
       "credit-boundedness"
       "CanAdjust ledger amount credit -> adjusted.credits <= adjusted.grossConsumed"
       "ASP-RFC-10.05-ALC-CREDIT-BOUND"
       []
   , theoremDeclaration
       "authorized_compensation_preserves_balance"
       "authorized-compensation"
       "CanCommit record proposal -> CanAdjust ledger proposal.amount proposal.kind -> Balanced ledger -> Balanced adjusted"
       "ASP-RFC-10.05-ALC-AUTHORIZED-COMMIT"
       ["Quot.sound", "propext"]
   , theoremDeclaration
       "compensation_commit_advances_revision"
       "compensation-cas"
       "committed.revision = record.revision + 1"
       "ASP-RFC-10.05-ALC-REVISION"
       []
   , theoremDeclaration
       "committed_record_rejects_replay"
       "compensation-cas"
       "not (CanCommit (commitRecord record proposal) proposal)"
       "ASP-RFC-10.05-ALC-REPLAY"
       ["propext"]
   ]

def manifest : Json :=
  Json.mkObj
    [ ("schemaId", toJson "asp.lean-proof-audit.v1")
    , ("schemaVersion", toJson "1")
    , ("leanVersion", toJson "4.32.2")
    , ("proofPackage", toJson "ASPProof")
    , ("module", toJson "ASPProof.SearchRouteAppendOnlyCompensation")
    , ("sourcePath",
        toJson "ASPProof/SearchRouteAppendOnlyCompensation.lean")
    , ("declarationCount", toJson declarations.size)
    , ("axiomFreeDeclarationCount", toJson 2)
    , ("axiomDependentDeclarationCount", toJson 5)
    , ("declarations", toJson declarations)
    , ("axiomInventory", toJson ["Quot.sound", "propext"])
    , ("hasSorryAx", toJson false)
    , ("rfc", toJson "00.58-append-only-compensation-ledger")
    , ("status", toJson "kernel-compiled")
    , ("obligations",
        toJson
          [ "adjustment_preserves_balance"
          , "gross_consumed_is_append_only"
          , "credits_are_append_only"
          , "allowed_credit_remains_bounded_by_history"
          , "authorized_compensation_preserves_balance"
          , "compensation_commit_advances_revision"
          , "committed_record_rejects_replay"
          ])
    ]

end ASPProof.Audit.SearchRouteAppendOnlyCompensation
