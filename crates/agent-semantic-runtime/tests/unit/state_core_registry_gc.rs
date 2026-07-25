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
