#[path = "../../src/server/runtime_provider_refresh.rs"]
mod runtime_provider_refresh;

#[test]
fn changed_launch_key_replaces_only_the_same_provider() {
    assert!(runtime_provider_refresh::provider_runtime_is_stale(
        "generation-a:rust:asp-rust",
        "rust",
        "asp-rust",
        "generation-b:rust:asp-rust",
        "rust",
        "asp-rust",
    ));
    assert!(!runtime_provider_refresh::provider_runtime_is_stale(
        "generation-a:rust:asp-rust",
        "rust",
        "asp-rust",
        "generation-b:python:asp-python",
        "python",
        "asp-python",
    ));
    assert!(!runtime_provider_refresh::provider_runtime_is_stale(
        "generation-a:rust:asp-rust",
        "rust",
        "asp-rust",
        "generation-a:rust:asp-rust",
        "rust",
        "asp-rust",
    ));
}
