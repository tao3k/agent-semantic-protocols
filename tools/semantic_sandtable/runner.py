"""Run language-harness sandtable scenarios against real CLI binaries."""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


DEFAULT_SCENARIO_GLOB = "sandtables/**/*.json"
TOKEN_PATTERN = re.compile(r"\{([A-Za-z_][A-Za-z0-9_]*)\}")


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
    steps: list[StepResult] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)
    skip_reason: str | None = None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m tools.semantic_sandtable",
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
        "--fail-on-warn",
        action="store_true",
        help="Return non-zero if any warning budget is exceeded.",
    )
    args = parser.parse_args(argv)

    repo_root = Path(args.repo_root).expanduser().resolve()
    scenario_paths = discover_scenarios(repo_root, args.scenarios)
    if args.list:
        for path in scenario_paths:
            scenario = load_scenario(path)
            print(
                f"{scenario.get('id', path.stem)}\t"
                f"{scenario.get('language', 'unknown')}\t"
                f"{path.relative_to(repo_root)}"
            )
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
    return [path for path in matches if path.is_file()]


def load_scenario(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def run_scenario(repo_root: Path, path: Path) -> ScenarioResult:
    scenario = load_scenario(path)
    scenario_id = require_str(scenario, "id", path.stem)
    language = require_str(scenario, "language", "unknown")
    workdir = resolve_workdir(repo_root, scenario.get("workdir"))
    result = ScenarioResult(
        scenario_id=scenario_id,
        language=language,
        path=path,
        status="pass",
        workdir=workdir,
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
    for index, step in enumerate(steps, start=1):
        step_result = run_step(
            workdir=workdir,
            scenario_id=scenario_id,
            step=step,
            index=index,
            env=env,
            captures=captures,
        )
        result.steps.append(step_result)
        total_lines += step_result.stdout_lines + step_result.stderr_lines
        if step_result.status == "fail":
            result.status = "fail"

    max_total_lines_warn = optional_int(budgets.get("maxTotalLinesWarn"))
    if max_total_lines_warn is not None and total_lines > max_total_lines_warn:
        result.warnings.append(
            f"totalLines={total_lines} exceeds maxTotalLinesWarn={max_total_lines_warn}"
        )

    if result.status != "fail" and has_warnings(result):
        result.status = "warn"
    return result


def run_step(
    *,
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
    validate_step(step, result, process.stdout, process.stderr)
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
    validate_stdout_json(expect, result, stdout)
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
) -> None:
    equals = expect.get("stdoutJsonEquals", {})
    contains = expect.get("stdoutJsonContains", {})
    if not isinstance(equals, dict):
        result.errors.append("expect.stdoutJsonEquals must be an object")
        equals = {}
    if not isinstance(contains, dict):
        result.errors.append("expect.stdoutJsonContains must be an object")
        contains = {}
    if not equals and not contains:
        return

    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        result.errors.append(f"stdout JSON parse failed: {error.msg}")
        return

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


def json_path(payload: Any, path: str) -> tuple[Any, bool]:
    current = payload
    for part in path.split("."):
        if isinstance(current, dict) and part in current:
            current = current[part]
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
