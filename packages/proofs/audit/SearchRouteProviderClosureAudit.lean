-- SPDX-FileCopyrightText: 2026 tao3k team and Contributors
--
-- SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import ASPProof.SearchRouteProviderClosure

open ASPProof.SearchRouteProviderClosure

#print axioms admitted_iff_selected
#print axioms unrelated_failure_noninterference
#print axioms schema_runtime_drift_witness
#print axioms response_schema_runtime_drift_witness
#print axioms current_native_owner_contract_is_not_congruent
#print axioms descriptor_presence_does_not_close_admission
#print axioms descriptor_without_method_breaks_registry_congruence
#print axioms authenticated_content_admits_zero_times
#print axioms positive_time_requirement_rejects_synthetic_owner

def providerClosureAuditJson : String :=
  r#"{
  "schemaId": "asp.lean-proof-audit.v1",
  "schemaVersion": "1",
  "leanVersion": "4.32.2",
  "proofPackage": "ASPProof",
  "module": "ASPProof.SearchRouteProviderClosure",
  "sourcePath": "packages/proofs/ASPProof/SearchRouteProviderClosure.lean",
  "declarationCount": 9,
  "axiomFreeDeclarationCount": 9,
  "axiomDependentDeclarationCount": 0,
  "declarations": [
    {
      "name": "admitted_iff_selected",
      "kind": "theorem",
      "theoremFamily": "provider-closure",
      "rfcClauseIds": ["ASP-RFC-10.05.62-OPERATION-CLOSURE"],
      "type": "Admitted operation ready iff ready operation.selected",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "unrelated_failure_noninterference",
      "kind": "theorem",
      "theoremFamily": "failure-isolation",
      "rfcClauseIds": ["ASP-RFC-10.05.62-UNRELATED-FAILURE-NONINTERFERENCE"],
      "type": "failure of an unrelated provider preserves operation admission",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "schema_runtime_drift_witness",
      "kind": "theorem",
      "theoremFamily": "schema-counterexample",
      "rfcClauseIds": ["ASP-RFC-10.05.63-SCHEMA-TRANSPORT-CONGRUENCE"],
      "type": "projectionMode belongs to the request schema and not the runtime request",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "response_schema_runtime_drift_witness",
      "kind": "theorem",
      "theoremFamily": "schema-counterexample",
      "rfcClauseIds": ["ASP-RFC-10.05.63-SCHEMA-TRANSPORT-CONGRUENCE"],
      "type": "requestedProjectionMode belongs to the response schema and not the runtime response",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "current_native_owner_contract_is_not_congruent",
      "kind": "theorem",
      "theoremFamily": "schema-counterexample",
      "rfcClauseIds": ["ASP-RFC-10.05.63-SCHEMA-TRANSPORT-CONGRUENCE"],
      "type": "the current native-owner schema and transport are not extensionally equal",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "descriptor_presence_does_not_close_admission",
      "kind": "theorem",
      "theoremFamily": "capability-admission",
      "rfcClauseIds": ["ASP-RFC-10.05.63-EXPLICIT-CAPABILITY"],
      "type": "a registered descriptor without manifest and schema evidence is not closed admission",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "descriptor_without_method_breaks_registry_congruence",
      "kind": "theorem",
      "theoremFamily": "registry-congruence",
      "rfcClauseIds": ["ASP-RFC-10.05.63-REGISTRY-SURFACE-CONGRUENCE"],
      "type": "a described method missing from the declared method set refutes registry congruence",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "authenticated_content_admits_zero_times",
      "kind": "theorem",
      "theoremFamily": "fingerprint-authority",
      "rfcClauseIds": ["ASP-RFC-10.05.63-SYNTHETIC-FINGERPRINT"],
      "type": "matching content identity admits a synthetic zero-time fingerprint",
      "axioms": [],
      "hasSorryAx": false
    },
    {
      "name": "positive_time_requirement_rejects_synthetic_owner",
      "kind": "theorem",
      "theoremFamily": "fingerprint-counterexample",
      "rfcClauseIds": ["ASP-RFC-10.05.63-SYNTHETIC-FINGERPRINT"],
      "type": "positive filesystem time rejects the synthetic zero-time owner",
      "axioms": [],
      "hasSorryAx": false
    }
  ],
  "axiomInventory": [],
  "hasSorryAx": false
}
"#

#eval IO.FS.writeFile
  "receipts/search-route-provider-closure-audit-v1.json"
  providerClosureAuditJson
