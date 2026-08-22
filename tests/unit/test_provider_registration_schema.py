import json
from pathlib import Path

from .schema_validation import schema_validator_for


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_PATH = ROOT / "schemas" / "provider-registration.schema.json"
PROVIDER_REGISTRATIONS = (
    ROOT / "languages/rust-lang-project-harness/schemas/asp-registration.json",
    ROOT / "languages/python-lang-project-harness/schemas/asp-registration.json",
    ROOT / "languages/JuliaLangProjectHarness.jl/schemas/asp-registration.json",
    ROOT / "languages/gerbil-scheme-language-project-harness/schemas/asp-registration.json",
    ROOT / "languages/typescript-lang-project-harness/schemas/asp-registration.json",
)


def test_all_language_provider_registrations_use_the_shared_schema() -> None:
    validator = schema_validator_for(SCHEMA_PATH)
    for registration_path in PROVIDER_REGISTRATIONS:
        registration = json.loads(registration_path.read_text(encoding="utf-8"))
        errors = sorted(validator.iter_errors(registration), key=lambda error: list(error.path))
        assert not errors, (
            f"{registration_path.relative_to(ROOT)}: "
            + "; ".join(error.message for error in errors)
        )
