use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_search::search_command_preflight::SearchPreflightLanguageId;
use agent_semantic_search::search_command_preflight::{
    SearchCommandPreflightBudget, SearchCommandPreflightOutcome, SearchCommandPreflightRequest,
    preflight_search_command, preflight_search_command_args, preflight_search_command_with_budget,
};

fn temp_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "agent-semantic-search-{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create search preflight fixture");
    root
}

#[test]
fn root_owner_is_rejected_before_provider_dispatch_for_every_registered_language() {
    let root = temp_root("root-owner-all-languages");
    let args = [
        "search",
        "owner",
        ".",
        "items",
        "--query",
        "typed block Boundary",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ]
    .map(str::to_owned);

    for language_id in ["rust", "typescript", "python", "julia", "gerbil-scheme"] {
        let typed_language_id = SearchPreflightLanguageId::from(language_id);
        let outcome = preflight_search_command_args(&typed_language_id, &args, &root);
        let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
            panic!("{language_id} must reject a workspace-root owner before provider dispatch");
        };
        assert!(
            error.contains(
                "[asp-search-query-error] code=invalid-owner owner=\".\" reason=workspace-root-owner"
            ),
            "{language_id} error={error}"
        );
    }
    std::fs::remove_dir_all(root).expect("remove preflight fixture");
}

#[test]
fn invalid_owner_parser_hot_path_is_sub_millisecond_per_request() {
    let root = temp_root("invalid-owner-hot-path");
    let args = [
        "search",
        "owner",
        ".",
        "items",
        "--query",
        "typed block Boundary",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ]
    .map(str::to_owned);
    let language_id = SearchPreflightLanguageId::from("gerbil-scheme");
    let started_at = Instant::now();
    for _ in 0..512 {
        assert!(matches!(
            preflight_search_command_args(&language_id, &args, &root),
            SearchCommandPreflightOutcome::Rejected(_)
        ));
    }
    let elapsed = started_at.elapsed();
    assert!(
        elapsed <= Duration::from_millis(50),
        "512 invalid-owner parser preflights exceeded 50ms: {elapsed:?}"
    );
    std::fs::remove_dir_all(root).expect("remove preflight fixture");
}

#[test]
fn concrete_owner_preflight_remains_inside_the_hot_path_budget() {
    let root = temp_root("concrete-owner-hot-path");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("src/lib.rs"), "fn main() {}\n").expect("write file owner");
    let language_id = SearchPreflightLanguageId::from("rust");
    let started_at = Instant::now();
    for _ in 0..512 {
        preflight_search_command_with_budget(
            SearchCommandPreflightRequest::owner_items(
                &language_id,
                Path::new("src/lib.rs"),
                Some(Path::new(".")),
                &root,
            ),
            SearchCommandPreflightBudget::new(Duration::from_millis(5)),
        )
        .expect("concrete file owner should pass preflight");
    }
    assert!(
        started_at.elapsed() <= Duration::from_millis(50),
        "concrete-owner preflight loop exceeded 50ms"
    );
    preflight_search_command(SearchCommandPreflightRequest::owner_items(
        &language_id,
        Path::new("src/lib.rs"),
        Some(Path::new(".")),
        &root,
    ))
    .expect("default-budget concrete owner preflight should pass");
    std::fs::remove_dir_all(root).expect("remove preflight fixture");
}
