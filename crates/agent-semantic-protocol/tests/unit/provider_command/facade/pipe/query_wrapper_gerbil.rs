use crate::provider_command::support::{asp_command, temp_project_root};

#[test]
fn asp_fd_query_finds_gerbil_config_filename_candidates() {
    let root = temp_project_root("asp-fd-query-gerbil-config");
    std::fs::write(
        root.join("gerbil.pkg"),
        "(package: sample/app\n depend: (\"git.cons.io/mighty-gerbils/gerbil-poo\"))\n",
    )
    .expect("write gerbil package");
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/main.ss"),
        "(package: sample/app)\n(def main #t)\n",
    )
    .expect("write gerbil source");

    let output = asp_command(&root)
        .args(["fd", "-query", "gerbil.pkg", "."])
        .output()
        .expect("run asp fd -query gerbil.pkg");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout");
    assert!(stdout.starts_with("[search-fd]"), "{stdout}");
    assert!(stdout.contains("ownerCandidates=gerbil.pkg"), "{stdout}");
    assert!(
        stdout.contains("clauseCoverage=C1 matched=gerbil.pkg missing=-"),
        "{stdout}"
    );
    assert!(!stdout.contains("reason=no-candidates"), "{stdout}");
    let _ = std::fs::remove_dir_all(root);
}
