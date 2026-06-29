use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_fd_query_avoids_owner_items_when_package_cohesion_is_low() {
    let root = temp_project_root("asp-fd-query-wrapper-low-cohesion-owner-items");
    for package in ["alpha", "beta", "gamma", "delta"] {
        let dir = root.join(package);
        std::fs::create_dir_all(&dir).expect("create package dir");
        std::fs::write(
            dir.join("scope_gate.rs"),
            "pub fn scope_gate_query_wrapper_cache_index() {}\n",
        )
        .expect("write package source");
    }

    let output = asp_command(&root)
        .args([
            "fd",
            "-query",
            "scope|gate|query|wrapper|cache|index",
            "--workspace",
            ".",
        ])
        .output()
        .expect("run asp fd -query");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(
        stdout.contains("packageCohesion=low")
            && stdout.contains("risk=single-flat-or-recall,broad-scope,low-package-cohesion"),
        "{stdout}"
    );
    assert!(!stdout.contains("actionFrontier="), "{stdout}");
    assert!(!stdout.contains("recommendedNext="), "{stdout}");
    assert!(!stdout.contains("rankedEvidence="), "{stdout}");
    assert!(!stdout.contains("evidenceFrontier="), "{stdout}");
    assert!(
        stdout
            .contains("nextCommand=asp rg -query 'scope|gate|query' -query 'wrapper|cache|index'"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(root);
}
