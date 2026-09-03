"""Verify that base-generation compilation is not a Python service surface."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, message


def test_compile_generation_is_rejected_as_a_removed_legacy_surface() -> None:
    session = AspPythonGraphsSession()
    session.handle(
        message(
            "hello",
            "hello-1",
            runtimeArtifactDigest=DIGEST_A,
            executionArtifactDigest=DIGEST_B,
        )
    )
    with pytest.raises(ServiceProtocolError) as error:
        session.handle(
            message(
                "compile-generation",
                "compile-removed",
                sequence=2,
                workspaceIdentity="workspace-a",
                generationDigest=DIGEST_A,
                payload={"graph": {"nodes": [], "edges": []}},
            )
        )
    assert error.value.code == "unsupported-message-kind"
