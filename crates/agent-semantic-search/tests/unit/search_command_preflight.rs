use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use agent_semantic_search::search_command_preflight::{
    OwnerItemsLanguageAdmission, SearchCommandPreflightBudget, SearchCommandPreflightOutcome,
    SearchCommandPreflightRequest, preflight_search_command, preflight_search_command_args,
    preflight_search_command_args_at_invocation_root,
    preflight_search_command_args_with_owner_language_admission,
    preflight_search_command_with_budget,
};

#[test]
fn owner_items_preflight_rejects_root_owner_for_any_language() {
    let root = temp_root("search-preflight-root-owner");
    let error = preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "python",
        Path::new("."),
        Some(Path::new(".")),
        &root,
    ))
    .expect_err("root owner should be rejected");

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\".\" reason=workspace-root-owner"
    ));
    assert!(error.contains(&format!(
        "nextCommand=asp python search pipe '<focused terms>' --workspace {} --view seeds",
        root.display()
    )));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_rejects_directory_owner_for_any_language() {
    let root = temp_root("search-preflight-directory-owner");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    let error = preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "typescript",
        Path::new("src"),
        Some(Path::new(".")),
        &root,
    ))
    .expect_err("directory owner should be rejected");

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\"src\" reason=directory-owner"
    ));
    assert!(error.contains(&format!(
        "nextCommand=asp typescript search pipe '<focused terms>' --workspace {} --view seeds",
        root.display()
    )));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_rejects_normalized_directory_traversal_root_owner() {
    let root = temp_root("search-preflight-normalized-root-owner");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");

    let error = preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "rust",
        Path::new("src/../"),
        Some(Path::new(".")),
        &root,
    ))
    .expect_err("normalized root owner should be rejected");

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\"src/../\" reason=workspace-root-owner"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_accepts_concrete_file_owner() {
    let root = temp_root("search-preflight-file-owner");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("src/lib.rs"), "fn main() {}\n").expect("write file owner");

    preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "rust",
        Path::new("src/../src/lib.rs"),
        Some(Path::new(".")),
        &root,
    ))
    .expect("normalized file owner should pass preflight");

    preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "rust",
        Path::new("src/lib.rs"),
        Some(Path::new(".")),
        &root,
    ))
    .expect("concrete file owner should pass preflight");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_rejects_missing_file_owner_for_any_language() {
    let root = temp_root("search-preflight-missing-owner");

    let error = preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "rust",
        Path::new("src/missing.rs"),
        Some(Path::new(".")),
        &root,
    ))
    .expect_err("missing file owner should be rejected");

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\"src/missing.rs\" reason=missing-owner"
    ));
    assert!(error.contains(&format!(
        "nextCommand=asp rust search pipe '<focused terms>' --workspace {} --view seeds",
        root.display()
    )));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_rejects_existing_owner_for_another_language() {
    let root = temp_root("search-preflight-owner-language-mismatch");
    std::fs::create_dir_all(root.join("docs")).expect("create document directory");
    std::fs::write(root.join("docs/guide.org"), "* Guide\n").expect("write document owner");
    let args = [
        "search",
        "owner",
        "docs/guide.org",
        "items",
        "--query",
        "Guide",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ]
    .map(str::to_string);
    let expected_extensions = vec!["ss".to_string(), "scm".to_string()];

    let outcome = preflight_search_command_args_with_owner_language_admission(
        "gerbil-scheme",
        &args,
        &root,
        OwnerItemsLanguageAdmission::new(&expected_extensions, Some("org")),
    );
    let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
        panic!("expected owner-language-mismatch rejection, got {outcome:?}");
    };

    assert!(error.contains(
        "code=owner-language-mismatch owner=\"docs/guide.org\" requestedLanguage=gerbil-scheme ownerExtension=org expectedExtensions=scm|ss suggestedLanguage=org"
    ));
    assert!(error.contains("no provider was started"));
    assert!(error.contains(
        "nextCommand=asp org search owner 'docs/guide.org' items --query '<symbol-or-a|b|c>'"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_rejects_existing_owner_outside_workspace() {
    let root = temp_root("search-preflight-workspace-owner");
    let external_root = temp_root("search-preflight-external-owner");
    let external_owner = external_root.join("outside.ss");
    std::fs::write(&external_owner, "(def outside #t)\n").expect("write external owner");

    let error = preflight_search_command(SearchCommandPreflightRequest::owner_items(
        "gerbil-scheme",
        &external_owner,
        Some(&root),
        &root,
    ))
    .expect_err("external file owner should be rejected");

    assert!(error.contains("code=invalid-owner"));
    assert!(error.contains("reason=outside-workspace"));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(external_root);
}

#[test]
fn raw_args_preflight_rejects_invalid_owner_before_provider_dispatch() {
    let root = temp_root("search-preflight-raw-args");
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
    .map(str::to_string);

    let outcome = preflight_search_command_args("gerbil-scheme", &args, &root);
    let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
        panic!("expected raw args invalid-owner rejection, got {outcome:?}");
    };

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\".\" reason=workspace-root-owner"
    ));
    assert!(error.contains("nextCommand=asp gerbil-scheme search pipe '<focused terms>'"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raw_args_preflight_rejects_search_owner_without_items_surface() {
    let root = temp_root("search-preflight-owner-without-items");
    let args = [
        "search",
        "owner",
        "src/lib.rs",
        "--query",
        "typed block Boundary",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ]
    .map(str::to_string);

    let outcome = preflight_search_command_args("gerbil-scheme", &args, &root);
    let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
        panic!("expected search-owner shape rejection, got {outcome:?}");
    };

    assert!(
        error.contains(
            "[asp-search-query-error] code=invalid-search-command owner=\"src/lib.rs\" reason=missing-items-surface"
        )
    );
    assert!(error.contains(
        "nextCommand=asp gerbil-scheme search owner src/lib.rs items --query '<symbol-or-a|b|c>'"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raw_args_preflight_rejects_search_owner_missing_owner_path() {
    let root = temp_root("search-preflight-owner-missing-owner");
    let args = [
        "search",
        "owner",
        "--query",
        "typed block Boundary",
        "--workspace",
        ".",
        "--view",
        "seeds",
    ]
    .map(str::to_string);

    let outcome = preflight_search_command_args("typescript", &args, &root);
    let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
        panic!("expected missing-owner path rejection, got {outcome:?}");
    };

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-search-command reason=missing-owner-path"
    ));
    assert!(error.contains(
        "nextCommand=asp typescript search owner <owner-path> items --query '<symbol-or-a|b|c>'"
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raw_args_preflight_uses_workspace_text_before_project_resolution() {
    let invocation_root = temp_root("search-preflight-invocation-root");
    let external_workspace = temp_root("search-preflight-external-workspace");
    let args = [
        "search",
        "owner",
        ".",
        "items",
        "--query",
        "typed block Boundary",
        "--workspace",
        external_workspace.to_str().expect("workspace path"),
        "--view",
        "seeds",
    ]
    .map(str::to_string);

    let outcome =
        preflight_search_command_args_at_invocation_root("gerbil-scheme", &args, &invocation_root);
    let SearchCommandPreflightOutcome::Rejected(error) = outcome else {
        panic!("expected raw args invalid-owner rejection, got {outcome:?}");
    };

    assert!(error.contains(
        "[asp-search-query-error] code=invalid-owner owner=\".\" reason=workspace-root-owner"
    ));
    assert!(error.contains(&format!(
        "--workspace {} --view seeds",
        external_workspace.display()
    )));
    let _ = std::fs::remove_dir_all(invocation_root);
    let _ = std::fs::remove_dir_all(external_workspace);
}

#[test]
fn owner_items_preflight_stays_inside_hot_path_budget() {
    let root = temp_root("search-preflight-budget");
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("src/lib.rs"), "fn main() {}\n").expect("write file owner");
    let budget = SearchCommandPreflightBudget::new(Duration::from_millis(5));
    let started_at = Instant::now();

    for _ in 0..512 {
        preflight_search_command_with_budget(
            SearchCommandPreflightRequest::owner_items(
                "rust",
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
        "preflight loop exceeded aggregate search-package hot path budget"
    );
    preflight_search_command_with_budget(
        SearchCommandPreflightRequest::owner_items(
            "rust",
            Path::new("src/lib.rs"),
            Some(Path::new(".")),
            &root,
        ),
        budget,
    )
    .expect("single preflight should stay inside explicit budget");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn owner_items_preflight_budget_does_not_mask_invalid_owner_error() {
    let root = temp_root("search-preflight-budget-mask");
    let budget = SearchCommandPreflightBudget::new(Duration::from_nanos(0));
    let result = preflight_search_command_with_budget(
        SearchCommandPreflightRequest::owner_items(
            "rust",
            Path::new("."),
            Some(Path::new(".")),
            &root,
        ),
        budget,
    );
    let error = result.expect_err("invalid owner should still be rejected");
    assert!(
        error.contains("code=invalid-owner"),
        "budget check must not overwrite invalid-owner error: {error}"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn temp_root(label: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("agent-semantic-search-{label}-{suffix}"));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
