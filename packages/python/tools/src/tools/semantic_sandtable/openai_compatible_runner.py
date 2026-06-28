"""Run a live OpenAI-compatible chat completion for sandtable agent steps."""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from typing import Any

_OUTPUT_FORMATS = {"text", "json", "stream-json", "summary-json"}


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        response = _chat_completion(args)
        summary = _summary_from_response(args, response)
        token_cost = summary.get("tokenCost")
        if args.require_live_usage and (
            not isinstance(token_cost, dict) or not token_cost.get("totalTokens")
        ):
            raise RuntimeError("live LLM response did not include provider token usage")
        _emit(args.output_format, response, summary)
    except Exception as error:  # pragma: no cover - exercised by CLI smoke tests.
        sys.stderr.write(f"openai-compatible runner failed: {error}\n")
        return 1
    return 0


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--provider", required=True)
    parser.add_argument("--prompt", required=True)
    parser.add_argument(
        "--output-format", required=True, choices=sorted(_OUTPUT_FORMATS)
    )
    parser.add_argument("--model")
    parser.add_argument("--base-url")
    parser.add_argument("--api-key-env")
    parser.add_argument("--require-live-usage", action="store_true")
    return parser.parse_args(argv)


def _chat_completion(args: argparse.Namespace) -> dict[str, Any]:
    provider_defaults = _provider_defaults(args.provider)
    base_url = (
        args.base_url
        or os.environ.get(provider_defaults["base_url_env"])
        or provider_defaults["base_url"]
    ).rstrip("/")
    api_key_env = args.api_key_env or provider_defaults["api_key_env"]
    api_key = os.environ.get(api_key_env)
    if not api_key:
        raise RuntimeError(f"{api_key_env} is not set")
    model = args.model or os.environ.get(provider_defaults["model_env"])
    if not model:
        raise RuntimeError(
            f"model is required via --model or {provider_defaults['model_env']}"
        )

    request = urllib.request.Request(
        f"{base_url}/chat/completions",
        data=json.dumps(
            {
                "model": model,
                "messages": [{"role": "user", "content": args.prompt}],
                "stream": False,
            }
        ).encode("utf-8"),
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            payload = response.read().decode("utf-8")
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"provider HTTP {error.code}: {body}") from error

    result = json.loads(payload)
    if not isinstance(result, dict):
        raise RuntimeError("provider response was not a JSON object")
    result["_sandtable"] = {
        "apiKeyEnv": api_key_env,
        "baseUrl": base_url,
        "model": model,
        "provider": args.provider,
    }
    return result


def _provider_defaults(provider: str) -> dict[str, str]:
    env_prefix = provider.upper().replace("-", "_")
    if provider == "deepseek":
        return {
            "api_key_env": "DEEPSEEK_API_KEY",
            "base_url": "https://api.deepseek.com",
            "base_url_env": "DEEPSEEK_BASE_URL",
            "model_env": "DEEPSEEK_MODEL",
        }
    return {
        "api_key_env": f"{env_prefix}_API_KEY",
        "base_url": "https://api.openai.com/v1",
        "base_url_env": f"{env_prefix}_BASE_URL",
        "model_env": f"{env_prefix}_MODEL",
    }


def _summary_from_response(
    args: argparse.Namespace,
    response: dict[str, Any],
) -> dict[str, Any]:
    sandtable = response.get("_sandtable")
    if not isinstance(sandtable, dict):
        sandtable = {}
    return {
        "type": "OpenAICompatibleSummary",
        "provider": sandtable.get("provider", args.provider),
        "model": sandtable.get("model", args.model),
        "finalAnswer": _answer_from_response(response),
        "tokenCost": _token_cost_from_usage(
            sandtable.get("provider", args.provider),
            sandtable.get("model", args.model),
            response.get("usage"),
        ),
    }


def _answer_from_response(response: dict[str, Any]) -> str:
    choices = response.get("choices")
    if not isinstance(choices, list) or not choices:
        return ""
    first = choices[0]
    if not isinstance(first, dict):
        return ""
    message = first.get("message")
    if not isinstance(message, dict):
        return ""
    content = message.get("content")
    return content if isinstance(content, str) else ""


def _token_cost_from_usage(
    provider: Any,
    model: Any,
    usage: Any,
) -> dict[str, Any]:
    if not isinstance(usage, dict):
        return {}
    prompt_tokens = _int_usage(usage, "prompt_tokens", "input_tokens")
    completion_tokens = _int_usage(usage, "completion_tokens", "output_tokens")
    total_tokens = _int_usage(usage, "total_tokens")
    if not total_tokens:
        total_tokens = prompt_tokens + completion_tokens
    token_cost: dict[str, Any] = {
        "apiCalls": 1,
        "source": "openai-compatible-live",
        "usageRecords": 1,
    }
    if isinstance(provider, str) and provider:
        token_cost["provider"] = provider
    if isinstance(model, str) and model:
        token_cost["model"] = model
    if prompt_tokens:
        token_cost["inputTokens"] = prompt_tokens
    if completion_tokens:
        token_cost["outputTokens"] = completion_tokens
    if total_tokens:
        token_cost["totalTokens"] = total_tokens
    return token_cost


def _int_usage(usage: dict[str, Any], *fields: str) -> int:
    for field in fields:
        value = usage.get(field)
        if isinstance(value, bool):
            continue
        if isinstance(value, int):
            return value
        if isinstance(value, float) and value.is_integer():
            return int(value)
    return 0


def _emit(
    output_format: str,
    response: dict[str, Any],
    summary: dict[str, Any],
) -> None:
    if output_format in {"stream-json", "summary-json"}:
        sys.stdout.write(f"{json.dumps(summary, sort_keys=True)}\n")
    elif output_format == "json":
        sys.stdout.write(
            f"{json.dumps([{'type': 'OpenAICompatibleMessage', 'response': response}, summary])}\n"
        )
    else:
        sys.stdout.write(f"{summary.get('finalAnswer', '')}\n")


if __name__ == "__main__":
    raise SystemExit(main())
