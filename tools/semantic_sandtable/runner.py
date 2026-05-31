"""Run language-harness sandtable scenarios against real CLI binaries."""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


DEFAULT_SCENARIO_GLOB = "sandtables/**/*.json"
SCENARIO_SCHEMA_PATH = Path("schemas/semantic-sandtable-scenario.v1.schema.json")
COVERAGE_POLICY_PATH = Path("sandtables/coverage-policy.json")
COVERAGE_POLICY_SCHEMA_PATH = Path(
    "schemas/semantic-sandtable-coverage-policy.v1.schema.json"
)
TOKEN_PATTERN = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)\}")


class ScenarioLoadError(Exception):
    """A scenario file is not valid enough to execute."""


class CoveragePolicyLoadError(Exception):
    """The coverage policy cannot be used for audit reporting."""


@dataclass
class StepResult:
    scenario_id: str
    step_id: str
    command: list[str]
    status: str
    exit_code: int | None
    elapsed_ms: int
    stdout_lines: int
    stderr_lines: int
    stdout_bytes: int
    stderr_bytes: int
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


@dataclass
class ScenarioResult:
    scenario_id: str
    language: str
    path: Path
    status: str
    workdir: Path | None
    coverage: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    steps: list[StepResult] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    skip_reason: str | None = None


@dataclass
class CoverageSurface:
    name: str
    scenario_ids: set[str] = field(default_factory=set)
    languages: set[str] = field(default_factory=set)
    step_ids: set[str] = field(default_factory=set)


@dataclass
class CoverageReport:
    scenario_count: int
    language_ids: set[str]
    expected_surfaces: list[str]
    surfaces: dict[str, CoverageSurface]
    policy_path: Path | None = None
    language_expected_surfaces: dict[str, list[str]] = field(default_factory=dict)
    errors: list[str] = field(default_factory=list)

    @property
    def missing(self) -> list[str]:
        return [
            surface
            for surface in self.expected_surfaces
            if surface not in self.surfaces
        ]

    @property
    def language_missing(self) -> dict[str, list[str]]:
        missing: dict[str, list[str]] = {}
        for language, expected in self.language_expected_surfaces.items():
            covered = self.covered_surfaces_for_language(language)
            language_missing = [surface for surface in expected if surface not in covered]
            if language_missing:
                missing[language] = language_missing
        return missing

    def covered_surfaces_for_language(self, language: str) -> set[str]:
        return {
            name
            for name, surface in self.surfaces.items()
            if language in surface.languages
        }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="semantic-sandtable",
        description="Run semantic language harness sandtable scenarios.",
    )
    parser.add_argument(
        "scenarios",
        nargs="*",
        help="Scenario JSON files. Defaults to sandtables/**/*.json.",
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Protocol repository root. Defaults to current directory.",
    )
    parser.add_argument("--json", action="store_true", help="Emit JSON report.")
    parser.add_argument(
        "--list",
        action="store_true",
        help="List discovered scenarios without running them.",
    )
    parser.add_argument(
        "--coverage",
        action="store_true",
        help="Audit declared scenario coverage without executing commands.",
    )
    parser.add_argument(
        "--coverage-policy",
        default=str(COVERAGE_POLICY_PATH),
        help="Coverage policy JSON for per-language audit expectations.",
    )
    parser.add_argument(
        "--fail-on-missing",
        action="store_true",
        help="Return non-zero if coverage audit reports missing surfaces.",
    )
    parser.add_argument(
        "--fail-on-warn",
        action="store_true",
        help="Return non-zero if any warning budget is exceeded.",
    )
    args = parser.parse_args(argv)

    repo_root = Path(args.repo_root).expanduser().resolve()
    scenario_paths = discover_scenarios(repo_root, args.scenarios)
    if args.list:
        for path in scenario_paths:
            try:
                scenario = load_scenario(path, repo_root)
            except ScenarioLoadError as error:
                print(f"[sandtable-error] {error}", file=sys.stderr)
                return 1
            print(
                f"{scenario.get('id', path.stem)}\t"
                f"{scenario.get('language', 'unknown')}\t"
                f"{path.relative_to(repo_root)}"
            )
        return 0

    if args.coverage:
        policy_path = resolve_path(repo_root, args.coverage_policy)
        coverage = coverage_report(repo_root, scenario_paths, policy_path=policy_path)
        if args.json:
            print(json.dumps(coverage_report_json(coverage), indent=2, sort_keys=True))
        else:
            print_coverage_report(coverage)
        missing = bool(coverage.missing or coverage.language_missing)
        if coverage.errors or (args.fail_on_missing and missing):
            return 1
        return 0

    results = [run_scenario(repo_root, path) for path in scenario_paths]
    if args.json:
        print(json.dumps(report_json(results), indent=2, sort_keys=True))
    else:
        print_text_report(repo_root, results)

    failed = any(result.status == "fail" for result in results)
    warned = any(has_warnings(result) for result in results)
    if failed or (args.fail_on_warn and warned):
        return 1
    return 0


def discover_scenarios(repo_root: Path, scenario_args: list[str]) -> list[Path]:
    if scenario_args:
        paths = [Path(arg).expanduser() for arg in scenario_args]
        return [path if path.is_absolute() else repo_root / path for path in paths]
    matches = sorted(repo_root.glob(DEFAULT_SCENARIO_GLOB))
    return [path for path in matches if discoverable_scenario_path(repo_root, path)]


def discoverable_scenario_path(repo_root: Path, path: Path) -> bool:
    if not path.is_file():
        return False
    try:
        relative = path.relative_to(repo_root)
    except ValueError:
        relative = path
    if relative == COVERAGE_POLICY_PATH:
        return False
    return not any(part.startswith(".") for part in relative.parts)


def load_scenario(path: Path, repo_root: Path | None = None) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            scenario = json.load(handle)
    except OSError as error:
        raise ScenarioLoadError(f"failed to read scenario: {error}") from error
    except json.JSONDecodeError as error:
        raise ScenarioLoadError(f"failed to parse scenario JSON: {error.msg}") from error
    if repo_root is not None:
        validate_scenario_schema(repo_root, path, scenario)
    return scenario


def validate_scenario_schema(repo_root: Path, path: Path, scenario: Any) -> None:
    schema_path = repo_root / SCENARIO_SCHEMA_PATH
    if not schema_path.exists():
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError as error:
        raise ScenarioLoadError(
            "scenario schema validation requires jsonschema; run through `uv run semantic-sandtable`"
        ) from error
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise ScenarioLoadError(f"failed to load scenario schema: {error}") from error

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(scenario), key=lambda error: list(error.path))
    if errors:
        messages = []
        for error in errors[:3]:
            location = ".".join(str(part) for part in error.path) or "$"
            messages.append(f"{location}: {error.message}")
        if len(errors) > 3:
            messages.append(f"... {len(errors) - 3} more")
        relative = path.relative_to(repo_root) if path.is_relative_to(repo_root) else path
        raise ScenarioLoadError(f"{relative} failed schema validation: {'; '.join(messages)}")


def coverage_report(
    repo_root: Path,
    scenario_paths: list[Path],
    policy_path: Path | None = None,
) -> CoverageReport:
    surfaces: dict[str, CoverageSurface] = {}
    languages: set[str] = set()
    scenario_count = 0
    errors: list[str] = []
    expected_surfaces = coverage_surfaces_from_schema(repo_root)
    language_expected_surfaces: dict[str, list[str]] = {}
    if policy_path is not None and policy_path.exists():
        try:
            language_expected_surfaces = load_coverage_policy(
                repo_root,
                policy_path,
                expected_surfaces,
            )
        except CoveragePolicyLoadError as error:
            errors.append(str(error))
    for path in scenario_paths:
        try:
            scenario = load_scenario(path, repo_root)
        except ScenarioLoadError as error:
            errors.append(str(error))
            continue
        scenario_count += 1
        scenario_id = require_str(scenario, "id", path.stem)
        language = require_str(scenario, "language", "unknown")
        languages.add(language)
        for surface in string_list(scenario.get("coverage", [])):
            add_coverage_surface(
                surfaces,
                surface,
                scenario_id=scenario_id,
                language=language,
            )
        for index, step in enumerate(scenario.get("steps", []), start=1):
            if not isinstance(step, dict):
                continue
            step_id = require_str(step, "id", f"step-{index}")
            for surface in string_list(step.get("coverage", [])):
                add_coverage_surface(
                    surfaces,
                    surface,
                    scenario_id=scenario_id,
                    language=language,
                    step_id=f"{scenario_id}:{step_id}",
                )
    display_policy_path = None
    if policy_path and policy_path.exists():
        display_policy_path = policy_path
        try:
            display_policy_path = policy_path.relative_to(repo_root)
        except ValueError:
            pass
    return CoverageReport(
        scenario_count=scenario_count,
        language_ids=languages,
        expected_surfaces=expected_surfaces,
        surfaces=surfaces,
        policy_path=display_policy_path,
        language_expected_surfaces=language_expected_surfaces,
        errors=errors,
    )


def add_coverage_surface(
    surfaces: dict[str, CoverageSurface],
    surface: str,
    *,
    scenario_id: str,
    language: str,
    step_id: str | None = None,
) -> None:
    entry = surfaces.setdefault(surface, CoverageSurface(name=surface))
    entry.scenario_ids.add(scenario_id)
    entry.languages.add(language)
    if step_id is not None:
        entry.step_ids.add(step_id)


def coverage_surfaces_from_schema(repo_root: Path) -> list[str]:
    schema_path = repo_root / SCENARIO_SCHEMA_PATH
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError):
        return []
    values = (
        schema.get("$defs", {})
        .get("coverageList", {})
        .get("items", {})
        .get("enum", [])
    )
    return [item for item in values if isinstance(item, str)]


def load_coverage_policy(
    repo_root: Path,
    path: Path,
    expected_surfaces: list[str],
) -> dict[str, list[str]]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            policy = json.load(handle)
    except OSError as error:
        raise CoveragePolicyLoadError(
            f"failed to read coverage policy: {error}"
        ) from error
    except json.JSONDecodeError as error:
        raise CoveragePolicyLoadError(
            f"failed to parse coverage policy JSON: {error.msg}"
        ) from error

    validate_coverage_policy_schema(repo_root, path, policy)
    if not isinstance(policy, dict):
        raise CoveragePolicyLoadError("coverage policy must be an object")

    expected_set = set(expected_surfaces)
    languages = policy.get("languages", [])
    if not isinstance(languages, list):
        raise CoveragePolicyLoadError("coverage policy languages must be an array")
    result: dict[str, list[str]] = {}
    for index, entry in enumerate(languages):
        if not isinstance(entry, dict):
            raise CoveragePolicyLoadError(
                f"coverage policy languages.{index} must be an object"
            )
        language = entry.get("languageId")
        if not isinstance(language, str):
            raise CoveragePolicyLoadError(
                f"coverage policy languages.{index}.languageId must be a string"
            )
        required = string_list(entry.get("requiredCoverage", []))
        unknown = [surface for surface in required if surface not in expected_set]
        if unknown:
            raise CoveragePolicyLoadError(
                f"coverage policy languageId {language} has unknown surfaces: "
                f"{','.join(unknown)}"
            )
        result[language] = required
    return result


def validate_coverage_policy_schema(repo_root: Path, path: Path, policy: Any) -> None:
    schema_path = repo_root / COVERAGE_POLICY_SCHEMA_PATH
    if not schema_path.exists():
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError as error:
        raise CoveragePolicyLoadError(
            "coverage policy validation requires jsonschema; run through `uv run semantic-sandtable`"
        ) from error
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise CoveragePolicyLoadError(
            f"failed to load coverage policy schema: {error}"
        ) from error

    validator = Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(policy), key=lambda error: list(error.path))
    if errors:
        messages = []
        for error in errors[:3]:
            location = ".".join(str(part) for part in error.path) or "$"
            messages.append(f"{location}: {error.message}")
        if len(errors) > 3:
            messages.append(f"... {len(errors) - 3} more")
        relative = path.relative_to(repo_root) if path.is_relative_to(repo_root) else path
        raise CoveragePolicyLoadError(
            f"{relative} failed schema validation: {'; '.join(messages)}"
        )


def run_scenario(repo_root: Path, path: Path) -> ScenarioResult:
    try:
        scenario = load_scenario(path, repo_root)
    except ScenarioLoadError as error:
        return ScenarioResult(
            scenario_id=path.stem,
            language="unknown",
            path=path,
            status="fail",
            workdir=None,
            errors=[str(error)],
        )
    scenario_id = require_str(scenario, "id", path.stem)
    language = require_str(scenario, "language", "unknown")
    workdir = resolve_workdir(repo_root, scenario.get("workdir"))
    result = ScenarioResult(
        scenario_id=scenario_id,
        language=language,
        path=path,
        status="pass",
        workdir=workdir,
        coverage=string_list(scenario.get("coverage", [])),
        tags=string_list(scenario.get("tags", [])),
    )
    if workdir is None:
        result.status = "skip"
        result.skip_reason = "no workdir candidate exists"
        return result

    budgets = scenario.get("budgets", {})
    max_commands = optional_int(budgets.get("maxCommands"))
    steps = scenario.get("steps", [])
    if not isinstance(steps, list):
        result.status = "fail"
        result.errors.append("scenario.steps must be an array")
        return result
    if max_commands is not None and len(steps) > max_commands:
        result.warnings.append(
            f"commands={len(steps)} exceeds maxCommands={max_commands}"
        )

    env = build_env(scenario.get("env", {}))
    captures: dict[str, str] = {}
    total_lines = 0
    total_elapsed_ms = 0
    total_stdout_bytes = 0
    total_stderr_bytes = 0
    for index, step in enumerate(steps, start=1):
        step_result = run_step(
            repo_root=repo_root,
            workdir=workdir,
            scenario_id=scenario_id,
            step=step,
            index=index,
            env=env,
            captures=captures,
        )
        result.steps.append(step_result)
        total_lines += step_result.stdout_lines + step_result.stderr_lines
        total_elapsed_ms += step_result.elapsed_ms
        total_stdout_bytes += step_result.stdout_bytes
        total_stderr_bytes += step_result.stderr_bytes
        if step_result.status == "fail":
            result.status = "fail"

    max_total_lines_warn = optional_int(budgets.get("maxTotalLinesWarn"))
    if max_total_lines_warn is not None and total_lines > max_total_lines_warn:
        result.warnings.append(
            f"totalLines={total_lines} exceeds maxTotalLinesWarn={max_total_lines_warn}"
        )
    warn_scenario_if_over(
        result,
        "totalElapsedMs",
        total_elapsed_ms,
        "maxTotalElapsedMsWarn",
        budgets.get("maxTotalElapsedMsWarn"),
    )
    warn_scenario_if_over(
        result,
        "totalStdoutBytes",
        total_stdout_bytes,
        "maxTotalStdoutBytesWarn",
        budgets.get("maxTotalStdoutBytesWarn"),
    )
    warn_scenario_if_over(
        result,
        "totalStderrBytes",
        total_stderr_bytes,
        "maxTotalStderrBytesWarn",
        budgets.get("maxTotalStderrBytesWarn"),
    )

    if result.status != "fail" and has_warnings(result):
        result.status = "warn"
    return result


def run_step(
    *,
    repo_root: Path,
    workdir: Path,
    scenario_id: str,
    step: Any,
    index: int,
    env: dict[str, str],
    captures: dict[str, str],
) -> StepResult:
    if not isinstance(step, dict):
        return empty_step_error(scenario_id, f"step-{index}", "step must be an object")

    step_id = require_str(step, "id", f"step-{index}")
    command, command_errors = expand_string_list(step.get("command"), captures)
    if command_errors:
        return empty_step_error(scenario_id, step_id, "; ".join(command_errors))
    if not command:
        return empty_step_error(
            scenario_id,
            step_id,
            "step.command must be a non-empty string array",
        )

    stdin = resolve_stdin(step, workdir, scenario_id, env, captures)
    if isinstance(stdin, StepResult):
        return stdin

    timeout_seconds = float(step.get("timeoutSeconds", 30))
    started = time.perf_counter()
    try:
        process = subprocess.run(
            command,
            cwd=workdir,
            env=env,
            input=stdin,
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
    except FileNotFoundError as error:
        return StepResult(
            scenario_id=scenario_id,
            step_id=step_id,
            command=command,
            status="fail",
            exit_code=None,
            elapsed_ms=round((time.perf_counter() - started) * 1000),
            stdout_lines=0,
            stderr_lines=0,
            stdout_bytes=0,
            stderr_bytes=0,
            errors=[f"command not found: {error.filename}"],
        )
    except subprocess.TimeoutExpired:
        return StepResult(
            scenario_id=scenario_id,
            step_id=step_id,
            command=command,
            status="fail",
            exit_code=None,
            elapsed_ms=round((time.perf_counter() - started) * 1000),
            stdout_lines=0,
            stderr_lines=0,
            stdout_bytes=0,
            stderr_bytes=0,
            errors=[f"timeout after {timeout_seconds:g}s"],
        )

    elapsed_ms = round((time.perf_counter() - started) * 1000)
    result = StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=command,
        status="pass",
        exit_code=process.returncode,
        elapsed_ms=elapsed_ms,
        stdout_lines=count_lines(process.stdout),
        stderr_lines=count_lines(process.stderr),
        stdout_bytes=len(process.stdout.encode("utf-8")),
        stderr_bytes=len(process.stderr.encode("utf-8")),
    )
    validate_step(step, result, process.stdout, process.stderr, repo_root)
    capture_values(step, result, process.stdout, captures)
    if result.errors:
        result.status = "fail"
    elif result.warnings:
        result.status = "warn"
    return result


def empty_step_error(scenario_id: str, step_id: str, error: str) -> StepResult:
    return StepResult(
        scenario_id=scenario_id,
        step_id=step_id,
        command=[],
        status="fail",
        exit_code=None,
        elapsed_ms=0,
        stdout_lines=0,
        stderr_lines=0,
        stdout_bytes=0,
        stderr_bytes=0,
        errors=[error],
    )


def resolve_stdin(
    step: dict[str, Any],
    workdir: Path,
    scenario_id: str,
    env: dict[str, str],
    captures: dict[str, str],
) -> str | None | StepResult:
    if "stdin" in step:
        value = step["stdin"]
        if not isinstance(value, str):
            return empty_step_error(
                scenario_id,
                require_str(step, "id", "stdin"),
                "step.stdin must be a string",
            )
        try:
            return expand_tokens(value, captures)
        except KeyError as error:
            return empty_step_error(
                scenario_id,
                require_str(step, "id", "stdin"),
                f"missing capture {error.args[0]!r}",
            )

    stdin_command = step.get("stdinCommand")
    if stdin_command is None:
        return None
    command, command_errors = expand_string_list(stdin_command, captures)
    if command_errors:
        return empty_step_error(
            scenario_id,
            require_str(step, "id", "stdin-command"),
            "; ".join(command_errors),
        )
    if not command:
        return empty_step_error(
            scenario_id,
            require_str(step, "id", "stdin-command"),
            "step.stdinCommand must be a non-empty string array",
        )
    try:
        process = subprocess.run(
            command,
            cwd=workdir,
            env=env,
            text=True,
            capture_output=True,
            timeout=float(step.get("stdinTimeoutSeconds", 15)),
            check=False,
        )
    except FileNotFoundError as error:
        return empty_step_error(
            scenario_id,
            require_str(step, "id", "stdin-command"),
            f"stdin command not found: {error.filename}",
        )
    except subprocess.TimeoutExpired:
        return empty_step_error(
            scenario_id,
            require_str(step, "id", "stdin-command"),
            "stdin command timeout",
        )

    allow_non_zero = bool(step.get("stdinCommandAllowNonZero", False))
    if process.returncode != 0 and not allow_non_zero:
        result = empty_step_error(
            scenario_id,
            require_str(step, "id", "stdin-command"),
            f"stdin command exited {process.returncode}",
        )
        result.command = command
        result.exit_code = process.returncode
        result.stdout_lines = count_lines(process.stdout)
        result.stderr_lines = count_lines(process.stderr)
        result.stdout_bytes = len(process.stdout.encode("utf-8"))
        result.stderr_bytes = len(process.stderr.encode("utf-8"))
        return result
    return process.stdout


def validate_step(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    stderr: str,
    repo_root: Path,
) -> None:
    expect = step.get("expect", {})
    if not isinstance(expect, dict):
        result.errors.append("step.expect must be an object")
        return

    expected_exit = int(expect.get("exitCode", 0))
    if result.exit_code != expected_exit:
        result.errors.append(
            f"exitCode={result.exit_code} expected={expected_exit}"
        )

    for needle in string_list(expect.get("stdoutContains", [])):
        if needle not in stdout:
            result.errors.append(f"stdout missing {needle!r}")
    for needle in string_list(expect.get("stderrContains", [])):
        if needle not in stderr:
            result.errors.append(f"stderr missing {needle!r}")
    for needle in string_list(expect.get("stdoutNotContains", [])):
        if needle in stdout:
            result.errors.append(f"stdout unexpectedly contains {needle!r}")
    for pattern in string_list(expect.get("stdoutMatches", [])):
        if re.search(pattern, stdout, flags=re.MULTILINE) is None:
            result.errors.append(f"stdout regex missed {pattern!r}")
    if bool(expect.get("stdoutEmpty", False)) and stdout:
        result.errors.append("stdout expected empty")
    validate_stdout_json(expect, result, stdout, repo_root)
    if bool(expect.get("lineProtocol", False)):
        validate_line_protocol(result, stdout)

    warn_if_over(
        result,
        "stdoutLines",
        result.stdout_lines,
        "maxStdoutLinesWarn",
        expect.get("maxStdoutLinesWarn"),
    )
    warn_if_over(
        result,
        "stderrLines",
        result.stderr_lines,
        "maxStderrLinesWarn",
        expect.get("maxStderrLinesWarn"),
    )
    warn_if_over(
        result,
        "stdoutBytes",
        result.stdout_bytes,
        "maxStdoutBytesWarn",
        expect.get("maxStdoutBytesWarn"),
    )
    warn_if_over(
        result,
        "elapsedMs",
        result.elapsed_ms,
        "maxElapsedMsWarn",
        expect.get("maxElapsedMsWarn"),
    )


def capture_values(
    step: dict[str, Any],
    result: StepResult,
    stdout: str,
    captures: dict[str, str],
) -> None:
    capture_spec = step.get("capture", {})
    if not isinstance(capture_spec, dict):
        return
    for name, pattern in capture_spec.items():
        if not isinstance(name, str) or not isinstance(pattern, str):
            result.errors.append("capture entries must be string to regex")
            continue
        match = re.search(pattern, stdout, flags=re.MULTILINE)
        if match is None:
            result.errors.append(f"capture {name!r} missed {pattern!r}")
            continue
        captures[name] = match.group(1) if match.groups() else match.group(0)


def validate_stdout_json(
    expect: dict[str, Any],
    result: StepResult,
    stdout: str,
    repo_root: Path,
) -> None:
    equals = expect.get("stdoutJsonEquals", {})
    contains = expect.get("stdoutJsonContains", {})
    schema_path = expect.get("stdoutJsonSchema")
    schemas_at = expect.get("stdoutJsonSchemaAt", {})
    if not isinstance(equals, dict):
        result.errors.append("expect.stdoutJsonEquals must be an object")
        equals = {}
    if not isinstance(contains, dict):
        result.errors.append("expect.stdoutJsonContains must be an object")
        contains = {}
    if schema_path is not None and not isinstance(schema_path, str):
        result.errors.append("expect.stdoutJsonSchema must be a string")
        schema_path = None
    if not isinstance(schemas_at, dict):
        result.errors.append("expect.stdoutJsonSchemaAt must be an object")
        schemas_at = {}
    if not equals and not contains and schema_path is None and not schemas_at:
        return

    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        result.errors.append(f"stdout JSON parse failed: {error.msg}")
        return

    if schema_path is not None:
        validate_json_value_against_schema(
            result,
            repo_root,
            payload,
            "$",
            schema_path,
        )
    for path, nested_schema_path in schemas_at.items():
        if not isinstance(path, str) or not isinstance(nested_schema_path, str):
            result.errors.append("stdoutJsonSchemaAt entries must be string to string")
            continue
        actual, found = json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
            continue
        validate_json_value_against_schema(
            result,
            repo_root,
            actual,
            path,
            nested_schema_path,
        )

    for path, expected in equals.items():
        if not isinstance(path, str):
            result.errors.append("stdoutJsonEquals paths must be strings")
            continue
        actual, found = json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
        elif actual != expected:
            result.errors.append(
                f"stdout JSON path {path!r}={actual!r} expected={expected!r}"
            )

    for path, needle in contains.items():
        if not isinstance(path, str) or not isinstance(needle, str):
            result.errors.append("stdoutJsonContains entries must be string to string")
            continue
        actual, found = json_path(payload, path)
        if not found:
            result.errors.append(f"stdout JSON path missing {path!r}")
            continue
        actual_text = actual if isinstance(actual, str) else json.dumps(actual, sort_keys=True)
        if needle not in actual_text:
            result.errors.append(
                f"stdout JSON path {path!r} missing substring {needle!r}"
            )


def validate_json_value_against_schema(
    result: StepResult,
    repo_root: Path,
    value: Any,
    value_path: str,
    schema_path_text: str,
) -> None:
    schema_path = resolve_path(repo_root, schema_path_text)
    if schema_path is None or not schema_path.exists():
        result.errors.append(f"stdout JSON schema not found {schema_path_text!r}")
        return
    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        result.errors.append("stdout JSON schema validation requires jsonschema")
        return
    try:
        with schema_path.open("r", encoding="utf-8") as handle:
            schema = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        result.errors.append(f"stdout JSON schema load failed {schema_path_text!r}: {error}")
        return
    errors = sorted(
        Draft202012Validator(schema).iter_errors(value),
        key=lambda error: list(error.path),
    )
    for error in errors[:3]:
        location = ".".join(str(part) for part in error.path) or "$"
        result.errors.append(
            f"stdout JSON schema {schema_path_text!r} failed at {value_path}.{location}: {error.message}"
        )
    if len(errors) > 3:
        result.errors.append(
            f"stdout JSON schema {schema_path_text!r} has {len(errors) - 3} more failures"
        )


def json_path(payload: Any, path: str) -> tuple[Any, bool]:
    current = payload
    for part in path.split("."):
        if isinstance(current, dict) and part in current:
            current = current[part]
            continue
        if isinstance(current, list) and part.isdigit():
            index = int(part)
            if index < len(current):
                current = current[index]
                continue
        return None, False
    return current, True


def validate_line_protocol(result: StepResult, stdout: str) -> None:
    lines = [line for line in stdout.splitlines() if line.strip()]
    if not lines:
        result.errors.append("stdout has no line protocol lines")
        return
    if not lines[0].startswith("["):
        result.errors.append("first stdout line does not start with '['")
    for line in lines[1:]:
        if not (line.startswith("|") or line.startswith("[")):
            result.errors.append(f"line protocol stray line: {line[:80]!r}")
            return


def warn_if_over(
    result: StepResult,
    name: str,
    actual: int,
    threshold_name: str,
    threshold: Any,
) -> None:
    limit = optional_int(threshold)
    if limit is not None and actual > limit:
        result.warnings.append(f"{name}={actual} exceeds {threshold_name}={limit}")


def warn_scenario_if_over(
    result: ScenarioResult,
    name: str,
    actual: int,
    threshold_name: str,
    threshold: Any,
) -> None:
    limit = optional_int(threshold)
    if limit is not None and actual > limit:
        result.warnings.append(f"{name}={actual} exceeds {threshold_name}={limit}")


def resolve_workdir(repo_root: Path, spec: Any) -> Path | None:
    if spec is None:
        return repo_root
    if isinstance(spec, str):
        return resolve_path(repo_root, spec)
    if not isinstance(spec, dict):
        return None

    env_name = spec.get("env")
    if isinstance(env_name, str):
        env_value = os.environ.get(env_name)
        if env_value:
            env_path = resolve_path(repo_root, env_value)
            if env_path and env_path.exists():
                return env_path

    relative = spec.get("relative")
    if isinstance(relative, str):
        relative_path = resolve_path(repo_root, relative)
        if relative_path and relative_path.exists():
            return relative_path

    for pattern in string_list(spec.get("candidates", [])):
        matches = resolve_glob(repo_root, pattern)
        if matches:
            return matches[0]
    return None


def resolve_path(repo_root: Path, value: str) -> Path | None:
    expanded = os.path.expandvars(os.path.expanduser(value))
    path = Path(expanded)
    if not path.is_absolute():
        path = repo_root / path
    return path.resolve()


def resolve_glob(repo_root: Path, pattern: str) -> list[Path]:
    expanded = os.path.expandvars(os.path.expanduser(pattern))
    if not Path(expanded).is_absolute():
        expanded = str(repo_root / expanded)
    matches = [Path(match).resolve() for match in glob.glob(expanded)]
    existing = [match for match in matches if match.exists()]
    return sorted(
        existing,
        key=lambda path: (path.stat().st_mtime, str(path)),
        reverse=True,
    )


def expand_string_list(value: Any, captures: dict[str, str]) -> tuple[list[str], list[str]]:
    raw_items = string_list(value)
    errors: list[str] = []
    expanded: list[str] = []
    for item in raw_items:
        try:
            expanded.append(expand_tokens(item, captures))
        except KeyError as error:
            errors.append(f"missing capture {error.args[0]!r}")
    return expanded, errors


def expand_tokens(value: str, captures: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        if name not in captures:
            raise KeyError(name)
        return captures[name]

    return TOKEN_PATTERN.sub(replace, value)


def build_env(value: Any) -> dict[str, str]:
    env = os.environ.copy()
    if not isinstance(value, dict):
        return env
    for key, item in value.items():
        if isinstance(key, str):
            env[key] = os.path.expandvars(str(item))
    return env


def require_str(mapping: dict[str, Any], key: str, default: str) -> str:
    value = mapping.get(key, default)
    if isinstance(value, str):
        return value
    return default


def string_list(value: Any) -> list[str]:
    if value is None:
        return []
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def optional_int(value: Any) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except (TypeError, ValueError):
        return None


def count_lines(text: str) -> int:
    if not text:
        return 0
    return len(text.splitlines())


def has_warnings(result: ScenarioResult) -> bool:
    return bool(result.warnings or any(step.warnings for step in result.steps))


def print_text_report(repo_root: Path, results: list[ScenarioResult]) -> None:
    summary = {"pass": 0, "warn": 0, "fail": 0, "skip": 0}
    for result in results:
        summary[result.status] = summary.get(result.status, 0) + 1
    print(
        "[sandtable] "
        f"scenarios={len(results)} pass={summary['pass']} warn={summary['warn']} "
        f"fail={summary['fail']} skip={summary['skip']}"
    )
    for result in results:
        relative_path = result.path
        try:
            relative_path = result.path.relative_to(repo_root)
        except ValueError:
            pass
        workdir = "-"
        if result.workdir is not None:
            try:
                workdir = str(result.workdir.relative_to(repo_root))
            except ValueError:
                workdir = str(result.workdir)
        print(
            f"[scenario] id={result.scenario_id} lang={result.language} "
            f"status={result.status} path={relative_path} workdir={workdir}"
        )
        if result.coverage:
            print(f"|coverage {','.join(result.coverage)}")
        if result.tags:
            print(f"|tags {','.join(result.tags)}")
        if result.skip_reason:
            print(f"|skip reason={quote_value(result.skip_reason)}")
        for warning in result.warnings:
            print(f"|warn {warning}")
        for error in result.errors:
            print(f"|error {error}")
        for step in result.steps:
            print(
                f"|step {step.step_id} status={step.status} "
                f"exit={step.exit_code} ms={step.elapsed_ms} "
                f"stdout_lines={step.stdout_lines} stderr_lines={step.stderr_lines} "
                f"stdout_bytes={step.stdout_bytes}"
            )
            for warning in step.warnings:
                print(f"|warn step={step.step_id} {warning}")
            for error in step.errors:
                print(f"|error step={step.step_id} {error}")


def quote_value(value: str) -> str:
    if re.fullmatch(r"[A-Za-z0-9_.:/=-]+", value):
        return value
    return json.dumps(value)


def print_coverage_report(report: CoverageReport) -> None:
    expected_count = len(report.expected_surfaces)
    covered_count = sum(
        1 for surface in report.expected_surfaces if surface in report.surfaces
    )
    language_missing_count = sum(
        len(missing) for missing in report.language_missing.values()
    )
    languages = ",".join(sorted(report.language_ids)) or "-"
    print(
        "[coverage] "
        f"scenarios={report.scenario_count} languages={languages} "
        f"surfaces={covered_count}/{expected_count} "
        f"missing={len(report.missing)} "
        f"language_missing={language_missing_count} errors={len(report.errors)}"
    )
    if report.policy_path is not None:
        print(f"|policy {report.policy_path}")
    for surface in sorted_coverage_surfaces(report):
        print(
            f"|surface {surface.name} "
            f"languages={','.join(sorted(surface.languages)) or '-'} "
            f"scenarios={','.join(sorted(surface.scenario_ids)) or '-'}"
        )
        if surface.step_ids:
            print(f"|steps {surface.name} {','.join(sorted(surface.step_ids))}")
    for missing in report.missing:
        print(f"|missing surface={missing}")
    for language, expected in sorted(report.language_expected_surfaces.items()):
        covered = report.covered_surfaces_for_language(language)
        missing = report.language_missing.get(language, [])
        print(
            f"|language {language} surfaces={len(covered.intersection(expected))}/"
            f"{len(expected)} missing={len(missing)}"
        )
        for surface in missing:
            print(f"|missing language={language} surface={surface}")
    for error in report.errors:
        print(f"|error {error}")


def sorted_coverage_surfaces(report: CoverageReport) -> list[CoverageSurface]:
    expected_order = {
        surface: index for index, surface in enumerate(report.expected_surfaces)
    }
    return sorted(
        report.surfaces.values(),
        key=lambda surface: (
            expected_order.get(surface.name, len(expected_order)),
            surface.name,
        ),
    )


def coverage_report_json(report: CoverageReport) -> dict[str, Any]:
    expected_count = len(report.expected_surfaces)
    covered_count = sum(
        1 for surface in report.expected_surfaces if surface in report.surfaces
    )
    language_missing_count = sum(
        len(missing) for missing in report.language_missing.values()
    )
    return {
        "summary": {
            "scenarios": report.scenario_count,
            "languages": sorted(report.language_ids),
            "surfaces": covered_count,
            "expectedSurfaces": expected_count,
            "missing": len(report.missing),
            "languageMissing": language_missing_count,
            "errors": len(report.errors),
        },
        "policy": str(report.policy_path) if report.policy_path is not None else None,
        "expectedSurfaces": report.expected_surfaces,
        "missing": report.missing,
        "languageCoverage": [
            {
                "language": language,
                "expectedSurfaces": expected,
                "coveredSurfaces": sorted(
                    report.covered_surfaces_for_language(language).intersection(
                        expected
                    )
                ),
                "missing": report.language_missing.get(language, []),
            }
            for language, expected in sorted(
                report.language_expected_surfaces.items()
            )
        ],
        "surfaces": [
            {
                "name": surface.name,
                "languages": sorted(surface.languages),
                "scenarios": sorted(surface.scenario_ids),
                "steps": sorted(surface.step_ids),
            }
            for surface in sorted_coverage_surfaces(report)
        ],
        "errors": report.errors,
    }


def report_json(results: list[ScenarioResult]) -> dict[str, Any]:
    return {
        "scenarios": [scenario_json(result) for result in results],
        "summary": {
            "total": len(results),
            "pass": sum(1 for result in results if result.status == "pass"),
            "warn": sum(1 for result in results if result.status == "warn"),
            "fail": sum(1 for result in results if result.status == "fail"),
            "skip": sum(1 for result in results if result.status == "skip"),
        },
    }


def scenario_json(result: ScenarioResult) -> dict[str, Any]:
    return {
        "id": result.scenario_id,
        "language": result.language,
        "path": str(result.path),
        "status": result.status,
        "workdir": str(result.workdir) if result.workdir is not None else None,
        "coverage": result.coverage,
        "tags": result.tags,
        "skipReason": result.skip_reason,
        "warnings": result.warnings,
        "errors": result.errors,
        "steps": [
            {
                "id": step.step_id,
                "command": step.command,
                "status": step.status,
                "exitCode": step.exit_code,
                "elapsedMs": step.elapsed_ms,
                "stdoutLines": step.stdout_lines,
                "stderrLines": step.stderr_lines,
                "stdoutBytes": step.stdout_bytes,
                "stderrBytes": step.stderr_bytes,
                "warnings": step.warnings,
                "errors": step.errors,
            }
            for step in result.steps
        ],
    }
