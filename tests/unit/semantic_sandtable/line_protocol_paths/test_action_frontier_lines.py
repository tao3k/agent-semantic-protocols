"""Line-protocol coverage for agent-facing action frontier rows."""

import json
import tempfile
import unittest
from pathlib import Path

from tools.semantic_sandtable.scenario_runner import run_scenario


class LineProtocolActionFrontierTests(unittest.TestCase):
    def test_line_protocol_accepts_action_frontier_without_graph_block(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "python.owner-items-action-frontier-line-protocol",
                        "language": "python",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "owner-items",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-owner] q=fastapi/routing.py owner=1 item=1')\n"
                                        "print('|query itemQuery=\"APIRouter\" status=hit next=item-skeleton')\n"
                                        "print('|owner fastapi/routing.py role=\"root,module\" next=python://fastapi/routing.py#item/class/APIRouter')\n"
                                        "print('A1=item-skeleton(selector=python://fastapi/routing.py#item/class/APIRouter,projection=skeleton,hint=fastapi/routing.py:1005:4956)!skeleton')\n"
                                        "print('actionFrontier=A1.item-skeleton')\n"
                                        "print('recommendedNext=A1.item-skeleton')\n"
                                        "print(\"nextCommand=asp python query --from-hook item-skeleton --selector 'python://fastapi/routing.py#item/class/APIRouter' --workspace . --names-only\")\n"
                                        "print('reason=owner-item-skeleton-ready')\n"
                                        "print('avoid=selector-code-before-exact,direct-source-read,manual-window-scan')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

            self.assertEqual("pass", result.status)

    def test_line_protocol_accepts_action_frontier_lines(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "rust.action-frontier-line-protocol",
                        "language": "rust",
                        "workdir": ".",
                        "steps": [
                            {
                                "id": "owner-items",
                                "command": [
                                    "python",
                                    "-c",
                                    (
                                        "print('[search-reasoning] q=owner-query alg=asp-fast-owner-query-v1')\n"
                                        "print('legend: ID=kind:role(value)!next; edge SRC>{DST:rel}; frontier ID.next')\n"
                                        "print('aliases: graph:{G=search,Q=query,T=test,O=owner,I=item}')\n"
                                        "print('Q=query:term(AsyncRead|poll_read)!query;T=test:path(src/io/async_read.rs)!tests;O=owner:path(src/io/async_read.rs)!owner;')\n"
                                        "print('I=item:symbol(AsyncRead)@rust://src/io/async_read.rs#item/trait/AsyncRead!syntax;')\n"
                                        "print(\"syntax I selector=rust://src/io/async_read.rs#item/trait/AsyncRead displayLineRange=44:60 sourceLocatorHint=src/io/async_read.rs:44:60 pattern='((trait_item name: (_) @type.name))'\")\n"
                                        "print('G>{Q:matches,T:covers,O:selects,I:contains}')\n"
                                        "print('rank=Q,T,O,I frontier=Q.query,T.tests,O.owner,I.syntax')\n"
                                        "print('A1=item-skeleton(selector=rust://src/io/async_read.rs#item/trait/AsyncRead,projection=skeleton,hint=src/io/async_read.rs:44:60)!skeleton')\n"
                                        "print('A2=syntax-outline(selector=rust://src/io/async_read.rs#item/trait/AsyncRead,projection=outline,hint=src/io/async_read.rs:44:60)!syntax')\n"
                                        "print('A3=query-code(selector=rust://src/io/async_read.rs#item/trait/AsyncRead,requiresExact=true,codePolicy=exact-only,hint=src/io/async_read.rs:44:60)!query-code')\n"
                                        "print('actionFrontier=A1.item-skeleton,A2.syntax-outline,A3.query-code')\n"
                                        "print('recommendedNext=A1.item-skeleton')\n"
                                        "print(\"nextCommand=asp rust query --from-hook item-skeleton --selector 'rust://src/io/async_read.rs#item/trait/AsyncRead' --workspace . --names-only\")\n"
                                        "print('reason=owner-item-skeleton-ready')\n"
                                        "print('avoid=selector-code-before-exact,direct-source-read,manual-window-scan')\n"
                                        "print('entries=owner-query(O,Q=>items+tests+dependency-usage)')"
                                    ),
                                ],
                                "expect": {"lineProtocol": True},
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)

            self.assertEqual("pass", result.status)
