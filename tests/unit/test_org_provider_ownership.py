# SPDX-FileCopyrightText: 2026 tao3k team and Contributors
#
# SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

import configparser
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
        / "crates/agent-semantic-client/src/command/hook_runtime_skill_render.rs"
    ).read_text()
    assert "languages/org/" not in skill_render
    assert "../../../../org/contracts/asp.skill.v1.org" in skill_render
    assert "../../../../org/templates/ASP_ORG_SKILL.org" in skill_render


def test_orgize_is_an_implementation_dependency_not_a_provider_identity() -> None:
    assert not (ROOT / "org/provider/asp-org-provider-manifest.json").exists()
    assert not (ROOT / "org/provider/asp-md-provider-manifest.json").exists()
    model = (ROOT / "languages/orgize/src/document/model.rs").read_text()
    packets = (ROOT / "languages/orgize/src/document/packets.rs").read_text()

    assert 'Self::Org => "asp-org"' in model
    assert 'Self::Markdown => "asp-md"' in model
    assert 'Self::Org => "orgize"' in model
    assert '"providerId": language.provider_id()' in packets
    assert '"binary": env!("CARGO_PKG_NAME")' in packets
