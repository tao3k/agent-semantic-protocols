"""Assert that Python cannot become a request-time search authority."""

from __future__ import annotations

import pytest

from asp_python_graphs.service_protocol import ServiceProtocolError
from asp_python_graphs.service_session import AspPythonGraphsSession
from service_session_support import DIGEST_A, DIGEST_B, message


def test_search_evidence_stream_is_not_a_python_service_operation() -> None:
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
        session.handle(message("search-evidence", "legacy-search-evidence", sequence=2))
    assert error.value.code == "unsupported-message-kind"
