# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

def digest(byte):
    return "blake3-256:" + byte * 64


def search_playbook_receipt():
    generation = "blake3-256:generation"
    owners = ["src/router.rs"]
    stages = [
        {"family": "acquire", "capabilityId": "search.source-byte-acquisition"},
        {"family": "acquire", "capabilityId": "search.resident-lexical-recall"},
        {"family": "syntax", "capabilityId": "search.native-syntax-playbook"},
        {"family": "acquire", "capabilityId": "search.tantivy-lexical"},
        {"family": "reason", "capabilityId": "search.rust-resident-graph"},
        {"family": "reason", "capabilityId": "search.python-graph"},
    ]
    zero_work = {
        "databaseReadCount": 0,
        "filesystemReadCount": 0,
        "providerProcessCount": 0,
        "socketOperationCount": 0,
        "schedulerTaskCount": 0,
    }
    native_syntax = {
        "state": "ready",
        "stageArtifactDigest": digest("a"),
        "projections": [
            {
                "ownerPath": owners[0],
                "contentDigest": digest("b"),
                "selectors": [
                    {
                        "selector": "rust://src/router.rs#item/function/route",
                        "byteStart": 12,
                        "byteEnd": 48,
                        "queryKeys": ["route", "router"],
                        "derivedProjectionDigest": digest("c"),
                    }
                ],
            }
        ],
        "relations": [{"ownerPath": owners[0], "relationDigest": digest("d")}],
        "diagnostics": [],
        "properByteRanges": True,
        "nonEmptyQueryKeys": True,
        "relationsBoundToOwners": True,
        "complete": True,
    }
    evidence = {
        "sourceAcquisition": {
            "state": "admitted",
            "stageArtifactDigest": digest("e"),
            "generationDigest": generation,
            "admittedOwnerCount": 100,
            "complete": True,
        },
        "nativeSyntax": native_syntax,
        "indexedLexical": {
            "state": "executed",
            "backend": "tantivy",
            "generationDigest": generation,
            "indexArtifactDigest": "blake3-256:index",
            "candidateOwnerIds": owners,
            "indexedOwnerCount": 100,
            "admittedOwnerCount": 1,
            "complete": True,
        },
        "byteEvidence": {
            "state": "skipped",
            "mode": "verify-candidates",
            "generationDigest": generation,
            "candidateOwnerIds": [],
            "coverageComplete": False,
        },
        "residentGraph": {
            "state": "executed",
            "backend": "rust-resident-graph",
            "generationDigest": generation,
            "resultDigest": digest("f"),
            "entryOwnerIds": owners,
            "entryNodeIds": ["item:route"],
            "candidateOwnerIds": owners,
            "closureState": "complete",
            "unresolvedFrontierCount": 0,
        },
        "ripgrep": {
            "state": "skipped",
            "reasonKind": "ready-path-does-not-execute-ripgrep",
            "mode": "not-requested",
            "generationDigest": generation,
            "coverageInputDigest": None,
            "candidateOwnerIds": [],
            "processCount": 0,
            "complete": False,
        },
        "pythonGraph": {
            "state": "skipped",
            "reasonKind": "optional-capability-not-requested",
            "backend": "asp-python-graphs",
            "generationDigest": generation,
            "projectionDigest": None,
            "candidateOwnerIds": [],
        },
        "correlation": {
            "lexicalGraphOverlapCount": 1,
            "lexicalGraphJaccardPermille": 1000,
            "graphMarginalCandidateCount": 0,
            "candidateUnionCount": 1,
        },
    }
    return {
        "schemaId": "asp.search.playbook-receipt",
        "schemaVersion": "1",
        "workspaceIdentity": "workspace-a",
        "languageId": "rust",
        "providerDigest": "blake3-256:provider",
        "indexArtifactDigest": "blake3-256:index",
        "generationDigest": generation,
        "sourceRootDigest": "blake3-256:root",
        "operationId": "search-1",
        "candidateCount": 1,
        "residentReadElapsedMicros": 80,
        "serviceElapsedMicros": 160,
        "elapsedMicros": 240,
        "workCounters": zero_work,
        "query": "find graph router owner",
        "intent": "conceptual",
        "state": "completed",
        "plan": {"stages": stages, "coverage": "candidates", "maxOwners": 32, "deadlineMs": 500},
        "evidence": evidence,
        "decision": {
            "chosenPath": "owner:router",
            "explanation": "lexical identity, graph ownership, and source bytes agree",
            "residualUncertainty": [],
            "selectors": ["rust://src/router.rs#item/function/route"],
            "ownerPaths": owners,
        },
        "metrics": {
            "totalElapsedMicros": 240,
            "stageElapsedMicros": {
                "nativeSyntax": 20,
                "indexedLexical": 80,
                "byteEvidence": 0,
                "residentGraph": 120,
                "ripgrep": 0,
                "pythonGraph": 0,
            },
            "commandCount": 1,
        },
    }
