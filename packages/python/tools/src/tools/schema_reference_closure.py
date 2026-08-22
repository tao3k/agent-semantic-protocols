from __future__ import annotations

import json
from collections.abc import Iterable
from pathlib import Path


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def schema_reference_closure(
    repo_root: Path,
    schema_names: Iterable[str],
) -> frozenset[str]:
    closure = set(schema_names)
    pending = list(closure)
    while pending:
        schema_name = pending.pop()
        schema_path = repo_root / "schemas" / schema_name
        if not schema_path.is_file():
            continue
        document = load_json(schema_path)
        values: list[object] = [document]
        while values:
            value = values.pop()
            if isinstance(value, dict):
                for reference_key in ("$ref", "const"):
                    reference = value.get(reference_key)
                    if isinstance(reference, str):
                        referenced_name = reference.split("#", 1)[0].rsplit("/", 1)[-1]
                        referenced_path = repo_root / "schemas" / referenced_name
                        if (
                            referenced_name.endswith(".json")
                            and referenced_path.is_file()
                            and referenced_name not in closure
                        ):
                            closure.add(referenced_name)
                            pending.append(referenced_name)
                values.extend(value.values())
            elif isinstance(value, list):
                values.extend(value)
    return frozenset(closure)


def schema_profile_contract_closure(
    repo_root: Path,
    schema_names: Iterable[str],
) -> frozenset[str]:
    roots = set(schema_names)
    roots.add("provider-manifest.schema.json")
    return schema_reference_closure(repo_root, roots)
