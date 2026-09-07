# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import configparser
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def test_org_submodule_is_distinct_from_language_providers() -> None:
    config = configparser.ConfigParser()
    config.read(ROOT / ".gitmodules")

    assert 'submodule "languages/org"' not in config
    assert config['submodule "org"']["path"] == "org"
    assert config['submodule "languages/orgize"']["path"] == "languages/orgize"
    skill_render = (
        ROOT
        / "crates/agent-semantic-protocol/src/command/hook_runtime_skill_render.rs"
    ).read_text()
    assert "languages/org/" not in skill_render
    assert "../../../../org/contracts/asp.skill.v1.org" in skill_render
    assert "../../../../org/templates/ASP_ORG_SKILL.org" in skill_render


def test_orgize_owns_org_and_markdown_provider_manifests() -> None:
    assert not (ROOT / "org/provider/asp-org-provider-manifest.json").exists()
    assert not (ROOT / "org/provider/asp-md-provider-manifest.json").exists()

    expected_languages = {
        "asp-org-provider.json": "org",
        "asp-md-provider.json": "md",
    }
    for manifest_name, language_id in expected_languages.items():
        manifest_path = ROOT / "languages/orgize/schemas" / manifest_name
        manifest = json.loads(manifest_path.read_text())
        assert manifest["providerId"] == "orgize"
        assert manifest["binary"] == "orgize"
        assert manifest["languageId"] == language_id
