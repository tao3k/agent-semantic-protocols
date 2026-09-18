# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

"""Validate document provider identity and packet publication surfaces."""

from __future__ import annotations

import json
import unittest

from .helpers import REPO_ROOT


class SemanticDocumentRegistrySchemaTests(unittest.TestCase):
    def test_canonical_identity_and_dependency_publish_document_packets(self) -> None:
        identity_schema = json.loads(
            (
                REPO_ROOT
                / "schemas"
                / "canonical-provider-identity.v1.schema.json"
            ).read_text()
        )
        identities = {
            (
                branch["properties"]["languageId"]["const"],
                branch["properties"]["providerId"]["const"],
            )
            for branch in identity_schema["oneOf"]
        }
        self.assertIn(("org", "asp-org"), identities)
        self.assertIn(("md", "asp-md"), identities)

        packets = (
            REPO_ROOT / "languages" / "orgize" / "src" / "document" / "packets.rs"
        ).read_text()
        self.assertNotIn("semantic-document-search-packet", packets)
        self.assertIn("semantic-document-query-packet", packets)
        self.assertIn('"method": "query/document"', packets)


if __name__ == "__main__":
    unittest.main()
