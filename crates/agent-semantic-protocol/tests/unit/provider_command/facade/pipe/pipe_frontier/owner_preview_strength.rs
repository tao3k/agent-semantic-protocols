use crate::provider_command::support::{
    asp_command, prepend_path, provider, temp_project_root, write_activation, write_marker_provider,
};

#[test]
fn low_cohesion_fd_preview_needs_strong_owner_seed_before_owner_items() {
    let root = temp_project_root("search-pipe-owner-preview-strength");
    let bin_dir = root.join(".bin");
    let marker = root.join("provider-called");
    for path in [
        "src/project",
        "src/worker",
        "src/lifecycle",
        "src/scope",
        "src/setup",
    ] {
        std::fs::create_dir_all(root.join(path)).expect("create source dir");
    }
    std::fs::write(
        root.join("src/project/project.ts"),
        "export class Project {}\n",
    )
    .expect("write project owner");
    std::fs::write(
        root.join("src/worker/testInfo.ts"),
        "export function fixtureLifecycleWorkerScopeSetup() { return true }\n",
    )
    .expect("write worker owner");
    std::fs::write(
        root.join("src/lifecycle/log.ts"),
        "export const lifecycle = true\n",
    )
    .expect("write lifecycle source");
    std::fs::write(
        root.join("src/scope/scope.ts"),
        "export const scope = true\n",
    )
    .expect("write scope source");
    std::fs::write(
        root.join("src/setup/setup.ts"),
        "export const setup = true\n",
    )
    .expect("write setup source");
    write_marker_provider(&bin_dir, "ts-harness", &marker);
    write_activation(&root, &[provider("typescript", Vec::new())]);

    let output = asp_command(&root)
        .env("PATH", prepend_path(&bin_dir))
        .env("PRJ_CACHE_HOME", root.join(".cache"))
        .args([
            "typescript",
            "search",
            "pipe",
            "Project fixture lifecycle worker scope setup",
            "--workspace",
            ".",
            "--view",
            "seeds",
        ])
        .output()
        .expect("run asp typescript search pipe");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.contains("packageCohesion=low"), "{stdout}");
    assert!(
        stdout.contains("fdPreview=ownerCandidates=src/project/project.ts"),
        "{stdout}"
    );
    assert!(
        stdout.contains("nextCommand=asp typescript search owner src/worker/testInfo.ts"),
        "{stdout}"
    );
    assert!(
        stdout.contains("recommendedNext=A1.owner-items"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
