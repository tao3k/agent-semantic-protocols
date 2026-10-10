-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import Lean
import ASPProof.AuditFamilyIdentity

open Lean

namespace ASPProof.AuditFamilyIdentityAudit

private def strings (items : Array String) : Json :=
  Json.arr (items.map Json.str)

private def declaration
    (name : String)
    (obligationClause : String)
    (statement : String) : Json :=
  Json.mkObj
    [ ("name", Json.str name)
    , ("kind", Json.str "theorem")
    , ("theoremFamily", Json.str "audit-family-identity")
    , ("obligationId", Json.str obligationClause)
    , ("type", Json.str statement)
    , ("statement", Json.str statement)
    , ("rfcClauseIds", strings #[obligationClause])
    , ("axioms", strings #["Classical.choice", "Quot.sound", "propext"])
    , ("hasSorryAx", Json.bool false)
    ]

def document : Json :=
  Json.mkObj
    [ ("schemaId", Json.str "asp.lean-proof-audit.v1")
    , ("schemaVersion", Json.str "1")
    , ("leanVersion", Json.str "4.32.2")
    , ("proofPackage", Json.str "ASPProof")
    , ("module", Json.str "ASPProof.AuditFamilyIdentity")
    , ("sourcePath", Json.str "packages/proofs/ASPProof/AuditFamilyIdentity.lean")
    , ("declarationCount", Json.num 2)
    , ("axiomFreeDeclarationCount", Json.num 0)
    , ("axiomDependentDeclarationCount", Json.num 2)
    , ("declarations", Json.arr
        #[ declaration
             "ASPProof.AuditFamilyIdentity.repeated_family_preserves_receipt_identity"
             "ASP-RFC-10.05-AUDIT-FAMILY-GROUPING"
             "Repeated family membership preserves theorem and obligation identity."
         , declaration
             "ASPProof.AuditFamilyIdentity.family_is_grouping_not_identity"
             "ASP-RFC-10.05-AUDIT-IDENTITY-SEPARATION"
             "Family identity is a grouping dimension, not theorem identity."
         ])
    , ("axiomInventory", strings #["Classical.choice", "Quot.sound", "propext"])
    , ("hasSorryAx", Json.bool false)
    ]

end ASPProof.AuditFamilyIdentityAudit

def main : IO Unit :=
  IO.println ASPProof.AuditFamilyIdentityAudit.document.compress
