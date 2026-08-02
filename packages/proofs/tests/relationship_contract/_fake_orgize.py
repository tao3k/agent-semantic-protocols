FAKE_ORGIZE_SOURCE = """#!/usr/bin/env python3
import json
from pathlib import Path
import sys

if sys.argv[1:3] == ["elements-query", "--packet"] and len(sys.argv) == 5:
    query = json.loads(sys.argv[3])
    source = Path(sys.argv[4])
    if query == {"schemaVersion": 1, "kind": "node-property"}:
        print(Path(str(source) + ".properties-response.json").read_text())
    else:
        fixture = json.loads(Path(str(source) + ".response.json").read_text())
        outline = query.get("outlinePathPrefix")
        if query != {
            "category": "section",
            "outlinePathExactLen": len(outline),
            "outlinePathPrefix": outline,
            "schemaVersion": 1,
        }:
            raise SystemExit("query is not an exact section outline query")
        records = [
            record for record in fixture["records"]
            if record.get("outlinePath") == outline
        ]
        print(json.dumps(records))
elif sys.argv[1:3] == ["contract", "trace"] and len(sys.argv) == 7:
    target = Path(sys.argv[6])
    print(Path(str(target) + ".contract-response.json").read_text())
else:
    raise SystemExit("unexpected invocation")
"""
