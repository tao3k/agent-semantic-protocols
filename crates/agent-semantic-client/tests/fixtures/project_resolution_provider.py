#!/usr/bin/env python3
import json
import sys


if len(sys.argv) != 3 or sys.argv[2] != "project-resolution-stdin":
    raise SystemExit(2)

config = json.loads(sys.argv[1])
request = json.load(sys.stdin)
source_root = config["sourceRoot"].strip("/") or "."
extension = "." + config["extension"].lstrip(".")
generation = request["candidateGeneration"]["digest"]

response = {
    "schemaId": "agent.semantic-protocols.provider-project-resolution-response",
    "schemaVersion": "1",
    "languageId": config["languageId"],
    "providerId": config["providerId"],
    "state": "resolved",
    "scope": {
        "schemaId": "agent.semantic-protocols.project-resolution",
        "schemaVersion": "1",
        "state": "resolved",
        "completeness": "exact",
        "languageId": config["languageId"],
        "providerId": config["providerId"],
        "parserId": config["parserId"],
        "candidateGenerationDigest": generation,
        "projectEntry": config["projectEntry"],
        "packageGraph": {
            "schemaId": "agent.semantic-protocols.language-package-graph",
            "schemaVersion": "1",
            "languageId": config["languageId"],
            "providerId": config["providerId"],
            "projectEntry": config["projectEntry"],
            "parserId": config["parserId"],
            "manifests": [{
                "path": config["projectEntry"],
                "kind": "fixture-manifest",
                "digest": "fixture-manifest",
            }],
            "lockfiles": [],
            "packages": [{
                "packageId": "fixture",
                "name": "fixture",
                "manifestPath": config["projectEntry"],
                "root": ".",
                "workspaceMember": True,
                "targets": [{
                    "targetId": "fixture:source",
                    "kind": "source",
                    "name": "fixture",
                    "explicit": True,
                    "sourceRoots": [source_root],
                    "entrypoints": [],
                    "generatedRoots": [],
                }],
            }],
            "internalDependencyEdges": [],
            "externalDependencies": [],
            "unresolved": [],
        },
        "sourceScopes": [{
            "scopeId": "fixture:source",
            "packageId": "fixture",
            "targetId": "fixture:source",
            "roots": [source_root],
            "explicitPaths": [],
            "extensions": [extension],
            "includeAuthority": "package-manager",
            "exclusions": [],
        }],
        "conflicts": [],
        "metrics": {
            "parsedManifestCount": 1,
            "parsedLockfileCount": 0,
            "affectedPackageCount": 1,
            "fullWorkspaceReads": 0,
            "fullManifestReparses": 0,
            "dbOpens": 0,
            "elapsedMicros": 1,
        },
    },
}

json.dump(response, sys.stdout, separators=(",", ":"))
