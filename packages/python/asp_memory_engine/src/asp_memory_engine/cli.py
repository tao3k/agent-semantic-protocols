"""CLI for the Python Xiuxian memory engine adaptation."""

from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence

from .graph_turbo_memory import read_memory_projection
from .plan_context import PlanMemoryContext
from .store import EpisodeStore, StoreConfig


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
    if args.command == "recall-plan":
        store = EpisodeStore(StoreConfig(path=args.state, embedding_dim=args.embedding_dim))
        store.load_state(args.state)
        context = PlanMemoryContext(
            project_id=args.project,
            session_id=args.session,
            plan_id=args.plan,
            branch_id=args.branch,
        )
        results = store.recall_for_plan(args.intent, context, top_k=args.top_k)
        sys.stdout.write(
            f"[recall-plan] engine=asp-memory-engine state={args.state} hits={len(results)}\n"
        )
        for episode, score in results:
            sys.stdout.write(
                "|episode "
                f"id={_field(episode.id)} "
                f"score={score:.6f} "
                f"project={_field(episode.project_id)} "
                f"session={_field(episode.session_id or '-')} "
                f"plan={_field(episode.plan_id or '-')} "
                f"branch={_field(episode.branch_id or '-')} "
                f"sharing={_field(episode.plan_sharing)} "
                f"intent={_field(episode.intent)} "
                f"experience={_field(episode.experience)}\n"
            )
        return 0
    raise SystemExit(f"unsupported command: {args.command}")


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    read_memory = subparsers.add_parser("graph-turbo-read-memory")
    read_memory.add_argument("--max-gap-lines", type=int, default=8)
    recall_plan = subparsers.add_parser("recall-plan")
    recall_plan.add_argument("--state", default=StoreConfig().path)
    recall_plan.add_argument("--intent", required=True)
    recall_plan.add_argument("--project", default="_global_project")
    recall_plan.add_argument("--session")
    recall_plan.add_argument("--plan")
    recall_plan.add_argument("--branch")
    recall_plan.add_argument("--top-k", type=int, default=5)
    recall_plan.add_argument("--embedding-dim", type=int, default=384)
    return parser.parse_args(argv)


def _field(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ")
    return f'"{escaped}"'


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
