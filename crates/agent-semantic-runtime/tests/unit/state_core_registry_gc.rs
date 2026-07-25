use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{ProjectRegistryGcOptions, ResolvedState};

#[test]
fn project_gc_is_dry_run_by_default_and_revalidates_apply() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-project-gc-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos()
    ));
    let checkout = fixture.join("active-checkout");
    let state_home = fixture.join("state");
    fs::create_dir_all(&checkout).expect("create active checkout");
    let state = ResolvedState::resolve_with_state_home(&checkout, &state_home)
        .expect("resolve fixture state");
    state.ensure_minimal_layout().expect("create fixture state");

    let stale_project = state.paths.projects_by_id_dir.join("repo-stale-fixture");
    let stale_workspace = stale_project
        .join("workspaces")
        .join("workspace-stale-fixture");
    fs::create_dir_all(&stale_workspace).expect("create stale workspace");
    fs::write(
        stale_project.join("project.json"),
        r#"{"repoId":"repo-stale-fixture","checkoutRoot":"/missing/asp-fixture"}"#,
    )
    .expect("write stale project metadata");
    fs::write(
        stale_workspace.join("workspace.json"),
        r#"{"workspaceId":"workspace-stale-fixture","root":"/missing/asp-fixture"}"#,
    )
    .expect("write stale workspace metadata");
    fs::write(stale_project.join(".last-seen-ms"), "1\n").expect("write stale activity");

    let dry_run = state
        .gc_project_registry(ProjectRegistryGcOptions {
            apply: false,
            grace_period_ms: 0,
        })
        .expect("scan project registry");
    assert_eq!(dry_run.candidate_count, 1);
    assert_eq!(dry_run.eligible_count, 1);
    assert_eq!(dry_run.removed_count, 0);
    assert!(stale_project.exists());

    let applied = state
        .gc_project_registry(ProjectRegistryGcOptions {
            apply: true,
            grace_period_ms: 0,
        })
        .expect("apply project registry GC");
    assert_eq!(applied.eligible_count, 1);
    assert_eq!(applied.removed_count, 1);
    assert!(!stale_project.exists());
    assert!(state.paths.project_dir.exists());

    fs::remove_dir_all(&fixture).expect("remove GC fixture");
}

#[test]
fn project_gc_removes_noncanonical_identity_for_existing_checkout() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-project-gc-identity-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos()
    ));
    let checkout = fixture.join("active-checkout");
    let state_home = fixture.join("state");
    fs::create_dir_all(checkout.join(".git")).expect("create active Git checkout");
    let state = ResolvedState::resolve_with_state_home(&checkout, &state_home)
        .expect("resolve canonical fixture state");
    state
        .ensure_minimal_layout()
        .expect("create canonical fixture state");

    let legacy_project = state
        .paths
        .projects_by_id_dir
        .join("repo-legacy-path-identity");
    let legacy_workspace = legacy_project
        .join("workspaces")
        .join("workspace-legacy-path-identity");
    fs::create_dir_all(&legacy_workspace).expect("create legacy workspace");
    fs::write(
        legacy_project.join("project.json"),
        format!(
            r#"{{"repoId":"repo-legacy-path-identity","checkoutRoot":"{}"}}"#,
            checkout.display()
        ),
    )
    .expect("write legacy project metadata");
    fs::write(
        legacy_workspace.join("workspace.json"),
        format!(
            r#"{{"workspaceId":"workspace-legacy-path-identity","root":"{}"}}"#,
            checkout.display()
        ),
    )
    .expect("write legacy workspace metadata");
    fs::write(legacy_project.join(".last-seen-ms"), "1\n").expect("write legacy activity");

    let dry_run = state
        .gc_project_registry(ProjectRegistryGcOptions {
            apply: false,
            grace_period_ms: 0,
        })
        .expect("scan legacy repository identity");
    assert_eq!(dry_run.candidate_count, 1);
    assert_eq!(dry_run.eligible_count, 1);
    assert!(legacy_project.exists());

    let applied = state
        .gc_project_registry(ProjectRegistryGcOptions {
            apply: true,
            grace_period_ms: 0,
        })
        .expect("remove legacy repository identity");
    assert_eq!(applied.removed_count, 1);
    assert!(!legacy_project.exists());
    assert!(state.paths.project_dir.exists());

    fs::remove_dir_all(&fixture).expect("remove identity GC fixture");
}

#[test]
fn project_gc_removes_legacy_path_identity_for_existing_non_git_root() {
    let fixture = std::env::temp_dir().join(format!(
        "asp-project-gc-non-git-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos()
    ));
    let current_checkout = fixture.join("current-checkout");
    let ordinary_root = fixture.join("plugins/cache");
    let state_home = fixture.join("state");
    fs::create_dir_all(current_checkout.join(".git")).expect("create current Git checkout");
    fs::create_dir_all(&ordinary_root).expect("create ordinary non-Git root");
    let state = ResolvedState::resolve_with_state_home(&current_checkout, &state_home)
        .expect("resolve current fixture state");
    state
        .ensure_minimal_layout()
        .expect("create current fixture state");

    let legacy_project = state
        .paths
        .projects_by_id_dir
        .join("repo-legacy-non-git-path");
    fs::create_dir_all(&legacy_project).expect("create legacy path project");
    fs::write(
        legacy_project.join("project.json"),
        format!(
            r#"{{"repoId":"repo-legacy-non-git-path","checkoutRoot":"{}","identityBasis":"path:{}"}}"#,
            ordinary_root.display(),
            ordinary_root.display()
        ),
    )
    .expect("write legacy path metadata");
    fs::write(legacy_project.join(".last-seen-ms"), "1\n").expect("write legacy path activity");

    let applied = state
        .gc_project_registry(ProjectRegistryGcOptions {
            apply: true,
            grace_period_ms: 0,
        })
        .expect("remove legacy non-Git path identity");
    assert_eq!(applied.removed_count, 1);
    assert!(!legacy_project.exists());
    assert!(ordinary_root.exists());
    assert!(state.paths.project_dir.exists());

    fs::remove_dir_all(&fixture).expect("remove non-Git identity GC fixture");
}
