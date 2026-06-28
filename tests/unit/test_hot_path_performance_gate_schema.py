import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


SCHEMA_DIR = Path(__file__).resolve().parents[2] / "schemas"


def _load_schema(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text())


def _schema_registry() -> Registry:
    resources = []
    for schema_path in SCHEMA_DIR.glob("*.schema.json"):
        schema = json.loads(schema_path.read_text())
        schema_id = schema.get("$id")
        if schema_id:
            resources.append((schema_id, Resource.from_contents(schema)))
    return Registry().with_resources(resources)


def test_hot_path_performance_gate_accepts_warm_broad_index_hit() -> None:
    schema = _load_schema("semantic-hot-path-performance-gate.v1.schema.json")
    validator = Draft202012Validator(schema, registry=_schema_registry())

    validator.validate(
        {
            "schemaId": "agent.semantic-protocols.semantic-hot-path-performance-gate",
            "schemaVersion": "1",
            "scenarioId": "python.query.selector-import.warm",
            "languageId": "python",
            "workspace": ".",
            "command": [
                "asp",
                "python",
                "query",
                "--selector",
                "packages/python/tools/src/tools/semantic_sandtable/scenario_runner.py",
                "--term",
                "import",
                "--workspace",
                ".",
                "--code",
            ],
            "phase": "warm",
            "expected": {
                "targetTotal": "5ms",
                "maxTotal": "25ms",
                "regressionBudget": "5ms",
                "maxProviderProcessCount": 0,
                "requireSourceIndexHit": True,
                "requireFactIndexHit": True,
                "allowedFirstRoutes": [
                    "owner-skeleton",
                    "item-skeleton",
                    "syntax-outline"
                ],
                "forbiddenRoutes": [
                    "prime",
                    "broad-rg",
                    "direct-read"
                ],
                "requireExactCodeIdentity": True,
                "requireNoExecutableLineRange": True
            },
            "observed": {
                "observedTotal": "3.4ms",
                "providerProcessCount": 0,
                "sourceIndexHit": True,
                "factIndexHit": True,
                "graphTurboDuration": "800us",
                "graphTurboCacheStatus": "hit",
                "firstRoute": "item-skeleton",
                "executedRoutes": [
                    "item-skeleton"
                ],
                "executableLineRangeSelectorCount": 0,
                "packetOutMode": "not-applicable"
            },
            "verdict": "pass",
            "evidenceRefs": [
                "receipt:python.query.selector-import.warm"
            ]
        }
    )
