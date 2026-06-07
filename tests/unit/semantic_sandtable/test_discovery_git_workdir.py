"""Focused semantic sandtable discovery and step tests."""

from __future__ import annotations

from ._discovery_steps_common import (
    Path,
    json,
    run_scenario,
    subprocess,
    sys,
    tempfile,
    unittest,
)


class DiscoveryAndStepRunnerTests(unittest.TestCase):
    def test_workdir_git_clones_into_sandtable_repo_cache(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp)
            origin = repo_root / "origin"
            origin.mkdir()
            subprocess.run(
                ["git", "init", str(origin)], check=True, capture_output=True
            )
            (origin / "README.md").write_text("fixture\n", encoding="utf-8")
            subprocess.run(
                ["git", "-C", str(origin), "add", "README.md"],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                [
                    "git",
                    "-C",
                    str(origin),
                    "-c",
                    "user.name=Sandtable",
                    "-c",
                    "user.email=sandtable@example.invalid",
                    "commit",
                    "-m",
                    "fixture",
                ],
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "-C", str(origin), "tag", "v1"],
                check=True,
                capture_output=True,
            )
            scenario_path = repo_root / "scenario.json"
            scenario_path.write_text(
                json.dumps(
                    {
                        "id": "root.cached-git-workdir",
                        "language": "root",
                        "workdir": {
                            "git": {
                                "url": origin.as_uri(),
                                "ref": "v1",
                                "depth": 1,
                                "cacheKey": "fixture-v1",
                                "subdir": ".",
                            }
                        },
                        "steps": [
                            {
                                "id": "touch-marker",
                                "command": [
                                    sys.executable,
                                    "-c",
                                    (
                                        "from pathlib import Path; "
                                        "Path('ran').write_text('ok')"
                                    ),
                                ],
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            result = run_scenario(repo_root, scenario_path)
            cache_checkout = repo_root / ".cache" / "sandtable-repos" / "fixture-v1"

            self.assertEqual("pass", result.status)
            self.assertEqual(cache_checkout.resolve(), result.workdir)
            self.assertTrue((cache_checkout / ".git").exists())
            self.assertTrue((cache_checkout / "ran").exists())
