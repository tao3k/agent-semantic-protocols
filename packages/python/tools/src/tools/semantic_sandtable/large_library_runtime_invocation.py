"""Validate and expand provider-owned benchmark invocation templates."""

from __future__ import annotations

import re
from typing import Any

from .large_library_runtime_types import Invocation
from .utils import dict_value, string_list


_TEMPLATE_FIELD = re.compile(r"\{([a-z][a-z0-9_]*)\}")


def benchmark_command_from_descriptor(
    descriptor: dict[str, Any],
    language: str,
    inputs: dict[str, str],
) -> Invocation:
    """Resolve a provider-owned invocation template to the public ASP command."""
    method = descriptor.get("method")
    view = descriptor.get("view")
    contract = dict_value(descriptor.get("benchmarkInvocation"))
    args = string_list(contract.get("args"))
    if not isinstance(method, str) or not method.startswith("search/"):
        raise ValueError("benchmark descriptor must advertise a search method")
    if not isinstance(view, str) or method != f"search/{view}":
        raise ValueError(f"{method}: benchmark descriptor view is invalid")
    if not args or args[:2] != ["search", view]:
        raise ValueError(f"{method}: benchmark args must begin with search {view}")
    if args[0] == "asp" or language in args:
        raise ValueError(f"{method}: benchmark args must be facade-relative")
    keeps_query = any("{query}" in argument for argument in args)
    if keeps_query and len(args) == 2:
        raise ValueError(f"{method}: benchmark args must stay query-parametric")
    if "{workspace}" not in args:
        raise ValueError(f"{method}: benchmark args must scope a workspace")
    expects_json = contract.get("expectsJson")
    max_elapsed_ms = contract.get("maxElapsedMs")
    if not isinstance(expects_json, bool) or not isinstance(max_elapsed_ms, int):
        raise ValueError(f"{method}: benchmark contract is incomplete")
    if max_elapsed_ms < 1:
        raise ValueError(f"{method}: benchmark maxElapsedMs must be positive")
    try:
        command_args = [substitute(argument, inputs) for argument in args]
        stdin_template = contract.get("stdinTemplate")
        stdin = substitute(stdin_template, inputs) if isinstance(stdin_template, str) else None
    except TypeError as error:
        raise ValueError(f"{method}: benchmark template is invalid") from error
    return Invocation(
        command=["asp", language, *command_args],
        stdin=stdin,
        expects_json=expects_json,
        max_elapsed_ms=max_elapsed_ms,
    )


def substitute(template: str, values: dict[str, str]) -> str:
    unknown = {match.group(1) for match in _TEMPLATE_FIELD.finditer(template)} - set(values)
    if unknown:
        raise ValueError(f"unknown benchmark placeholders: {','.join(sorted(unknown))}")
    return _TEMPLATE_FIELD.sub(lambda match: values[match.group(1)], template)
