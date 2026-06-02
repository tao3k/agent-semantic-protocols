"""Tokenizer adapters for parser compact token-cost reports."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Callable


@dataclass(frozen=True)
class Tokenizer:
    tokenizer_id: str
    count: Callable[[str], int]


def load_tokenizer(spec: str) -> Tokenizer:
    if spec == "byte":
        return Tokenizer("byte", lambda text: len(text.encode("utf-8")))
    if spec.startswith("tiktoken:"):
        return _load_tiktoken(spec)
    if spec.startswith("hf:"):
        return _load_huggingface_tokenizer(spec)
    raise SystemExit(f"Unsupported tokenizer spec: {spec}")


def _load_tiktoken(spec: str) -> Tokenizer:
    encoding_name = spec.split(":", 1)[1]
    try:
        import tiktoken  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit(
            "tiktoken is a project dependency; run through `uv run parser-compact-snapshots` "
            "or choose --tokenizer byte"
        ) from exc
    encoding = tiktoken.get_encoding(encoding_name)
    return Tokenizer(spec, lambda text: len(encoding.encode(text)))


def _load_huggingface_tokenizer(spec: str) -> Tokenizer:
    tokenizer_ref = spec.split(":", 1)[1]
    try:
        from tokenizers import Tokenizer as HuggingFaceTokenizer  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit("Install tokenizers or choose --tokenizer byte") from exc
    tokenizer_path = Path(tokenizer_ref)
    if tokenizer_path.exists():
        tokenizer = HuggingFaceTokenizer.from_file(str(tokenizer_path))
    elif hasattr(HuggingFaceTokenizer, "from_pretrained"):
        tokenizer = HuggingFaceTokenizer.from_pretrained(tokenizer_ref)
    else:
        raise SystemExit(
            "tokenizers does not support from_pretrained; use hf:<tokenizer-json-path>"
        )
    return Tokenizer(spec, lambda text: len(tokenizer.encode(text).ids))
