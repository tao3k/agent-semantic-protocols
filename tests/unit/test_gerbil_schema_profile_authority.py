import json
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def test_gerbil_profile_and_provider_registry_have_one_schema_authority_each() -> None:
    registry = json.loads(
        (REPO_ROOT / "schemas" / "language-schema-profiles.json").read_text(
            encoding="utf-8"
        )
    )
    gerbil = next(
        profile
        for profile in registry["profiles"]
        if profile["languageId"] == "gerbil-scheme"
    )

    assert gerbil["providerOwned"] == [
        "semantic-gerbil-scheme-harness-info.v1.schema.json"
    ]
    assert gerbil["rootSets"]
    assert gerbil["roots"]

    provider_registry = (
        REPO_ROOT
        / "languages"
        / "gerbil-scheme-language-project-harness"
        / "src"
        / "protocol"
        / "registry.ss"
    ).read_text(encoding="utf-8")
    assert provider_registry.count('(path "schemas/') == 1
    assert (
        '(path "schemas/semantic-gerbil-scheme-harness-info.v1.schema.json")'
        in provider_registry
    )


def test_ci_projects_canonical_profiles_and_checks_the_provider_registry() -> None:
    root_ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    assert "canonical_registry_verifies_every_registered_language_bundle" in root_ci
    assert (
        "canonical_profiles_project_ready_and_unchanged_in_process_under_one_millisecond"
        in root_ci
    )

    gerbil_ci = (
        REPO_ROOT
        / "languages"
        / "gerbil-scheme-language-project-harness"
        / ".github"
        / "workflows"
        / "ci.yml"
    ).read_text(encoding="utf-8")
    assert "t/provider-owned-schema-registry-test.ss" in gerbil_ci
