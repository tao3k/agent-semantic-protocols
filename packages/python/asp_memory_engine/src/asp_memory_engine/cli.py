"""CLI for the Python Xiuxian memory engine adaptation."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence

from .binary_build import DEFAULT_BINARY_INTERPRETER, build_memory_engine_binary
from .checkpoint import Checkpoint
from .graph_turbo_memory import read_memory_projection
from .plan_rank import rank_plan_candidates
from .store import EpisodeStore, StoreConfig
from .worker import serve_memory_worker, serve_memory_worker_socket


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    if args.command == "graph-turbo-read-memory":
        payload = json.load(sys.stdin)
        projection = read_memory_projection(
            payload.get("candidateSelectors", []),
            payload.get("seenSelectors", []),
            max_gap_lines=args.max_gap_lines,
        )
        sys.stdout.write(
            json.dumps(
                {
                    "seenSelectors": list(projection.seen_selectors),
                    "suppressedSelectors": list(projection.suppressed_selectors),
                },
                sort_keys=True,
            )
            + "\n"
        )
        return 0
    if args.command == "rank-plans":
        return _rank_plans(args)
    if args.command == "checkpoint-put":
        return _checkpoint_put(args)
    if args.command == "checkpoint-list":
        return _checkpoint_list(args)
    if args.command == "build-binary":
        binary = build_memory_engine_binary(
            args.output,
            interpreter=args.interpreter,
            compressed=args.compress,
        )
        sys.stdout.write(
            "[build-binary] "
            "engine=asp-memory-engine "
            "kind=zipapp "
            f"path={_field(str(binary))} "
            f"interpreter={_field(args.interpreter)} "
            f"compressed={str(args.compress).lower()} "
            "venv=false\n"
        )
        return 0
    if args.command == "worker":
        if args.socket:
            return serve_memory_worker_socket(
                args.socket,
                default_state=args.state,
                default_embedding_dim=args.embedding_dim,
            )
        return serve_memory_worker(
            default_state=args.state,
            default_embedding_dim=args.embedding_dim,
        )
    raise SystemExit(f"unsupported command: {args.command}")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    read_memory = subparsers.add_parser("graph-turbo-read-memory")
    read_memory.add_argument("--max-gap-lines", type=int, default=8)
    rank_plans = subparsers.add_parser("rank-plans")
    rank_plans.add_argument("--state", default=StoreConfig().path)
    rank_plans.add_argument("--project", default="_global_project")
    rank_plans.add_argument("--session")
    rank_plans.add_argument("--branch")
    rank_plans.add_argument("--top-k", type=int, default=5)
    rank_plans.add_argument("--embedding-dim", type=int, default=384)
    checkpoint_put = subparsers.add_parser("checkpoint-put")
    checkpoint_put.add_argument("--state", default=StoreConfig().path)
    checkpoint_put.add_argument("--embedding-dim", type=int, default=384)
    checkpoint_list = subparsers.add_parser("checkpoint-list")
    checkpoint_list.add_argument("--state", default=StoreConfig().path)
    checkpoint_list.add_argument("--project")
    checkpoint_list.add_argument("--session")
    checkpoint_list.add_argument("--plan")
    checkpoint_list.add_argument("--branch")
    checkpoint_list.add_argument("--status")
    checkpoint_list.add_argument("--top-k", type=int)
    checkpoint_list.add_argument("--embedding-dim", type=int, default=384)
    build_binary = subparsers.add_parser("build-binary")
    build_binary.add_argument("--output", required=True)
    build_binary.add_argument("--interpreter", default=DEFAULT_BINARY_INTERPRETER)
    build_binary.add_argument("--compress", action="store_true")
    worker = subparsers.add_parser("worker")
    worker.add_argument("--state", default=StoreConfig().path)
    worker.add_argument("--embedding-dim", type=int, default=384)
    worker.add_argument("--socket")
    return parser.parse_args(argv)


def _checkpoint_put(args: argparse.Namespace) -> int:
    payload = json.load(sys.stdin)
    store = EpisodeStore(StoreConfig(path=args.state, embedding_dim=args.embedding_dim))
    store.load_state(args.state)
    checkpoint = Checkpoint.from_mapping(payload)
    store.store_checkpoint(checkpoint)
    store.save_state(args.state)
    sys.stdout.write(
        json.dumps(
            {
                "schemaId": "agent.semantic-protocols.memory-checkpoint-receipt",
                "schemaVersion": "1",
                "ok": True,
                "state": args.state,
                "checkpoint": checkpoint.to_mapping(),
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


def _checkpoint_list(args: argparse.Namespace) -> int:
    store = EpisodeStore(StoreConfig(path=args.state, embedding_dim=args.embedding_dim))
    store.load_state(args.state)
    checkpoints = store.list_checkpoints(
        project_id=args.project,
        session_id=args.session,
        plan_id=args.plan,
        branch_id=args.branch,
        status=args.status,
        top_k=args.top_k,
    )
    sys.stdout.write(
        json.dumps(
            {
                "schemaId": "agent.semantic-protocols.memory-checkpoint-list",
                "schemaVersion": "1",
                "state": args.state,
                "checkpoints": [checkpoint.to_mapping() for checkpoint in checkpoints],
            },
            sort_keys=True,
        )
        + "\n"
    )
    return 0


def _rank_plans(args: argparse.Namespace) -> int:
    payload = json.load(sys.stdin)
    store = EpisodeStore(StoreConfig(path=args.state, embedding_dim=args.embedding_dim))
    store.load_state(args.state)
    sys.stdout.write(
        json.dumps(
            rank_plan_candidates(
                payload,
                store=store,
                project=args.project,
                session=args.session,
                branch=args.branch,
                top_k=args.top_k,
            ),
            sort_keys=True,
        )
        + "\n"
    )
    return 0


def _field(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ")
    return f'"{escaped}"'


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
