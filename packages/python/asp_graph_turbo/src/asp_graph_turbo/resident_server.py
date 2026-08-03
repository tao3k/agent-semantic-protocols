from __future__ import annotations

import json
import sys
from typing import IO, Mapping

from .resident_session import ResidentGraphTurboSession, ResidentProtocolError

__all__ = ["ResidentGraphTurboSession", "ResidentProtocolError", "serve_json_lines"]


def serve_json_lines(reader: IO[str], writer: IO[str]) -> int:
    session = ResidentGraphTurboSession()
    for line in reader:
        if not line.strip():
            continue
        request_id: object = None
        try:
            message = json.loads(line)
            if not isinstance(message, Mapping):
                raise ResidentProtocolError(
                    "invalid-message", "message must be a JSON object"
                )
            request_id = message.get("requestId")
            receipt = session.handle(message)
        except (json.JSONDecodeError, ResidentProtocolError) as error:
            code = (
                error.code if isinstance(error, ResidentProtocolError) else "invalid-json"
            )
            receipt = {
                "schemaId": "agent.semantic-protocols.semantic-graph-turbo-resident-receipt",
                "schemaVersion": "1",
                "protocolId": "agent.semantic-protocols.semantic-language",
                "protocolVersion": "1",
                "packetKind": "graph-turbo-resident-receipt",
                "requestId": request_id,
                "status": "rejected",
                "failure": {"code": code, "message": str(error)},
            }
        writer.write(json.dumps(receipt, sort_keys=True, separators=(",", ":")) + "\n")
        writer.flush()
        if session.closed:
            return 0
    return 0


def main() -> int:
    return serve_json_lines(sys.stdin, sys.stdout)


if __name__ == "__main__":
    raise SystemExit(main())
