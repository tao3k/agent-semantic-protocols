"""Local JSON memory state backend."""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass, field
from pathlib import Path

from .checkpoint import Checkpoint
from .episode import Episode


@dataclass(frozen=True)
class MemoryStateSnapshot:
    episodes: tuple[Episode, ...] = ()
    checkpoints: tuple[Checkpoint, ...] = ()
    q_values: dict[str, float] = field(default_factory=dict)
    recall_feedback_bias_by_scope: dict[str, float] = field(default_factory=dict)


class LocalMemoryStateStore:
    def __init__(self, path: str | Path) -> None:
        self.path = Path(path)

    @property
    def backend_name(self) -> str:
        if self._is_sqlite:
            return "sqlite"
        return "local"

    @property
    def _is_sqlite(self) -> bool:
        return self.path.suffix.lower() in {".db", ".sqlite", ".sqlite3"}

    def load_snapshot(self) -> MemoryStateSnapshot:
        if self._is_sqlite:
            return self._load_sqlite_snapshot()
        if not self.path.exists():
            return MemoryStateSnapshot()
        payload = json.loads(self.path.read_text(encoding="utf-8"))
        return MemoryStateSnapshot(
            episodes=tuple(Episode.from_mapping(item) for item in payload.get("episodes", [])),
            checkpoints=tuple(
                Checkpoint.from_mapping(item) for item in payload.get("checkpoints", [])
            ),
            q_values={str(k): float(v) for k, v in payload.get("q_values", {}).items()},
            recall_feedback_bias_by_scope={
                str(k): float(v)
                for k, v in payload.get("recall_feedback_bias_by_scope", {}).items()
            },
        )

    def save_snapshot(self, snapshot: MemoryStateSnapshot) -> None:
        if self._is_sqlite:
            self._save_sqlite_snapshot(snapshot)
            return
        self.path.parent.mkdir(parents=True, exist_ok=True)
        payload = {
            "episodes": [episode.to_mapping() for episode in snapshot.episodes],
            "checkpoints": [checkpoint.to_mapping() for checkpoint in snapshot.checkpoints],
            "q_values": snapshot.q_values,
            "recall_feedback_bias_by_scope": snapshot.recall_feedback_bias_by_scope,
        }
        self.path.write_text(json.dumps(payload, sort_keys=True, indent=2) + "\n", encoding="utf-8")

    def _load_sqlite_snapshot(self) -> MemoryStateSnapshot:
        if not self.path.exists():
            return MemoryStateSnapshot()
        with sqlite3.connect(self.path) as conn:
            self._ensure_sqlite_schema(conn)
            episodes = tuple(
                Episode.from_mapping(json.loads(row[0]))
                for row in conn.execute("SELECT payload FROM episodes ORDER BY id")
            )
            checkpoints = tuple(
                Checkpoint.from_mapping(json.loads(row[0]))
                for row in conn.execute(
                    "SELECT payload FROM checkpoints ORDER BY updated_at DESC, id"
                )
            )
            q_values = {
                str(key): float(value)
                for key, value in conn.execute("SELECT key, value FROM q_values")
            }
            recall_feedback_bias_by_scope = {
                str(key): float(value)
                for key, value in conn.execute(
                    "SELECT key, value FROM recall_feedback_bias_by_scope"
                )
            }
        return MemoryStateSnapshot(
            episodes=episodes,
            checkpoints=checkpoints,
            q_values=q_values,
            recall_feedback_bias_by_scope=recall_feedback_bias_by_scope,
        )

    def _save_sqlite_snapshot(self, snapshot: MemoryStateSnapshot) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        with sqlite3.connect(self.path) as conn:
            self._ensure_sqlite_schema(conn)
            conn.execute("DELETE FROM episodes")
            conn.execute("DELETE FROM checkpoints")
            conn.execute("DELETE FROM q_values")
            conn.execute("DELETE FROM recall_feedback_bias_by_scope")
            conn.executemany(
                "INSERT INTO episodes (id, payload) VALUES (?, ?)",
                [
                    (episode.id, json.dumps(episode.to_mapping(), sort_keys=True))
                    for episode in snapshot.episodes
                ],
            )
            conn.executemany(
                """
                INSERT INTO checkpoints
                    (id, session_id, project_id, plan_id, branch_id, status, updated_at, payload)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                [
                    (
                        checkpoint.id,
                        checkpoint.session_id,
                        checkpoint.project_id,
                        checkpoint.plan_id,
                        checkpoint.branch_id,
                        checkpoint.status,
                        checkpoint.updated_at,
                        json.dumps(checkpoint.to_mapping(), sort_keys=True),
                    )
                    for checkpoint in snapshot.checkpoints
                ],
            )
            conn.executemany(
                "INSERT INTO q_values (key, value) VALUES (?, ?)",
                sorted(snapshot.q_values.items()),
            )
            conn.executemany(
                "INSERT INTO recall_feedback_bias_by_scope (key, value) VALUES (?, ?)",
                sorted(snapshot.recall_feedback_bias_by_scope.items()),
            )

    def _ensure_sqlite_schema(self, conn: sqlite3.Connection) -> None:
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS episodes (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS checkpoints (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                plan_id TEXT,
                branch_id TEXT,
                status TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                payload TEXT NOT NULL
            )
            """
        )
        conn.execute(
            """
            CREATE INDEX IF NOT EXISTS idx_checkpoints_session_updated
            ON checkpoints(session_id, updated_at DESC)
            """
        )
        conn.execute(
            """
            CREATE INDEX IF NOT EXISTS idx_checkpoints_plan
            ON checkpoints(project_id, session_id, plan_id, branch_id)
            """
        )
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS q_values (
                key TEXT PRIMARY KEY,
                value REAL NOT NULL
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS recall_feedback_bias_by_scope (
                key TEXT PRIMARY KEY,
                value REAL NOT NULL
            )
            """
        )
