use super::fixtures::{
    EnvVarGuard, isolate_home, run_git, temp_root, write_gerbil_activation_with_command_prefix,
};
use crate::{cache_cli::run_cache, source_index::refresh_source_index};
use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, LanguageId};

#[test]
fn cache_source_index_refresh_updates_dirty_tracked_worktree() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-dirty-tracked-worktree");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    let source_path = source_dir.join("usage.ss");
    std::fs::write(
        &source_path,
        "(def (poo-read input)\n  ;; gerbil-poo://usage\n  input)\n",
    )
    .expect("write gerbil source");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "gerbil.pkg", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild clean source index");

    std::fs::write(
        &source_path,
        "(def (poo-read input)\n  ;; gerbil-poo://dirty\n  input)\n",
    )
    .expect("dirty tracked source");
    let refreshed = refresh_source_index(&root)
        .expect("refresh dirty source index")
        .expect("refresh must publish changed tracked source without requiring rebuild");
    assert!(
        !refreshed.reused_generation,
        "the first dirty refresh must publish changed source content"
    );
    let reused = refresh_source_index(&root)
        .expect("reuse unchanged dirty source index")
        .expect("unchanged dirty source must retain its source-index generation");
    assert!(
        reused.reused_generation,
        "the second dirty refresh must reuse the unchanged full-source generation"
    );
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "dirty",
        8,
    )
    .expect("lookup refreshed dirty source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_detects_clean_committed_source_change() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-clean-committed-change");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    let source_path = source_dir.join("usage.ss");
    std::fs::write(
        &source_path,
        "(def (old-committed-symbol input)\n  input)\n",
    )
    .expect("write initial gerbil source");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "gerbil.pkg", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild initial clean source index");

    std::fs::write(
        &source_path,
        "(def (new-committed-symbol input)\n  input)\n",
    )
    .expect("write changed gerbil source");
    run_git(&root, ["add", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "change indexed source",
        ],
    );
    let refreshed = refresh_source_index(&root)
        .expect("refresh clean committed source index")
        .expect("existing generation must refresh");
    assert!(
        !refreshed.reused_generation,
        "a clean committed source change must publish a new generation"
    );
    let result = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "new-committed-symbol",
        8,
    )
    .expect("lookup refreshed clean committed source index");
    assert_eq!(result.state.as_str(), "hit");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].path, "src/usage.ss");
    let reused = refresh_source_index(&root)
        .expect("refresh unchanged clean committed source index")
        .expect("unchanged generation must remain available");
    assert!(
        reused.reused_generation,
        "the next full-source refresh must reuse the unchanged generation"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_tracks_rename_then_delete_without_stale_owner() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-rename-delete");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        source_dir.join("usage.ss"),
        "(def (rename-delete-symbol input)\n  input)\n",
    )
    .expect("write source that will be renamed and deleted");
    std::fs::write(
        source_dir.join("keeper.ss"),
        "(def (keeper-symbol input)\n  input)\n",
    )
    .expect("write retained source");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(
        &root,
        ["add", "gerbil.pkg", "src/usage.ss", "src/keeper.ss"],
    );
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild initial source index");
    let initial = refresh_source_index(&root)
        .expect("refresh initial source index")
        .expect("initial generation must exist");
    assert!(initial.reused_generation);

    std::fs::rename(source_dir.join("usage.ss"), source_dir.join("renamed.ss"))
        .expect("rename indexed source");
    run_git(&root, ["add", "-A"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "rename indexed source",
        ],
    );
    let renamed = refresh_source_index(&root)
        .expect("refresh renamed source index")
        .expect("renamed generation must exist");
    assert!(!renamed.reused_generation);
    assert_ne!(renamed.source_snapshot, initial.source_snapshot);
    assert_ne!(renamed.index_artifact_digest, initial.index_artifact_digest);
    let renamed_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "rename-delete-symbol",
        8,
    )
    .expect("lookup renamed source");
    assert_eq!(renamed_lookup.state.as_str(), "hit");
    assert_eq!(renamed_lookup.candidates.len(), 1);
    assert_eq!(renamed_lookup.candidates[0].path, "src/renamed.ss");

    std::fs::remove_file(source_dir.join("renamed.ss")).expect("delete renamed source");
    run_git(&root, ["add", "-A"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "delete indexed source",
        ],
    );
    let deleted = refresh_source_index(&root)
        .expect("refresh deleted source index")
        .expect("retained source must keep a generation");
    assert!(!deleted.reused_generation);
    assert_ne!(deleted.source_snapshot, renamed.source_snapshot);
    assert_ne!(deleted.index_artifact_digest, renamed.index_artifact_digest);
    let deleted_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "rename-delete-symbol",
        8,
    )
    .expect("lookup deleted source");
    assert_eq!(deleted_lookup.state.as_str(), "miss");
    assert!(deleted_lookup.candidates.is_empty());
    let keeper_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "keeper-symbol",
        8,
    )
    .expect("lookup retained source");
    assert_eq!(keeper_lookup.state.as_str(), "hit");
    assert_eq!(keeper_lookup.candidates[0].path, "src/keeper.ss");
    let reused = refresh_source_index(&root)
        .expect("reuse post-delete source index")
        .expect("post-delete generation must remain available");
    assert!(reused.reused_generation);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_detects_content_edit_without_stale_artifact() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-content-edit");
    let _home_env = isolate_home(&root);
    let source_dir = root.join("src");
    std::fs::create_dir_all(&source_dir).expect("create source dir");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    let source_path = source_dir.join("usage.ss");
    std::fs::write(
        &source_path,
        "(def (content-before-symbol input)\n  input)\n",
    )
    .expect("write initial source content");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["src"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "gerbil.pkg", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "initial content",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild initial source index");
    let initial = refresh_source_index(&root)
        .expect("refresh initial source index")
        .expect("initial generation must exist");
    assert!(initial.reused_generation);
    let initial_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "content-before-symbol",
        8,
    )
    .expect("lookup initial source content");
    assert_eq!(initial_lookup.state.as_str(), "hit");
    assert_eq!(initial_lookup.candidates[0].path, "src/usage.ss");

    std::fs::write(
        &source_path,
        "(def (content-after-symbol input)\n  input)\n",
    )
    .expect("edit source content in place");
    run_git(&root, ["add", "src/usage.ss"]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "edit indexed content",
        ],
    );
    let edited = refresh_source_index(&root)
        .expect("refresh edited source index")
        .expect("edited generation must exist");
    assert!(!edited.reused_generation);
    assert_eq!(
        edited.source_snapshot.provider_digest, initial.source_snapshot.provider_digest,
        "content edits must not change provider identity"
    );
    assert_ne!(edited.source_snapshot, initial.source_snapshot);
    assert_ne!(edited.index_artifact_digest, initial.index_artifact_digest);
    let stale_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "content-before-symbol",
        8,
    )
    .expect("lookup stale source content");
    assert_eq!(stale_lookup.state.as_str(), "miss");
    assert!(stale_lookup.candidates.is_empty());
    let edited_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "content-after-symbol",
        8,
    )
    .expect("lookup edited source content");
    assert_eq!(edited_lookup.state.as_str(), "hit");
    assert_eq!(edited_lookup.candidates.len(), 1);
    assert_eq!(edited_lookup.candidates[0].path, "src/usage.ss");
    let reused = refresh_source_index(&root)
        .expect("reuse edited source index")
        .expect("edited generation must remain available");
    assert!(reused.reused_generation);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_source_index_refresh_switches_roots_and_provider_digest_without_leakage() {
    let _guard = crate::test_support::CACHE_TEST_LOCK
        .lock()
        .expect("cache test lock");
    let root = temp_root("source-index-cross-root-provider-digest");
    let _home_env = isolate_home(&root);
    std::fs::create_dir_all(root.join("source-a")).expect("create source-a");
    std::fs::create_dir_all(root.join("source-b")).expect("create source-b");
    std::fs::write(root.join("gerbil.pkg"), "(package source-index-refresh)\n")
        .expect("write gerbil package anchor");
    std::fs::write(
        root.join("source-a/only-a.ss"),
        "(def (source-a-symbol input)\n  input)\n",
    )
    .expect("write source-a file");
    std::fs::write(
        root.join("source-b/only-b.ss"),
        "(def (source-b-symbol input)\n  input)\n",
    )
    .expect("write source-b file");
    let activation_path =
        write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["source-a"]);
    let _activation_env = EnvVarGuard::set(
        ASP_PROVIDER_ACTIVATION_PATH_ENV,
        activation_path.as_os_str(),
    );
    run_git(&root, ["init", "--quiet"]);
    run_git(&root, ["add", "."]);
    run_git(
        &root,
        [
            "-c",
            "user.email=source-index@example.invalid",
            "-c",
            "user.name=Source Index",
            "commit",
            "--quiet",
            "-m",
            "cross-root fixture",
        ],
    );
    run_cache(
        &root,
        None,
        &["source-index".to_string(), "rebuild".to_string()],
        false,
    )
    .expect("rebuild source-a index");
    let source_a = refresh_source_index(&root)
        .expect("refresh source-a index")
        .expect("source-a generation must exist");
    assert!(source_a.reused_generation);
    let source_a_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "source-a-symbol",
        8,
    )
    .expect("lookup source-a symbol");
    assert_eq!(source_a_lookup.state.as_str(), "hit");
    assert_eq!(source_a_lookup.candidates[0].path, "source-a/only-a.ss");

    write_gerbil_activation_with_command_prefix(&root, vec!["true".to_string()], &["source-b"]);
    let source_b = refresh_source_index(&root)
        .expect("refresh source-b index")
        .expect("source-b generation must exist");
    assert!(!source_b.reused_generation);
    assert_ne!(
        source_b.source_snapshot.provider_digest, source_a.source_snapshot.provider_digest,
        "provider coverage is part of provider identity"
    );
    assert_ne!(source_b.source_snapshot, source_a.source_snapshot);
    assert_ne!(
        source_b.index_artifact_digest,
        source_a.index_artifact_digest
    );
    let stale_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "source-a-symbol",
        8,
    )
    .expect("lookup source-a symbol after root switch");
    assert_eq!(stale_lookup.state.as_str(), "miss");
    assert!(stale_lookup.candidates.is_empty());
    let current_lookup = crate::test_support::lookup_current_source_index_for_language(
        &root,
        Some(&LanguageId::from("gerbil-scheme")),
        "source-b-symbol",
        8,
    )
    .expect("lookup source-b symbol after root switch");
    assert_eq!(current_lookup.state.as_str(), "hit");
    assert_eq!(current_lookup.candidates[0].path, "source-b/only-b.ss");
    let reused = refresh_source_index(&root)
        .expect("reuse source-b index")
        .expect("source-b generation must remain available");
    assert!(reused.reused_generation);
    let _ = std::fs::remove_dir_all(root);
}
